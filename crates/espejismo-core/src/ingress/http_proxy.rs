use anyhow::{bail, Context, Result};
use base64::Engine;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::ProxyAuth;

#[derive(Clone, Debug)]
pub struct HttpTarget {
    pub authority: String,
    pub prebuffer: Vec<u8>,
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
    let mut byte = [0_u8; 1];
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() >= 32 * 1024 {
            bail!("HTTP proxy header too large");
        }
        stream.read_exact(&mut byte).await?;
        header.push(byte[0]);
    }

    let text = std::str::from_utf8(&header).context("HTTP proxy header is not UTF-8")?;
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
            prebuffer: Vec::new(),
        });
    }

    let (authority, path) = parse_absolute_http_target(target)?;
    let rewritten = rewrite_absolute_request(method, &path, version, &header_lines);

    Ok(HttpTarget {
        authority,
        prebuffer: rewritten.into_bytes(),
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
