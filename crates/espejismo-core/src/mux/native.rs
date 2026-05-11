mod frame;
mod pending;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use anyhow::{bail, Context as AnyhowContext, Result};
use futures::Stream;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::protocol::request::StreamPriority;
use frame::{
    read_frame, write_frame, FRAME_DATA, FRAME_FIN, FRAME_GOAWAY, FRAME_OPEN, FRAME_PING,
    FRAME_RST, FRAME_WINDOW_UPDATE, MAX_PAYLOAD,
};
use pending::{PendingFrame, PendingFrames};

pub use frame::validate_frame_bytes_for_fuzz;

const COMMAND_CHANNEL_EXTRA_FRAMES: usize = 1024;
const CONTROL_PENDING_EXTRA_FRAMES: usize = 1024;

#[derive(Clone, Copy, Debug)]
pub struct NativeMuxConfig {
    pub max_streams: usize,
    pub initial_window_bytes: usize,
    pub stream_buffer_frames: usize,
    pub send_queue_frames: usize,
    pub session_idle_timeout: Duration,
    pub drain_timeout: Duration,
}

impl Default for NativeMuxConfig {
    fn default() -> Self {
        Self {
            max_streams: 256,
            initial_window_bytes: 1024 * 1024,
            stream_buffer_frames: 128,
            send_queue_frames: 64,
            session_idle_timeout: Duration::from_secs(300),
            drain_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Clone)]
pub struct NativeControl {
    tx: mpsc::Sender<Command>,
}

pub struct NativeSession {
    accept_rx: mpsc::Receiver<Result<NativeStream, io::Error>>,
    task: JoinHandle<()>,
}

pub struct NativeStream {
    id: u32,
    priority: StreamPriority,
    tx: mpsc::Sender<Command>,
    rx: mpsc::Receiver<Vec<u8>>,
    flow: Arc<Mutex<FlowState>>,
    read_buf: Vec<u8>,
    read_offset: usize,
    pending_window_update: usize,
    remote_closed: bool,
    local_closed: bool,
}

enum Command {
    Open {
        priority: StreamPriority,
        reply: oneshot::Sender<Result<NativeStream>>,
    },
    Data {
        stream_id: u32,
        priority: StreamPriority,
        payload: Vec<u8>,
    },
    Fin {
        stream_id: u32,
    },
    Rst {
        stream_id: u32,
    },
    WindowUpdate {
        stream_id: u32,
        amount: u32,
    },
    Ping {
        nonce: u64,
        reply: Option<oneshot::Sender<Duration>>,
    },
    Goaway,
}

struct StreamEntry {
    tx: mpsc::Sender<Vec<u8>>,
    flow: Arc<Mutex<FlowState>>,
}

#[derive(Debug)]
struct FlowState {
    available: usize,
    queued_send_frames: usize,
    send_queue_limit: usize,
    closed: bool,
    waker: Option<Waker>,
}

impl FlowState {
    fn new(window: usize, send_queue_limit: usize) -> Self {
        Self {
            available: window,
            queued_send_frames: 0,
            send_queue_limit,
            closed: false,
            waker: None,
        }
    }

    fn add_window(&mut self, amount: usize) {
        self.available = self.available.saturating_add(amount);
        self.wake();
    }

    fn reserve_send_slot(&mut self) -> bool {
        if self.closed {
            return false;
        }
        if self.queued_send_frames >= self.send_queue_limit {
            return false;
        }
        self.queued_send_frames += 1;
        true
    }

    fn release_send_slot(&mut self) {
        self.queued_send_frames = self.queued_send_frames.saturating_sub(1);
        self.wake();
    }

    fn close(&mut self) {
        self.closed = true;
        self.wake();
    }

    fn wake(&mut self) {
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

pub fn client_session<T>(transport: T, config: NativeMuxConfig) -> (NativeControl, NativeSession)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    start_session(transport, 1, config)
}

pub fn server_session<T>(transport: T, config: NativeMuxConfig) -> (NativeControl, NativeSession)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    start_session(transport, 2, config)
}

fn start_session<T>(
    transport: T,
    first_stream_id: u32,
    config: NativeMuxConfig,
) -> (NativeControl, NativeSession)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let config = sanitize_config(config);
    let (command_tx, command_rx) = mpsc::channel(command_channel_limit(&config));
    let (accept_tx, accept_rx) = mpsc::channel(config.max_streams);
    let control = NativeControl {
        tx: command_tx.clone(),
    };
    let task = tokio::spawn(run_session(
        transport,
        command_tx,
        command_rx,
        accept_tx,
        first_stream_id,
        config,
    ));
    (control, NativeSession { accept_rx, task })
}

fn command_channel_limit(config: &NativeMuxConfig) -> usize {
    config
        .max_streams
        .saturating_mul(config.send_queue_frames.saturating_add(8))
        .saturating_add(COMMAND_CHANNEL_EXTRA_FRAMES)
        .max(1)
}

fn pending_frame_limit(config: &NativeMuxConfig) -> usize {
    config
        .max_streams
        .saturating_mul(config.send_queue_frames.saturating_add(4))
        .saturating_add(CONTROL_PENDING_EXTRA_FRAMES)
        .max(1)
}

fn sanitize_config(config: NativeMuxConfig) -> NativeMuxConfig {
    NativeMuxConfig {
        max_streams: config.max_streams.max(1),
        initial_window_bytes: config.initial_window_bytes.max(1),
        stream_buffer_frames: config.stream_buffer_frames.max(1),
        send_queue_frames: config.send_queue_frames.max(1),
        session_idle_timeout: config.session_idle_timeout.max(Duration::from_millis(1)),
        drain_timeout: config.drain_timeout.max(Duration::from_millis(1)),
    }
}

impl NativeControl {
    pub async fn open_stream(&mut self, priority: StreamPriority) -> Result<NativeStream> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Command::Open { priority, reply })
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "native mux stopped"))?;
        rx.await.context("native mux open stream reply dropped")?
    }

    pub async fn ping_rtt(&self) -> Result<Duration> {
        let nonce = rand::random::<u64>();
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Command::Ping {
                nonce,
                reply: Some(reply),
            })
            .await
            .map_err(|_| anyhow::anyhow!("native mux stopped"))?;
        rx.await.context("native mux ping reply dropped")
    }

    pub fn goaway(&self) -> Result<()> {
        self.tx
            .try_send(Command::Goaway)
            .map_err(|_| anyhow::anyhow!("native mux stopped"))
    }
}

impl Stream for NativeSession {
    type Item = Result<NativeStream, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.accept_rx.poll_recv(cx)
    }
}

impl Drop for NativeSession {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl AsyncRead for NativeStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            if self.read_offset < self.read_buf.len() {
                let available = &self.read_buf[self.read_offset..];
                let n = available.len().min(buf.remaining());
                buf.put_slice(&available[..n]);
                self.read_offset += n;
                self.pending_window_update = self.pending_window_update.saturating_add(n);
                if self.read_offset == self.read_buf.len() {
                    self.read_buf.clear();
                    self.read_offset = 0;
                    self.flush_pending_window_update();
                }
                return Poll::Ready(Ok(()));
            }
            if self.remote_closed {
                self.flush_pending_window_update();
                return Poll::Ready(Ok(()));
            }
            match Pin::new(&mut self.rx).poll_recv(cx) {
                Poll::Ready(Some(payload)) => {
                    self.read_buf = payload;
                    self.read_offset = 0;
                }
                Poll::Ready(None) => {
                    self.remote_closed = true;
                    self.flush_pending_window_update();
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl NativeStream {
    fn flush_pending_window_update(&mut self) {
        let mut amount = self.pending_window_update;
        self.pending_window_update = 0;
        while amount > 0 {
            let chunk = amount.min(u32::MAX as usize);
            let _ = self.tx.try_send(Command::WindowUpdate {
                stream_id: self.id,
                amount: chunk as u32,
            });
            amount -= chunk;
        }
    }
}

impl AsyncWrite for NativeStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.local_closed {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "native mux stream closed",
            )));
        }
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        let n = {
            let mut flow = this
                .flow
                .lock()
                .map_err(|_| io::Error::other("native mux flow state poisoned"))?;
            if flow.closed {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "native mux stream closed by peer",
                )));
            }
            if flow.available == 0 || !flow.reserve_send_slot() {
                flow.waker = Some(cx.waker().clone());
                return Poll::Pending;
            }
            let n = buf.len().min(MAX_PAYLOAD).min(flow.available);
            flow.available -= n;
            n
        };

        if this
            .tx
            .try_send(Command::Data {
                stream_id: this.id,
                priority: this.priority,
                payload: buf[..n].to_vec(),
            })
            .is_err()
        {
            if let Ok(mut flow) = this.flow.lock() {
                flow.release_send_slot();
            }
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "native mux stopped",
            )));
        }
        Poll::Ready(Ok(n))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if !self.local_closed {
            self.local_closed = true;
            self.tx
                .try_send(Command::Fin { stream_id: self.id })
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "native mux stopped"))?;
        }
        Poll::Ready(Ok(()))
    }
}

impl Drop for NativeStream {
    fn drop(&mut self) {
        if !self.local_closed {
            let _ = self.tx.try_send(Command::Rst { stream_id: self.id });
            self.local_closed = true;
        }
    }
}

async fn run_session<T>(
    mut transport: T,
    command_tx: mpsc::Sender<Command>,
    mut command_rx: mpsc::Receiver<Command>,
    accept_tx: mpsc::Sender<Result<NativeStream, io::Error>>,
    mut next_stream_id: u32,
    config: NativeMuxConfig,
) where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let mut streams = HashMap::<u32, StreamEntry>::new();
    let mut pending = PendingFrames::new(pending_frame_limit(&config));
    let mut pending_pings = HashMap::<u64, (Instant, oneshot::Sender<Duration>)>::new();
    let mut draining_since: Option<Instant> = None;
    loop {
        if flush_pending(&mut transport, &mut streams, &mut pending)
            .await
            .is_err()
        {
            break;
        }
        if should_stop(draining_since, config.drain_timeout, streams.is_empty()) {
            break;
        }
        tokio::select! {
            biased;

            command = command_rx.recv() => {
                let Some(command) = command else {
                    enqueue_control(&mut pending, FRAME_GOAWAY, 0, Vec::new()).ok();
                    draining_since.get_or_insert_with(Instant::now);
                    continue;
                };
                match handle_command(command, CommandContext {
                    command_tx: &command_tx,
                    streams: &mut streams,
                    next_stream_id: &mut next_stream_id,
                    config: &config,
                    pending: &mut pending,
                    pending_pings: &mut pending_pings,
                    draining: draining_since.is_some(),
                }).await {
                    Ok(CommandEffect::Continue) => {}
                    Ok(CommandEffect::StartDrain) => {
                        draining_since.get_or_insert_with(Instant::now);
                    }
                    Err(_) => break,
                }
            }
            frame = read_frame(&mut transport) => {
                match frame {
                    Ok(Some((kind, stream_id, payload))) => {
                        match handle_frame(kind, stream_id, payload, FrameContext {
                            command_tx: &command_tx,
                            accept_tx: &accept_tx,
                            streams: &mut streams,
                            config: &config,
                            pending: &mut pending,
                            pending_pings: &mut pending_pings,
                            draining: draining_since.is_some(),
                        }).await {
                            Ok(FrameEffect::Continue) => {}
                            Ok(FrameEffect::StartDrain) => {
                                draining_since.get_or_insert_with(Instant::now);
                            }
                            Err(err) => {
                                let _ = accept_tx
                                    .send(Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        err.to_string(),
                                    )))
                                    .await;
                                break;
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        let _ = accept_tx
                            .send(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                err.to_string(),
                            )))
                            .await;
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(config.session_idle_timeout), if streams.is_empty() && draining_since.is_none() => {
                enqueue_control(&mut pending, FRAME_GOAWAY, 0, Vec::new()).ok();
                draining_since = Some(Instant::now());
            }
            _ = tokio::time::sleep(config.drain_timeout), if draining_since.is_some() => {
                break;
            }
        }
    }
}

fn should_stop(draining_since: Option<Instant>, drain_timeout: Duration, no_streams: bool) -> bool {
    draining_since.is_some_and(|started| no_streams || started.elapsed() >= drain_timeout)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandEffect {
    Continue,
    StartDrain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameEffect {
    Continue,
    StartDrain,
}

struct CommandContext<'a> {
    command_tx: &'a mpsc::Sender<Command>,
    streams: &'a mut HashMap<u32, StreamEntry>,
    next_stream_id: &'a mut u32,
    config: &'a NativeMuxConfig,
    pending: &'a mut PendingFrames,
    pending_pings: &'a mut HashMap<u64, (Instant, oneshot::Sender<Duration>)>,
    draining: bool,
}

async fn handle_command(command: Command, ctx: CommandContext<'_>) -> Result<CommandEffect> {
    match command {
        Command::Open { priority, reply } => {
            if ctx.draining {
                let _ = reply.send(Err(anyhow::anyhow!("native mux is draining")));
                return Ok(CommandEffect::Continue);
            }
            if ctx.streams.len() >= ctx.config.max_streams {
                let _ = reply.send(Err(anyhow::anyhow!("native mux max streams reached")));
                return Ok(CommandEffect::Continue);
            }
            let stream_id = *ctx.next_stream_id;
            *ctx.next_stream_id = ctx.next_stream_id.saturating_add(2);
            let (stream, entry) = new_stream(
                stream_id,
                priority,
                ctx.command_tx.clone(),
                ctx.config.initial_window_bytes,
                ctx.config.stream_buffer_frames,
                ctx.config.send_queue_frames,
            );
            ctx.streams.insert(stream_id, entry);
            enqueue_control(ctx.pending, FRAME_OPEN, stream_id, vec![priority as u8])?;
            let _ = reply.send(Ok(stream));
        }
        Command::Data {
            stream_id,
            priority,
            payload,
        } => {
            if ctx.streams.contains_key(&stream_id) {
                if let Err(err) = enqueue_data(ctx.pending, priority, stream_id, payload) {
                    release_send_slot(ctx.streams, stream_id);
                    return Err(err);
                }
            } else {
                release_send_slot(ctx.streams, stream_id);
            }
        }
        Command::Fin { stream_id } => {
            enqueue_control(ctx.pending, FRAME_FIN, stream_id, Vec::new())?;
        }
        Command::Rst { stream_id } => {
            ctx.streams.remove(&stream_id);
            enqueue_control(ctx.pending, FRAME_RST, stream_id, Vec::new())?;
        }
        Command::WindowUpdate { stream_id, amount } => {
            enqueue_control(
                ctx.pending,
                FRAME_WINDOW_UPDATE,
                stream_id,
                amount.to_be_bytes().to_vec(),
            )?;
        }
        Command::Ping { nonce, reply } => {
            if let Some(reply) = reply {
                ctx.pending_pings.insert(nonce, (Instant::now(), reply));
            }
            enqueue_control(ctx.pending, FRAME_PING, 0, nonce.to_be_bytes().to_vec())?;
        }
        Command::Goaway => {
            enqueue_control(ctx.pending, FRAME_GOAWAY, 0, Vec::new())?;
            return Ok(CommandEffect::StartDrain);
        }
    }
    Ok(CommandEffect::Continue)
}

struct FrameContext<'a> {
    command_tx: &'a mpsc::Sender<Command>,
    accept_tx: &'a mpsc::Sender<Result<NativeStream, io::Error>>,
    streams: &'a mut HashMap<u32, StreamEntry>,
    config: &'a NativeMuxConfig,
    pending: &'a mut PendingFrames,
    pending_pings: &'a mut HashMap<u64, (Instant, oneshot::Sender<Duration>)>,
    draining: bool,
}

async fn handle_frame(
    kind: u8,
    stream_id: u32,
    payload: Vec<u8>,
    ctx: FrameContext<'_>,
) -> Result<FrameEffect> {
    match kind {
        FRAME_OPEN => {
            if ctx.draining {
                enqueue_control(ctx.pending, FRAME_RST, stream_id, Vec::new())?;
                return Ok(FrameEffect::Continue);
            }
            if ctx.streams.len() >= ctx.config.max_streams {
                enqueue_control(ctx.pending, FRAME_RST, stream_id, Vec::new())?;
                return Ok(FrameEffect::Continue);
            }
            let priority = payload
                .first()
                .copied()
                .map(StreamPriority::try_from)
                .transpose()?
                .unwrap_or(StreamPriority::Interactive);
            let (stream, entry) = new_stream(
                stream_id,
                priority,
                ctx.command_tx.clone(),
                ctx.config.initial_window_bytes,
                ctx.config.stream_buffer_frames,
                ctx.config.send_queue_frames,
            );
            ctx.streams.insert(stream_id, entry);
            ctx.accept_tx
                .send(Ok(stream))
                .await
                .map_err(|_| anyhow::anyhow!("native mux accept receiver dropped"))?;
        }
        FRAME_DATA => {
            let Some(stream) = ctx.streams.get(&stream_id) else {
                enqueue_control(ctx.pending, FRAME_RST, stream_id, Vec::new())?;
                return Ok(FrameEffect::Continue);
            };
            stream
                .tx
                .send(payload)
                .await
                .map_err(|_| anyhow::anyhow!("native mux stream receiver dropped"))?;
        }
        FRAME_WINDOW_UPDATE => {
            if payload.len() != 4 {
                bail!("native mux malformed window update");
            }
            let amount =
                u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
            if let Some(stream) = ctx.streams.get(&stream_id) {
                let mut flow = stream
                    .flow
                    .lock()
                    .map_err(|_| anyhow::anyhow!("native mux flow state poisoned"))?;
                flow.add_window(amount);
            }
        }
        FRAME_FIN | FRAME_RST => {
            mark_stream_closed(ctx.streams, stream_id);
            ctx.streams.remove(&stream_id);
        }
        FRAME_PING => {
            if payload.len() != 8 {
                bail!("native mux malformed ping");
            }
            let nonce = u64::from_be_bytes(payload[..8].try_into().expect("payload len checked"));
            if let Some((started, reply)) = ctx.pending_pings.remove(&nonce) {
                let _ = reply.send(started.elapsed());
            } else {
                enqueue_control(ctx.pending, FRAME_PING, 0, payload)?;
            }
        }
        FRAME_GOAWAY => return Ok(FrameEffect::StartDrain),
        _ => bail!("unknown native mux frame type {kind}"),
    }
    Ok(FrameEffect::Continue)
}

fn enqueue_control(
    pending: &mut PendingFrames,
    kind: u8,
    stream_id: u32,
    payload: Vec<u8>,
) -> Result<()> {
    pending.push_control(PendingFrame {
        kind,
        stream_id,
        payload,
        queued_stream: None,
    })
}

fn enqueue_data(
    pending: &mut PendingFrames,
    priority: StreamPriority,
    stream_id: u32,
    payload: Vec<u8>,
) -> Result<()> {
    let frame = PendingFrame {
        kind: FRAME_DATA,
        stream_id,
        payload,
        queued_stream: Some(stream_id),
    };
    pending.push_data(priority, frame)
}

async fn flush_pending<W>(
    writer: &mut W,
    streams: &mut HashMap<u32, StreamEntry>,
    pending: &mut PendingFrames,
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    while let Some(frame) = pending.pop_next() {
        write_frame(writer, frame.kind, frame.stream_id, &frame.payload).await?;
        if let Some(stream_id) = frame.queued_stream {
            release_send_slot(streams, stream_id);
        }
    }
    Ok(())
}

fn release_send_slot(streams: &HashMap<u32, StreamEntry>, stream_id: u32) {
    if let Some(stream) = streams.get(&stream_id) {
        if let Ok(mut flow) = stream.flow.lock() {
            flow.release_send_slot();
        }
    }
}

fn mark_stream_closed(streams: &HashMap<u32, StreamEntry>, stream_id: u32) {
    if let Some(stream) = streams.get(&stream_id) {
        if let Ok(mut flow) = stream.flow.lock() {
            flow.close();
        }
    }
}

fn new_stream(
    stream_id: u32,
    priority: StreamPriority,
    command_tx: mpsc::Sender<Command>,
    initial_window_bytes: usize,
    stream_buffer_frames: usize,
    send_queue_frames: usize,
) -> (NativeStream, StreamEntry) {
    let (tx, rx) = mpsc::channel(stream_buffer_frames);
    let flow = Arc::new(Mutex::new(FlowState::new(
        initial_window_bytes,
        send_queue_frames,
    )));
    let stream = NativeStream {
        id: stream_id,
        priority,
        tx: command_tx,
        rx,
        flow: flow.clone(),
        read_buf: Vec::new(),
        read_offset: 0,
        remote_closed: false,
        local_closed: false,
        pending_window_update: 0,
    };
    let entry = StreamEntry { tx, flow };
    (stream, entry)
}
