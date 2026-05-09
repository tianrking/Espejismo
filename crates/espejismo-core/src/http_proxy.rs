use anyhow::{bail, Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Clone, Debug)]
pub struct HttpTarget {
    pub authority: String,
    pub prebuffer: Vec<u8>,
}

pub async fn accept_http_proxy<S>(stream: &mut S) -> Result<HttpTarget>
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
    let rest = text
        .split_once("\r\n")
        .map(|(_, rest)| rest)
        .unwrap_or("\r\n");
    let rewritten = format!("{method} {path} {version}\r\n{rest}");

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
