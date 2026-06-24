use std::collections::HashMap;

use anyhow::{bail, ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use sha1::{Digest, Sha1};
use tokio::io::{duplex, split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};
use tracing::debug;

const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const HEADER_LIMIT: usize = 16 * 1024;
const IO_BUFFER: usize = 16 * 1024;
const DEFAULT_WEBSOCKET_MAX_FRAME: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebSocketRole {
    Client,
    Server,
}

#[derive(Clone, Debug)]
struct HttpHeaders {
    request_or_status: String,
    fields: HashMap<String, String>,
}

pub async fn connect_websocket_underlay<S>(
    mut stream: S,
    host: &str,
    path: &str,
    max_frame_bytes: usize,
) -> Result<DuplexStream>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let key = websocket_key();
    let request = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         \r\n"
    );
    stream.write_all(request.as_bytes()).await?;
    stream.flush().await?;

    let response = read_http_headers(&mut stream).await?;
    ensure!(
        response.request_or_status.starts_with("HTTP/1.1 101")
            || response.request_or_status.starts_with("HTTP/1.0 101"),
        "websocket upgrade rejected: {}",
        response.request_or_status
    );
    let accept = response
        .fields
        .get("sec-websocket-accept")
        .context("websocket response missing Sec-WebSocket-Accept")?;
    ensure!(
        accept == &websocket_accept(&key),
        "websocket response has invalid Sec-WebSocket-Accept"
    );
    Ok(spawn_websocket_io(
        stream,
        WebSocketRole::Client,
        max_frame_bytes,
    ))
}

pub async fn accept_websocket_underlay<S>(
    mut stream: S,
    expected_path: &str,
    max_frame_bytes: usize,
) -> Result<DuplexStream>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let request = read_http_headers(&mut stream).await?;
    ensure!(
        websocket_request_matches(&request, expected_path),
        "invalid websocket upgrade request"
    );
    let key = request
        .fields
        .get("sec-websocket-key")
        .context("websocket request missing Sec-WebSocket-Key")?;
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\
         \r\n",
        websocket_accept(key)
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(spawn_websocket_io(
        stream,
        WebSocketRole::Server,
        max_frame_bytes,
    ))
}

pub fn websocket_upgrade_header_matches(header: &[u8], expected_path: &str) -> bool {
    let Ok(text) = std::str::from_utf8(header) else {
        return false;
    };
    let Some(end) = text.find("\r\n\r\n") else {
        return false;
    };
    parse_http_headers(&text[..end + 4])
        .map(|headers| websocket_request_matches(&headers, expected_path))
        .unwrap_or(false)
}

fn spawn_websocket_io<S>(stream: S, role: WebSocketRole, max_frame_bytes: usize) -> DuplexStream
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let max_frame_bytes = max_frame_bytes.max(1024);
    let (app_stream, pump_stream) = duplex(max_frame_bytes.max(IO_BUFFER) * 2);
    let (mut app_reader, mut app_writer) = split(pump_stream);
    let (mut wire_reader, mut wire_writer) = split(stream);

    tokio::spawn(async move {
        loop {
            match read_ws_frame(&mut wire_reader, role, max_frame_bytes).await {
                Ok(Some(payload)) => {
                    if app_writer.write_all(&payload).await.is_err() {
                        break;
                    }
                }
                Ok(None) => {
                    let _ = app_writer.shutdown().await;
                    break;
                }
                Err(err) => {
                    debug!(error = %err, "websocket underlay reader stopped");
                    let _ = app_writer.shutdown().await;
                    break;
                }
            }
        }
    });

    tokio::spawn(async move {
        let mut buf = vec![0_u8; IO_BUFFER.min(max_frame_bytes)];
        loop {
            match app_reader.read(&mut buf).await {
                Ok(0) => {
                    let _ = write_ws_frame(&mut wire_writer, role, 0x8, &[]).await;
                    let _ = wire_writer.shutdown().await;
                    break;
                }
                Ok(n) => {
                    if let Err(err) = write_ws_frame(&mut wire_writer, role, 0x2, &buf[..n]).await {
                        debug!(error = %err, "websocket underlay writer stopped");
                        break;
                    }
                }
                Err(err) => {
                    debug!(error = %err, "websocket app reader stopped");
                    break;
                }
            }
        }
    });

    app_stream
}

async fn read_ws_frame<R>(
    reader: &mut R,
    role: WebSocketRole,
    max_frame_bytes: usize,
) -> Result<Option<Vec<u8>>>
where
    R: AsyncRead + Unpin,
{
    let mut head = [0_u8; 2];
    reader.read_exact(&mut head).await?;
    let opcode = head[0] & 0x0f;
    let masked = head[1] & 0x80 != 0;
    let mut len = u64::from(head[1] & 0x7f);
    if len == 126 {
        len = u64::from(reader.read_u16().await?);
    } else if len == 127 {
        len = reader.read_u64().await?;
    }
    ensure!(
        len <= max_frame_bytes as u64,
        "websocket frame exceeds configured limit"
    );
    let expected_masked = role == WebSocketRole::Server;
    ensure!(
        masked == expected_masked,
        "websocket frame mask bit did not match peer role"
    );
    let mut mask = [0_u8; 4];
    if masked {
        reader.read_exact(&mut mask).await?;
    }
    let mut payload = vec![0_u8; len as usize];
    if !payload.is_empty() {
        reader.read_exact(&mut payload).await?;
    }
    if masked {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[i % 4];
        }
    }
    match opcode {
        0x0 | 0x2 => Ok(Some(payload)),
        0x8 => Ok(None),
        0x9 | 0xa => Ok(Some(Vec::new())),
        other => bail!("unsupported websocket opcode {other}"),
    }
}

async fn write_ws_frame<W>(
    writer: &mut W,
    role: WebSocketRole,
    opcode: u8,
    payload: &[u8],
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let masked = role == WebSocketRole::Client;
    let mut header = Vec::with_capacity(14);
    header.push(0x80 | (opcode & 0x0f));
    let mask_bit = if masked { 0x80 } else { 0x00 };
    match payload.len() {
        0..=125 => header.push(mask_bit | payload.len() as u8),
        126..=65_535 => {
            header.push(mask_bit | 126);
            header.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        }
        _ => {
            header.push(mask_bit | 127);
            header.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }
    }
    let mut mask = [0_u8; 4];
    if masked {
        rand::thread_rng().fill_bytes(&mut mask);
        header.extend_from_slice(&mask);
    }
    writer.write_all(&header).await?;
    if masked {
        let mut masked_payload = payload.to_vec();
        for (i, byte) in masked_payload.iter_mut().enumerate() {
            *byte ^= mask[i % 4];
        }
        writer.write_all(&masked_payload).await?;
    } else {
        writer.write_all(payload).await?;
    }
    writer.flush().await?;
    Ok(())
}

async fn read_http_headers<S>(stream: &mut S) -> Result<HttpHeaders>
where
    S: AsyncRead + Unpin,
{
    let mut buf = Vec::with_capacity(1024);
    let mut byte = [0_u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        ensure!(buf.len() < HEADER_LIMIT, "websocket HTTP header too large");
        stream.read_exact(&mut byte).await?;
        buf.push(byte[0]);
    }
    let text = std::str::from_utf8(&buf).context("websocket HTTP header is not UTF-8")?;
    parse_http_headers(text)
}

fn parse_http_headers(text: &str) -> Result<HttpHeaders> {
    let mut lines = text.split("\r\n");
    let request_or_status = lines
        .next()
        .context("websocket HTTP header missing request/status line")?
        .to_string();
    let mut fields = HashMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        fields.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }
    Ok(HttpHeaders {
        request_or_status,
        fields,
    })
}

fn websocket_request_matches(headers: &HttpHeaders, expected_path: &str) -> bool {
    let mut parts = headers.request_or_status.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    if method != "GET" || path != expected_path {
        return false;
    }
    header_contains(&headers.fields, "upgrade", "websocket")
        && header_contains(&headers.fields, "connection", "upgrade")
        && headers
            .fields
            .get("sec-websocket-version")
            .is_some_and(|version| version == "13")
        && headers.fields.contains_key("sec-websocket-key")
}

fn header_contains(fields: &HashMap<String, String>, key: &str, needle: &str) -> bool {
    fields
        .get(key)
        .map(|value| {
            value
                .split(',')
                .any(|part| part.trim().eq_ignore_ascii_case(needle))
        })
        .unwrap_or(false)
}

fn websocket_key() -> String {
    let mut raw = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut raw);
    BASE64.encode(raw)
}

fn websocket_accept(key: &str) -> String {
    let mut sha1 = Sha1::new();
    sha1.update(key.as_bytes());
    sha1.update(WEBSOCKET_GUID.as_bytes());
    BASE64.encode(sha1.finalize())
}

pub fn default_websocket_max_frame_bytes() -> usize {
    DEFAULT_WEBSOCKET_MAX_FRAME
}

#[cfg(test)]
mod tests {
    use super::{connect_websocket_underlay, websocket_accept, websocket_upgrade_header_matches};
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    #[test]
    fn websocket_accept_matches_rfc_example() {
        assert_eq!(
            websocket_accept("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn websocket_upgrade_header_requires_expected_path() {
        let header = b"GET /espejismo HTTP/1.1\r\nHost: example.com\r\nUpgrade: websocket\r\nConnection: keep-alive, Upgrade\r\nSec-WebSocket-Key: x\r\nSec-WebSocket-Version: 13\r\n\r\n";
        assert!(websocket_upgrade_header_matches(header, "/espejismo"));
        assert!(!websocket_upgrade_header_matches(header, "/other"));
    }

    #[tokio::test]
    async fn websocket_underlay_roundtrips_binary_bytes() {
        let (client, server) = duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            super::accept_websocket_underlay(server, "/espejismo", 64 * 1024)
                .await
                .unwrap()
        });
        let mut client = connect_websocket_underlay(client, "example.com", "/espejismo", 64 * 1024)
            .await
            .unwrap();
        let mut server = server_task.await.unwrap();

        client.write_all(b"hello over websocket").await.unwrap();
        let mut received = vec![0_u8; "hello over websocket".len()];
        server.read_exact(&mut received).await.unwrap();
        assert_eq!(&received, b"hello over websocket");

        server.write_all(b"reply").await.unwrap();
        let mut reply = [0_u8; 5];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"reply");
    }

    #[tokio::test]
    async fn websocket_underlay_carries_crypto_handshake() {
        let (client, server) = duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            let mut server = super::accept_websocket_underlay(server, "/espejismo", 64 * 1024)
                .await
                .unwrap();
            crate::crypto::accept_handshake(
                &mut server,
                &crate::crypto::HandshakeConfig::new(
                    b"test-secret-that-is-long-enough".to_vec(),
                    30,
                    128,
                    4,
                ),
            )
            .await
            .unwrap()
        });
        let mut client = connect_websocket_underlay(client, "example.com", "/espejismo", 64 * 1024)
            .await
            .unwrap();
        let client_keys = crate::crypto::connect_handshake(
            &mut client,
            &crate::crypto::HandshakeConfig::new(
                b"test-secret-that-is-long-enough".to_vec(),
                30,
                128,
                4,
            ),
        )
        .await
        .unwrap();
        let server_keys = server_task.await.unwrap();
        assert_eq!(
            client_keys.stealth_selector(),
            server_keys.stealth_selector()
        );
    }
}
