//! Local administrative HTTP endpoint and runtime control actions.
//!
//! The endpoint exposes operational state and authenticated actions; it is
//! intended for a trusted local management network, not public proxy traffic.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};
use tracing::{debug, info};

use crate::metrics::Metrics;
use crate::runtime_state::{RuntimeState, RuntimeStateSnapshot};

// Bound local control-plane clients that stop sending an unauthenticated request midway.
const ADMIN_HEADER_TIMEOUT: Duration = Duration::from_secs(15);
const ADMIN_BODY_TIMEOUT: Duration = Duration::from_secs(15);
const ADMIN_ACTION_TIMEOUT: Duration = Duration::from_secs(30);
const ADMIN_MAX_CONCURRENT_CLIENTS: usize = 32;
// Admin responses are machine-readable and never need to execute or embed content.
const ADMIN_CONTENT_SECURITY_POLICY: &str =
    "default-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";

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
    let clients = Arc::new(Semaphore::new(ADMIN_MAX_CONCURRENT_CLIENTS));
    loop {
        let (stream, peer) = listener.accept().await?;
        let state = state.clone();
        let clients = clients.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_admin_peer_limited(stream, state, clients).await {
                debug!(%peer, error = %err, "admin request ended");
            }
        });
    }
}

async fn handle_admin_peer_limited<S>(
    mut stream: S,
    state: AdminState,
    clients: Arc<Semaphore>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Ok(_permit) = clients.try_acquire_owned() else {
        write_response(&mut stream, 503, "text/plain", b"admin capacity reached").await?;
        return Ok(());
    };
    handle_admin_peer(stream, state).await
}

async fn handle_admin_peer<S>(mut stream: S, state: AdminState) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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

    // Keep the fixed liveness response usable by load balancers without
    // granting unauthenticated access to runtime or administrative data.
    let health_probe = is_health_probe(method, path);
    if !health_probe && !authorized(&headers, state.token.as_deref()) {
        write_response(&mut stream, 401, "text/plain", b"unauthorized").await?;
        return Ok(());
    }

    // A liveness probe has no body semantics. Answer immediately so a stale
    // or bogus Content-Length cannot make the probe wait for body bytes.
    if health_probe {
        write_response(&mut stream, 200, "text/plain", b"ok\n").await?;
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
            match run_admin_action(reload, None, ADMIN_ACTION_TIMEOUT).await {
                Err(err) if err.to_string() == "admin action timed out" => {
                    write_response(
                        &mut stream,
                        504,
                        "application/json",
                        br#"{"error":"admin action timed out"}"#,
                    )
                    .await?;
                }
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
            match run_admin_action(reload, Some(body), ADMIN_ACTION_TIMEOUT).await {
                Err(err) if err.to_string() == "admin action timed out" => {
                    write_response(
                        &mut stream,
                        504,
                        "application/json",
                        br#"{"error":"admin action timed out"}"#,
                    )
                    .await?;
                }
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

async fn run_admin_action(
    action: AdminAction,
    body: Option<String>,
    limit: Duration,
) -> Result<serde_json::Value> {
    timeout(limit, action(body))
        .await
        .map_err(|_| anyhow::anyhow!("admin action timed out"))?
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

fn is_health_probe(method: &str, path: &str) -> bool {
    method == "GET" && path == "/healthz"
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

async fn write_response<S>(stream: &mut S, code: u16, content_type: &str, body: &[u8]) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    let reason = match code {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nContent-Security-Policy: {ADMIN_CONTENT_SECURITY_POLICY}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
    use super::{
        authorized, content_length, handle_admin_peer, handle_admin_peer_limited, is_health_probe,
        render_runtime_prometheus, AdminState,
    };
    use crate::runtime_state::{RuntimeStateSnapshot, TunnelLaneSnapshot};
    use crate::{metrics::Metrics, runtime_state::RuntimeState};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Semaphore;
    use tokio::time::Duration;

    async fn request(state: AdminState, request: &str) -> String {
        let (mut client, server_stream) = duplex(4096);
        let server = tokio::spawn(async move {
            handle_admin_peer(server_stream, state).await.unwrap();
        });
        client.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        server.await.unwrap();
        String::from_utf8(response).unwrap()
    }

    #[tokio::test]
    async fn admin_action_timeout_returns_gateway_timeout() {
        let action: super::AdminAction = Arc::new(|_| {
            Box::pin(async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok(serde_json::json!({"ok": true}))
            })
        });
        let result = super::run_admin_action(action, None, Duration::from_millis(1)).await;
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn admin_client_limit_rejects_excess_connection() {
        let permits = Arc::new(Semaphore::new(1));
        let held = permits.clone().try_acquire_owned().unwrap();
        let (mut client, server_stream) = duplex(256);
        let server = tokio::spawn(async move {
            handle_admin_peer_limited(server_stream, admin_state(None), permits)
                .await
                .unwrap();
        });
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        server.await.unwrap();
        drop(held);
        let response = String::from_utf8(response).unwrap();
        assert!(response.starts_with("HTTP/1.1 503"));
        assert!(response.contains(&format!(
            "Content-Security-Policy: {}\r\n",
            super::ADMIN_CONTENT_SECURITY_POLICY
        )));
    }

    fn admin_state(reload: Option<super::AdminAction>) -> AdminState {
        AdminState {
            role: "test".to_string(),
            metrics: Metrics::default(),
            runtime: RuntimeState::default(),
            token: Some("admin-secret".to_string()),
            reload,
        }
    }

    #[tokio::test]
    async fn protected_admin_routes_reject_missing_and_invalid_credentials() {
        for path in ["/status", "/connections", "/metrics"] {
            let response = request(
                admin_state(None),
                &format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n"),
            )
            .await;
            assert!(response.starts_with("HTTP/1.1 401"), "{path}: {response}");
            assert!(response.ends_with("unauthorized"));
        }

        for auth in [
            "Authorization: Bearer wrong",
            "Authorization: Basic admin-secret",
        ] {
            let response = request(
                admin_state(None),
                &format!("GET /status HTTP/1.1\r\nHost: localhost\r\n{auth}\r\n\r\n"),
            )
            .await;
            assert!(response.starts_with("HTTP/1.1 401"), "{auth}: {response}");
        }

        let response = request(
            admin_state(None),
            "POST /apply HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\n\r\nx=1",
        )
        .await;
        assert!(response.starts_with("HTTP/1.1 401"));
    }

    #[tokio::test]
    async fn every_admin_response_has_fixed_restrictive_csp() {
        let cases = [
            ("GET /healthz HTTP/1.1\r\n\r\n", "HTTP/1.1 200"),
            ("GET /status HTTP/1.1\r\n\r\n", "HTTP/1.1 401"),
            (
                "GET /status HTTP/1.1\r\nAuthorization: Bearer admin-secret\r\nContent-Security-Policy: default-src *\r\n\r\n",
                "HTTP/1.1 200",
            ),
            (
                "GET /missing HTTP/1.1\r\nAuthorization: Bearer admin-secret\r\n\r\n",
                "HTTP/1.1 404",
            ),
            (
                "PATCH /status HTTP/1.1\r\nAuthorization: Bearer admin-secret\r\n\r\n",
                "HTTP/1.1 405",
            ),
        ];
        let expected = format!(
            "Content-Security-Policy: {}",
            super::ADMIN_CONTENT_SECURITY_POLICY
        );
        for (request_text, status) in cases {
            let response = request(admin_state(None), request_text).await;
            assert!(response.starts_with(status), "{response}");
            let (headers, _) = response.split_once("\r\n\r\n").unwrap();
            assert_eq!(headers.matches("Content-Security-Policy:").count(), 1);
            assert!(headers.lines().any(|line| line == expected), "{headers}");
        }
    }

    #[tokio::test]
    async fn only_health_is_public_and_unauthorized_apply_has_no_side_effect() {
        let calls = Arc::new(AtomicUsize::new(0));
        let action_calls = calls.clone();
        let action: super::AdminAction = Arc::new(move |_| {
            let calls = action_calls.clone();
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!({"ok": true}))
            })
        });

        let health = request(
            admin_state(Some(action.clone())),
            "GET /healthz HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await;
        assert!(health.starts_with("HTTP/1.1 200"));
        assert!(health.ends_with("ok\n"));

        let denied = request(
            admin_state(Some(action.clone())),
            "POST /apply HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\n\r\nx=1",
        )
        .await;
        assert!(denied.starts_with("HTTP/1.1 401"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let accepted = request(
            admin_state(Some(action)),
            "POST /apply HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer admin-secret\r\nContent-Length: 3\r\n\r\nx=1",
        ).await;
        assert!(accepted.starts_with("HTTP/1.1 200"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn health_probe_ignores_invalid_or_unfinished_request_body() {
        for headers in [
            "Content-Length: nope\r\n",
            "Content-Length: 16777216\r\n",
            "Content-Length: 4\r\n",
        ] {
            let response = request(
                admin_state(None),
                &format!("GET /healthz HTTP/1.1\r\nHost: localhost\r\n{headers}\r\n"),
            )
            .await;
            assert!(response.starts_with("HTTP/1.1 200"), "{headers}: {response}");
            assert!(response.ends_with("ok\n"), "{headers}: {response}");
        }
    }

    #[tokio::test]
    async fn every_admin_action_obeys_the_authorization_matrix() {
        // Keep this route list aligned with the dispatch table above: every
        // data or control endpoint must reject absent/invalid credentials.
        let routes = [
            ("GET", "/status", ""),
            ("GET", "/connections", ""),
            ("GET", "/metrics", ""),
            ("POST", "/reload", ""),
            ("POST", "/apply", "x=1"),
        ];
        let calls = Arc::new(AtomicUsize::new(0));
        let action_calls = calls.clone();
        let action: super::AdminAction = Arc::new(move |_| {
            let calls = action_calls.clone();
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!({"ok": true}))
            })
        });

        for (method, path, body) in routes {
            let content_length = if body.is_empty() {
                ""
            } else {
                "Content-Length: 3\r\n"
            };
            for credential in [None, Some("Authorization: Bearer wrong\r\n")] {
                let auth = credential.unwrap_or("");
                let response = request(
                    admin_state(Some(action.clone())),
                    &format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\n{auth}{content_length}\r\n{body}"),
                ).await;
                assert!(
                    response.starts_with("HTTP/1.1 401"),
                    "{method} {path} without valid auth: {response}"
                );
            }

            for credential in [
                "Authorization: Bearer admin-secret\r\n",
                "X-Espejismo-Admin-Token: admin-secret\r\n",
            ] {
                let response = request(
                    admin_state(Some(action.clone())),
                    &format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\n{credential}{content_length}\r\n{body}"),
                ).await;
                assert!(
                    response.starts_with("HTTP/1.1 200"),
                    "{method} {path} with valid auth: {response}"
                );
            }
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            4,
            "only authenticated control actions run"
        );

        let health = request(
            admin_state(None),
            "GET /healthz HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await;
        assert!(health.starts_with("HTTP/1.1 200"));
        assert!(health.ends_with("ok\n"));
    }

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
        assert!(!authorized(&[], Some("admin-secret")));
        assert!(!authorized(
            &["Authorization: Basic admin-secret"],
            Some("admin-secret")
        ));
        assert!(!authorized(
            &["Authorization: Bearer admin-secret-extra"],
            Some("admin-secret")
        ));
        assert!(authorized(
            &[
                "Authorization: Bearer wrong-secret",
                "X-Espejismo-Admin-Token: admin-secret"
            ],
            Some("admin-secret")
        ));
    }

    #[test]
    fn only_get_health_probe_bypasses_admin_authorization() {
        assert!(is_health_probe("GET", "/healthz"));
        assert!(!is_health_probe("POST", "/healthz"));
        assert!(!is_health_probe("GET", "/status"));
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
