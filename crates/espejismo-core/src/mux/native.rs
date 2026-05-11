use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use anyhow::{bail, Context as AnyhowContext, Result};
use futures::Stream;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

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
    pub session_idle_timeout: Duration,
}

impl Default for NativeMuxConfig {
    fn default() -> Self {
        Self {
            max_streams: 256,
            initial_window_bytes: 1024 * 1024,
            stream_buffer_frames: 128,
            session_idle_timeout: Duration::from_secs(300),
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
    tx: mpsc::UnboundedSender<Command>,
    rx: mpsc::Receiver<Vec<u8>>,
    flow: Arc<Mutex<FlowState>>,
    read_buf: Vec<u8>,
    read_offset: usize,
    remote_closed: bool,
    local_closed: bool,
}

enum Command {
    Open {
        reply: oneshot::Sender<Result<NativeStream>>,
    },
    Data {
        stream_id: u32,
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
    Ping,
    Goaway,
}

struct StreamEntry {
    tx: mpsc::Sender<Vec<u8>>,
    flow: Arc<Mutex<FlowState>>,
}

#[derive(Debug)]
struct FlowState {
    available: usize,
    waker: Option<Waker>,
}

impl FlowState {
    fn new(window: usize) -> Self {
        Self {
            available: window,
            waker: None,
        }
    }

    fn add_window(&mut self, amount: usize) {
        self.available = self.available.saturating_add(amount);
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
        session_idle_timeout: config.session_idle_timeout.max(Duration::from_millis(1)),
    }
}

impl NativeControl {
    pub async fn open_stream(&mut self) -> Result<NativeStream> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Command::Open { reply })
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "native mux stopped"))?;
        rx.await.context("native mux open stream reply dropped")?
    }

    pub fn ping(&self) -> Result<()> {
        self.tx
            .send(Command::Ping)
            .map_err(|_| anyhow::anyhow!("native mux stopped"))
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
                let _ = self.tx.send(Command::WindowUpdate {
                    stream_id: self.id,
                    amount: n.min(u32::MAX as usize) as u32,
                });
                if self.read_offset == self.read_buf.len() {
                    self.read_buf.clear();
                    self.read_offset = 0;
                }
                return Poll::Ready(Ok(()));
            }
            if self.remote_closed {
                return Poll::Ready(Ok(()));
            }
            match Pin::new(&mut self.rx).poll_recv(cx) {
                Poll::Ready(Some(payload)) => {
                    self.read_buf = payload;
                    self.read_offset = 0;
                }
                Poll::Ready(None) => {
                    self.remote_closed = true;
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => return Poll::Pending,
            }
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
            if flow.available == 0 {
                flow.waker = Some(cx.waker().clone());
                return Poll::Pending;
            }
            let n = buf.len().min(MAX_PAYLOAD).min(flow.available);
            flow.available -= n;
            n
        };

        this.tx
            .send(Command::Data {
                stream_id: this.id,
                payload: buf[..n].to_vec(),
            })
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "native mux stopped"))?;
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
    loop {
        tokio::select! {
            command = command_rx.recv() => {
                let Some(command) = command else {
                    let _ = write_frame(&mut transport, FRAME_GOAWAY, 0, &[]).await;
                    break;
                };
                match handle_command(command, &mut transport, &command_tx, &mut streams, &mut next_stream_id, &config).await {
                    Ok(KeepRunning::Yes) => {}
                    Ok(KeepRunning::No) | Err(_) => break,
                }
            }
            frame = read_frame(&mut transport) => {
                match frame {
                    Ok(Some((kind, stream_id, payload))) => {
                        match handle_frame(kind, stream_id, payload, &command_tx, &accept_tx, &mut streams, &config).await {
                            Ok(KeepRunning::Yes) => {}
                            Ok(KeepRunning::No) => break,
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
            _ = tokio::time::sleep(config.session_idle_timeout), if streams.is_empty() => {
                let _ = write_frame(&mut transport, FRAME_GOAWAY, 0, &[]).await;
                break;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KeepRunning {
    Yes,
    No,
}

async fn handle_command<T>(
    command: Command,
    transport: &mut T,
    command_tx: &mpsc::UnboundedSender<Command>,
    streams: &mut HashMap<u32, StreamEntry>,
    next_stream_id: &mut u32,
    config: &NativeMuxConfig,
) -> Result<KeepRunning>
where
    T: AsyncWrite + Unpin,
{
    match command {
        Command::Open { reply } => {
            if streams.len() >= config.max_streams {
                let _ = reply.send(Err(anyhow::anyhow!("native mux max streams reached")));
                return Ok(KeepRunning::Yes);
            }
            let stream_id = *next_stream_id;
            *next_stream_id = next_stream_id.saturating_add(2);
            let (stream, entry) = new_stream(
                stream_id,
                command_tx.clone(),
                config.initial_window_bytes,
                config.stream_buffer_frames,
            );
            streams.insert(stream_id, entry);
            write_frame(transport, FRAME_OPEN, stream_id, &[]).await?;
            let _ = reply.send(Ok(stream));
        }
        Command::Data { stream_id, payload } => {
            if streams.contains_key(&stream_id) {
                write_frame(transport, FRAME_DATA, stream_id, &payload).await?;
            }
        }
        Command::Fin { stream_id } => {
            write_frame(transport, FRAME_FIN, stream_id, &[]).await?;
        }
        Command::Rst { stream_id } => {
            streams.remove(&stream_id);
            write_frame(transport, FRAME_RST, stream_id, &[]).await?;
        }
        Command::WindowUpdate { stream_id, amount } => {
            write_frame(
                transport,
                FRAME_WINDOW_UPDATE,
                stream_id,
                &amount.to_be_bytes(),
            )
            .await?;
        }
        Command::Ping => {
            write_frame(transport, FRAME_PING, 0, &[]).await?;
        }
        Command::Goaway => {
            write_frame(transport, FRAME_GOAWAY, 0, &[]).await?;
            return Ok(KeepRunning::No);
        }
    }
    Ok(KeepRunning::Yes)
}

async fn handle_frame(
    kind: u8,
    stream_id: u32,
    payload: Vec<u8>,
    command_tx: &mpsc::UnboundedSender<Command>,
    accept_tx: &mpsc::UnboundedSender<Result<NativeStream, io::Error>>,
    streams: &mut HashMap<u32, StreamEntry>,
    config: &NativeMuxConfig,
) -> Result<KeepRunning> {
    match kind {
        FRAME_OPEN => {
            if streams.len() >= config.max_streams {
                let _ = command_tx.send(Command::Rst { stream_id });
                return Ok(KeepRunning::Yes);
            }
            let (stream, entry) = new_stream(
                stream_id,
                command_tx.clone(),
                config.initial_window_bytes,
                config.stream_buffer_frames,
            );
            streams.insert(stream_id, entry);
            accept_tx
                .send(Ok(stream))
                .map_err(|_| anyhow::anyhow!("native mux accept receiver dropped"))?;
        }
        FRAME_DATA => {
            let Some(stream) = streams.get(&stream_id) else {
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
            if let Some(stream) = streams.get(&stream_id) {
                let mut flow = stream
                    .flow
                    .lock()
                    .map_err(|_| anyhow::anyhow!("native mux flow state poisoned"))?;
                flow.add_window(amount);
            }
        }
        FRAME_FIN | FRAME_RST => {
            streams.remove(&stream_id);
        }
        FRAME_PING => {
            let _ = command_tx.send(Command::Ping);
        }
        FRAME_GOAWAY => return Ok(KeepRunning::No),
        _ => bail!("unknown native mux frame type {kind}"),
    }
    Ok(KeepRunning::Yes)
}

fn new_stream(
    stream_id: u32,
    command_tx: mpsc::UnboundedSender<Command>,
    initial_window_bytes: usize,
    stream_buffer_frames: usize,
) -> (NativeStream, StreamEntry) {
    let (tx, rx) = mpsc::channel(stream_buffer_frames);
    let flow = Arc::new(Mutex::new(FlowState::new(initial_window_bytes)));
    let stream = NativeStream {
        id: stream_id,
        tx: command_tx,
        rx,
        flow: flow.clone(),
        read_buf: Vec::new(),
        read_offset: 0,
        remote_closed: false,
        local_closed: false,
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{client_session, server_session, NativeMuxConfig};
    use futures::StreamExt;
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn native_mux_opens_stream_and_roundtrips_data() {
        let (client_io, server_io) = duplex(64 * 1024);
        let (mut client_control, mut client_session) =
            client_session(client_io, NativeMuxConfig::default());
        let (_server_control, mut server_session) =
            server_session(server_io, NativeMuxConfig::default());

        tokio::spawn(async move { while client_session.next().await.is_some() {} });

        let mut client_stream = client_control.open_stream().await.unwrap();
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
    async fn native_mux_enforces_max_streams() {
        let (client_io, server_io) = duplex(64 * 1024);
        let config = NativeMuxConfig {
            max_streams: 1,
            ..NativeMuxConfig::default()
        };
        let (mut client_control, mut client_session) = client_session(client_io, config);
        let (_server_control, mut server_session) = server_session(server_io, config);

        tokio::spawn(async move { while client_session.next().await.is_some() {} });

        let first_stream = client_control.open_stream().await.unwrap();
        let first_server_stream = server_session.next().await.unwrap().unwrap();
        assert!(client_control.open_stream().await.is_err());
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

        let mut client_stream = client_control.open_stream().await.unwrap();
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
