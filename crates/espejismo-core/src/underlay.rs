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
use tokio::sync::Mutex;
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
    let (mut wire_reader, wire_writer) = split(stream);
    let wire_writer = std::sync::Arc::new(Mutex::new(wire_writer));
    let ping_writer = wire_writer.clone();

    tokio::spawn(async move {
        let mut fragmented_message: Option<Vec<u8>> = None;
        loop {
            match read_ws_frame(&mut wire_reader, role, max_frame_bytes).await {
                Ok(Some(WsFrame::Data(payload))) => {
                    if fragmented_message.is_some()
                        || app_writer.write_all(&payload).await.is_err()
                    {
                        break;
                    }
                }
                Ok(Some(WsFrame::DataStart(payload))) => {
                    if fragmented_message.is_some() || payload.len() > max_frame_bytes {
                        break;
                    }
                    fragmented_message = Some(payload);
                }
                Ok(Some(WsFrame::Continuation(payload, final_fragment))) => {
                    let Some(message) = fragmented_message.as_mut() else {
                        break;
                    };
                    if message.len().saturating_add(payload.len()) > max_frame_bytes {
                        break;
                    }
                    message.extend_from_slice(&payload);
                    if final_fragment {
                        let message = fragmented_message.take().expect("message is present");
                        if app_writer.write_all(&message).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(Some(WsFrame::Ping(payload))) => {
                    if write_ws_frame(&mut *ping_writer.lock().await, role, 0xa, &payload)
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Some(WsFrame::Close(payload))) => {
                    let _ =
                        write_ws_frame(&mut *ping_writer.lock().await, role, 0x8, &payload).await;
                    let _ = app_writer.shutdown().await;
                    break;
                }
                Ok(Some(WsFrame::Pong)) => {}
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
        let _ = app_writer.shutdown().await;
    });

    tokio::spawn(async move {
        let wire_writer = wire_writer;
        let mut buf = vec![0_u8; IO_BUFFER.min(max_frame_bytes)];
        loop {
            match app_reader.read(&mut buf).await {
                Ok(0) => {
                    let mut wire_writer = wire_writer.lock().await;
                    let _ = write_ws_frame(&mut *wire_writer, role, 0x8, &[]).await;
                    let _ = wire_writer.shutdown().await;
                    break;
                }
                Ok(n) => {
                    if let Err(err) =
                        write_ws_frame(&mut *wire_writer.lock().await, role, 0x2, &buf[..n]).await
                    {
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
                    // Returning capacity is what allows the peer to make
                    // progress after either the stream or connection window
                    // is exhausted. Treat a rejected update as a dead reader
                    // instead of silently pinning the peer at zero credit.
                    if let Err(err) = recv_stream.flow_control().release_capacity(len) {
                        debug!(error = %err, "http2 underlay failed to release receive capacity");
                        let _ = app_writer.shutdown().await;
                        break;
                    }
                }
                Some(Err(err)) => {
                    // A peer reset is a stream-local termination. Keep its
                    // HTTP/2 reason visible for diagnosis while exposing EOF
                    // to the byte-stream adapter.
                    debug!(error = %err, reason = ?err.reason(), "http2 underlay reader stopped");
                    let _ = app_writer.shutdown().await;
                    break;
                }
                None => {
                    // HTTP/2 trailers end the body but are metadata, not tunnel
                    // bytes. RecvStream::data() returning None closes the app
                    // read half after all preceding DATA has been delivered.
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
                    let mut offset = 0;
                    while offset < n {
                        send_stream.reserve_capacity(n - offset);
                        let capacity =
                            std::future::poll_fn(|cx| send_stream.poll_capacity(cx)).await;
                        let Some(Ok(capacity)) = capacity else {
                            debug!("http2 underlay writer stopped while waiting for capacity");
                            return;
                        };
                        if capacity == 0 {
                            continue;
                        }
                        let len = capacity.min(n - offset);
                        if let Err(err) = send_stream
                            .send_data(Bytes::copy_from_slice(&buf[offset..offset + len]), false)
                        {
                            debug!(error = %err, "http2 underlay writer stopped");
                            return;
                        }
                        offset += len;
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

#[derive(Debug)]
enum WsFrame {
    Data(Vec<u8>),
    DataStart(Vec<u8>),
    Continuation(Vec<u8>, bool),
    Ping(Vec<u8>),
    Pong,
    Close(Vec<u8>),
}

async fn read_ws_frame<R>(
    reader: &mut R,
    role: WebSocketRole,
    max_frame_bytes: usize,
) -> Result<Option<WsFrame>>
where
    R: AsyncRead + Unpin,
{
    let mut head = [0_u8; 2];
    reader.read_exact(&mut head).await?;
    let opcode = head[0] & 0x0f;
    ensure!(head[0] & 0x70 == 0, "websocket reserved bits are set");
    let final_fragment = head[0] & 0x80 != 0;
    let masked = head[1] & 0x80 != 0;
    let mut len = u64::from(head[1] & 0x7f);
    if len == 126 {
        len = u64::from(reader.read_u16().await?);
        ensure!(
            len >= 126,
            "websocket frame length is not minimally encoded"
        );
    } else if len == 127 {
        len = reader.read_u64().await?;
        ensure!(
            len & (1_u64 << 63) == 0,
            "websocket frame length has its high bit set"
        );
        ensure!(
            len > 65_535,
            "websocket frame length is not minimally encoded"
        );
    }
    ensure!(
        len <= max_frame_bytes as u64,
        "websocket frame exceeds configured limit"
    );
    let payload_len = usize::try_from(len).context("websocket frame length does not fit usize")?;
    if opcode >= 0x8 {
        ensure!(final_fragment, "websocket control frame is fragmented");
        ensure!(len <= 125, "websocket control frame exceeds 125 bytes");
    }
    ensure!(
        websocket_mask_matches_peer_role(masked, role),
        "websocket frame mask bit did not match peer role"
    );
    let mut mask = [0_u8; 4];
    if masked {
        reader.read_exact(&mut mask).await?;
    }
    let mut payload = vec![0_u8; payload_len];
    if !payload.is_empty() {
        reader.read_exact(&mut payload).await?;
    }
    if masked {
        apply_websocket_mask(&mut payload, &mask);
    }
    match opcode {
        0x1 => {
            // RFC 6455 text messages must contain valid UTF-8, even though
            // this byte-stream adapter only carries binary messages.
            std::str::from_utf8(&payload).context("websocket text payload is not UTF-8")?;
            bail!("websocket text frames are unsupported")
        }
        0x0 => Ok(Some(WsFrame::Continuation(payload, final_fragment))),
        0x2 if final_fragment => Ok(Some(WsFrame::Data(payload))),
        0x2 => Ok(Some(WsFrame::DataStart(payload))),
        0x8 => {
            // RFC 6455 permits an empty close payload, or a two-byte status
            // code followed by a UTF-8 reason. A one-byte code is truncated.
            ensure!(
                payload.is_empty() || payload.len() >= 2,
                "websocket close payload is truncated"
            );
            if payload.len() >= 2 {
                let code = u16::from_be_bytes([payload[0], payload[1]]);
                ensure!(
                    valid_websocket_close_code(code),
                    "invalid websocket close code {code}"
                );
                std::str::from_utf8(&payload[2..])
                    .context("websocket close reason is not UTF-8")?;
            }
            Ok(Some(WsFrame::Close(payload)))
        }
        0x9 => Ok(Some(WsFrame::Ping(payload))),
        0xa => Ok(Some(WsFrame::Pong)),
        other => bail!("unsupported websocket opcode {other}"),
    }
}

fn valid_websocket_close_code(code: u16) -> bool {
    matches!(code, 1000..=1003 | 1007..=1014 | 2000..=2999 | 3000..=4999)
}

fn apply_websocket_mask(payload: &mut [u8], mask: &[u8; 4]) {
    for (i, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[i % mask.len()];
    }
}

fn websocket_mask_matches_peer_role(masked: bool, role: WebSocketRole) -> bool {
    masked == (role == WebSocketRole::Server)
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
        apply_websocket_mask(&mut masked_payload, &mask);
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
    // This adapter does not implement WebSocket extensions. In particular,
    // accepting permessage-deflate here would make the peer interpret RSV1
    // frames differently from our byte-stream parser (and require compressor
    // state/window limits we do not negotiate).
    valid_status
        && header_contains(&headers.fields, "upgrade", "websocket")
        && header_contains(&headers.fields, "connection", "upgrade")
        && !headers.fields.contains_key("sec-websocket-extensions")
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

    #[tokio::test]
    async fn websocket_text_frames_validate_utf8_before_rejection() {
        // The adapter transports binary frames only, but malformed text must
        // still fail at the WebSocket UTF-8 boundary (RFC 6455).
        for (payload, error) in [
            (&[0xff][..], "text payload is not UTF-8"),
            (&[0xe2, 0x82][..], "text payload is not UTF-8"),
            (&[0xf0, 0x9f, 0x92][..], "text payload is not UTF-8"),
            (&[b'a', 0xc0, 0xaf][..], "text payload is not UTF-8"),
        ] {
            let mut frame = vec![0x81, payload.len() as u8];
            frame.extend_from_slice(payload);
            let (mut wire, mut peer) = duplex(32);
            peer.write_all(&frame).await.unwrap();
            drop(peer);
            let result = super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 1024)
                .await;
            assert!(result.unwrap_err().to_string().contains(error));
        }

        // UTF-8 permits Unicode control code points; valid text is rejected
        // only because the tunnel contract carries binary frames.
        let payload = "\0\u{1f}".as_bytes();
        let mut frame = vec![0x81, payload.len() as u8];
        frame.extend_from_slice(payload);
        let (mut wire, mut peer) = duplex(32);
        peer.write_all(&frame).await.unwrap();
        drop(peer);
        let error = super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 1024)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("text frames are unsupported"));
    }

    #[test]
    fn websocket_mask_xor_repeats_key_every_four_bytes() {
        let mask = [0x12, 0x34, 0x56, 0x78];
        for size in [0, 1, 3, 4, 5, 8, 9] {
            let original = vec![0xa5; size];
            let mut masked = original.clone();
            super::apply_websocket_mask(&mut masked, &mask);
            for (index, byte) in masked.iter().enumerate() {
                assert_eq!(*byte, original[index] ^ mask[index % 4], "size {size}");
            }
            super::apply_websocket_mask(&mut masked, &mask);
            assert_eq!(masked, original, "mask must be reversible at size {size}");
        }
    }

    #[tokio::test]
    async fn websocket_mask_roles_and_truncated_mask_boundaries_are_enforced() {
        // A server accepts masked client frames; a client accepts unmasked
        // server frames. The mask key must be complete before payload bytes.
        for (frame, role, should_pass) in [
            (
                &[0x82, 0x80, 1, 2, 3, 4][..],
                super::WebSocketRole::Server,
                true,
            ),
            (
                &[0x82, 0x80, 1, 2, 3][..],
                super::WebSocketRole::Server,
                false,
            ),
            (
                &[0x82, 0x80, 1, 2, 3, 4][..],
                super::WebSocketRole::Client,
                false,
            ),
            (&[0x82, 0x00][..], super::WebSocketRole::Client, true),
            (&[0x82, 0x00][..], super::WebSocketRole::Server, false),
        ] {
            let (mut wire, mut peer) = duplex(16);
            peer.write_all(frame).await.unwrap();
            drop(peer);
            let result = super::read_ws_frame(&mut wire, role, 1024).await;
            assert_eq!(
                result.is_ok(),
                should_pass,
                "frame {frame:?}, role {role:?}"
            );
        }
    }

    #[tokio::test]
    async fn websocket_masked_payload_matches_rfc_vector_and_unmasked_client_frame_is_rejected() {
        // RFC 6455's "Hello" example pins both the key byte order and the
        // repeating four-byte XOR cycle through the frame reader.
        let masked_hello = [
            0x82, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58,
        ];
        let (mut wire, mut peer) = duplex(32);
        peer.write_all(&masked_hello).await.unwrap();
        drop(peer);
        let frame = super::read_ws_frame(&mut wire, super::WebSocketRole::Server, 1024)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(frame, super::WsFrame::Data(payload) if payload == b"Hello"));

        let (mut wire, mut peer) = duplex(32);
        peer.write_all(&[0x82, 0x05, b'H', b'e', b'l', b'l', b'o'])
            .await
            .unwrap();
        drop(peer);
        assert!(
            super::read_ws_frame(&mut wire, super::WebSocketRole::Server, 1024)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn websocket_ping_payload_boundaries_roundtrip_and_reject_oversize() {
        for payload in [Vec::new(), vec![0x5a; 125]] {
            let expected = payload.clone();
            let (mut wire, mut peer) = duplex(512);
            let write = tokio::spawn(async move {
                super::write_ws_frame(&mut peer, super::WebSocketRole::Client, 0x9, &payload)
                    .await
                    .unwrap();
            });
            let frame = super::read_ws_frame(&mut wire, super::WebSocketRole::Server, 1024)
                .await
                .unwrap()
                .unwrap();
            match frame {
                super::WsFrame::Ping(actual) => assert_eq!(actual, expected),
                _ => panic!("expected PING frame"),
            }
            write.await.unwrap();
        }

        // Control frames cannot use the 16-bit extended length form.
        let (mut wire, mut peer) = duplex(512);
        peer.write_all(&[0x89, 126, 0, 126]).await.unwrap();
        assert!(
            super::read_ws_frame(&mut wire, super::WebSocketRole::Server, 1024)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn websocket_data_length_boundaries_roundtrip_and_reject_noncanonical_lengths() {
        for size in [0, 1, 125, 126, 127, 65_535, 65_536] {
            let payload = vec![0x5a; size];
            let expected = payload.clone();
            let (mut wire, mut peer) = duplex(size.saturating_mul(2).max(256));
            let writer = tokio::spawn(async move {
                super::write_ws_frame(&mut peer, super::WebSocketRole::Client, 0x2, &payload)
                    .await
                    .unwrap();
            });
            match super::read_ws_frame(&mut wire, super::WebSocketRole::Server, 65_536)
                .await
                .unwrap()
                .unwrap()
            {
                super::WsFrame::Data(actual) => assert_eq!(actual, expected, "size {size}"),
                _ => panic!("expected binary data frame"),
            }
            writer.await.unwrap();
        }

        // Lengths below 126 and 65,536 must use their shorter header form.
        for frame in [
            [0x82, 126, 0, 125].as_slice(),
            [0x82, 127, 0, 0, 0, 0, 0, 0, 0, 126].as_slice(),
        ] {
            let (mut wire, mut peer) = duplex(32);
            peer.write_all(frame).await.unwrap();
            assert!(
                super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 1024)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn websocket_fragment_frame_boundaries_are_parsed() {
        for (frame, continuation) in [(&[0x02, 0x00][..], false), (&[0x80, 0x00][..], true)] {
            let (mut wire, mut peer) = duplex(16);
            peer.write_all(frame).await.unwrap();
            let parsed = super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 1024)
                .await
                .unwrap()
                .unwrap();
            assert!(if continuation {
                matches!(parsed, super::WsFrame::Continuation(payload, true) if payload.is_empty())
            } else {
                matches!(parsed, super::WsFrame::DataStart(payload) if payload.is_empty())
            });
        }
    }

    #[tokio::test]
    async fn websocket_data_frame_size_limit_accepts_limit_and_rejects_one_over() {
        let payload = vec![0x5a; 125];
        let (mut wire, mut peer) = duplex(256);
        let writer = tokio::spawn(async move {
            super::write_ws_frame(&mut peer, super::WebSocketRole::Server, 0x2, &payload)
                .await
                .unwrap();
        });
        match super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 125)
            .await
            .unwrap()
            .unwrap()
        {
            super::WsFrame::Data(actual) => assert_eq!(actual.len(), 125),
            _ => panic!("expected binary data frame"),
        }
        writer.await.unwrap();

        // The declared length is rejected before the payload is read or allocated.
        let (mut wire, mut peer) = duplex(16);
        peer.write_all(&[0x82, 126, 0, 126]).await.unwrap();
        let error = super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 125)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("exceeds configured limit"));
    }

    #[tokio::test]
    async fn websocket_underlay_echoes_ping_payload_as_pong() {
        let (mut peer, server_wire) = duplex(1024);
        let _app = super::spawn_websocket_io(server_wire, super::WebSocketRole::Server, 1024);
        // Client PING, masked with 01 02 03 04; decoded payload is 09 08.
        peer.write_all(&[0x89, 0x82, 1, 2, 3, 4, 8, 10])
            .await
            .unwrap();
        let mut response = [0; 4];
        peer.read_exact(&mut response).await.unwrap();
        assert_eq!(response, [0x8a, 2, 9, 8]);
    }

    #[tokio::test]
    async fn websocket_ping_can_interleave_a_fragmented_binary_message() {
        let (mut peer, wire) = duplex(1024);
        let mut app = super::spawn_websocket_io(wire, super::WebSocketRole::Client, 1024);
        peer.write_all(&[0x02, 2, b'a', b'b', 0x89, 1, b'!', 0x80, 2, b'c', b'd'])
            .await
            .unwrap();

        let pong = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            super::read_ws_frame(&mut peer, super::WebSocketRole::Server, 1024),
        )
        .await
        .expect("PING between fragments should be answered")
        .unwrap()
        .unwrap();
        assert!(matches!(pong, super::WsFrame::Pong));
        let mut message = [0_u8; 4];
        app.read_exact(&mut message).await.unwrap();
        assert_eq!(&message, b"abcd");
    }

    #[tokio::test]
    async fn websocket_close_interrupts_fragment_reassembly() {
        let (mut peer, wire) = duplex(1024);
        let mut app = super::spawn_websocket_io(wire, super::WebSocketRole::Client, 1024);
        peer.write_all(&[0x02, 2, b'a', b'b', 0x88, 0])
            .await
            .unwrap();
        let close = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            super::read_ws_frame(&mut peer, super::WebSocketRole::Server, 1024),
        )
        .await
        .expect("CLOSE should be acknowledged")
        .unwrap()
        .unwrap();
        assert!(matches!(close, super::WsFrame::Close(payload) if payload.is_empty()));
        let mut byte = [0_u8; 1];
        assert_eq!(app.read(&mut byte).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn websocket_rejects_invalid_data_frame_interleaving() {
        for frames in [
            vec![0x80, 1, b'x'],
            vec![0x02, 1, b'a', 0x82, 1, b'b'],
        ] {
            let (mut peer, wire) = duplex(1024);
            let mut app = super::spawn_websocket_io(wire, super::WebSocketRole::Client, 1024);
            peer.write_all(&frames).await.unwrap();
            let mut output = Vec::new();
            tokio::time::timeout(std::time::Duration::from_secs(1), app.read_to_end(&mut output))
                .await
                .expect("invalid sequence should close app stream")
                .unwrap();
            assert!(output.is_empty(), "invalid message leaked bytes");
        }

        let (mut peer, wire) = duplex(4096);
        let mut app = super::spawn_websocket_io(wire, super::WebSocketRole::Client, 1024);
        let fragment = vec![b'x'; 600];
        peer.write_all(&[0x02, 126, 2, 88]).await.unwrap();
        peer.write_all(&fragment).await.unwrap();
        peer.write_all(&[0x80, 126, 2, 88]).await.unwrap();
        peer.write_all(&fragment).await.unwrap();
        let mut output = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(1), app.read_to_end(&mut output))
            .await
            .expect("oversized reassembled message should close app stream")
            .unwrap();
        assert!(output.is_empty(), "partial oversized message leaked bytes");
    }

    #[tokio::test]
    async fn websocket_ping_burst_is_answered_without_waiting_for_peer_pongs() {
        let (mut peer, server_wire) = duplex(1024);
        let _app = super::spawn_websocket_io(server_wire, super::WebSocketRole::Server, 1024);

        // The adapter answers each inbound PING independently; WebSocket PONG
        // frames are unsolicited acknowledgements and are not required here.
        for ping_id in 0_u8..32 {
            peer.write_all(&[0x89, 0x81, 1, 2, 3, 4, ping_id ^ 1])
                .await
                .unwrap();
            let mut response = [0_u8; 3];
            tokio::time::timeout(
                std::time::Duration::from_secs(1),
                peer.read_exact(&mut response),
            )
            .await
            .expect("each PING should receive a timely PONG")
            .unwrap();
            assert_eq!(response, [0x8a, 1, ping_id]);
        }
    }

    #[tokio::test]
    async fn websocket_incomplete_ping_read_is_bounded_by_caller_timeout() {
        let (mut wire, mut peer) = duplex(16);
        // A PING header declares one masked byte, but the peer never sends its
        // mask key. Frame parsing has no built-in deadline; callers bound it.
        peer.write_all(&[0x89, 0x81]).await.unwrap();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(10),
            super::read_ws_frame(&mut wire, super::WebSocketRole::Server, 1024),
        )
        .await;
        assert!(result.is_err(), "incomplete PING should remain pending");
    }

    #[tokio::test]
    async fn websocket_peer_close_is_acknowledged_and_disconnects_app_stream() {
        let (mut peer, server_wire) = duplex(1024);
        let mut app = super::spawn_websocket_io(server_wire, super::WebSocketRole::Client, 1024);
        // An empty server CLOSE is unmasked; the client must return a masked CLOSE.
        peer.write_all(&[0x88, 0x00]).await.unwrap();
        let close = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            super::read_ws_frame(&mut peer, super::WebSocketRole::Server, 1024),
        )
        .await
        .expect("close response should arrive")
        .unwrap()
        .unwrap();
        assert!(matches!(close, super::WsFrame::Close(payload) if payload.is_empty()));
        let mut byte = [0; 1];
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), app.read(&mut byte))
                .await
                .expect("app side should disconnect")
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn websocket_close_frame_boundaries_validate_payload() {
        for payload in [Vec::new(), [0x03, 0xe8].into(), {
            let mut payload = vec![0x03, 0xe8]; // Normal closure (1000).
            payload.extend(std::iter::repeat_n(b'a', 123));
            payload
        }] {
            let mut wire_bytes = vec![0x88, payload.len() as u8];
            wire_bytes.extend_from_slice(&payload);
            let (mut wire, mut peer) = duplex(256);
            peer.write_all(&wire_bytes).await.unwrap();
            assert!(matches!(
                super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 1024)
                    .await
                    .unwrap()
                    .unwrap(),
                super::WsFrame::Close(_)
            ));
        }

        // A close frame cannot carry half a status code, an invalid code, or
        // a non-UTF-8 reason. The 125-byte control-frame limit is inclusive.
        for payload in [
            vec![0x03],
            vec![0x03, 0xed], // 1005 is reserved and cannot appear on wire.
            vec![0x03, 0xe8, 0xff],
        ] {
            let mut wire_bytes = vec![0x88, payload.len() as u8];
            wire_bytes.extend_from_slice(&payload);
            let (mut wire, mut peer) = duplex(256);
            peer.write_all(&wire_bytes).await.unwrap();
            assert!(
                super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 1024)
                    .await
                    .is_err()
            );
        }

        // The registered and private-use ranges are accepted at their edges;
        // reserved pseudo-codes and values outside those ranges are rejected.
        for (code, valid) in [
            (999_u16, false),
            (1000, true),
            (1003, true),
            (1004, false),
            (1006, false),
            (1014, true),
            (1015, false),
            (1999, false),
            (2000, true),
            (2999, true),
            (3000, true),
            (4999, true),
            (5000, false),
        ] {
            let payload = code.to_be_bytes();
            let mut frame = vec![0x88, 2];
            frame.extend_from_slice(&payload);
            let (mut wire, mut peer) = duplex(16);
            peer.write_all(&frame).await.unwrap();
            assert_eq!(
                super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 1024)
                    .await
                    .is_ok(),
                valid,
                "close code {code} validity"
            );
        }

        let (mut wire, mut peer) = duplex(256);
        peer.write_all(&[0x88, 126, 0, 126]).await.unwrap();
        assert!(
            super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 1024)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn websocket_close_read_timeout_covers_incomplete_frame() {
        let (mut wire, mut peer) = duplex(16);
        // A close header declares a two-byte status, but the peer sends only
        // one byte. The parser must remain pending until the caller's timeout.
        peer.write_all(&[0x88, 2, 0x03]).await.unwrap();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(10),
            super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 1024),
        )
        .await;
        assert!(
            result.is_err(),
            "incomplete close should be bounded by timeout"
        );
    }

    #[tokio::test]
    async fn websocket_application_half_close_sends_close_then_eof() {
        let (mut peer, server_wire) = duplex(1024);
        let mut app = super::spawn_websocket_io(server_wire, super::WebSocketRole::Server, 1024);
        app.shutdown().await.unwrap();

        let close = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            super::read_ws_frame(&mut peer, super::WebSocketRole::Client, 1024),
        )
        .await
        .expect("local shutdown should send CLOSE")
        .unwrap()
        .unwrap();
        assert!(matches!(close, super::WsFrame::Close(payload) if payload.is_empty()));

        let mut byte = [0; 1];
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), peer.read(&mut byte))
                .await
                .expect("wire should close after CLOSE")
                .unwrap(),
            0
        );
    }

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

    // SETTINGS entries are six bytes (identifier plus value). Feed raw frames
    // after the preface so the h2 decoder's protocol validation is exercised.
    async fn http2_server_after_settings(
        stream_id: u32,
        flags: u8,
        payload: &[u8],
    ) -> (h2::server::Connection<DuplexStream, Bytes>, DuplexStream) {
        let (mut peer, server_io) = duplex(1024);
        let mut wire = HTTP2_PREFACE.to_vec();
        wire.extend_from_slice(&raw_frame(4, 0, 0, &[]));
        wire.extend_from_slice(&raw_frame(4, flags, stream_id, payload));
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

    // GOAWAY is connection-scoped (stream zero) and carries last-stream-id,
    // error code, then optional debug bytes. Keep the h2 crate's boundary
    // behavior pinned using raw frames over memory.
    #[tokio::test]
    async fn http2_goaway_rejects_invalid_stream_and_payload_boundaries() {
        for (stream_id, payload) in [
            (1, vec![0; 8]), // GOAWAY must use stream zero
            (0, vec![0; 7]),
            (0, vec![0; 6]),
        ] {
            let frame = raw_frame(7, 0, stream_id, &payload);
            let (mut server, _peer) = http2_server_after_raw_frames(&frame).await;
            let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
                .await
                .expect("malformed GOAWAY must be processed");
            assert!(result.is_none() || result.unwrap().is_err());
        }
    }

    // A minimal request opens stream 1; raw DATA frames then exercise PADDED
    // length handling in h2's frame decoder without relying on loopback sockets.
    async fn http2_server_after_padded_data(
        data_payload: &[u8],
    ) -> (h2::server::Connection<DuplexStream, Bytes>, DuplexStream) {
        let (mut peer, server_io) = duplex(1024);
        let mut wire = HTTP2_PREFACE.to_vec();
        wire.extend_from_slice(&raw_frame(4, 0, 0, &[]));
        // HPACK indexed GET, https, /, then literal :authority = "x".
        wire.extend_from_slice(&raw_frame(1, 4, 1, &[0x82, 0x87, 0x84, 0x01, 0x01, b'x']));
        wire.extend_from_slice(&raw_frame(0, 0x9, 1, data_payload));
        peer.write_all(&wire).await.unwrap();
        let connection = h2::server::Builder::new()
            .handshake::<_, Bytes>(server_io)
            .await
            .unwrap();
        (connection, peer)
    }

    #[tokio::test]
    async fn http2_data_padding_strips_boundary_padding() {
        for (payload, expected) in [
            (vec![0, b'a'], b"a".as_slice()), // PADDED flag with zero padding
            (vec![2, b'a', 0, 0], b"a".as_slice()), // maximal valid padding
            (vec![0], b"".as_slice()),        // empty data, zero padding
        ] {
            let (mut server, _peer) = http2_server_after_padded_data(&payload).await;
            let (request, mut respond) =
                tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
            respond
                .send_response(http::Response::new(()), true)
                .unwrap();
            tokio::spawn(async move { while server.accept().await.is_some() {} });
            let mut body = request.into_body();
            let mut received = Vec::new();
            while let Some(chunk) =
                tokio::time::timeout(std::time::Duration::from_secs(1), body.data())
                    .await
                    .unwrap()
            {
                let chunk = chunk.unwrap();
                received.extend_from_slice(&chunk);
                body.flow_control().release_capacity(chunk.len()).unwrap();
            }
            assert_eq!(received, expected);
        }
    }

    #[tokio::test]
    async fn http2_data_padding_rejects_length_past_payload() {
        // The pad length byte cannot exceed the remaining DATA payload.
        let (mut server, _peer) = http2_server_after_padded_data(&[2, b'x']).await;
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .unwrap();
        assert!(result.is_none() || result.unwrap().is_err());
    }

    #[tokio::test]
    async fn http2_goaway_propagates_reason_and_last_stream_id() {
        let (client_io, mut peer) = duplex(64 * 1024);
        let (mut client, driver) = h2::client::Builder::new()
            .handshake::<_, Bytes>(client_io)
            .await
            .unwrap();
        tokio::spawn(async move { let _ = driver.await; });

        // RFC 9113 §6.8: no stream (last-stream-id zero) was processed, and
        // REFUSED_STREAM must reach the rejected request as the error reason.
        let mut goaway = vec![0; 8];
        goaway[4..8].copy_from_slice(&u32::from(h2::Reason::REFUSED_STREAM).to_be_bytes());
        peer.write_all(&raw_frame(7, 0, 0, &goaway)).await.unwrap();

        // Drive the client until it observes GOAWAY; a subsequent stream is
        // outside last-stream-id and must fail with the peer's reason.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let request = http::Request::builder().uri("/").body(()).unwrap();
        let error = client.send_request(request, false).unwrap_err();
        assert!(error.is_go_away());
        assert_eq!(error.reason(), Some(h2::Reason::REFUSED_STREAM));
    }

    #[tokio::test]
    async fn http2_graceful_goaway_refuses_streams_after_shutdown() {
        let (client_io, server_io) = duplex(64 * 1024);
        let (mut client, client_driver) = h2::client::Builder::new()
            .handshake::<_, Bytes>(client_io)
            .await
            .unwrap();
        tokio::spawn(async move { let _ = client_driver.await; });
        let mut server = h2::server::Builder::new()
            .handshake::<_, Bytes>(server_io)
            .await
            .unwrap();

        let request = http::Request::builder().uri("/first").body(()).unwrap();
        let (_response, _send) = client.send_request(request, false).unwrap();
        let (first, mut respond) = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            server.accept(),
        )
        .await
        .unwrap()
        .unwrap()
        .unwrap();
        drop(first);
        respond
            .send_response(
                http::Response::builder().status(200).body(()).unwrap(),
                true,
            )
            .unwrap();

        server.graceful_shutdown();
        let server_driver = tokio::spawn(async move { while server.accept().await.is_some() {} });
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let request = http::Request::builder()
            .uri("/after-shutdown")
            .body(())
            .unwrap();
        let error = client.send_request(request, false).unwrap_err();
        assert!(error.is_go_away());
        server_driver.abort();
    }

    #[tokio::test]
    async fn http2_settings_rejects_invalid_lengths_ids_and_values() {
        // RFC 9113 requires a multiple of six bytes, stream zero, empty ACK,
        // and bounded values for ENABLE_PUSH, INITIAL_WINDOW_SIZE and
        // MAX_FRAME_SIZE. Keep malformed peers entirely in memory.
        let invalid = [
            (0, 0, vec![0; 5]),                // truncated entry
            (1, 0, vec![]),                    // SETTINGS on nonzero stream
            (0, 1, vec![0; 6]),                // ACK with payload
            (0, 0, vec![0, 2, 0, 0, 0, 2]),    // ENABLE_PUSH > 1
            (0, 0, vec![0, 4, 0x80, 0, 0, 0]), // window > 2^31 - 1
            (0, 0, vec![0, 5, 0, 0, 0, 1]),    // frame size below 16 KiB
            (0, 0, vec![0, 5, 1, 0, 0, 0]),    // frame size above 2^24 - 1
        ];
        for (stream_id, flags, payload) in invalid {
            let (mut server, _peer) = http2_server_after_settings(stream_id, flags, &payload).await;
            let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
                .await
                .expect("invalid SETTINGS should be processed");
            assert!(result.is_none() || result.unwrap().is_err(), "{payload:?}");
        }
    }

    #[tokio::test]
    async fn http2_settings_accepts_boundaries_and_ignores_unknown_ids() {
        // Lowest legal MAX_FRAME_SIZE, highest legal stream window, and an
        // extension setting unknown to this implementation are valid.
        let payload = [
            0, 4, 0x7f, 0xff, 0xff, 0xff, // INITIAL_WINDOW_SIZE = 2^31 - 1
            0, 5, 0, 0, 0x40, 0, // MAX_FRAME_SIZE = 16,384
            0xff, 0xff, 0, 0, 0, 1, // unknown setting is ignored
        ];
        let (mut server, _peer) = http2_server_after_settings(0, 0, &payload).await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), server.accept())
                .await
                .is_err(),
            "valid SETTINGS should leave the connection open"
        );
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

    #[tokio::test]
    async fn http2_priority_dependency_and_weight_boundaries_keep_connection_usable() {
        // RFC 9113 encodes weight as value - 1, so the wire range 0..=255
        // represents weights 1..=256. The high dependency bit is exclusive.
        // The adapter delegates this tree bookkeeping to h2; verify these
        // legal wire variants are accepted before a subsequent request.
        let priorities = [
            [0, 0, 0, 0, 0],      // root dependency, weight 1
            [0x80, 0, 0, 3, 255], // exclusive dependency 3, weight 256
        ];
        let mut frames = Vec::new();
        for payload in priorities {
            frames.extend_from_slice(&raw_frame(2, 0, 1, &payload));
        }
        let block = [0x82, 0x86, 0x84, 0x01, 0x01, b'x'];
        frames.extend_from_slice(&raw_frame(1, 5, 1, &block));
        let (mut server, _peer) = http2_server_after_raw_frames(&frames).await;
        let accepted = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("request after valid PRIORITY frames should be processed")
            .expect("connection should remain open")
            .expect("request headers should decode");
        assert_eq!(accepted.0.method(), http::Method::GET);
        assert_eq!(accepted.0.uri().path(), "/");
    }

    #[tokio::test]
    async fn http2_priority_cycle_and_deep_chain_do_not_break_connection() {
        // PRIORITY frames may refer to idle streams. Create a dependency cycle
        // and a long (but bounded) chain, then verify normal request decoding.
        // h2 currently accepts these frames but does not maintain a scheduling
        // tree (see its proto::connection PRIORITY handler); this pins wire
        // handling, not cycle repair or weighted scheduling behavior.
        let mut frames = Vec::new();
        frames.extend_from_slice(&raw_frame(2, 0, 1, &[0, 0, 0, 3, 15]));
        frames.extend_from_slice(&raw_frame(2, 0, 3, &[0, 0, 0, 1, 31]));

        // 64 dependency edges exercise substantially greater depth than the
        // adapter's ordinary requests without imposing an artificial maximum.
        let mut parent = 5;
        for child in (7..=133).step_by(2) {
            let mut payload = (parent as u32).to_be_bytes().to_vec();
            payload.push((child % 256) as u8);
            frames.extend_from_slice(&raw_frame(2, 0, child, &payload));
            parent = child;
        }

        let block = [0x82, 0x86, 0x84, 0x01, 0x01, b'x'];
        frames.extend_from_slice(&raw_frame(1, 5, 1, &block));
        let (mut server, _peer) = http2_server_after_raw_frames(&frames).await;
        let accepted = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("request after cycle and deep dependency chain should be processed")
            .expect("connection should remain open")
            .expect("request headers should decode");
        assert_eq!(accepted.0.method(), http::Method::GET);
        assert_eq!(accepted.0.uri().path(), "/");
    }

    // WINDOW_UPDATE has a 31-bit increment, and zero is a protocol error.
    // Exercise raw frames so the h2 decoder's connection-versus-stream
    // handling remains pinned without introducing a second frame parser.
    #[tokio::test]
    async fn http2_window_update_rejects_zero_connection_increment() {
        let frames = raw_frame(8, 0, 0, &[0, 0, 0, 0]);
        let (mut server, _peer) = http2_server_after_raw_frames(&frames).await;
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("zero connection increment should be processed");
        assert!(result.is_none() || result.unwrap().is_err());
    }

    #[tokio::test]
    async fn http2_window_update_accepts_positive_stream_increment() {
        let headers = [0x82, 0x86, 0x84, 0x01, 0x01, b'x'];
        let mut frames = raw_frame(1, 4, 1, &headers);
        frames.extend_from_slice(&raw_frame(8, 0, 1, &[0, 0, 0, 1]));
        let (mut server, _peer) = http2_server_after_raw_frames(&frames).await;
        let accepted = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("valid stream update should be processed")
            .expect("connection should remain open")
            .expect("request should be accepted");
        assert_eq!(accepted.0.uri().path(), "/");
    }

    #[tokio::test]
    async fn http2_window_update_rejects_zero_stream_increment() {
        let headers = [0x82, 0x86, 0x84, 0x01, 0x01, b'x'];
        let mut frames = raw_frame(1, 4, 1, &headers);
        frames.extend_from_slice(&raw_frame(8, 0, 1, &[0, 0, 0, 0]));
        // Open a second stream after the malformed update so the decoder's
        // response is observable through the server connection API.
        frames.extend_from_slice(&raw_frame(1, 5, 3, &headers));
        let (mut server, _peer) = http2_server_after_raw_frames(&frames).await;
        let first = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("first request should be processed");
        assert!(first.is_some());
        let next = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("zero stream increment should be rejected");
        assert!(next.is_some_and(|result| result.is_err()));
    }

    #[tokio::test]
    async fn http2_window_update_rejects_max_increment_overflow() {
        // 2^31-1 is the largest legal increment field, but adding it to the
        // default receive window overflows HTTP/2's 31-bit window limit.
        let frames = raw_frame(8, 0, 0, &[0x7f, 0xff, 0xff, 0xff]);
        let (mut server, _peer) = http2_server_after_raw_frames(&frames).await;
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("overflowing connection window should be processed");
        assert!(result.is_none() || result.unwrap().is_err());
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

    #[tokio::test]
    async fn http2_continuation_rejects_interleaved_ping() {
        let block = [0x82, 0x86, 0x84, 0x01, 0x01, b'x'];
        let mut frames = raw_frame(1, 0, 1, &block[..3]);
        // Even connection-level control frames cannot interrupt a header block.
        frames.extend_from_slice(&raw_frame(6, 0, 0, &[0; 8]));
        frames.extend_from_slice(&raw_frame(9, 4, 1, &block[3..]));
        let (mut server, _peer) = http2_server_after_raw_frames(&frames).await;
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("interleaved PING should be processed");
        assert!(result.is_none() || result.unwrap().is_err());
    }

    #[tokio::test]
    async fn http2_continuation_header_list_limit_applies_across_frames() {
        // Set a small receive limit and exceed it only after CONTINUATION.
        // The HPACK block repeats a compact indexed field representation.
        let (mut peer, server_io) = duplex(4096);
        let mut wire = HTTP2_PREFACE.to_vec();
        wire.extend_from_slice(&raw_frame(4, 0, 0, &[]));
        let block = vec![0x82; 64];
        wire.extend_from_slice(&raw_frame(1, 0, 1, &block[..32]));
        wire.extend_from_slice(&raw_frame(9, 4, 1, &block[32..]));
        peer.write_all(&wire).await.unwrap();
        let mut builder = h2::server::Builder::new();
        builder.max_header_list_size(64);
        let mut server = builder.handshake::<_, Bytes>(server_io).await.unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("oversized header list should be processed");
        assert!(result.is_none() || result.unwrap().is_err());
    }

    #[tokio::test]
    async fn http2_continuation_waits_for_completion_then_observes_disconnect() {
        let block = [0x82, 0x86, 0x84, 0x01, 0x01, b'x'];
        let (mut peer, server_io) = duplex(1024);
        let mut wire = HTTP2_PREFACE.to_vec();
        wire.extend_from_slice(&raw_frame(4, 0, 0, &[]));
        wire.extend_from_slice(&raw_frame(1, 0, 1, &block[..3]));
        peer.write_all(&wire).await.unwrap();
        let mut server = h2::server::Builder::new()
            .handshake::<_, Bytes>(server_io)
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), server.accept())
                .await
                .is_err(),
            "partial header block must not be delivered"
        );
        drop(peer);
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("closed peer should release pending header block");
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

    #[tokio::test]
    async fn http2_server_push_rejects_unsafe_promised_method() {
        let (client_io, server_io) = duplex(4096);
        let (mut client, client_driver) = h2::client::Builder::new()
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
                .unwrap();
        let pushed = http::Request::builder()
            .method("POST")
            .uri("/upload")
            .body(())
            .unwrap();
        assert!(respond.push_request(pushed).is_err());
    }

    // Push is not used by the tunnel. Pin the enabled-peer boundary as well as
    // cancellation before headers are sent: after RST_STREAM, a response cannot
    // be started on the promised stream.
    #[tokio::test]
    async fn http2_server_push_promise_can_be_cancelled() {
        let (client_io, server_io) = duplex(4096);
        let (mut client, client_driver) = h2::client::Builder::new()
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
                .unwrap();
        let pushed = http::Request::builder()
            .method("GET")
            .uri("/asset")
            .body(())
            .unwrap();
        let mut pushed_response = respond
            .push_request(pushed)
            .expect("client permits server push");
        pushed_response.send_reset(h2::Reason::CANCEL);
        assert!(pushed_response
            .send_response(
                http::Response::builder().status(200).body(()).unwrap(),
                true
            )
            .is_err());
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

        // Any extension response is unsolicited: the client does not offer
        // extensions, including permessage-deflate window/context options.
        for extension in [
            "permessage-deflate",
            "permessage-deflate; server_max_window_bits=8",
            "permessage-deflate; client_no_context_takeover; server_no_context_takeover",
            "x-unknown-extension",
            "x-unknown-extension; mode=opaque; level=7",
        ] {
            let response = super::parse_http_headers(&format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Extensions: {extension}\r\n\r\n"
            ))
            .unwrap();
            assert!(!super::websocket_response_matches(&response), "{extension}");
        }
    }

    #[tokio::test]
    async fn websocket_server_ignores_unknown_extension_offers_without_negotiating_them() {
        // An offer is optional: a server without extension support may
        // complete the base handshake, but must omit the extension response.
        let (server_io, mut peer) = duplex(4096);
        let server = tokio::spawn(async move {
            super::accept_websocket_underlay(server_io, "/espejismo", 1024).await
        });
        peer.write_all(b"GET /espejismo HTTP/1.1\r\nHost: example.com\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Extensions: x-unknown-extension; mode=opaque; level=7, permessage-deflate; server_max_window_bits=8\r\n\r\n")
            .await
            .unwrap();

        let mut response = Vec::new();
        let mut byte = [0_u8; 1];
        while !response.ends_with(b"\r\n\r\n") {
            peer.read_exact(&mut byte).await.unwrap();
            response.push(byte[0]);
        }
        let response = String::from_utf8(response).unwrap();
        assert!(response.starts_with("HTTP/1.1 101 "));
        assert!(!response.to_ascii_lowercase().contains("sec-websocket-extensions:"));
        drop(peer);
        drop(server.await.unwrap().unwrap());
    }

    #[tokio::test]
    async fn websocket_rejects_compressed_rsv1_frame() {
        // RSV1 marks a permessage-deflate payload. With no negotiated
        // extension, compressed frames must fail before reaching tunnel bytes.
        let (mut wire, mut peer) = duplex(32);
        peer.write_all(&[0xc2, 0x00]).await.unwrap();
        assert!(
            super::read_ws_frame(&mut wire, super::WebSocketRole::Client, 1024)
                .await
                .is_err()
        );
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

        let empty_cookie =
            super::parse_http_headers("GET /espejismo HTTP/1.1\r\nCookie:\t \r\n\r\n").unwrap();
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

    // END_STREAM closes only the sender's HTTP/2 half. The opposite direction
    // remains usable, and dropping the application stream releases its adapter
    // pumps instead of leaving an idle body reader behind.
    #[tokio::test]
    async fn http2_underlay_half_close_keeps_reverse_direction_alive() {
        let (client_io, server_io) = duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            super::accept_http2_underlay(
                server_io,
                "/half-close",
                super::Http2UnderlayOptions::default(),
            )
            .await
            .unwrap()
        });
        let mut client = connect_http2_underlay(
            client_io,
            "example.com",
            "/half-close",
            super::Http2UnderlayOptions::default(),
        )
        .await
        .unwrap();
        let mut server = server_task.await.unwrap();

        client.write_all(b"request half").await.unwrap();
        client.shutdown().await.unwrap();
        let mut request = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(1), server.read_to_end(&mut request))
            .await
            .expect("client END_STREAM should reach server as EOF")
            .unwrap();
        assert_eq!(request, b"request half");

        server.write_all(b"response after request EOF").await.unwrap();
        server.shutdown().await.unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(1), client.read_to_end(&mut response))
            .await
            .expect("server response should remain available after request half-close")
            .unwrap();
        assert_eq!(response, b"response after request EOF");
    }

    // Trailers terminate an HTTP/2 body but are not tunnel bytes. Exercise the
    // boundary through h2's in-memory transport; the adapter must expose only
    // DATA and then EOF, and HTTP's header-name parser rejects pseudo names as
    // trailer-field names before serialization.
    #[tokio::test]
    async fn http2_underlay_ignores_trailers_and_rejects_invalid_trailer_names() {
        let (client_io, server_io) = duplex(64 * 1024);
        let (mut client_conn, client_driver) = h2::client::Builder::new()
            .handshake::<_, Bytes>(client_io)
            .await
            .unwrap();
        let mut server_conn = h2::server::Builder::new()
            .handshake::<_, Bytes>(server_io)
            .await
            .unwrap();
        tokio::spawn(async move {
            let _ = client_driver.await;
        });

        let request = http::Request::builder()
            .method("POST")
            .uri("/trailers")
            .body(())
            .unwrap();
        let (_response, mut send) = client_conn.send_request(request, false).unwrap();
        let (request, mut respond) = server_conn.accept().await.unwrap().unwrap();
        let response_send = respond
            .send_response(
                http::Response::builder().status(200).body(()).unwrap(),
                false,
            )
            .unwrap();
        tokio::spawn(async move { while server_conn.accept().await.is_some() {} });

        send.send_data(Bytes::from_static(b"payload"), false)
            .unwrap();
        let mut trailers = http::HeaderMap::new();
        trailers.insert("x-checksum", http::HeaderValue::from_static("abc"));
        send.send_trailers(trailers).unwrap();
        let mut app = super::spawn_http2_io(response_send, request.into_body());
        let mut got = Vec::new();
        app.read_to_end(&mut got).await.unwrap();
        assert_eq!(got, b"payload");

        assert!(http::HeaderName::from_bytes(b":status").is_err());
    }

    #[tokio::test]
    async fn http2_rejects_truncated_trailer_frame() {
        let (mut peer, server_io) = duplex(1024);
        let mut wire = HTTP2_PREFACE.to_vec();
        wire.extend_from_slice(&raw_frame(4, 0, 0, &[]));
        // Complete request HEADERS, then cut off the following trailer HEADERS
        // frame after two of its advertised five HPACK bytes.
        wire.extend_from_slice(&raw_frame(1, 4, 1, &[0x83, 0x86, 0x84]));
        wire.extend_from_slice(&[0, 0, 5, 1, 4, 0, 0, 0, 1, 0x83, 0x86]);
        peer.write_all(&wire).await.unwrap();
        peer.shutdown().await.unwrap();
        let mut server = h2::server::Builder::new()
            .handshake::<_, Bytes>(server_io)
            .await
            .unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
            .await
            .expect("truncated frame must not hang");
        assert!(matches!(result, Some(Err(_)) | None));
    }

    // The smallest supported stream window is 65,535 bytes. Transfer several
    // windows in each direction to exercise exhaustion and WINDOW_UPDATE on
    // both the stream and connection flow-control paths without sockets.
    #[tokio::test]
    async fn http2_underlay_replenishes_exhausted_flow_control_windows() {
        let options = super::Http2UnderlayOptions {
            initial_stream_window_bytes: 65_535,
            initial_connection_window_bytes: 65_535,
            max_frame_bytes: 16_384,
        };
        let (client_io, server_io) = duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            super::accept_http2_underlay(server_io, "/flow", options)
                .await
                .unwrap()
        });
        let client = connect_http2_underlay(client_io, "example.com", "/flow", options)
            .await
            .unwrap();
        let server = server_task.await.unwrap();
        let (mut client_read, mut client_write) = tokio::io::split(client);
        let (mut server_read, mut server_write) = tokio::io::split(server);

        let payload: Vec<u8> = (0..512 * 1024).map(|n| (n % 251) as u8).collect();
        let expected = payload.clone();
        let writer = async {
            client_write.write_all(&payload).await.unwrap();
            client_write.shutdown().await.unwrap();
        };
        let mut received = Vec::new();
        let reader = server_read.read_to_end(&mut received);
        let ((), read_result) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(writer, reader)
        })
        .await
        .expect("first flow-control window update timed out");
        read_result.unwrap();
        assert_eq!(received, expected);

        let reply = vec![0xa5; 512 * 1024];
        let expected_reply = reply.clone();
        let writer = async {
            server_write.write_all(&reply).await.unwrap();
            server_write.shutdown().await.unwrap();
        };
        let mut received_reply = Vec::new();
        let reader = client_read.read_to_end(&mut received_reply);
        let ((), read_result) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(writer, reader)
        })
        .await
        .expect("reverse flow-control window update timed out");
        read_result.unwrap();
        assert_eq!(received_reply, expected_reply);
    }

    // Keep connection credit above the payload size so only the stream window
    // can block the sender. This isolates stream-level WINDOW_UPDATE recovery
    // from the connection-level exhaustion covered by the test above.
    #[tokio::test]
    async fn http2_underlay_recovers_from_stream_window_exhaustion() {
        let options = super::Http2UnderlayOptions {
            initial_stream_window_bytes: 65_535,
            initial_connection_window_bytes: 256 * 1024,
            max_frame_bytes: 16_384,
        };
        let (client_io, server_io) = duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            super::accept_http2_underlay(server_io, "/stream-flow", options)
                .await
                .unwrap()
        });
        let client = connect_http2_underlay(client_io, "example.com", "/stream-flow", options)
            .await
            .unwrap();
        let mut server = server_task.await.unwrap();
        let (_, mut client_write) = tokio::io::split(client);

        let payload: Vec<u8> = (0..192 * 1024).map(|n| (n % 239) as u8).collect();
        let expected = payload.clone();
        let writer = async {
            client_write.write_all(&payload).await.unwrap();
            client_write.shutdown().await.unwrap();
        };
        let mut received = Vec::new();
        let reader = server.read_to_end(&mut received);
        let ((), read_result) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(writer, reader)
        })
        .await
        .expect("stream flow-control update timed out");
        read_result.unwrap();
        assert_eq!(received, expected);
    }

    // A peer RST_STREAM must terminate the adapter's application read side.
    // Use in-memory transport so this protocol edge case also runs in the sandbox.
    #[tokio::test]
    async fn http2_underlay_closes_reader_after_unknown_peer_reset() {
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
        server_send.send_reset(h2::Reason::from(0xdead_beef));
        // Repeating a reset for the already-closed stream must not turn the
        // stream-local termination into an application read hang.
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

    #[tokio::test]
    async fn http2_rst_stream_rejects_zero_stream_and_non_four_byte_payload() {
        // RFC 9113 §6.4 requires a nonzero stream ID and exactly four payload
        // bytes. The h2 decoder owns these wire checks; assert malformed
        // frames terminate the connection promptly rather than being accepted
        // as stream resets.
        for (stream_id, payload) in [
            (0, vec![0; 4]),
            (1, vec![0; 3]),
            (1, vec![0; 5]),
            (3, vec![0; 4]), // valid framing, but stream 3 is idle
        ] {
            let wire_frame = raw_frame(3, 0, stream_id, &payload);
            let (mut server, _peer) = http2_server_after_raw_frames(&wire_frame).await;
            let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
                .await
                .expect("invalid RST_STREAM must be processed");
            assert!(
                result.is_none() || result.unwrap().is_err(),
                "stream {stream_id}, len {}",
                payload.len()
            );
        }
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
    async fn http2_ping_wait_can_be_bounded_when_peer_does_not_respond() {
        let (client_io, server_io) = duplex(64 * 1024);
        let (_client, mut client_conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(client_io)
            .await
            .unwrap();
        // Keep the peer connection undriven: it cannot read the PING or emit
        // its ACK. The application-level timeout must bound the wait.
        let _server = h2::server::Builder::new()
            .handshake::<_, bytes::Bytes>(server_io)
            .await
            .unwrap();
        let mut ping = client_conn.ping_pong().expect("client ping handle");
        tokio::spawn(async move {
            let _ = client_conn.await;
        });

        ping.send_ping(h2::Ping::opaque()).unwrap();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            futures::future::poll_fn(|cx| ping.poll_pong(cx)),
        )
        .await;
        assert!(
            result.is_err(),
            "unanswered PING must hit the caller timeout"
        );
        assert!(
            ping.send_ping(h2::Ping::opaque()).is_err(),
            "timed-out probe remains in flight until connection teardown"
        );
    }

    // ACKs are matched by h2's connection state machine. Raw ACK frames let
    // this pin behavior for unsolicited and repeated acknowledgements without
    // relying on a real socket or reaching into h2 internals.
    #[tokio::test]
    async fn http2_ping_ignores_unsolicited_and_duplicate_acks() {
        let (client_io, mut peer_io) = duplex(64 * 1024);
        let (_client, mut client_conn) = h2::client::Builder::new()
            .handshake::<_, bytes::Bytes>(client_io)
            .await
            .unwrap();
        let mut ping = client_conn.ping_pong().expect("client ping handle");
        let driver = tokio::spawn(async move { client_conn.await });

        // An ACK with no outstanding PING must not fabricate a successful
        // pong or poison the connection.
        peer_io
            .write_all(&raw_frame(6, 1, 0, &[0; 8]))
            .await
            .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            futures::future::poll_fn(|cx| ping.poll_pong(cx)),
        )
        .await
        .expect_err("unsolicited ACK must not complete a nonexistent probe");

        ping.send_ping(h2::Ping::opaque()).unwrap();
        // Consume the client preface and frames until its PING is on the wire;
        // SETTINGS and other control frames may precede it.
        let mut preface = vec![0; HTTP2_PREFACE.len()];
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            peer_io.read_exact(&mut preface),
        )
        .await
        .expect("client preface should arrive")
        .unwrap();
        assert_eq!(preface, HTTP2_PREFACE);
        let ping_payload = loop {
            let mut header = [0; 9];
            tokio::time::timeout(
                std::time::Duration::from_secs(1),
                peer_io.read_exact(&mut header),
            )
            .await
            .expect("client frame should arrive")
            .unwrap();
            let len = (usize::from(header[0]) << 16)
                | (usize::from(header[1]) << 8)
                | usize::from(header[2]);
            let mut payload = vec![0; len];
            peer_io.read_exact(&mut payload).await.unwrap();
            if header[3] == 6 && header[4] & 1 == 0 {
                break payload;
            }
        };
        peer_io
            .write_all(&raw_frame(6, 1, 0, &ping_payload))
            .await
            .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            futures::future::poll_fn(|cx| ping.poll_pong(cx)),
        )
        .await
        .expect("matching ACK should arrive")
        .unwrap();

        // The same ACK after the probe completed is now unsolicited.
        peer_io
            .write_all(&raw_frame(6, 1, 0, &ping_payload))
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                futures::future::poll_fn(|cx| ping.poll_pong(cx)),
            )
            .await
            .is_err(),
            "duplicate ACK must not produce a second pong"
        );
        drop(peer_io);
        let _ = driver.await;
    }

    #[tokio::test]
    async fn http2_ping_rejects_non_eight_byte_payloads() {
        // RFC 9113 §6.7 requires PING payloads to be exactly eight octets.
        // Feed malformed frames to h2's decoder over memory (no loopback).
        for len in [0, 7, 9] {
            let frame = raw_frame(6, 0, 0, &vec![0; len]);
            let (mut server, _peer) = http2_server_after_raw_frames(&frame).await;
            let result = tokio::time::timeout(std::time::Duration::from_secs(1), server.accept())
                .await
                .expect("malformed PING must be processed");
            assert!(
                result.is_none() || result.unwrap().is_err(),
                "PING payload length {len} must terminate the connection"
            );
        }
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
