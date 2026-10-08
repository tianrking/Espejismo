//! HTTP/2 and WebSocket stream adapters for carrying tunnel bytes.
//!
//! These underlays provide protocol-compatible carriers; they do not replace
//! Espejismo's authenticated encrypted tunnel layer.

use std::collections::HashMap;

use anyhow::{bail, ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bytes::Bytes;
use rand::RngCore;
use sha1::{Digest, Sha1};
use tokio::io::{duplex, split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};
use tracing::debug;

const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const HEADER_LIMIT: usize = 16 * 1024;
const IO_BUFFER: usize = 16 * 1024;
const DEFAULT_WEBSOCKET_MAX_FRAME: usize = 1024 * 1024;
pub const HTTP2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

#[derive(Clone, Copy, Debug)]
pub struct Http2UnderlayOptions {
    pub initial_stream_window_bytes: u32,
    pub initial_connection_window_bytes: u32,
    pub max_frame_bytes: u32,
}

impl Default for Http2UnderlayOptions {
    fn default() -> Self {
        Self {
            initial_stream_window_bytes: 8 * 1024 * 1024,
            initial_connection_window_bytes: 16 * 1024 * 1024,
            max_frame_bytes: 64 * 1024,
        }
    }
}

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
        websocket_response_matches(&response),
        "invalid websocket upgrade response: {}",
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

pub async fn connect_http2_underlay<S>(
    stream: S,
    authority: &str,
    path: &str,
    options: Http2UnderlayOptions,
) -> Result<DuplexStream>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let (mut client, connection) = h2::client::Builder::new()
        .initial_window_size(options.initial_stream_window_bytes)
        .initial_connection_window_size(options.initial_connection_window_bytes)
        .max_frame_size(options.max_frame_bytes)
        .handshake(stream)
        .await?;
    tokio::spawn(async move {
        if let Err(err) = connection.await {
            debug!(error = %err, "http2 underlay client connection stopped");
        }
    });
    let request = http::Request::builder()
        .method("POST")
        .uri(path)
        .header("scheme", "http")
        .header("authority", authority)
        .header("content-type", "application/octet-stream")
        .body(())?;
    let (response, send_stream) = client.send_request(request, false)?;
    let response = response.await?;
    ensure!(
        response.status().is_success(),
        "http2 underlay request failed with {}",
        response.status()
    );
    Ok(spawn_http2_io(send_stream, response.into_body()))
}

pub async fn accept_http2_underlay<S>(
    stream: S,
    expected_path: &str,
    options: Http2UnderlayOptions,
) -> Result<DuplexStream>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let mut connection = h2::server::Builder::new()
        .initial_window_size(options.initial_stream_window_bytes)
        .initial_connection_window_size(options.initial_connection_window_bytes)
        .max_frame_size(options.max_frame_bytes)
        .handshake(stream)
        .await?;
    let Some(request) = connection.accept().await else {
        bail!("http2 underlay connection closed before request");
    };
    let (request, mut respond) = request?;
    ensure!(
        request.method() == http::Method::POST && request.uri().path() == expected_path,
        "invalid http2 underlay request"
    );
    let response = http::Response::builder().status(200).body(())?;
    let send_stream = respond.send_response(response, false)?;
    tokio::spawn(async move {
        while let Some(request) = connection.accept().await {
            if let Err(err) = request {
                debug!(error = %err, "http2 underlay server connection stopped");
                break;
            }
        }
    });
    Ok(spawn_http2_io(send_stream, request.into_body()))
}

pub fn http2_preface_matches(prefix: &[u8]) -> bool {
    prefix.starts_with(HTTP2_PREFACE)
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

fn spawn_http2_io(
    mut send_stream: h2::SendStream<Bytes>,
    mut recv_stream: h2::RecvStream,
) -> DuplexStream {
    let (app_stream, pump_stream) = duplex(IO_BUFFER * 8);
    let (mut app_reader, mut app_writer) = split(pump_stream);

    tokio::spawn(async move {
        loop {
            match recv_stream.data().await {
                Some(Ok(chunk)) => {
                    let len = chunk.len();
                    if app_writer.write_all(&chunk).await.is_err() {
                        break;
                    }
                    let _ = recv_stream.flow_control().release_capacity(len);
                }
                Some(Err(err)) => {
                    debug!(error = %err, "http2 underlay reader stopped");
                    let _ = app_writer.shutdown().await;
                    break;
                }
                None => {
                    let _ = app_writer.shutdown().await;
                    break;
                }
            }
        }
    });

    tokio::spawn(async move {
        let mut buf = vec![0_u8; IO_BUFFER];
        loop {
            match app_reader.read(&mut buf).await {
                Ok(0) => {
                    let _ = send_stream.send_data(Bytes::new(), true);
                    break;
                }
                Ok(n) => {
                    if let Err(err) =
                        send_stream.send_data(Bytes::copy_from_slice(&buf[..n]), false)
                    {
                        debug!(error = %err, "http2 underlay writer stopped");
                        break;
                    }
                }
                Err(err) => {
                    debug!(error = %err, "http2 app reader stopped");
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
        let (name, value) = line
            .split_once(':')
            .context("malformed websocket HTTP header field")?;
        ensure!(
            !name.is_empty() && name.bytes().all(is_http_token),
            "invalid websocket HTTP header name"
        );
        let key = name.to_ascii_lowercase();
        ensure!(
            !fields.contains_key(&key),
            "duplicate websocket HTTP header field"
        );
        let value = value.trim();
        ensure!(
            !value.bytes().any(|b| b == b'\r' || b == b'\n'),
            "invalid websocket HTTP header value"
        );
        fields.insert(key, value.to_string());
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
    let version = parts.next().unwrap_or_default();
    if method != "GET" || path != expected_path || version != "HTTP/1.1" || parts.next().is_some() {
        return false;
    }
    header_contains(&headers.fields, "upgrade", "websocket")
        && header_contains(&headers.fields, "connection", "upgrade")
        && headers
            .fields
            .get("sec-websocket-version")
            .is_some_and(|version| version == "13")
        && headers
            .fields
            .get("sec-websocket-key")
            .is_some_and(|key| valid_websocket_key(key))
}

fn websocket_response_matches(headers: &HttpHeaders) -> bool {
    let mut parts = headers.request_or_status.split_whitespace();
    let valid_status =
        parts.next() == Some("HTTP/1.1") && parts.next() == Some("101") && parts.next().is_some();
    valid_status
        && header_contains(&headers.fields, "upgrade", "websocket")
        && header_contains(&headers.fields, "connection", "upgrade")
}

fn valid_websocket_key(key: &str) -> bool {
    BASE64.decode(key).is_ok_and(|decoded| decoded.len() == 16)
}

fn is_http_token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
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
    use super::{
        connect_http2_underlay, connect_websocket_underlay, http2_preface_matches,
        websocket_accept, websocket_upgrade_header_matches, HTTP2_PREFACE,
    };
    use bytes::Bytes;
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, DuplexStream};

    async fn http2_server_after_raw_priority(
        payload: &[u8],
        stream_id: u32,
    ) -> (h2::server::Connection<DuplexStream, Bytes>, DuplexStream) {
        let (mut peer, server_io) = duplex(1024);
        let mut wire = HTTP2_PREFACE.to_vec();
        // Empty client SETTINGS frame, followed by a raw PRIORITY frame.
        wire.extend_from_slice(&[0, 0, 0, 4, 0, 0, 0, 0, 0]);
        wire.extend_from_slice(&[
            (payload.len() >> 16) as u8,
            (payload.len() >> 8) as u8,
            payload.len() as u8,
            2,
            0,
        ]);
        wire.extend_from_slice(&stream_id.to_be_bytes());
        wire.extend_from_slice(payload);
        peer.write_all(&wire).await.unwrap();
        let connection = h2::server::Builder::new()
            .handshake::<_, Bytes>(server_io)
            .await
            .unwrap();
        (connection, peer)
    }

    async fn http2_server_after_raw_frames(
        frames: &[u8],
    ) -> (h2::server::Connection<DuplexStream, Bytes>, DuplexStream) {
        let (mut peer, server_io) = duplex(1024);
        let mut wire = HTTP2_PREFACE.to_vec();
        // Empty client SETTINGS frame, followed by caller-supplied frames.
        wire.extend_from_slice(&[0, 0, 0, 4, 0, 0, 0, 0, 0]);
        wire.extend_from_slice(frames);
        peer.write_all(&wire).await.unwrap();
        let connection = h2::server::Builder::new()
            .handshake::<_, Bytes>(server_io)
            .await
            .unwrap();
        (connection, peer)
    }

    fn raw_frame(frame_type: u8, flags: u8, stream_id: u32, payload: &[u8]) -> Vec<u8> {
        let mut frame = vec![
            (payload.len() >> 16) as u8,
            (payload.len() >> 8) as u8,
            payload.len() as u8,
            frame_type,
            flags,
        ];
        frame.extend_from_slice(&stream_id.to_be_bytes());
        frame.extend_from_slice(payload);
        frame
    }

    // Exercise the h2 frame decoder with in-memory wire bytes; this pins the
    // RFC PRIORITY payload length and stream identifier boundary behavior.
    #[tokio::test]
    async fn http2_priority_rejects_payload_lengths_around_five_bytes() {
        for length in [0, 4, 6] {
            let (mut server, _peer) = http2_server_after_raw_priority(&vec![0; length], 1).await;
            let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
                .await
                .expect("malformed PRIORITY should be processed");
            assert!(
                result.is_none() || result.unwrap().is_err(),
                "length {length}"
            );
        }
    }

    #[tokio::test]
    async fn http2_priority_accepts_five_byte_payload() {
        let (mut server, _peer) = http2_server_after_raw_priority(&[0, 0, 0, 0, 0], 1).await;
        // A valid PRIORITY frame on an idle stream is ignored by the server;
        // the connection remains open awaiting the next request.
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), server.accept(),)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn http2_priority_rejects_stream_zero() {
        let (mut server, _peer) = http2_server_after_raw_priority(&[0; 5], 0).await;
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("stream zero PRIORITY should be processed");
        assert!(result.is_none() || result.unwrap().is_err());
    }

    #[tokio::test]
    async fn http2_priority_self_dependency_is_stream_error_not_connection_error() {
        let (mut server, _peer) = http2_server_after_raw_priority(&[0, 0, 0, 1, 0], 1).await;
        // RFC 9113 §5.3.1 requires a stream error; the connection stays open.
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), server.accept())
                .await
                .is_err()
        );
    }

    // A split HPACK block must continue on the same stream and finish with
    // END_HEADERS; the h2 crate owns these wire-level framing rules.
    #[tokio::test]
    async fn http2_continuation_completes_split_headers() {
        let block = [0x82, 0x86, 0x84, 0x01, 0x01, b'x'];
        let mut frames = raw_frame(1, 0, 1, &block[..3]);
        frames.extend_from_slice(&raw_frame(9, 4, 1, &block[3..]));
        let (mut server, _peer) = http2_server_after_raw_frames(&frames).await;
        let accepted = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("completed CONTINUATION should be processed")
            .expect("valid request should be accepted")
            .expect("valid header block should decode");
        assert_eq!(accepted.0.method(), http::Method::GET);
        assert_eq!(accepted.0.uri().path(), "/");
    }

    #[tokio::test]
    async fn http2_continuation_rejects_different_stream_id() {
        let block = [0x82, 0x86, 0x84, 0x01, 0x01, b'x'];
        let mut frames = raw_frame(1, 0, 1, &block[..3]);
        frames.extend_from_slice(&raw_frame(9, 4, 3, &block[3..]));
        let (mut server, _peer) = http2_server_after_raw_frames(&frames).await;
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("invalid CONTINUATION should be processed");
        assert!(result.is_none() || result.unwrap().is_err());
    }

    #[tokio::test]
    async fn http2_continuation_requires_end_headers_before_another_frame() {
        let block = [0x82, 0x86, 0x84, 0x01, 0x01, b'x'];
        let mut frames = raw_frame(1, 0, 1, &block[..3]);
        frames.extend_from_slice(&raw_frame(9, 0, 1, &block[3..]));
        // SETTINGS is illegal while the header block remains open.
        frames.extend_from_slice(&raw_frame(4, 0, 0, &[]));
        let (mut server, _peer) = http2_server_after_raw_frames(&frames).await;
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("unterminated header block should be processed");
        assert!(result.is_none() || result.unwrap().is_err());
    }

    // The tunnel uses one POST stream and never needs server push. Pin the
    // h2 API boundary so future adapter changes cannot accidentally rely on
    // pushing when the client has disabled it.
    #[tokio::test]
    async fn http2_server_push_rejects_peer_disabled_push() {
        let (client_io, server_io) = duplex(4096);
        let mut client_builder = h2::client::Builder::new();
        client_builder.enable_push(false);
        let (mut client, client_driver) = client_builder
            .handshake::<_, Bytes>(client_io)
            .await
            .unwrap();
        let mut server = h2::server::Builder::new()
            .handshake::<_, Bytes>(server_io)
            .await
            .unwrap();
        tokio::spawn(async move {
            let _ = client_driver.await;
        });

        let request = http::Request::builder()
            .method("GET")
            .uri("/")
            .body(())
            .unwrap();
        let (_response, _send) = client.send_request(request, true).unwrap();
        let (_request, mut respond) =
            tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
                .await
                .expect("request should reach server")
                .expect("connection remains open")
                .expect("request should decode");
        let pushed = http::Request::builder()
            .method("GET")
            .uri("/asset")
            .body(())
            .unwrap();
        assert!(respond.push_request(pushed).is_err());
    }

    #[test]
    fn websocket_accept_matches_rfc_example() {
        assert_eq!(
            websocket_accept("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn websocket_upgrade_header_requires_expected_path() {
        let header = b"GET /espejismo HTTP/1.1\r\nHost: example.com\r\nUpgrade: websocket\r\nConnection: keep-alive, Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n";
        assert!(websocket_upgrade_header_matches(header, "/espejismo"));
        assert!(!websocket_upgrade_header_matches(header, "/other"));
    }

    #[test]
    fn websocket_upgrade_rejects_malformed_and_ambiguous_headers() {
        let valid = b"GET /espejismo HTTP/1.1\r\nHost: example.com\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n";
        for invalid in [
            &b"GET /espejismoX HTTP/1.1\r\nHost: example.com\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"[..],
            &b"GET /espejismo HTTP/1.1 EXTRA\r\nHost: example.com\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"[..],
            &b"GET /espejismo HTTP/1.1\r\nHost: example.com\r\nBad Header: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"[..],
            &b"GET /espejismo HTTP/1.1\r\nHost: example.com\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: duplicate\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"[..],
            &b"GET /espejismo HTTP/1.1\r\nHost: example.com\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: x\r\nSec-WebSocket-Version: 13\r\n\r\n"[..],
        ] {
            assert!(!websocket_upgrade_header_matches(invalid, "/espejismo"));
        }
        assert!(websocket_upgrade_header_matches(valid, "/espejismo"));
    }

    #[test]
    fn websocket_response_requires_exact_switching_protocols_and_headers() {
        let valid = super::parse_http_headers(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: keep-alive, Upgrade\r\n\r\n",
        )
        .unwrap();
        assert!(super::websocket_response_matches(&valid));
        for status in [
            "HTTP/1.1 1010 Switching Protocols",
            "HTTP/1.1 200 Switching Protocols",
            "HTTP/1.0 101 Switching Protocols",
        ] {
            let headers = super::parse_http_headers(&format!(
                "{status}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n"
            ))
            .unwrap();
            assert!(!super::websocket_response_matches(&headers), "{status}");
        }
        let missing_upgrade = super::parse_http_headers(
            "HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\n\r\n",
        )
        .unwrap();
        assert!(!super::websocket_response_matches(&missing_upgrade));
    }

    #[test]
    fn websocket_http_headers_parse_cookie_field_boundaries() {
        // Cookie is an opaque HTTP field here; preserve its delimiters and
        // embedded '=' characters while trimming surrounding field whitespace.
        let headers = super::parse_http_headers(
            "GET /espejismo HTTP/1.1\r\nCookie: session=; theme=dark; token=a=b\r\n\r\n",
        )
        .unwrap();
        assert_eq!(
            headers.fields.get("cookie").map(String::as_str),
            Some("session=; theme=dark; token=a=b")
        );

        let empty_cookie = super::parse_http_headers(
            "GET /espejismo HTTP/1.1\r\nCookie:\t \r\n\r\n",
        )
        .unwrap();
        assert_eq!(
            empty_cookie.fields.get("cookie").map(String::as_str),
            Some("")
        );

        let duplicate_cookie = super::parse_http_headers(
            "GET /espejismo HTTP/1.1\r\nCookie: a=1\r\nCookie: b=2\r\n\r\n",
        );
        assert!(duplicate_cookie.is_err());
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

    #[test]
    fn http2_preface_matcher_requires_standard_preface() {
        assert!(http2_preface_matches(HTTP2_PREFACE));
        assert!(http2_preface_matches(
            b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\nextra"
        ));
        assert!(!http2_preface_matches(
            &HTTP2_PREFACE[..HTTP2_PREFACE.len() - 1]
        ));
        assert!(!http2_preface_matches(b""));
        assert!(!http2_preface_matches(b"PRI * HTTP/2.0\r\n\r\nSX\r\n\r\n"));
        assert!(!http2_preface_matches(b"GET / HTTP/1.1\r\n"));
    }

    // HPACK belongs to the h2 crate; this checks that its encoded header block
    // preserves field boundaries and values used by our HTTP/2 adapter.
    #[tokio::test]
    async fn http2_hpack_preserves_empty_duplicate_and_large_header_values() {
        let (client_io, server_io) = duplex(256 * 1024);
        let (mut client, client_driver) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(client_io)
            .await
            .unwrap();
        let mut server = h2::server::Builder::new()
            .handshake::<_, bytes::Bytes>(server_io)
            .await
            .unwrap();
        tokio::spawn(async move {
            let _ = client_driver.await;
        });

        let large_value = "x".repeat(16 * 1024);
        let mut request = http::Request::builder()
            .method("POST")
            .uri("/hpack-boundary")
            .body(())
            .unwrap();
        request
            .headers_mut()
            .insert("x-empty", http::HeaderValue::from_static(""));
        request
            .headers_mut()
            .append("x-repeat", http::HeaderValue::from_static("first"));
        request
            .headers_mut()
            .append("x-repeat", http::HeaderValue::from_static("second"));
        request.headers_mut().insert(
            "x-large",
            http::HeaderValue::from_str(&large_value).unwrap(),
        );

        let (_response, _send_stream) = client.send_request(request, true).unwrap();
        let (received, _respond) = server.accept().await.unwrap().unwrap();
        assert_eq!(received.headers()["x-empty"], "");
        assert_eq!(
            received
                .headers()
                .get_all("x-repeat")
                .iter()
                .map(|value| value.to_str().unwrap())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(received.headers()["x-large"], large_value);
    }

    #[tokio::test]
    async fn http2_underlay_roundtrips_binary_bytes() {
        let (client, server) = duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            super::accept_http2_underlay(
                server,
                "/espejismo",
                super::Http2UnderlayOptions::default(),
            )
            .await
            .unwrap()
        });
        let mut client = connect_http2_underlay(
            client,
            "example.com",
            "/espejismo",
            super::Http2UnderlayOptions::default(),
        )
        .await
        .unwrap();
        let mut server = server_task.await.unwrap();

        client.write_all(b"hello over h2").await.unwrap();
        let mut received = vec![0_u8; "hello over h2".len()];
        server.read_exact(&mut received).await.unwrap();
        assert_eq!(&received, b"hello over h2");

        server.write_all(b"reply").await.unwrap();
        let mut reply = [0_u8; 5];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"reply");
    }

    // A peer RST_STREAM must terminate the adapter's application read side.
    // Use in-memory transport so this protocol edge case also runs in the sandbox.
    #[tokio::test]
    async fn http2_underlay_closes_reader_after_peer_reset() {
        let (client_io, server_io) = duplex(64 * 1024);
        let (mut client_conn, client_driver) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(client_io)
            .await
            .unwrap();
        let mut server_conn = h2::server::Builder::new()
            .handshake::<_, bytes::Bytes>(server_io)
            .await
            .unwrap();
        tokio::spawn(async move {
            let _ = client_driver.await;
        });

        let request = http::Request::builder()
            .method("POST")
            .uri("/reset")
            .body(())
            .unwrap();
        let (response, client_send) = client_conn.send_request(request, false).unwrap();
        let (request, mut respond) = server_conn.accept().await.unwrap().unwrap();
        drop(request);
        let mut server_send = respond
            .send_response(
                http::Response::builder().status(200).body(()).unwrap(),
                false,
            )
            .unwrap();
        tokio::spawn(async move { while server_conn.accept().await.is_some() {} });

        let response = response.await.unwrap();
        let mut app = super::spawn_http2_io(client_send, response.into_body());
        server_send.send_reset(h2::Reason::CANCEL);
        let mut byte = [0_u8; 1];
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), app.read(&mut byte))
                .await
                .expect("RST_STREAM should close the adapter reader")
                .unwrap(),
            0
        );
    }

    // h2 exposes HTTP/2 PING as a connection-level health check. Keep its
    // single-in-flight boundary covered independently of the tunnel stream.
    #[tokio::test]
    async fn http2_ping_allows_one_outstanding_probe_at_a_time() {
        let (client_io, server_io) = duplex(64 * 1024);
        let (_client, mut client_conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(client_io)
            .await
            .unwrap();
        let mut server_conn = h2::server::Builder::new()
            .handshake::<_, bytes::Bytes>(server_io)
            .await
            .unwrap();
        let mut client_ping = client_conn.ping_pong().expect("client ping handle");
        let mut server_ping = server_conn.ping_pong().expect("server ping handle");

        tokio::spawn(async move {
            let _ = client_conn.await;
        });
        tokio::spawn(async move { while server_conn.accept().await.is_some() {} });

        client_ping.send_ping(h2::Ping::opaque()).unwrap();
        assert!(client_ping.send_ping(h2::Ping::opaque()).is_err());
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            futures::future::poll_fn(|cx| client_ping.poll_pong(cx)),
        )
        .await
        .expect("PING should be acknowledged")
        .unwrap();

        // The peer's own PING must also make the round trip after the first
        // probe has completed, exercising both directions of the connection.
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            server_ping.ping(h2::Ping::opaque()),
        )
            .await
            .expect("peer PING should be acknowledged")
            .unwrap();
    }

    #[tokio::test]
    async fn http2_underlay_carries_crypto_handshake() {
        let (client, server) = duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            let mut server = super::accept_http2_underlay(
                server,
                "/espejismo",
                super::Http2UnderlayOptions::default(),
            )
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
        let mut client = connect_http2_underlay(
            client,
            "example.com",
            "/espejismo",
            super::Http2UnderlayOptions::default(),
        )
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
