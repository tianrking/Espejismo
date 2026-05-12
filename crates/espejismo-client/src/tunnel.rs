use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use espejismo_core::{
    connect_handshake, connect_tcp_stream, spawn_frame_transport, FrameOptions, HandshakeConfig,
    Metrics, RuntimeState, StreamPriority, TcpConfig, TransportConnector, TransportTarget,
    TunnelLaneSnapshot, TunnelPoolConfig,
};
use futures::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;
use tracing::debug;

use crate::mux::{client_session, MuxControl, MuxRuntimeConfig, MuxStream};

const LANE_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

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
    last_mux_rtt_ms: Option<u64>,
    mux_rtt_trend_ms: VecDeque<u64>,
    connected_at: Option<Instant>,
    last_error: Option<String>,
}

struct TunnelLane {
    id: usize,
    kind: LaneKind,
    control: Mutex<Option<MuxControl>>,
    connect_lock: Mutex<()>,
    health: Mutex<LaneHealth>,
}

pub(crate) struct TunnelManager {
    server: String,
    handshake: HandshakeConfig,
    frames: FrameOptions,
    mux: MuxRuntimeConfig,
    tunnel_buffer: usize,
    max_reconnect_attempts: u32,
    max_connection_age: Duration,
    metrics: Metrics,
    runtime_state: RuntimeState,
    connector: Arc<dyn TransportConnector>,
    lanes: Vec<Arc<TunnelLane>>,
}

pub(crate) struct TunnelService {
    inner: RwLock<Arc<TunnelManager>>,
}

pub(crate) struct TunnelManagerConfig {
    pub(crate) server: String,
    pub(crate) handshake: HandshakeConfig,
    pub(crate) frames: FrameOptions,
    pub(crate) tcp: TcpConfig,
    pub(crate) mux: MuxRuntimeConfig,
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

impl TunnelService {
    pub(crate) fn new(manager: TunnelManager) -> Self {
        Self {
            inner: RwLock::new(Arc::new(manager)),
        }
    }

    pub(crate) async fn replace(&self, manager: TunnelManager) {
        *self.inner.write().await = Arc::new(manager);
    }

    pub(crate) async fn open_stream(&self, priority: StreamPriority) -> Result<TunnelStream> {
        let manager = self.inner.read().await.clone();
        manager.open_stream(priority).await
    }

    pub(crate) async fn record_stream_bytes(
        &self,
        lane_id: usize,
        client_to_remote: u64,
        remote_to_client: u64,
    ) {
        let manager = self.inner.read().await.clone();
        manager
            .record_stream_bytes(lane_id, client_to_remote, remote_to_client)
            .await;
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
        let mut frames = config.frames;
        frames.metrics = Some(metrics.clone());
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
                    connect_lock: Mutex::new(()),
                    health: Mutex::new(LaneHealth::default()),
                })
            })
            .collect();
        Self {
            server: config.server,
            handshake: config.handshake,
            frames,
            mux: config.mux,
            tunnel_buffer: config.tunnel_buffer,
            max_reconnect_attempts: config.pool.max_reconnect_attempts.max(1),
            max_connection_age: Duration::from_secs(config.pool.max_connection_age_secs.max(1)),
            metrics,
            runtime_state,
            connector: Arc::new(TcpTransportConnector {
                options: config.tcp,
            }),
            lanes,
        }
    }

    pub(crate) async fn open_stream(&self, priority: StreamPriority) -> Result<TunnelStream> {
        let lane = self
            .select_lane(priority)
            .context("no tunnel lanes configured")?;
        let lane_id = lane.id;
        let inner = self.open_stream_on_lane(lane, priority).await?;
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

    async fn open_stream_on_lane(
        &self,
        lane: Arc<TunnelLane>,
        priority: StreamPriority,
    ) -> Result<MuxStream> {
        let started = Instant::now();
        let max_attempts = self.max_reconnect_attempts.max(1);
        for attempt in 1..=max_attempts {
            if let Err(err) = self.ensure_lane_control(lane.clone()).await {
                self.metrics.inc_stream_failed_reason("lane_connect");
                return Err(err);
            }
            let result = {
                let mut guard = lane.control.lock().await;
                let Some(control) = guard.as_mut() else {
                    continue;
                };
                control.open_stream_with_priority(priority).await
            };
            match result {
                Ok(stream) => {
                    self.record_open_success(&lane, started.elapsed()).await;
                    return Ok(stream);
                }
                Err(err) => {
                    {
                        let mut guard = lane.control.lock().await;
                        *guard = None;
                    }
                    self.record_lane_error(
                        &lane,
                        format!("mux stream open attempt {attempt}/{max_attempts} failed: {err}"),
                    )
                    .await;
                    self.metrics.inc_stream_failed_reason("mux_open");
                }
            }
        }
        anyhow::bail!("mux stream open failed after {max_attempts} attempts")
    }

    async fn ensure_lane_control(&self, lane: Arc<TunnelLane>) -> Result<()> {
        if self.lane_connection_expired(&lane).await {
            *lane.control.lock().await = None;
        }
        if lane.control.lock().await.is_some() {
            return Ok(());
        }

        let _connect_guard = lane.connect_lock.lock().await;
        if lane.control.lock().await.is_some() {
            return Ok(());
        }

        let control = self.connect_lane(lane.clone()).await?;
        *lane.control.lock().await = Some(control);
        Ok(())
    }

    async fn lane_connection_expired(&self, lane: &Arc<TunnelLane>) -> bool {
        lane.health
            .lock()
            .await
            .connected_at
            .is_some_and(|connected_at| connected_at.elapsed() >= self.max_connection_age)
    }

    async fn connect_lane(&self, lane: Arc<TunnelLane>) -> Result<MuxControl> {
        self.runtime_state.set_tunnel_state("connecting");
        apply_reconnect_backoff(&self.runtime_state).await;
        let mut upstream = match timeout(
            LANE_CONNECT_TIMEOUT,
            self.connector.connect(TransportTarget {
                endpoint: self.server.clone(),
            }),
        )
        .await
        {
            Ok(Ok(stream)) => stream,
            Ok(Err(err)) => {
                self.record_lane_error(&lane, format!("connect upstream: {err}"))
                    .await;
                return Err(err);
            }
            Err(err) => {
                let err = anyhow::anyhow!("connect upstream timed out: {err}");
                self.record_lane_error(&lane, err.to_string()).await;
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
            if health.reconnect_count > 0 {
                self.metrics.inc_session_rotation();
            }
            health.reconnect_count = health.reconnect_count.saturating_add(1);
            health.connected_at = Some(Instant::now());
            health.last_error = None;
            self.publish_lane(&lane, &health, "connected");
        }
        let mut frames = self.frames.clone();
        if frames.is_stealth() {
            frames.stealth_frame_size = frames.select_stealth_frame_size(keys.stealth_selector());
        }
        let transport = spawn_frame_transport(upstream, keys, frames, self.tunnel_buffer);
        let (control, mut session) = client_session(transport, self.mux);
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
        if let Ok(Some(rtt)) = control.ping_rtt().await {
            let mut health = lane.health.lock().await;
            let rtt_ms = rtt.as_millis().min(u64::MAX as u128) as u64;
            health.last_mux_rtt_ms = Some(rtt_ms);
            health.mux_rtt_trend_ms.push_back(rtt_ms);
            while health.mux_rtt_trend_ms.len() > 16 {
                health.mux_rtt_trend_ms.pop_front();
            }
            self.publish_lane(&lane, &health, "connected");
        }
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
            last_mux_rtt_ms: health.last_mux_rtt_ms,
            mux_rtt_trend_ms: health.mux_rtt_trend_ms.iter().copied().collect(),
            session_age_secs: health
                .connected_at
                .map(|connected_at| connected_at.elapsed().as_secs()),
            last_error: health.last_error.clone(),
        });
    }
}

#[derive(Clone)]
struct TcpTransportConnector {
    options: TcpConfig,
}

impl TransportConnector for TcpTransportConnector {
    fn connect<'a>(
        &'a self,
        target: TransportTarget,
    ) -> espejismo_core::extension::BoxFutureResult<'a, Box<dyn espejismo_core::TransportStream>>
    {
        Box::pin(async move {
            let stream = connect_tcp_stream(&target.endpoint, &self.options).await?;
            Ok(Box::new(stream) as Box<dyn espejismo_core::TransportStream>)
        })
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
