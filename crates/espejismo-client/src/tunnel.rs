use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use espejismo_core::{
    connect_handshake, connect_http2_underlay, connect_tcp_stream, connect_websocket_underlay,
    spawn_frame_transport, split_authority, FrameOptions, HandshakeConfig, Metrics,
    PortHoppingConfig, RuntimeState, StreamPriority, TcpConfig, TransportConnector,
    TransportTarget, TunnelLaneSnapshot, TunnelPoolConfig, UnderlayConfig, UnderlayMode,
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
    pending_stream_opens: u64,
    streams_opened: u64,
    stream_open_failures: u64,
    bytes_client_to_remote: u64,
    bytes_remote_to_client: u64,
    recent_client_to_remote_bps: u64,
    recent_remote_to_client_bps: u64,
    last_open_latency_ms: u64,
    last_mux_rtt_ms: Option<u64>,
    mux_rtt_trend_ms: VecDeque<u64>,
    connected_at: Option<Instant>,
    last_activity_unix_secs: Option<u64>,
    last_error: Option<String>,
    last_error_unix_secs: Option<u64>,
}

struct TunnelLane {
    id: usize,
    kind: LaneKind,
    control: Mutex<Option<MuxControl>>,
    connect_lock: Mutex<()>,
    health: Mutex<LaneHealth>,
    inflight_client_to_remote: AtomicU64,
    inflight_remote_to_client: AtomicU64,
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
    port_hopping: PortHoppingConfig,
    select_lock: Mutex<()>,
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
    pub(crate) underlay: UnderlayConfig,
    pub(crate) port_hopping: PortHoppingConfig,
    pub(crate) mux: MuxRuntimeConfig,
    pub(crate) tunnel_buffer: usize,
    pub(crate) pool: TunnelPoolConfig,
}

pub(crate) struct TunnelStream {
    inner: MuxStream,
    lane_id: usize,
    lane: Arc<TunnelLane>,
    runtime_state: RuntimeState,
}

pub(crate) struct MeteredTunnelStream {
    inner: TunnelStream,
    client_to_remote: u64,
    remote_to_client: u64,
}

impl TunnelStream {
    pub(crate) fn lane_id(&self) -> usize {
        self.lane_id
    }
}

impl MeteredTunnelStream {
    pub(crate) fn new(inner: TunnelStream) -> Self {
        Self {
            inner,
            client_to_remote: 0,
            remote_to_client: 0,
        }
    }

    pub(crate) fn lane_id(&self) -> usize {
        self.inner.lane_id()
    }

    pub(crate) fn byte_counts(&self) -> (u64, u64) {
        (self.client_to_remote, self.remote_to_client)
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
        elapsed: Duration,
    ) {
        let manager = self.inner.read().await.clone();
        manager
            .record_stream_bytes(lane_id, client_to_remote, remote_to_client, elapsed)
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

impl AsyncRead for MeteredTunnelStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &result {
            let read = buf.filled().len().saturating_sub(before) as u64;
            if read > 0 {
                self.remote_to_client = self.remote_to_client.saturating_add(read);
                self.inner
                    .lane
                    .inflight_remote_to_client
                    .fetch_add(read, Ordering::Relaxed);
                self.inner
                    .runtime_state
                    .add_tunnel_lane_bytes(self.inner.lane_id, 0, read);
            }
        }
        result
    }
}

impl AsyncWrite for MeteredTunnelStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(written)) = &result {
            if *written > 0 {
                let written = *written as u64;
                self.client_to_remote = self.client_to_remote.saturating_add(written);
                self.inner
                    .lane
                    .inflight_client_to_remote
                    .fetch_add(written, Ordering::Relaxed);
                self.inner
                    .runtime_state
                    .add_tunnel_lane_bytes(self.inner.lane_id, written, 0);
            }
        }
        result
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
                    inflight_client_to_remote: AtomicU64::new(0),
                    inflight_remote_to_client: AtomicU64::new(0),
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
                underlay: config.underlay,
            }),
            port_hopping: config.port_hopping,
            select_lock: Mutex::new(()),
            lanes,
        }
    }

    pub(crate) async fn open_stream(&self, priority: StreamPriority) -> Result<TunnelStream> {
        let lane = {
            let _select_guard = self.select_lock.lock().await;
            let lane = self
                .select_lane(priority)
                .context("no tunnel lanes configured")?;
            self.reserve_lane_open(&lane).await;
            lane
        };
        let lane_id = lane.id;
        match self.open_stream_on_lane(lane.clone(), priority).await {
            Ok(inner) => Ok(TunnelStream {
                inner,
                lane_id,
                lane,
                runtime_state: self.runtime_state.clone(),
            }),
            Err(err) => {
                self.release_lane_reservation(&lane).await;
                Err(err)
            }
        }
    }

    pub(crate) async fn record_stream_bytes(
        &self,
        lane_id: usize,
        client_to_remote: u64,
        remote_to_client: u64,
        elapsed: Duration,
    ) {
        if let Some(lane) = self.lanes.iter().find(|lane| lane.id == lane_id) {
            saturating_atomic_sub(&lane.inflight_client_to_remote, client_to_remote);
            saturating_atomic_sub(&lane.inflight_remote_to_client, remote_to_client);
            let mut health = lane.health.lock().await;
            health.bytes_client_to_remote = health
                .bytes_client_to_remote
                .saturating_add(client_to_remote);
            health.bytes_remote_to_client = health
                .bytes_remote_to_client
                .saturating_add(remote_to_client);
            update_recent_throughput(&mut health, client_to_remote, remote_to_client, elapsed);
            health.active_streams = health.active_streams.saturating_sub(1);
            health.last_activity_unix_secs = Some(unix_now_secs());
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
                endpoint: hopped_endpoint(&self.server, &self.port_hopping),
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
            health.last_activity_unix_secs = Some(unix_now_secs());
            health.last_error = None;
            health.last_error_unix_secs = None;
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
            health.last_activity_unix_secs = Some(unix_now_secs());
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
            .min_by_key(|lane| lane_score(lane))
            .or_else(|| self.lanes.iter().min_by_key(|lane| lane_score(lane)))
            .cloned()
    }

    async fn record_open_success(&self, lane: &Arc<TunnelLane>, elapsed: Duration) {
        let mut health = lane.health.lock().await;
        health.pending_stream_opens = health.pending_stream_opens.saturating_sub(1);
        health.active_streams = health.active_streams.saturating_add(1);
        health.streams_opened = health.streams_opened.saturating_add(1);
        health.last_open_latency_ms = elapsed.as_millis().min(u64::MAX as u128) as u64;
        health.last_activity_unix_secs = Some(unix_now_secs());
        health.last_error = None;
        health.last_error_unix_secs = None;
        self.publish_lane(lane, &health, "connected");
    }

    async fn record_lane_error(&self, lane: &Arc<TunnelLane>, error: String) {
        let mut health = lane.health.lock().await;
        health.stream_open_failures = health.stream_open_failures.saturating_add(1);
        health.last_activity_unix_secs = Some(unix_now_secs());
        health.last_error = Some(error.clone());
        health.last_error_unix_secs = Some(unix_now_secs());
        self.publish_lane(lane, &health, "degraded");
        self.runtime_state.record_error(error);
    }

    async fn reserve_lane_open(&self, lane: &Arc<TunnelLane>) {
        let mut health = lane.health.lock().await;
        health.pending_stream_opens = health.pending_stream_opens.saturating_add(1);
        health.last_activity_unix_secs = Some(unix_now_secs());
        self.publish_lane(lane, &health, "connected");
    }

    async fn release_lane_reservation(&self, lane: &Arc<TunnelLane>) {
        let mut health = lane.health.lock().await;
        health.pending_stream_opens = health.pending_stream_opens.saturating_sub(1);
        health.last_activity_unix_secs = Some(unix_now_secs());
        self.publish_lane(lane, &health, "degraded");
    }

    fn publish_lane(&self, lane: &TunnelLane, health: &LaneHealth, state: &str) {
        let inflight_client_to_remote = lane.inflight_client_to_remote.load(Ordering::Relaxed);
        let inflight_remote_to_client = lane.inflight_remote_to_client.load(Ordering::Relaxed);
        self.runtime_state.update_tunnel_lane(TunnelLaneSnapshot {
            id: lane.id,
            lane: lane.kind.as_str().to_string(),
            state: state.to_string(),
            reconnect_count: health.reconnect_count,
            active_streams: health.active_streams,
            pending_stream_opens: health.pending_stream_opens,
            streams_opened: health.streams_opened,
            stream_open_failures: health.stream_open_failures,
            bytes_client_to_remote: health
                .bytes_client_to_remote
                .saturating_add(inflight_client_to_remote),
            bytes_remote_to_client: health
                .bytes_remote_to_client
                .saturating_add(inflight_remote_to_client),
            recent_client_to_remote_bps: health.recent_client_to_remote_bps,
            recent_remote_to_client_bps: health.recent_remote_to_client_bps,
            adaptive_score: lane_score_from_health(health),
            last_open_latency_ms: health.last_open_latency_ms,
            last_mux_rtt_ms: health.last_mux_rtt_ms,
            mux_rtt_trend_ms: health.mux_rtt_trend_ms.iter().copied().collect(),
            session_age_secs: health
                .connected_at
                .map(|connected_at| connected_at.elapsed().as_secs()),
            last_activity_unix_secs: health.last_activity_unix_secs,
            last_error: health.last_error.clone(),
            last_error_unix_secs: health.last_error_unix_secs,
        });
    }
}

fn lane_score(lane: &TunnelLane) -> u64 {
    lane.health
        .try_lock()
        .map(|health| lane_score_from_health(&health))
        .unwrap_or(u64::MAX / 2)
}

fn lane_score_from_health(health: &LaneHealth) -> u64 {
    let load = health
        .active_streams
        .saturating_add(health.pending_stream_opens);
    let attempts = health
        .streams_opened
        .saturating_add(health.stream_open_failures)
        .max(1);
    let failure_penalty = health.stream_open_failures.saturating_mul(200_000_000) / attempts;
    let error_penalty = u64::from(health.last_error.is_some()) * 500_000_000;
    let rtt_penalty = average_mux_rtt_ms(health).saturating_mul(1_000);
    let open_latency_penalty = health.last_open_latency_ms.saturating_mul(100);
    let throughput_credit = health
        .recent_client_to_remote_bps
        .saturating_add(health.recent_remote_to_client_bps)
        .saturating_div(1_000)
        .min(500_000);
    load.saturating_mul(1_000_000)
        .saturating_add(failure_penalty)
        .saturating_add(error_penalty)
        .saturating_add(rtt_penalty)
        .saturating_add(open_latency_penalty)
        .saturating_sub(throughput_credit)
}

fn average_mux_rtt_ms(health: &LaneHealth) -> u64 {
    if !health.mux_rtt_trend_ms.is_empty() {
        let sum = health
            .mux_rtt_trend_ms
            .iter()
            .fold(0_u64, |sum, rtt| sum.saturating_add(*rtt));
        return sum / health.mux_rtt_trend_ms.len() as u64;
    }
    health.last_mux_rtt_ms.unwrap_or_default()
}

fn update_recent_throughput(
    health: &mut LaneHealth,
    client_to_remote: u64,
    remote_to_client: u64,
    elapsed: Duration,
) {
    let elapsed_nanos = elapsed.as_nanos();
    if elapsed_nanos == 0 {
        return;
    }
    let client_bps = bits_per_second(client_to_remote, elapsed_nanos);
    let remote_bps = bits_per_second(remote_to_client, elapsed_nanos);
    health.recent_client_to_remote_bps = ewma_bps(health.recent_client_to_remote_bps, client_bps);
    health.recent_remote_to_client_bps = ewma_bps(health.recent_remote_to_client_bps, remote_bps);
}

fn bits_per_second(bytes: u64, elapsed_nanos: u128) -> u64 {
    ((bytes as u128)
        .saturating_mul(8)
        .saturating_mul(1_000_000_000)
        / elapsed_nanos)
        .min(u64::MAX as u128) as u64
}

fn ewma_bps(previous: u64, sample: u64) -> u64 {
    if previous == 0 {
        sample
    } else {
        previous
            .saturating_mul(3)
            .saturating_add(sample)
            .saturating_div(4)
    }
}

fn saturating_atomic_sub(value: &AtomicU64, amount: u64) {
    let _ = value.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_sub(amount))
    });
}

fn hopped_endpoint(endpoint: &str, port_hopping: &PortHoppingConfig) -> String {
    let Some(port) = port_hopping.selected_port_at(unix_now_secs()) else {
        return endpoint.to_string();
    };
    let Ok((host, _)) = split_authority(endpoint) else {
        return endpoint.to_string();
    };
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

#[derive(Clone)]
struct TcpTransportConnector {
    options: TcpConfig,
    underlay: UnderlayConfig,
}

impl TransportConnector for TcpTransportConnector {
    fn connect<'a>(
        &'a self,
        target: TransportTarget,
    ) -> espejismo_core::extension::BoxFutureResult<'a, Box<dyn espejismo_core::TransportStream>>
    {
        Box::pin(async move {
            let stream = connect_tcp_stream(&target.endpoint, &self.options).await?;
            match self.underlay.mode {
                UnderlayMode::Tcp => {
                    Ok(Box::new(stream) as Box<dyn espejismo_core::TransportStream>)
                }
                UnderlayMode::WebSocket => {
                    let host = self
                        .underlay
                        .websocket
                        .host
                        .clone()
                        .unwrap_or_else(|| target.endpoint.clone());
                    let stream = connect_websocket_underlay(
                        stream,
                        &host,
                        &self.underlay.websocket.path,
                        self.underlay.websocket.max_frame_bytes,
                    )
                    .await?;
                    Ok(Box::new(stream) as Box<dyn espejismo_core::TransportStream>)
                }
                UnderlayMode::Http2 => {
                    let authority = self
                        .underlay
                        .http2
                        .authority
                        .clone()
                        .unwrap_or_else(|| target.endpoint.clone());
                    let stream = connect_http2_underlay(
                        stream,
                        &authority,
                        &self.underlay.http2.path,
                        (&self.underlay.http2).into(),
                    )
                    .await?;
                    Ok(Box::new(stream) as Box<dyn espejismo_core::TransportStream>)
                }
            }
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

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{lane_score, update_recent_throughput, LaneHealth, LaneKind, TunnelLane};
    use tokio::sync::Mutex;

    fn lane_with_health(health: LaneHealth) -> TunnelLane {
        TunnelLane {
            id: 0,
            kind: LaneKind::Bulk,
            control: Mutex::new(None),
            connect_lock: Mutex::new(()),
            health: Mutex::new(health),
            inflight_client_to_remote: Default::default(),
            inflight_remote_to_client: Default::default(),
        }
    }

    #[test]
    fn lane_score_prefers_idle_lane_over_lower_latency_loaded_lane() {
        let idle = lane_with_health(LaneHealth {
            last_open_latency_ms: 500,
            ..LaneHealth::default()
        });
        let loaded = lane_with_health(LaneHealth {
            pending_stream_opens: 1,
            last_open_latency_ms: 1,
            ..LaneHealth::default()
        });

        assert!(lane_score(&idle) < lane_score(&loaded));
    }

    #[test]
    fn lane_score_penalizes_stream_open_failures() {
        let healthy = lane_with_health(LaneHealth {
            streams_opened: 10,
            stream_open_failures: 0,
            ..LaneHealth::default()
        });
        let failing = lane_with_health(LaneHealth {
            streams_opened: 10,
            stream_open_failures: 5,
            ..LaneHealth::default()
        });

        assert!(lane_score(&healthy) < lane_score(&failing));
    }

    #[test]
    fn lane_score_uses_rtt_trend_and_recent_throughput() {
        let fast = lane_with_health(LaneHealth {
            mux_rtt_trend_ms: [20, 22, 18].into(),
            recent_remote_to_client_bps: 200_000_000,
            ..LaneHealth::default()
        });
        let slow = lane_with_health(LaneHealth {
            mux_rtt_trend_ms: [180, 190, 200].into(),
            recent_remote_to_client_bps: 1_000_000,
            ..LaneHealth::default()
        });

        assert!(lane_score(&fast) < lane_score(&slow));
    }

    #[test]
    fn recent_throughput_uses_ewma() {
        let mut health = LaneHealth::default();
        update_recent_throughput(&mut health, 1_000_000, 0, Duration::from_secs(1));
        assert_eq!(health.recent_client_to_remote_bps, 8_000_000);

        update_recent_throughput(&mut health, 2_000_000, 0, Duration::from_secs(1));
        assert_eq!(health.recent_client_to_remote_bps, 10_000_000);
    }
}
