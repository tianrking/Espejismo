use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use espejismo_core::{
    connect_handshake, connect_tcp_stream, spawn_frame_transport, FrameOptions, HandshakeConfig,
    Metrics, RuntimeState, TcpConfig,
};
use futures::StreamExt;
use tokio::sync::Mutex;
use tokio_yamux::{Config as YamuxConfig, Control, Session, StreamHandle};
use tracing::debug;

pub(crate) struct TunnelManager {
    server: String,
    handshake: HandshakeConfig,
    frames: FrameOptions,
    tcp: TcpConfig,
    tunnel_buffer: usize,
    metrics: Metrics,
    runtime_state: RuntimeState,
    control: Arc<Mutex<Option<Control>>>,
}

impl TunnelManager {
    pub(crate) fn new(
        server: String,
        handshake: HandshakeConfig,
        frames: FrameOptions,
        tcp: TcpConfig,
        tunnel_buffer: usize,
        metrics: Metrics,
        runtime_state: RuntimeState,
    ) -> Self {
        Self {
            server,
            handshake,
            frames,
            tcp,
            tunnel_buffer,
            metrics,
            runtime_state,
            control: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) async fn open_stream(&self) -> Result<StreamHandle> {
        let mut guard = self.control.lock().await;
        if guard.is_none() {
            *guard = Some(
                connect_mux(
                    self.server.clone(),
                    self.handshake.clone(),
                    self.frames.clone(),
                    self.tcp.clone(),
                    self.tunnel_buffer,
                    self.metrics.clone(),
                    self.runtime_state.clone(),
                )
                .await?,
            );
        }
        if let Some(control) = guard.as_mut() {
            match control.open_stream().await {
                Ok(stream) => return Ok(stream),
                Err(err) => {
                    debug!(error = %err, "yamux stream open failed; reconnecting tunnel");
                    self.runtime_state
                        .record_error(format!("yamux stream open failed: {err}"));
                    *guard = None;
                }
            }
        }
        *guard = Some(
            connect_mux(
                self.server.clone(),
                self.handshake.clone(),
                self.frames.clone(),
                self.tcp.clone(),
                self.tunnel_buffer,
                self.metrics.clone(),
                self.runtime_state.clone(),
            )
            .await?,
        );
        guard
            .as_mut()
            .context("tunnel reconnect did not install control")?
            .open_stream()
            .await
            .context("open yamux stream after reconnect")
    }
}

async fn connect_mux(
    server: String,
    cfg: HandshakeConfig,
    options: FrameOptions,
    tcp: TcpConfig,
    tunnel_buffer: usize,
    metrics: Metrics,
    runtime_state: RuntimeState,
) -> Result<Control> {
    runtime_state.set_tunnel_state("connecting");
    apply_reconnect_backoff(&runtime_state).await;
    let mut upstream = match connect_tcp_stream(server.as_str(), &tcp).await {
        Ok(stream) => stream,
        Err(err) => {
            runtime_state.record_error(format!("connect {server}: {err}"));
            return Err(err);
        }
    };
    metrics.inc_active_physical();
    let keys = match connect_handshake(&mut upstream, &cfg).await {
        Ok(keys) => {
            metrics.inc_handshake_success();
            keys
        }
        Err(err) => {
            metrics.inc_handshake_failure();
            metrics.dec_active_physical();
            runtime_state.record_error(format!("handshake {server}: {err}"));
            return Err(err);
        }
    };
    runtime_state.record_connect_success();
    let transport = spawn_frame_transport(upstream, keys, options, tunnel_buffer);
    let mut session = Session::new_client(transport, YamuxConfig::default());
    let control = session.control();

    tokio::spawn(async move {
        while let Some(event) = session.next().await {
            if let Err(err) = event {
                debug!(error = %err, "yamux client session stopped");
                runtime_state.record_error(format!("yamux client session stopped: {err}"));
                break;
            }
        }
        runtime_state.set_tunnel_state("disconnected");
        metrics.dec_active_physical();
    });

    Ok(control)
}

async fn apply_reconnect_backoff(runtime_state: &RuntimeState) {
    let failures = runtime_state.snapshot().consecutive_failures;
    if failures == 0 {
        return;
    }
    let exponent = failures.min(6) as u32;
    let delay = Duration::from_millis(250_u64.saturating_mul(1_u64 << exponent));
    tokio::time::sleep(delay).await;
}
