use anyhow::{bail, Context, Result};
use base64::Engine;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{timeout, Duration};

use super::ProxyAuth;

// Bound slow local proxy clients while allowing ordinary headers to arrive incrementally.
const HTTP_PROXY_HEADER_TIMEOUT: Duration = Duration::from_secs(15);
// Match the hard cap to the bytes accepted, including the terminating CRLF pair.
const HTTP_PROXY_MAX_HEADER_SIZE: usize = 32 * 1024;

#[derive(Clone, Debug)]
pub struct HttpTarget {
    pub authority: String,
    pub method: String,
    pub path: String,
    pub prebuffer: Vec<u8>,
    pub prebuffer_body_bytes: usize,
    pub content_length: Option<u64>,
}

pub async fn accept_http_proxy<S>(stream: &mut S) -> Result<HttpTarget>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    accept_http_proxy_with_auth(stream, None).await
}

pub async fn accept_http_proxy_with_auth<S>(
    stream: &mut S,
    auth: Option<&ProxyAuth>,
) -> Result<HttpTarget>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut header = Vec::with_capacity(2048);
    let mut read_buf = [0_u8; 2048];
    loop {
        let remaining = HTTP_PROXY_MAX_HEADER_SIZE.saturating_sub(header.len());
        if remaining == 0 {
            bail!("HTTP proxy header too large");
        }
        let read_len = remaining.min(read_buf.len());
        let n = timeout(
            HTTP_PROXY_HEADER_TIMEOUT,
            stream.read(&mut read_buf[..read_len]),
        )
        .await
        .context("HTTP proxy header read timeout")??;
        if n == 0 {
            bail!("HTTP proxy connection closed before headers complete");
        }
        let prev_len = header.len();
        header.extend_from_slice(&read_buf[..n]);
        if let Some(pos) = find_header_end(&header, prev_len.saturating_sub(3)) {
            let header_end = pos + 4;
            let overflow = if header_end < header.len() {
                let extra = header[header_end..].to_vec();
                header.truncate(header_end);
                Some(extra)
            } else {
                None
            };
            return parse_and_respond(stream, &header, auth, overflow).await;
        }
    }
}

async fn parse_and_respond<S>(
    stream: &mut S,
    header: &[u8],
    auth: Option<&ProxyAuth>,
    overflow: Option<Vec<u8>>,
) -> Result<HttpTarget>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let text = std::str::from_utf8(header).context("HTTP proxy header is not UTF-8")?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().context("missing HTTP request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().context("missing HTTP method")?;
    let target = parts.next().context("missing HTTP target")?;
    let version = parts.next().unwrap_or("HTTP/1.1");
    let header_lines: Vec<&str> = lines.filter(|line| !line.is_empty()).collect();

    if let Some(auth) = auth {
        if !has_valid_proxy_auth(&header_lines, auth) {
            stream
                .write_all(
                    b"HTTP/1.1 407 Proxy Authentication Required\r\n\
                      Proxy-Authenticate: Basic realm=\"Espejismo\"\r\n\
                      Content-Length: 0\r\n\r\n",
                )
                .await?;
            bail!("HTTP proxy authentication failed");
        }
    }

    if method.eq_ignore_ascii_case("CONNECT") {
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        return Ok(HttpTarget {
            authority: target.to_string(),
            method: method.to_string(),
            path: String::new(),
            prebuffer: overflow.unwrap_or_default(),
            prebuffer_body_bytes: 0,
            content_length: None,
        });
    }

    validate_transfer_encoding(&header_lines)?;
    let content_length = parse_content_length(&header_lines);
    let (authority, path) = parse_absolute_http_target(target)?;
    let rewritten = rewrite_absolute_request(method, &path, version, &header_lines);
    let mut prebuffer = rewritten.into_bytes();
    let mut prebuffer_body_bytes = 0;
    if let Some(extra) = overflow {
        // Keep already-read body bytes opaque: chunk framing and HTTP trailers
        // are forwarded downstream as part of the original request body.
        prebuffer_body_bytes = extra.len();
        prebuffer.extend_from_slice(&extra);
    }

    Ok(HttpTarget {
        authority,
        method: method.to_string(),
        path,
        prebuffer,
        prebuffer_body_bytes,
        content_length,
    })
}

fn parse_content_length(lines: &[&str]) -> Option<u64> {
    lines.iter().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<u64>().ok())
            .flatten()
    })
}

// This proxy forwards request bodies without decoding them. Accept only the
// framing it can safely account for, and reject ambiguous framing before the
// request is sent to an upstream server.
fn validate_transfer_encoding(lines: &[&str]) -> Result<()> {
    let mut encodings = Vec::new();
    let mut has_content_length = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            has_content_length = true;
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            encodings.extend(value.split(',').map(str::trim));
        }
    }
    if encodings.is_empty() {
        return Ok(());
    }
    if has_content_length {
        bail!("HTTP request has both Transfer-Encoding and Content-Length");
    }
    if encodings.len() != 1 || !encodings[0].eq_ignore_ascii_case("chunked") {
        bail!("HTTP proxy only supports a single chunked Transfer-Encoding");
    }
    Ok(())
}

fn parse_absolute_http_target(target: &str) -> Result<(String, String)> {
    let without_scheme = target
        .strip_prefix("http://")
        .context("HTTP proxy only supports CONNECT or absolute http:// requests")?;
    let (authority, path) = match without_scheme.find('/') {
        Some(idx) => (&without_scheme[..idx], &without_scheme[idx..]),
        None => (without_scheme, "/"),
    };
    if authority.is_empty() {
        bail!("empty HTTP authority");
    }
    let authority = if authority.contains(':') {
        authority.to_string()
    } else {
        format!("{authority}:80")
    };
    Ok((authority, path.to_string()))
}

fn has_valid_proxy_auth(lines: &[&str], auth: &ProxyAuth) -> bool {
    let Some(value) = lines.iter().find_map(|line| {
        line.split_once(':').and_then(|(name, value)| {
            name.eq_ignore_ascii_case("proxy-authorization")
                .then_some(value.trim())
        })
    }) else {
        return false;
    };

    let Some(encoded) = value
        .strip_prefix("Basic ")
        .or_else(|| value.strip_prefix("basic "))
    else {
        return false;
    };
    let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded.trim()) else {
        return false;
    };
    let Some((username, password)) = decoded.split(|byte| *byte == b':').next().and_then(|user| {
        let password_start = user.len() + 1;
        (password_start <= decoded.len())
            .then_some((&decoded[..user.len()], &decoded[password_start..]))
    }) else {
        return false;
    };
    auth.matches(username, password)
}

fn rewrite_absolute_request(method: &str, path: &str, version: &str, lines: &[&str]) -> String {
    let mut rewritten = format!("{method} {path} {version}\r\n");
    for line in lines {
        if line
            .split_once(':')
            .is_some_and(|(name, _)| name.eq_ignore_ascii_case("proxy-authorization"))
        {
            continue;
        }
        // Keep Expect: 100-continue intact. The upstream server owns the
        // interim response; the bidirectional proxy path relays it to the client.
        rewritten.push_str(line);
        rewritten.push_str("\r\n");
    }
    rewritten.push_str("\r\n");
    rewritten
}

fn find_header_end(data: &[u8], search_from: usize) -> Option<usize> {
    if data.len() < 4 {
        return None;
    }
    let start = search_from.min(data.len().saturating_sub(4));
    (start..=data.len() - 4).find(|&i| &data[i..i + 4] == b"\r\n\r\n")
}

#[cfg(test)]
mod tests {
    use super::{accept_http_proxy, parse_content_length, validate_transfer_encoding};
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    #[test]
    fn parses_content_length_case_insensitively() {
        let lines = ["Host: example.test", "content-length: 1048576"];
        assert_eq!(parse_content_length(&lines), Some(1_048_576));
    }

    #[test]
    fn ignores_invalid_content_length() {
        let lines = ["Content-Length: nope"];
        assert_eq!(parse_content_length(&lines), None);
    }

    #[test]
    fn accepts_only_one_chunked_transfer_encoding() {
        assert!(validate_transfer_encoding(&["Transfer-Encoding: ChUnKeD"]).is_ok());
        for headers in [
            vec!["Transfer-Encoding: gzip, chunked"],
            vec!["Transfer-Encoding: chunked", "Transfer-Encoding: chunked"],
            vec!["Transfer-Encoding: gzip"],
            vec!["Transfer-Encoding:"],
            vec!["Transfer-Encoding: chunked", "Content-Length: 0"],
        ] {
            assert!(validate_transfer_encoding(&headers).is_err(), "{headers:?}");
        }
        assert!(validate_transfer_encoding(&["Content-Length: 0"]).is_ok());
    }

    #[tokio::test]
    async fn absolute_http_target_preserves_method_and_path() {
        let (mut client, mut proxy) = duplex(4096);
        let writer = tokio::spawn(async move {
            client
                .write_all(
                    b"GET http://example.test/files/256m.bin?mirror=hk HTTP/1.1\r\n\
                      Host: example.test\r\n\r\n",
                )
                .await
                .unwrap();
        });

        let target = accept_http_proxy(&mut proxy).await.unwrap();
        assert_eq!(target.authority, "example.test:80");
        assert_eq!(target.method, "GET");
        assert_eq!(target.path, "/files/256m.bin?mirror=hk");
        assert!(target
            .prebuffer
            .starts_with(b"GET /files/256m.bin?mirror=hk HTTP/1.1\r\nHost: example.test\r\n"));
        writer.await.unwrap();
    }

    #[tokio::test]
    async fn chunked_request_preserves_declared_and_received_trailers() {
        let (mut client, mut proxy) = duplex(4096);
        let writer = tokio::spawn(async move {
            client
                .write_all(
                    b"POST http://example.test/upload HTTP/1.1\r\n\
                      Host: example.test\r\n\
                      Transfer-Encoding: chunked\r\n\
                      Trailer: Digest, X-Request-Id\r\n\r\n\
                      3\r\nabc\r\n0\r\nDigest: sha-256=abc\r\nX-Request-Id: 7\r\n\r\n",
                )
                .await
                .unwrap();
        });

        let target = accept_http_proxy(&mut proxy).await.unwrap();
        assert_eq!(target.authority, "example.test:80");
        assert_eq!(target.content_length, None);
        let body_start = target
            .prebuffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap()
            + 4;
        assert_eq!(
            target.prebuffer_body_bytes,
            target.prebuffer.len() - body_start
        );
        assert!(target
            .prebuffer
            .windows(b"Trailer: Digest, X-Request-Id\r\n".len())
            .any(|window| window == b"Trailer: Digest, X-Request-Id\r\n"));
        assert!(target
            .prebuffer
            .ends_with(b"3\r\nabc\r\n0\r\nDigest: sha-256=abc\r\nX-Request-Id: 7\r\n\r\n"));
        writer.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_conflicting_transfer_encoding_and_content_length() {
        let (mut client, mut proxy) = duplex(4096);
        client
            .write_all(
                b"POST http://example.test/upload HTTP/1.1\r\n\
                  Transfer-Encoding: chunked\r\n\
                  Content-Length: 3\r\n\r\n\
                  3\r\nabc\r\n0\r\n\r\n",
            )
            .await
            .unwrap();

        assert!(accept_http_proxy(&mut proxy).await.is_err());
    }

    #[tokio::test]
    async fn connect_target_preserves_method_without_path() {
        let (mut client, mut proxy) = duplex(4096);
        let writer = tokio::spawn(async move {
            client
                .write_all(b"CONNECT example.test:443 HTTP/1.1\r\nHost: example.test\r\n\r\n")
                .await
                .unwrap();
            let mut response = Vec::new();
            client.read_to_end(&mut response).await.unwrap();
            response
        });

        let target = accept_http_proxy(&mut proxy).await.unwrap();
        assert_eq!(target.authority, "example.test:443");
        assert_eq!(target.method, "CONNECT");
        assert_eq!(target.path, "");
        drop(proxy);
        let response = writer.await.unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200 Connection Established\r\n\r\n"));
    }
}
