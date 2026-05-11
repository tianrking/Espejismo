use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use espejismo_core::{
    connect_handshake, connect_tcp_stream, spawn_frame_transport, FrameOptions, HandshakeConfig,
    Metrics, MuxMode, RuntimeState, StreamPriority, TcpConfig, TunnelLaneSnapshot,
    TunnelPoolConfig,
};
use futures::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::Mutex;
use tracing::debug;

use crate::mux::{client_session, MuxControl, MuxStream};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaneKind {
    Interactive,
    Bulk,
}

impl LaneKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Bulk => "bulk",
        }
    }
}

#[derive(Default)]
struct LaneHealth {
    reconnect_count: u64,
    active_streams: u64,
    bytes_client_to_remote: u64,
    bytes_remote_to_client: u64,
    last_open_latency_ms: u64,
    last_error: Option<String>,
}

struct TunnelLane {
    id: usize,
    kind: LaneKind,
    control: Mutex<Option<MuxControl>>,
    health: Mutex<LaneHealth>,
}

pub(crate) struct TunnelManager {
    server: String,
    handshake: HandshakeConfig,
    frames: FrameOptions,
    tcp: TcpConfig,
    mux_mode: MuxMode,
    tunnel_buffer: usize,
    metrics: Metrics,
    runtime_state: RuntimeState,
    lanes: Vec<Arc<TunnelLane>>,
}

pub(crate) struct TunnelManagerConfig {
    pub(crate) server: String,
    pub(crate) handshake: HandshakeConfig,
    pub(crate) frames: FrameOptions,
    pub(crate) tcp: TcpConfig,
    pub(crate) mux_mode: MuxMode,
    pub(crate) tunnel_buffer: usize,
    pub(crate) pool: TunnelPoolConfig,
}

pub(crate) struct TunnelStream {
    inner: MuxStream,
    lane_id: usize,
}

impl TunnelStream {
    pub(crate) fn lane_id(&self) -> usize {
        self.lane_id
    }
}

impl AsyncRead for TunnelStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for TunnelStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl TunnelManager {
    pub(crate) fn new(
        config: TunnelManagerConfig,
        metrics: Metrics,
        runtime_state: RuntimeState,
    ) -> Self {
        let mut kinds = Vec::new();
        for _ in 0..config.pool.interactive_lanes.max(1) {
            kinds.push(LaneKind::Interactive);
        }
        for _ in 0..config.pool.bulk_lanes {
            kinds.push(LaneKind::Bulk);
        }
        kinds.truncate(config.pool.max_connections.max(1));
        while kinds.len()
            < config
                .pool
                .min_connections
                .min(config.pool.max_connections)
                .max(1)
        {
            kinds.push(LaneKind::Interactive);
        }
        let lanes = kinds
            .into_iter()
            .enumerate()
            .map(|(id, kind)| {
                Arc::new(TunnelLane {
                    id,
                    kind,
                    control: Mutex::new(None),
                    health: Mutex::new(LaneHealth::default()),
                })
            })
            .collect();
        Self {
            server: config.server,
            handshake: config.handshake,
            frames: config.frames,
            tcp: config.tcp,
            mux_mode: config.mux_mode,
            tunnel_buffer: config.tunnel_buffer,
            metrics,
            runtime_state,
            lanes,
        }
    }

    pub(crate) async fn open_stream(&self, priority: StreamPriority) -> Result<TunnelStream> {
        let lane = self
            .select_lane(priority)
            .context("no tunnel lanes configured")?;
        let lane_id = lane.id;
        let inner = self.open_stream_on_lane(lane).await?;
        Ok(TunnelStream { inner, lane_id })
    }

    pub(crate) async fn record_stream_bytes(
        &self,
        lane_id: usize,
        client_to_remote: u64,
        remote_to_client: u64,
    ) {
        if let Some(lane) = self.lanes.iter().find(|lane| lane.id == lane_id) {
            let mut health = lane.health.lock().await;
            health.bytes_client_to_remote = health
                .bytes_client_to_remote
                .saturating_add(client_to_remote);
            health.bytes_remote_to_client = health
                .bytes_remote_to_client
                .saturating_add(remote_to_client);
            health.active_streams = health.active_streams.saturating_sub(1);
            self.publish_lane(lane, &health, "connected");
        }
    }

    async fn open_stream_on_lane(&self, lane: Arc<TunnelLane>) -> Result<MuxStream> {
        let started = Instant::now();
        let mut guard = lane.control.lock().await;
        if guard.is_none() {
            *guard = Some(self.connect_lane(lane.clone()).await?);
        }
        if let Some(control) = guard.as_mut() {
            match control.open_stream().await {
                Ok(stream) => {
                    self.record_open_success(&lane, started.elapsed()).await;
                    return Ok(stream);
                }
                Err(err) => {
                    self.record_lane_error(&lane, format!("mux stream open failed: {err}"))
                        .await;
                    *guard = None;
                }
            }
        }
        *guard = Some(self.connect_lane(lane.clone()).await?);
        let stream = guard
            .as_mut()
            .context("tunnel reconnect did not install control")?
            .open_stream()
            .await
            .context("open mux stream after reconnect")?;
        self.record_open_success(&lane, started.elapsed()).await;
        Ok(stream)
    }

    async fn connect_lane(&self, lane: Arc<TunnelLane>) -> Result<MuxControl> {
        self.runtime_state.set_tunnel_state("connecting");
        apply_reconnect_backoff(&self.runtime_state).await;
        let mut upstream = match connect_tcp_stream(self.server.as_str(), &self.tcp).await {
            Ok(stream) => stream,
            Err(err) => {
                self.record_lane_error(&lane, format!("connect {}: {err}", self.server))
                    .await;
                return Err(err);
            }
        };
        self.metrics.inc_active_physical();
        let keys = match connect_handshake(&mut upstream, &self.handshake).await {
            Ok(keys) => {
                self.metrics.inc_handshake_success();
                keys
            }
            Err(err) => {
                self.metrics.inc_handshake_failure();
                self.metrics.dec_active_physical();
                self.record_lane_error(&lane, format!("handshake {}: {err}", self.server))
                    .await;
                return Err(err);
            }
        };
        self.runtime_state.record_connect_success();
        {
            let mut health = lane.health.lock().await;
            health.reconnect_count = health.reconnect_count.saturating_add(1);
            health.last_error = None;
            self.publish_lane(&lane, &health, "connected");
        }
        let transport =
            spawn_frame_transport(upstream, keys, self.frames.clone(), self.tunnel_buffer);
        let (control, mut session) = client_session(transport, self.mux_mode);
        let metrics = self.metrics.clone();
        let runtime_state = self.runtime_state.clone();
        let lane_for_task = lane.clone();
        tokio::spawn(async move {
            while let Some(event) = session.next().await {
                if let Err(err) = event {
                    debug!(lane = lane_for_task.id, error = %err, "mux client session stopped");
                    runtime_state.record_error(format!("mux client session stopped: {err}"));
                    break;
                }
            }
            runtime_state.set_tunnel_state("disconnected");
            metrics.dec_active_physical();
        });
        Ok(control)
    }

    fn select_lane(&self, priority: StreamPriority) -> Option<Arc<TunnelLane>> {
        let preferred = match priority {
            StreamPriority::Interactive => LaneKind::Interactive,
            StreamPriority::Bulk => LaneKind::Bulk,
        };
        self.lanes
            .iter()
            .filter(|lane| lane.kind == preferred)
            .chain(self.lanes.iter())
            .min_by_key(|lane| {
                lane.health
                    .try_lock()
                    .map(|health| {
                        let penalty = u64::from(health.last_error.is_some()) * 1_000;
                        health.active_streams * 10 + health.last_open_latency_ms + penalty
                    })
                    .unwrap_or(u64::MAX / 2)
            })
            .cloned()
    }

    async fn record_open_success(&self, lane: &Arc<TunnelLane>, elapsed: Duration) {
        let mut health = lane.health.lock().await;
        health.active_streams = health.active_streams.saturating_add(1);
        health.last_open_latency_ms = elapsed.as_millis().min(u64::MAX as u128) as u64;
        health.last_error = None;
        self.publish_lane(lane, &health, "connected");
    }

    async fn record_lane_error(&self, lane: &Arc<TunnelLane>, error: String) {
        let mut health = lane.health.lock().await;
        health.last_error = Some(error.clone());
        self.publish_lane(lane, &health, "degraded");
        self.runtime_state.record_error(error);
    }

    fn publish_lane(&self, lane: &TunnelLane, health: &LaneHealth, state: &str) {
        self.runtime_state.update_tunnel_lane(TunnelLaneSnapshot {
            id: lane.id,
            lane: lane.kind.as_str().to_string(),
            state: state.to_string(),
            reconnect_count: health.reconnect_count,
            active_streams: health.active_streams,
            bytes_client_to_remote: health.bytes_client_to_remote,
            bytes_remote_to_client: health.bytes_remote_to_client,
            last_open_latency_ms: health.last_open_latency_ms,
            last_error: health.last_error.clone(),
        });
    }
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
