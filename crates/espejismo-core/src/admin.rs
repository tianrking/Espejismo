use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};

use crate::metrics::Metrics;
use crate::runtime_state::RuntimeState;

pub type AdminAction = Arc<
    dyn Fn(Option<String>) -> Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct AdminState {
    pub role: String,
    pub metrics: Metrics,
    pub runtime: RuntimeState,
    pub token: Option<String>,
    pub reload: Option<AdminAction>,
}

#[derive(Serialize)]
struct StatusResponse {
    role: String,
    version: &'static str,
    metrics: crate::metrics::MetricsSnapshot,
    runtime: crate::runtime_state::RuntimeStateSnapshot,
}

pub fn spawn_admin_server(addr: SocketAddr, state: AdminState) {
    tokio::spawn(async move {
        if let Err(err) = run_admin_server(addr, state).await {
            debug!(error = %err, "admin endpoint stopped");
        }
    });
}

async fn run_admin_server(addr: SocketAddr, state: AdminState) -> Result<()> {
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind admin endpoint {addr}"))?;
    info!(listen = %addr, role = %state.role, "admin endpoint listening");
    loop {
        let (stream, peer) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_admin_peer(stream, state).await {
                debug!(%peer, error = %err, "admin request ended");
            }
        });
    }
}

async fn handle_admin_peer(mut stream: TcpStream, state: AdminState) -> Result<()> {
    let mut buffer = Vec::with_capacity(2048);
    let mut byte = [0_u8; 1];
    while !buffer.ends_with(b"\r\n\r\n") {
        if buffer.len() >= 16 * 1024 {
            write_response(&mut stream, 431, "text/plain", b"request header too large").await?;
            return Ok(());
        }
        if stream.read_exact(&mut byte).await.is_err() {
            return Ok(());
        }
        buffer.push(byte[0]);
    }
    let request = std::str::from_utf8(&buffer).context("admin request is not UTF-8")?;
    let mut lines = request.split("\r\n");
    let request_line = lines.next().context("missing admin request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    let headers: Vec<&str> = lines.filter(|line| !line.is_empty()).collect();

    if !authorized(&headers, state.token.as_deref()) {
        write_response(&mut stream, 401, "text/plain", b"unauthorized").await?;
        return Ok(());
    }

    let content_length = content_length(&headers)?;
    let mut body = vec![0_u8; content_length];
    if content_length > 0 {
        if content_length > 1024 * 1024 {
            write_response(&mut stream, 413, "text/plain", b"request body too large").await?;
            return Ok(());
        }
        stream.read_exact(&mut body).await?;
    }

    match (method, path) {
        ("GET", "/status") => {
            let response = StatusResponse {
                role: state.role.clone(),
                version: env!("CARGO_PKG_VERSION"),
                metrics: state.metrics.snapshot(&state.role),
                runtime: state.runtime.snapshot(),
            };
            let body = serde_json::to_vec_pretty(&response)?;
            write_response(&mut stream, 200, "application/json", &body).await?;
        }
        ("GET", "/connections") => {
            let body = serde_json::to_vec_pretty(&json!({
                "role": state.role,
                "metrics": state.metrics.snapshot(&state.role),
                "runtime": state.runtime.snapshot(),
            }))?;
            write_response(&mut stream, 200, "application/json", &body).await?;
        }
        ("GET", "/metrics") => {
            let body = state.metrics.render_prometheus(&state.role);
            write_response(
                &mut stream,
                200,
                "text/plain; version=0.0.4",
                body.as_bytes(),
            )
            .await?;
        }
        ("GET", "/healthz") => {
            write_response(&mut stream, 200, "text/plain", b"ok\n").await?;
        }
        ("POST", "/reload") => {
            let Some(reload) = state.reload else {
                write_response(
                    &mut stream,
                    503,
                    "application/json",
                    br#"{"error":"reload unavailable"}"#,
                )
                .await?;
                return Ok(());
            };
            match reload(None).await {
                Ok(value) => {
                    let body = serde_json::to_vec_pretty(&value)?;
                    write_response(&mut stream, 200, "application/json", &body).await?;
                }
                Err(err) => {
                    let body = serde_json::to_vec_pretty(&json!({
                        "ok": false,
                        "error": err.to_string(),
                    }))?;
                    write_response(&mut stream, 500, "application/json", &body).await?;
                }
            }
        }
        ("POST", "/apply") => {
            let Some(reload) = state.reload else {
                write_response(
                    &mut stream,
                    503,
                    "application/json",
                    br#"{"error":"apply unavailable"}"#,
                )
                .await?;
                return Ok(());
            };
            let body = String::from_utf8(body).context("apply body is not UTF-8")?;
            match reload(Some(body)).await {
                Ok(value) => {
                    let body = serde_json::to_vec_pretty(&value)?;
                    write_response(&mut stream, 200, "application/json", &body).await?;
                }
                Err(err) => {
                    let body = serde_json::to_vec_pretty(&json!({
                        "ok": false,
                        "error": err.to_string(),
                    }))?;
                    write_response(&mut stream, 500, "application/json", &body).await?;
                }
            }
        }
        ("POST", _) => {
            write_response(&mut stream, 404, "text/plain", b"not found").await?;
        }
        ("GET", _) => {
            write_response(&mut stream, 404, "text/plain", b"not found").await?;
        }
        _ => {
            write_response(&mut stream, 405, "text/plain", b"method not allowed").await?;
        }
    }
    Ok(())
}

fn authorized(headers: &[&str], token: Option<&str>) -> bool {
    let Some(token) = token else {
        return true;
    };
    headers.iter().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            let value = value.trim();
            (name.eq_ignore_ascii_case("authorization")
                && value
                    .strip_prefix("Bearer ")
                    .is_some_and(|candidate| token_matches(candidate, token)))
                || (name.eq_ignore_ascii_case("x-espejismo-admin-token")
                    && token_matches(value, token))
        })
    })
}

fn token_matches(candidate: &str, expected: &str) -> bool {
    candidate.as_bytes().ct_eq(expected.as_bytes()).into()
}

fn content_length(headers: &[&str]) -> Result<usize> {
    let Some(value) = headers.iter().find_map(|line| {
        line.split_once(':').and_then(|(name, value)| {
            name.eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })
    }) else {
        return Ok(0);
    };
    value.parse().context("invalid content-length")
}

async fn write_response(
    stream: &mut TcpStream,
    code: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let reason = match code {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{authorized, content_length};

    #[test]
    fn authorization_accepts_bearer_and_legacy_header() {
        assert!(authorized(&[], None));
        assert!(authorized(
            &["Authorization: Bearer admin-secret"],
            Some("admin-secret")
        ));
        assert!(authorized(
            &["X-Espejismo-Admin-Token: admin-secret"],
            Some("admin-secret")
        ));
        assert!(!authorized(
            &["Authorization: Bearer wrong-secret"],
            Some("admin-secret")
        ));
    }

    #[test]
    fn content_length_defaults_to_zero_and_parses_case_insensitive_header() {
        assert_eq!(content_length(&[]).unwrap(), 0);
        assert_eq!(content_length(&["content-length: 42"]).unwrap(), 42);
        assert_eq!(content_length(&["Content-Length: 7"]).unwrap(), 7);
    }

    #[test]
    fn content_length_rejects_invalid_values() {
        assert!(content_length(&["Content-Length: nope"]).is_err());
    }
}
