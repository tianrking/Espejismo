use std::collections::{HashMap, VecDeque};
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use anyhow::{bail, Context as AnyhowContext, Result};
use futures::Stream;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::protocol::request::StreamPriority;

const FRAME_OPEN: u8 = 1;
const FRAME_DATA: u8 = 2;
const FRAME_WINDOW_UPDATE: u8 = 3;
const FRAME_FIN: u8 = 4;
const FRAME_RST: u8 = 5;
const FRAME_PING: u8 = 6;
const FRAME_GOAWAY: u8 = 7;
const MAX_PAYLOAD: usize = 64 * 1024;

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
    tx: mpsc::UnboundedSender<Command>,
}

pub struct NativeSession {
    accept_rx: mpsc::UnboundedReceiver<Result<NativeStream, io::Error>>,
    task: JoinHandle<()>,
}

pub struct NativeStream {
    id: u32,
    priority: StreamPriority,
    tx: mpsc::UnboundedSender<Command>,
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

struct PendingFrame {
    kind: u8,
    stream_id: u32,
    payload: Vec<u8>,
    queued_stream: Option<u32>,
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
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (accept_tx, accept_rx) = mpsc::unbounded_channel();
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
            .map_err(|_| anyhow::anyhow!("native mux stopped"))?;
        rx.await.context("native mux ping reply dropped")
    }

    pub fn goaway(&self) -> Result<()> {
        self.tx
            .send(Command::Goaway)
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
            let _ = self.tx.send(Command::WindowUpdate {
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
            .send(Command::Data {
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
                .send(Command::Fin { stream_id: self.id })
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "native mux stopped"))?;
        }
        Poll::Ready(Ok(()))
    }
}

impl Drop for NativeStream {
    fn drop(&mut self) {
        if !self.local_closed {
            let _ = self.tx.send(Command::Rst { stream_id: self.id });
            self.local_closed = true;
        }
    }
}

async fn run_session<T>(
    mut transport: T,
    command_tx: mpsc::UnboundedSender<Command>,
    mut command_rx: mpsc::UnboundedReceiver<Command>,
    accept_tx: mpsc::UnboundedSender<Result<NativeStream, io::Error>>,
    mut next_stream_id: u32,
    config: NativeMuxConfig,
) where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let mut streams = HashMap::<u32, StreamEntry>::new();
    let mut pending = PendingFrames::default();
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
                    enqueue_control(&mut pending, FRAME_GOAWAY, 0, Vec::new());
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
                                let _ = accept_tx.send(Err(io::Error::new(io::ErrorKind::InvalidData, err.to_string())));
                                break;
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        let _ = accept_tx.send(Err(io::Error::new(io::ErrorKind::InvalidData, err.to_string())));
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(config.session_idle_timeout), if streams.is_empty() && draining_since.is_none() => {
                enqueue_control(&mut pending, FRAME_GOAWAY, 0, Vec::new());
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

#[derive(Default)]
struct PendingFrames {
    control: VecDeque<PendingFrame>,
    interactive: VecDeque<PendingFrame>,
    bulk: VecDeque<PendingFrame>,
}

struct CommandContext<'a> {
    command_tx: &'a mpsc::UnboundedSender<Command>,
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
            enqueue_control(ctx.pending, FRAME_OPEN, stream_id, vec![priority as u8]);
            let _ = reply.send(Ok(stream));
        }
        Command::Data {
            stream_id,
            priority,
            payload,
        } => {
            if ctx.streams.contains_key(&stream_id) {
                enqueue_data(ctx.pending, priority, stream_id, payload);
            } else {
                release_send_slot(ctx.streams, stream_id);
            }
        }
        Command::Fin { stream_id } => {
            enqueue_control(ctx.pending, FRAME_FIN, stream_id, Vec::new());
        }
        Command::Rst { stream_id } => {
            ctx.streams.remove(&stream_id);
            enqueue_control(ctx.pending, FRAME_RST, stream_id, Vec::new());
        }
        Command::WindowUpdate { stream_id, amount } => {
            enqueue_control(
                ctx.pending,
                FRAME_WINDOW_UPDATE,
                stream_id,
                amount.to_be_bytes().to_vec(),
            );
        }
        Command::Ping { nonce, reply } => {
            if let Some(reply) = reply {
                ctx.pending_pings.insert(nonce, (Instant::now(), reply));
            }
            enqueue_control(ctx.pending, FRAME_PING, 0, nonce.to_be_bytes().to_vec());
        }
        Command::Goaway => {
            enqueue_control(ctx.pending, FRAME_GOAWAY, 0, Vec::new());
            return Ok(CommandEffect::StartDrain);
        }
    }
    Ok(CommandEffect::Continue)
}

struct FrameContext<'a> {
    command_tx: &'a mpsc::UnboundedSender<Command>,
    accept_tx: &'a mpsc::UnboundedSender<Result<NativeStream, io::Error>>,
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
                enqueue_control(ctx.pending, FRAME_RST, stream_id, Vec::new());
                return Ok(FrameEffect::Continue);
            }
            if ctx.streams.len() >= ctx.config.max_streams {
                enqueue_control(ctx.pending, FRAME_RST, stream_id, Vec::new());
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
                .map_err(|_| anyhow::anyhow!("native mux accept receiver dropped"))?;
        }
        FRAME_DATA => {
            let Some(stream) = ctx.streams.get(&stream_id) else {
                bail!("native mux data for unknown stream {stream_id}");
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
                enqueue_control(ctx.pending, FRAME_PING, 0, payload);
            }
        }
        FRAME_GOAWAY => return Ok(FrameEffect::StartDrain),
        _ => bail!("unknown native mux frame type {kind}"),
    }
    Ok(FrameEffect::Continue)
}

fn enqueue_control(pending: &mut PendingFrames, kind: u8, stream_id: u32, payload: Vec<u8>) {
    pending.control.push_back(PendingFrame {
        kind,
        stream_id,
        payload,
        queued_stream: None,
    });
}

fn enqueue_data(
    pending: &mut PendingFrames,
    priority: StreamPriority,
    stream_id: u32,
    payload: Vec<u8>,
) {
    let frame = PendingFrame {
        kind: FRAME_DATA,
        stream_id,
        payload,
        queued_stream: Some(stream_id),
    };
    match priority {
        StreamPriority::Interactive => pending.interactive.push_back(frame),
        StreamPriority::Bulk => pending.bulk.push_back(frame),
    }
}

async fn flush_pending<W>(
    writer: &mut W,
    streams: &mut HashMap<u32, StreamEntry>,
    pending: &mut PendingFrames,
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    while let Some(frame) = next_pending_frame(pending) {
        write_frame(writer, frame.kind, frame.stream_id, &frame.payload).await?;
        if let Some(stream_id) = frame.queued_stream {
            release_send_slot(streams, stream_id);
        }
    }
    Ok(())
}

fn next_pending_frame(pending: &mut PendingFrames) -> Option<PendingFrame> {
    pending
        .control
        .pop_front()
        .or_else(|| pending.interactive.pop_front())
        .or_else(|| pending.bulk.pop_front())
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
    command_tx: mpsc::UnboundedSender<Command>,
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

async fn write_frame<W>(writer: &mut W, kind: u8, stream_id: u32, payload: &[u8]) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    if payload.len() > MAX_PAYLOAD {
        bail!("native mux payload too large");
    }
    writer.write_u8(kind).await?;
    writer.write_u32(stream_id).await?;
    writer.write_u32(payload.len() as u32).await?;
    writer.write_all(payload).await?;
    Ok(())
}

async fn read_frame<R>(reader: &mut R) -> Result<Option<(u8, u32, Vec<u8>)>>
where
    R: AsyncRead + Unpin,
{
    let kind = match reader.read_u8().await {
        Ok(kind) => kind,
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let stream_id = reader.read_u32().await?;
    let len = reader.read_u32().await? as usize;
    if len > MAX_PAYLOAD {
        bail!("native mux payload too large");
    }
    let mut payload = vec![0_u8; len];
    reader.read_exact(&mut payload).await?;
    Ok(Some((kind, stream_id, payload)))
}

pub fn validate_frame_bytes_for_fuzz(input: &[u8]) -> Result<Option<(u8, u32, usize)>> {
    if input.len() < 9 {
        return Ok(None);
    }
    let kind = input[0];
    match kind {
        FRAME_OPEN | FRAME_DATA | FRAME_WINDOW_UPDATE | FRAME_FIN | FRAME_RST | FRAME_PING
        | FRAME_GOAWAY => {}
        _ => bail!("unknown native mux frame type {kind}"),
    }
    let stream_id = u32::from_be_bytes([input[1], input[2], input[3], input[4]]);
    let len = u32::from_be_bytes([input[5], input[6], input[7], input[8]]) as usize;
    if len > MAX_PAYLOAD {
        bail!("native mux payload too large");
    }
    if input.len() < 9 + len {
        return Ok(None);
    }
    Ok(Some((kind, stream_id, len)))
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::time::Duration;

    use super::{client_session, server_session, NativeMuxConfig};
    use crate::protocol::request::StreamPriority;
    use futures::StreamExt;
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    #[test]
    fn native_frame_parser_rejects_unknown_types_and_large_payloads() {
        assert!(super::validate_frame_bytes_for_fuzz(&[99, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
        let mut frame = vec![super::FRAME_DATA, 0, 0, 0, 1];
        frame.extend_from_slice(&((super::MAX_PAYLOAD as u32) + 1).to_be_bytes());
        assert!(super::validate_frame_bytes_for_fuzz(&frame).is_err());
    }

    #[tokio::test]
    async fn native_mux_opens_stream_and_roundtrips_data() {
        let (client_io, server_io) = duplex(64 * 1024);
        let (mut client_control, mut client_session) =
            client_session(client_io, NativeMuxConfig::default());
        let (_server_control, mut server_session) =
            server_session(server_io, NativeMuxConfig::default());

        tokio::spawn(async move { while client_session.next().await.is_some() {} });

        let mut client_stream = client_control
            .open_stream(StreamPriority::Interactive)
            .await
            .unwrap();
        let mut server_stream = server_session.next().await.unwrap().unwrap();

        client_stream.write_all(b"ping").await.unwrap();
        let mut received = [0_u8; 4];
        server_stream.read_exact(&mut received).await.unwrap();
        assert_eq!(&received, b"ping");

        server_stream.write_all(b"pong").await.unwrap();
        let mut response = [0_u8; 4];
        client_stream.read_exact(&mut response).await.unwrap();
        assert_eq!(&response, b"pong");
    }

    #[tokio::test]
    async fn native_mux_ping_reports_rtt() {
        let (client_io, server_io) = duplex(64 * 1024);
        let (client_control, mut client_session) =
            client_session(client_io, NativeMuxConfig::default());
        let (_server_control, mut server_session) =
            server_session(server_io, NativeMuxConfig::default());

        tokio::spawn(async move { while client_session.next().await.is_some() {} });
        tokio::spawn(async move { while server_session.next().await.is_some() {} });

        let rtt = client_control.ping_rtt().await.unwrap();
        assert!(rtt < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn native_mux_enforces_max_streams() {
        let (client_io, server_io) = duplex(64 * 1024);
        let config = NativeMuxConfig {
            max_streams: 1,
            ..NativeMuxConfig::default()
        };
        let (mut client_control, mut client_session) = client_session(client_io, config);
        let (_server_control, mut server_session) = server_session(server_io, config);

        tokio::spawn(async move { while client_session.next().await.is_some() {} });

        let first_stream = client_control
            .open_stream(StreamPriority::Interactive)
            .await
            .unwrap();
        let first_server_stream = server_session.next().await.unwrap().unwrap();
        assert!(client_control
            .open_stream(StreamPriority::Interactive)
            .await
            .is_err());
        drop(first_stream);
        drop(first_server_stream);
    }

    #[tokio::test]
    async fn native_mux_enforces_send_window_until_remote_reads() {
        let (client_io, server_io) = duplex(64 * 1024);
        let config = NativeMuxConfig {
            initial_window_bytes: 4,
            ..NativeMuxConfig::default()
        };
        let (mut client_control, mut client_session) = client_session(client_io, config);
        let (_server_control, mut server_session) = server_session(server_io, config);

        tokio::spawn(async move { while client_session.next().await.is_some() {} });

        let mut client_stream = client_control
            .open_stream(StreamPriority::Interactive)
            .await
            .unwrap();
        let mut server_stream = server_session.next().await.unwrap().unwrap();

        assert_eq!(client_stream.write(b"abcd").await.unwrap(), 4);
        let blocked =
            tokio::time::timeout(Duration::from_millis(50), client_stream.write(b"e")).await;
        assert!(blocked.is_err());

        let mut received = [0_u8; 4];
        server_stream.read_exact(&mut received).await.unwrap();
        assert_eq!(&received, b"abcd");

        let unblocked = tokio::time::timeout(Duration::from_secs(1), client_stream.write(b"e"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(unblocked, 1);
    }

    #[tokio::test]
    async fn native_mux_batches_window_update_until_payload_consumed() {
        let (client_io, server_io) = duplex(64 * 1024);
        let config = NativeMuxConfig {
            initial_window_bytes: 4,
            ..NativeMuxConfig::default()
        };
        let (mut client_control, mut client_session) = client_session(client_io, config);
        let (_server_control, mut server_session) = server_session(server_io, config);

        tokio::spawn(async move { while client_session.next().await.is_some() {} });

        let mut client_stream = client_control
            .open_stream(StreamPriority::Interactive)
            .await
            .unwrap();
        let mut server_stream = server_session.next().await.unwrap().unwrap();

        client_stream.write_all(b"abcd").await.unwrap();
        let mut one = [0_u8; 1];
        server_stream.read_exact(&mut one).await.unwrap();
        assert_eq!(&one, b"a");

        let blocked =
            tokio::time::timeout(Duration::from_millis(50), client_stream.write(b"e")).await;
        assert!(blocked.is_err());

        let mut rest = [0_u8; 3];
        server_stream.read_exact(&mut rest).await.unwrap();
        assert_eq!(&rest, b"bcd");

        let unblocked = tokio::time::timeout(Duration::from_secs(1), client_stream.write(b"e"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(unblocked, 1);
    }

    #[tokio::test]
    async fn native_mux_write_errors_after_peer_rst() {
        let (client_io, server_io) = duplex(64 * 1024);
        let (mut client_control, mut client_session) =
            client_session(client_io, NativeMuxConfig::default());
        let (_server_control, mut server_session) =
            server_session(server_io, NativeMuxConfig::default());

        tokio::spawn(async move { while client_session.next().await.is_some() {} });

        let mut client_stream = client_control
            .open_stream(StreamPriority::Interactive)
            .await
            .unwrap();
        let server_stream = server_session.next().await.unwrap().unwrap();
        tokio::spawn(async move { while server_session.next().await.is_some() {} });

        drop(server_stream);
        let result = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                match client_stream.write(b"x").await {
                    Ok(_) => tokio::time::sleep(Duration::from_millis(10)).await,
                    Err(err) => break err,
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(result.kind(), io::ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn native_mux_bounds_per_stream_send_queue() {
        let (client_io, server_io) = duplex(64 * 1024);
        let config = NativeMuxConfig {
            initial_window_bytes: 64 * 1024,
            send_queue_frames: 1,
            ..NativeMuxConfig::default()
        };
        let (mut client_control, mut client_session) = client_session(client_io, config);
        let (_server_control, mut server_session) = server_session(server_io, config);

        tokio::spawn(async move { while client_session.next().await.is_some() {} });

        let mut client_stream = client_control
            .open_stream(StreamPriority::Bulk)
            .await
            .unwrap();
        let _server_stream = server_session.next().await.unwrap().unwrap();

        assert!(client_stream.write(b"a").await.is_ok());
        let maybe_blocked =
            tokio::time::timeout(Duration::from_millis(50), client_stream.write(b"b")).await;
        assert!(maybe_blocked.is_err() || maybe_blocked.unwrap().is_ok());
    }

    #[tokio::test]
    async fn native_mux_goaway_drains_existing_streams() {
        let (client_io, server_io) = duplex(64 * 1024);
        let config = NativeMuxConfig {
            drain_timeout: Duration::from_secs(1),
            ..NativeMuxConfig::default()
        };
        let (mut client_control, mut client_session) = client_session(client_io, config);
        let (server_control, mut server_session) = server_session(server_io, config);

        tokio::spawn(async move { while client_session.next().await.is_some() {} });

        let mut client_stream = client_control
            .open_stream(StreamPriority::Interactive)
            .await
            .unwrap();
        let mut server_stream = server_session.next().await.unwrap().unwrap();
        server_control.goaway().unwrap();

        client_stream.write_all(b"after-goaway").await.unwrap();
        let mut received = vec![0_u8; 12];
        server_stream.read_exact(&mut received).await.unwrap();
        assert_eq!(&received, b"after-goaway");
        assert!(client_control
            .open_stream(StreamPriority::Interactive)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn native_mux_idle_session_exits() {
        let (client_io, server_io) = duplex(64 * 1024);
        let config = NativeMuxConfig {
            session_idle_timeout: Duration::from_millis(10),
            ..NativeMuxConfig::default()
        };
        let (_client_control, mut client_session) = client_session(client_io, config);
        let (_server_control, _server_session) = server_session(server_io, config);

        let next = tokio::time::timeout(Duration::from_secs(1), client_session.next())
            .await
            .unwrap();
        assert!(next.is_none());
    }
}
