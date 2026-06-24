use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use espejismo_core::{
    accept_handshake_with_users, accept_http2_underlay, accept_websocket_underlay,
    http2_preface_matches, read_tunnel_request, spawn_frame_transport,
    websocket_upgrade_header_matches, AuthenticatedSession, EgressPolicy, Metrics, ReplayCache,
    TrafficEvent, TrafficObserver, TransportStream, TunnelRequest, UnderlayMode,
};
use futures::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::{sleep, timeout};
use tracing::{debug, info};

use crate::fallback::{fallback_or_reject, route_http_fallback, should_route_to_http_fallback};
use crate::limits::UserLimitRegistry;
use crate::mux::{server_session, MuxStream};
use crate::relay::{connect_egress_tcp, limited_copy_bidirectional, relay_udp_datagram};
use crate::tarpit;
use crate::RemoteRuntime;

const STREAM_PERMIT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) async fn handle_peer(
    mut inbound: TcpStream,
    runtime: RemoteRuntime,
    replay: Arc<tokio::sync::Mutex<ReplayCache>>,
    tarpit: tarpit::TarpitManager,
    metrics: Metrics,
) -> Result<()> {
    let settings = runtime.settings.read().await.clone();
    if settings.underlay.mode == UnderlayMode::WebSocket
        && should_accept_websocket(&mut inbound, &settings).await?
    {
        let websocket = accept_websocket_underlay(
            inbound,
            &settings.underlay.websocket.path,
            settings.underlay.websocket.max_frame_bytes,
        )
        .await?;
        return handle_websocket_peer(websocket, runtime, settings, replay, metrics).await;
    }
    if settings.underlay.mode == UnderlayMode::Http2
        && should_accept_http2(&mut inbound, &settings).await?
    {
        let http2 = accept_http2_underlay(
            inbound,
            &settings.underlay.http2.path,
            (&settings.underlay.http2).into(),
        )
        .await?;
        return handle_websocket_peer(http2, runtime, settings, replay, metrics).await;
    }

    if should_route_to_http_fallback(&mut inbound, &settings.fallback_http).await? {
        route_http_fallback(inbound, &settings.fallback_http).await?;
        return Ok(());
    }

    metrics.inc_active_physical();
    runtime.runtime_state.set_tunnel_state("authenticating");
    let handshake_started = Instant::now();
    let keys = match timeout(
        settings.handshake_timeout,
        accept_handshake_with_users(&mut inbound, &settings.users, replay),
    )
    .await
    {
        Ok(Ok(keys)) => {
            metrics.inc_handshake_success();
            metrics.inc_user_handshake_success(&keys.user);
            keys
        }
        Ok(Err(err)) => {
            metrics.inc_handshake_failure();
            metrics.dec_active_physical();
            runtime
                .runtime_state
                .record_error(format!("handshake rejected: {err}"));
            fallback_or_reject(
                inbound,
                &settings.fallback_http,
                settings.reject_delay,
                &tarpit,
            )
            .await;
            return Err(err);
        }
        Err(err) => {
            metrics.inc_handshake_failure();
            metrics.dec_active_physical();
            runtime
                .runtime_state
                .record_error(format!("handshake timeout: {err}"));
            fallback_or_reject(
                inbound,
                &settings.fallback_http,
                settings.reject_delay,
                &tarpit,
            )
            .await;
            return Err(err.into());
        }
    };
    run_authenticated_tunnel(
        inbound,
        keys,
        runtime,
        settings,
        metrics,
        handshake_started.elapsed(),
    )
    .await
}

async fn handle_websocket_peer<S>(
    mut inbound: S,
    runtime: RemoteRuntime,
    settings: crate::RemoteSettings,
    replay: Arc<tokio::sync::Mutex<ReplayCache>>,
    metrics: Metrics,
) -> Result<()>
where
    S: TransportStream + 'static,
{
    metrics.inc_active_physical();
    runtime.runtime_state.set_tunnel_state("authenticating");
    let handshake_started = Instant::now();
    let keys = match timeout(
        settings.handshake_timeout,
        accept_handshake_with_users(&mut inbound, &settings.users, replay),
    )
    .await
    {
        Ok(Ok(keys)) => {
            metrics.inc_handshake_success();
            metrics.inc_user_handshake_success(&keys.user);
            keys
        }
        Ok(Err(err)) => {
            metrics.inc_handshake_failure();
            metrics.dec_active_physical();
            runtime
                .runtime_state
                .record_error(format!("websocket handshake rejected: {err}"));
            return Err(err);
        }
        Err(err) => {
            metrics.inc_handshake_failure();
            metrics.dec_active_physical();
            runtime
                .runtime_state
                .record_error(format!("websocket handshake timeout: {err}"));
            return Err(err.into());
        }
    };
    run_authenticated_tunnel(
        inbound,
        keys,
        runtime,
        settings,
        metrics,
        handshake_started.elapsed(),
    )
    .await
}

async fn run_authenticated_tunnel<S>(
    inbound: S,
    keys: AuthenticatedSession,
    runtime: RemoteRuntime,
    settings: crate::RemoteSettings,
    metrics: Metrics,
    handshake_elapsed: Duration,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let cold_start_started = Instant::now();
    if !settings.cold_start_delay.is_zero() {
        sleep(settings.cold_start_delay).await;
    }
    let cold_start_elapsed = cold_start_started.elapsed();

    let user = keys.user;
    runtime.runtime_state.record_connect_success();
    info!(
        user = %user,
        handshake_ms = handshake_elapsed.as_millis(),
        cold_start_ms = cold_start_elapsed.as_millis(),
        "authenticated tunnel accepted"
    );
    let mut frames = settings.frames.clone();
    if frames.is_stealth() {
        frames.stealth_frame_size = frames.select_stealth_frame_size(keys.keys.stealth_selector());
    }
    frames.metrics = Some(metrics.clone());
    let transport = spawn_frame_transport(inbound, keys.keys, frames, runtime.tunnel_buffer);
    let mut session = server_session(transport, settings.mux);
    let stream_limit = Arc::new(Semaphore::new(settings.max_streams as usize));
    while let Some(stream) = session.next().await {
        let stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                metrics.dec_active_physical();
                runtime
                    .runtime_state
                    .record_error(format!("mux server session stopped: {err}"));
                return Err(err);
            }
        };

        if settings.max_streams == 0 {
            debug!(
                max = settings.max_streams,
                "rejecting stream: stream limit disabled"
            );
            continue;
        }

        let global_stream_permit = runtime
            .global_stream_limit
            .clone()
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("global stream limit reached"))?;
        let Ok(stream_permit) =
            timeout(STREAM_PERMIT_TIMEOUT, stream_limit.clone().acquire_owned())
                .await
                .map_err(|_| anyhow::anyhow!("stream limit wait timed out"))?
        else {
            return Err(anyhow::anyhow!("stream limit closed"));
        };
        let metrics = metrics.clone();
        let current = runtime.settings.read().await.clone();
        let egress = current.egress.clone();
        let limits = current.limits.clone();
        let traffic = current.traffic.clone();
        let idle = current.idle_timeout;
        let user = user.clone();
        tokio::spawn(async move {
            let _global_stream_permit = global_stream_permit;
            let _stream_permit = stream_permit;
            if let Err(err) =
                handle_mux_stream(stream, metrics, egress, limits, traffic, idle, user).await
            {
                debug!(error = %err, "mux stream ended");
            }
        });
    }
    metrics.dec_active_physical();
    runtime.runtime_state.set_tunnel_state("idle");
    Ok(())
}

async fn should_accept_websocket(
    stream: &mut TcpStream,
    settings: &crate::RemoteSettings,
) -> Result<bool> {
    let mut buf = vec![0_u8; 4096];
    let n = match timeout(settings.fallback_http.probe_timeout, stream.peek(&mut buf)).await {
        Ok(Ok(n)) => n,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => return Ok(false),
    };
    Ok(websocket_upgrade_header_matches(
        &buf[..n],
        &settings.underlay.websocket.path,
    ))
}

async fn should_accept_http2(
    stream: &mut TcpStream,
    settings: &crate::RemoteSettings,
) -> Result<bool> {
    let mut buf = vec![0_u8; espejismo_core::HTTP2_PREFACE.len()];
    let n = match timeout(settings.fallback_http.probe_timeout, stream.peek(&mut buf)).await {
        Ok(Ok(n)) => n,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => return Ok(false),
    };
    Ok(http2_preface_matches(&buf[..n]))
}

async fn handle_mux_stream(
    mut stream: MuxStream,
    metrics: Metrics,
    egress: EgressPolicy,
    limits: UserLimitRegistry,
    traffic: Arc<dyn TrafficObserver>,
    idle: Duration,
    user: String,
) -> Result<()> {
    limits.ensure_open(&user).await?;
    metrics.inc_active_stream();
    metrics.inc_stream_opened();
    metrics.inc_user_stream_opened(&user);
    let result = handle_mux_stream_inner(
        &mut stream,
        metrics.clone(),
        egress,
        limits,
        traffic.clone(),
        idle,
        &user,
    )
    .await;
    if let Err(err) = &result {
        let reason = classify_stream_failure(err);
        metrics.inc_stream_failed_reason(reason);
        if reason == "egress_denied" {
            metrics.inc_egress_denied();
        }
        traffic.observe(TrafficEvent {
            event: "stream_failed",
            user,
            authority: None,
            client_to_remote: 0,
            remote_to_client: 0,
            reason: Some(reason.to_string()),
        });
    }
    metrics.dec_active_stream();
    result
}

async fn handle_mux_stream_inner(
    stream: &mut MuxStream,
    metrics: Metrics,
    egress: EgressPolicy,
    limits: UserLimitRegistry,
    traffic: Arc<dyn TrafficObserver>,
    idle: Duration,
    user: &str,
) -> Result<()> {
    let stream_started = Instant::now();
    let request_started = Instant::now();
    let request = timeout(REQUEST_READ_TIMEOUT, read_tunnel_request(stream))
        .await
        .map_err(|_| anyhow::anyhow!("tunnel request read timed out"))??;
    let request_elapsed = request_started.elapsed();
    match request {
        TunnelRequest::TcpConnect {
            authority,
            priority,
        } => {
            let egress_started = Instant::now();
            let mut remote = connect_egress_tcp(&authority, &egress).await?;
            let egress_elapsed = egress_started.elapsed();
            info!(
                target = %authority,
                priority = ?priority,
                request_ms = request_elapsed.as_millis(),
                egress_connect_ms = egress_elapsed.as_millis(),
                "mux TCP relay opened"
            );
            let copy_started = Instant::now();
            let (client_to_remote, remote_to_client) =
                limited_copy_bidirectional(stream, &mut remote, idle, &limits, user).await?;
            let copy_elapsed = copy_started.elapsed();
            metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
            metrics.add_user_tunnel_bytes(user, client_to_remote, remote_to_client);
            debug!(
                target = %authority,
                user,
                priority = ?priority,
                request_ms = request_elapsed.as_millis(),
                egress_connect_ms = egress_elapsed.as_millis(),
                copy_ms = copy_elapsed.as_millis(),
                total_ms = stream_started.elapsed().as_millis(),
                client_to_remote,
                remote_to_client,
                remote_to_client_bps = throughput_bps(remote_to_client, copy_elapsed),
                "perf remote TCP stream completed"
            );
            traffic.observe(TrafficEvent {
                event: "tcp_stream_closed",
                user: user.to_string(),
                authority: Some(authority),
                client_to_remote,
                remote_to_client,
                reason: None,
            });
        }
        TunnelRequest::UdpDatagram {
            authority,
            priority,
            payload,
        } => {
            limits
                .account_and_throttle(user, payload.len() as u64)
                .await?;
            debug!(target = %authority, priority = ?priority, "mux UDP relay opened");
            let response = relay_udp_datagram(&authority, &payload, &egress, idle).await?;
            limits
                .account_and_throttle(user, response.len() as u64)
                .await?;
            metrics.add_tunnel_bytes(payload.len() as u64, response.len() as u64);
            metrics.add_user_tunnel_bytes(user, payload.len() as u64, response.len() as u64);
            traffic.observe(TrafficEvent {
                event: "udp_datagram_relayed",
                user: user.to_string(),
                authority: Some(authority.clone()),
                client_to_remote: payload.len() as u64,
                remote_to_client: response.len() as u64,
                reason: None,
            });
            anyhow::ensure!(
                response.len() <= u16::MAX as usize,
                "UDP response too large"
            );
            stream.write_u16(response.len() as u16).await?;
            stream.write_all(&response).await?;
            stream.shutdown().await?;
        }
    }
    Ok(())
}

fn throughput_bps(bytes: u64, elapsed: Duration) -> u64 {
    let nanos = elapsed.as_nanos();
    if nanos == 0 {
        return 0;
    }
    ((bytes as u128)
        .saturating_mul(8)
        .saturating_mul(1_000_000_000)
        / nanos) as u64
}

fn classify_stream_failure(err: &anyhow::Error) -> &'static str {
    let text = err.to_string();
    if text.contains("egress policy")
        || text.contains("egress host")
        || text.contains("egress port")
        || text.contains("egress IP")
        || text.contains("no allowed UDP egress")
    {
        "egress_denied"
    } else if text.contains("timed out") {
        "timeout"
    } else if text.contains("quota") {
        "quota"
    } else {
        "other"
    }
}
