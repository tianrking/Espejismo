use std::net::SocketAddr;

use anyhow::{Context, Result};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};

use crate::metrics::Metrics;

#[derive(Clone)]
pub struct AdminState {
    pub role: String,
    pub metrics: Metrics,
    pub token: Option<String>,
}

#[derive(Serialize)]
struct StatusResponse {
    role: String,
    version: &'static str,
    metrics: crate::metrics::MetricsSnapshot,
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

    if method != "GET" {
        write_response(&mut stream, 405, "text/plain", b"method not allowed").await?;
        return Ok(());
    }
    if !authorized(&headers, state.token.as_deref()) {
        write_response(&mut stream, 401, "text/plain", b"unauthorized").await?;
        return Ok(());
    }

    match path {
        "/status" => {
            let response = StatusResponse {
                role: state.role.clone(),
                version: env!("CARGO_PKG_VERSION"),
                metrics: state.metrics.snapshot(&state.role),
            };
            let body = serde_json::to_vec_pretty(&response)?;
            write_response(&mut stream, 200, "application/json", &body).await?;
        }
        "/metrics" => {
            let body = state.metrics.render_prometheus(&state.role);
            write_response(
                &mut stream,
                200,
                "text/plain; version=0.0.4",
                body.as_bytes(),
            )
            .await?;
        }
        "/healthz" => {
            write_response(&mut stream, 200, "text/plain", b"ok\n").await?;
        }
        _ => {
            write_response(&mut stream, 404, "text/plain", b"not found").await?;
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
                && value.strip_prefix("Bearer ") == Some(token))
                || (name.eq_ignore_ascii_case("x-espejismo-admin-token") && value == token)
        })
    })
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
        431 => "Request Header Fields Too Large",
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
