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
use tokio::time::{timeout, Duration};
use tracing::{debug, info};

use crate::metrics::Metrics;
use crate::runtime_state::{RuntimeState, RuntimeStateSnapshot};

const ADMIN_HEADER_TIMEOUT: Duration = Duration::from_secs(15);
const ADMIN_BODY_TIMEOUT: Duration = Duration::from_secs(15);

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
        match timeout(ADMIN_HEADER_TIMEOUT, stream.read_exact(&mut byte)).await {
            Ok(Ok(_)) => {}
            Ok(Err(_)) => return Ok(()),
            Err(_) => {
                write_response(&mut stream, 408, "text/plain", b"request timeout").await?;
                return Ok(());
            }
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
        match timeout(ADMIN_BODY_TIMEOUT, stream.read_exact(&mut body)).await {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => return Err(err.into()),
            Err(_) => {
                write_response(&mut stream, 408, "text/plain", b"request timeout").await?;
                return Ok(());
            }
        }
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
            let mut body = state.metrics.render_prometheus(&state.role);
            body.push_str(&render_runtime_prometheus(
                &state.role,
                &state.runtime.snapshot(),
            ));
            write_response(
                &mut stream,
                200,
                concat!("text/plain; version=", env!("CARGO_PKG_VERSION")),
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
                    debug!(error = %err, "admin reload failed");
                    let body = serde_json::to_vec_pretty(&json!({
                        "ok": false,
                        "error": "reload failed; check service logs",
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
                    debug!(error = %err, "admin apply failed");
                    let body = serde_json::to_vec_pretty(&json!({
                        "ok": false,
                        "error": "apply failed; check service logs",
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

fn render_runtime_prometheus(role: &str, snapshot: &RuntimeStateSnapshot) -> String {
    let mut out = String::new();
    out.push_str(
        "# HELP espejismo_tunnel_lane_active_streams Active logical streams on a local tunnel lane.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_active_streams gauge\n");
    out.push_str(
        "# HELP espejismo_tunnel_lane_streams_opened Total logical streams opened on a local tunnel lane.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_streams_opened counter\n");
    out.push_str(
        "# HELP espejismo_tunnel_lane_pending_stream_opens Logical stream opens reserved on a local tunnel lane.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_pending_stream_opens gauge\n");
    out.push_str(
        "# HELP espejismo_tunnel_lane_stream_open_failures Total stream open failures on a local tunnel lane.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_stream_open_failures counter\n");
    out.push_str(
        "# HELP espejismo_tunnel_lane_bytes_client_to_remote Total bytes sent from client to remote by lane.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_bytes_client_to_remote counter\n");
    out.push_str(
        "# HELP espejismo_tunnel_lane_bytes_remote_to_client Total bytes sent from remote to client by lane.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_bytes_remote_to_client counter\n");
    out.push_str(
        "# HELP espejismo_tunnel_lane_recent_client_to_remote_bps Recent EWMA throughput from client to remote by lane.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_recent_client_to_remote_bps gauge\n");
    out.push_str(
        "# HELP espejismo_tunnel_lane_recent_remote_to_client_bps Recent EWMA throughput from remote to client by lane.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_recent_remote_to_client_bps gauge\n");
    out.push_str(
        "# HELP espejismo_tunnel_lane_adaptive_score Current adaptive lane-selection score; lower is preferred.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_adaptive_score gauge\n");
    out.push_str("# HELP espejismo_tunnel_lane_reconnect_count Total reconnects by lane.\n");
    out.push_str("# TYPE espejismo_tunnel_lane_reconnect_count counter\n");
    out.push_str(
        "# HELP espejismo_tunnel_lane_last_open_latency_ms Last stream open latency by lane.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_last_open_latency_ms gauge\n");
    out.push_str("# HELP espejismo_tunnel_lane_last_mux_rtt_ms Last mux ping RTT by lane.\n");
    out.push_str("# TYPE espejismo_tunnel_lane_last_mux_rtt_ms gauge\n");
    out.push_str("# HELP espejismo_tunnel_lane_session_age_secs Current lane session age.\n");
    out.push_str("# TYPE espejismo_tunnel_lane_session_age_secs gauge\n");
    out.push_str(
        "# HELP espejismo_tunnel_lane_last_activity_unix_secs Last lane activity time as Unix seconds.\n",
    );
    out.push_str("# TYPE espejismo_tunnel_lane_last_activity_unix_secs gauge\n");

    for lane in &snapshot.tunnel_lanes {
        let labels = format!(
            "role=\"{}\",lane_id=\"{}\",lane_kind=\"{}\",state=\"{}\"",
            escape_label_value(role),
            lane.id,
            escape_label_value(&lane.lane),
            escape_label_value(&lane.state)
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_active_streams",
            &labels,
            lane.active_streams,
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_streams_opened",
            &labels,
            lane.streams_opened,
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_pending_stream_opens",
            &labels,
            lane.pending_stream_opens,
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_stream_open_failures",
            &labels,
            lane.stream_open_failures,
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_bytes_client_to_remote",
            &labels,
            lane.bytes_client_to_remote,
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_bytes_remote_to_client",
            &labels,
            lane.bytes_remote_to_client,
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_recent_client_to_remote_bps",
            &labels,
            lane.recent_client_to_remote_bps,
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_recent_remote_to_client_bps",
            &labels,
            lane.recent_remote_to_client_bps,
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_adaptive_score",
            &labels,
            lane.adaptive_score,
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_reconnect_count",
            &labels,
            lane.reconnect_count,
        );
        push_metric(
            &mut out,
            "espejismo_tunnel_lane_last_open_latency_ms",
            &labels,
            lane.last_open_latency_ms,
        );
        if let Some(rtt) = lane.last_mux_rtt_ms {
            push_metric(
                &mut out,
                "espejismo_tunnel_lane_last_mux_rtt_ms",
                &labels,
                rtt,
            );
        }
        if let Some(age) = lane.session_age_secs {
            push_metric(
                &mut out,
                "espejismo_tunnel_lane_session_age_secs",
                &labels,
                age,
            );
        }
        if let Some(activity) = lane.last_activity_unix_secs {
            push_metric(
                &mut out,
                "espejismo_tunnel_lane_last_activity_unix_secs",
                &labels,
                activity,
            );
        }
    }
    out
}

fn push_metric(out: &mut String, name: &str, labels: &str, value: u64) {
    out.push_str(name);
    out.push('{');
    out.push_str(labels);
    out.push_str("} ");
    out.push_str(&value.to_string());
    out.push('\n');
}

fn escape_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::{authorized, content_length, render_runtime_prometheus};
    use crate::runtime_state::{RuntimeStateSnapshot, TunnelLaneSnapshot};

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

    #[test]
    fn runtime_prometheus_includes_lane_counters() {
        let body = render_runtime_prometheus(
            "local",
            &RuntimeStateSnapshot {
                started_at_unix_secs: 1,
                config_applied_unix_secs: 1,
                tunnel_state: "connected".to_string(),
                tunnel_reconnect_count: 1,
                consecutive_failures: 0,
                recent_errors: Vec::new(),
                egress_policy_version: 1,
                tunnel_lanes: vec![TunnelLaneSnapshot {
                    id: 2,
                    lane: "bulk".to_string(),
                    state: "connected".to_string(),
                    reconnect_count: 3,
                    active_streams: 4,
                    pending_stream_opens: 2,
                    streams_opened: 5,
                    stream_open_failures: 1,
                    bytes_client_to_remote: 6,
                    bytes_remote_to_client: 7,
                    recent_client_to_remote_bps: 100,
                    recent_remote_to_client_bps: 200,
                    adaptive_score: 300,
                    last_open_latency_ms: 8,
                    last_mux_rtt_ms: Some(9),
                    mux_rtt_trend_ms: vec![9],
                    session_age_secs: Some(10),
                    last_activity_unix_secs: Some(11),
                    last_error: None,
                    last_error_unix_secs: None,
                }],
            },
        );
        assert!(body.contains(
            "espejismo_tunnel_lane_streams_opened{role=\"local\",lane_id=\"2\",lane_kind=\"bulk\",state=\"connected\"} 5"
        ));
        assert!(body.contains(
            "espejismo_tunnel_lane_pending_stream_opens{role=\"local\",lane_id=\"2\",lane_kind=\"bulk\",state=\"connected\"} 2"
        ));
        assert!(body.contains("espejismo_tunnel_lane_last_mux_rtt_ms"));
        assert!(body.contains("espejismo_tunnel_lane_recent_client_to_remote_bps"));
        assert!(body.contains("espejismo_tunnel_lane_adaptive_score"));
    }
}
