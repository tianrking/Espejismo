use anyhow::{bail, Context, Result};
use base64::Engine;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{timeout, Duration};

use super::ProxyAuth;

const HTTP_PROXY_HEADER_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug)]
pub struct HttpTarget {
    pub authority: String,
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
        if header.len() >= 32 * 1024 {
            bail!("HTTP proxy header too large");
        }
        let n = timeout(HTTP_PROXY_HEADER_TIMEOUT, stream.read(&mut read_buf))
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
            prebuffer: overflow.unwrap_or_default(),
            prebuffer_body_bytes: 0,
            content_length: None,
        });
    }

    let content_length = parse_content_length(&header_lines);
    let (authority, path) = parse_absolute_http_target(target)?;
    let rewritten = rewrite_absolute_request(method, &path, version, &header_lines);
    let mut prebuffer = rewritten.into_bytes();
    let mut prebuffer_body_bytes = 0;
    if let Some(extra) = overflow {
        prebuffer_body_bytes = extra.len();
        prebuffer.extend_from_slice(&extra);
    }

    Ok(HttpTarget {
        authority,
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
    use super::parse_content_length;

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
}
