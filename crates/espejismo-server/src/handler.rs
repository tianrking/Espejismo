use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use espejismo_core::{
    accept_handshake_with_users, read_tunnel_request, spawn_frame_transport, EgressPolicy, Metrics,
    ReplayCache, TunnelRequest,
};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;
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
    if should_route_to_http_fallback(&mut inbound, &settings.fallback_http).await? {
        route_http_fallback(inbound, &settings.fallback_http).await?;
        return Ok(());
    }

    metrics.inc_active_physical();
    runtime.runtime_state.set_tunnel_state("authenticating");
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

    if !settings.cold_start_delay.is_zero() {
        sleep(settings.cold_start_delay).await;
    }

    let user = keys.user;
    runtime.runtime_state.record_connect_success();
    info!(user = %user, "authenticated tunnel accepted");
    let transport = spawn_frame_transport(
        inbound,
        keys.keys,
        settings.frames.clone(),
        runtime.tunnel_buffer,
    );
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
        let idle = current.idle_timeout;
        let user = user.clone();
        tokio::spawn(async move {
            let _global_stream_permit = global_stream_permit;
            let _stream_permit = stream_permit;
            if let Err(err) = handle_mux_stream(stream, metrics, egress, limits, idle, user).await {
                debug!(error = %err, "mux stream ended");
            }
        });
    }
    metrics.dec_active_physical();
    runtime.runtime_state.set_tunnel_state("idle");
    Ok(())
}

async fn handle_mux_stream(
    mut stream: MuxStream,
    metrics: Metrics,
    egress: EgressPolicy,
    limits: UserLimitRegistry,
    idle: Duration,
    user: String,
) -> Result<()> {
    limits.ensure_open(&user).await?;
    metrics.inc_active_stream();
    metrics.inc_stream_opened();
    metrics.inc_user_stream_opened(&user);
    let result =
        handle_mux_stream_inner(&mut stream, metrics.clone(), egress, limits, idle, &user).await;
    if result.is_err() {
        metrics.inc_stream_failed();
    }
    metrics.dec_active_stream();
    result
}

async fn handle_mux_stream_inner(
    stream: &mut MuxStream,
    metrics: Metrics,
    egress: EgressPolicy,
    limits: UserLimitRegistry,
    idle: Duration,
    user: &str,
) -> Result<()> {
    match timeout(REQUEST_READ_TIMEOUT, read_tunnel_request(stream))
        .await
        .map_err(|_| anyhow::anyhow!("tunnel request read timed out"))??
    {
        TunnelRequest::TcpConnect {
            authority,
            priority,
        } => {
            let mut remote = connect_egress_tcp(&authority, &egress).await?;
            info!(target = %authority, priority = ?priority, "mux TCP relay opened");
            let (client_to_remote, remote_to_client) =
                limited_copy_bidirectional(stream, &mut remote, idle, &limits, user).await?;
            metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
            metrics.add_user_tunnel_bytes(user, client_to_remote, remote_to_client);
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
