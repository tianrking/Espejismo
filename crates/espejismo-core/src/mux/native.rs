use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{bail, Context as AnyhowContext, Result};
use futures::Stream;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::{mpsc, oneshot};

const FRAME_OPEN: u8 = 1;
const FRAME_DATA: u8 = 2;
const FRAME_WINDOW_UPDATE: u8 = 3;
const FRAME_FIN: u8 = 4;
const FRAME_RST: u8 = 5;
const FRAME_PING: u8 = 6;
const FRAME_GOAWAY: u8 = 7;
const MAX_PAYLOAD: usize = 64 * 1024;

#[derive(Clone)]
pub struct NativeControl {
    tx: mpsc::UnboundedSender<Command>,
}

pub struct NativeSession {
    accept_rx: mpsc::UnboundedReceiver<Result<NativeStream, io::Error>>,
}

pub struct NativeStream {
    id: u32,
    tx: mpsc::UnboundedSender<Command>,
    rx: mpsc::UnboundedReceiver<Vec<u8>>,
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
    },
    Ping,
    Goaway,
}

struct StreamEntry {
    tx: mpsc::UnboundedSender<Vec<u8>>,
}

pub fn client_session<T>(transport: T) -> (NativeControl, NativeSession)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    start_session(transport, 1)
}

pub fn server_session<T>(transport: T) -> (NativeControl, NativeSession)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    start_session(transport, 2)
}

fn start_session<T>(transport: T, first_stream_id: u32) -> (NativeControl, NativeSession)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (accept_tx, accept_rx) = mpsc::unbounded_channel();
    let control = NativeControl {
        tx: command_tx.clone(),
    };
    tokio::spawn(run_session(
        transport,
        command_tx,
        command_rx,
        accept_tx,
        first_stream_id,
    ));
    (control, NativeSession { accept_rx })
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
        _cx: &mut Context<'_>,
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
        let n = buf.len().min(MAX_PAYLOAD);
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
                if handle_command(command, &mut transport, &command_tx, &mut streams, &mut next_stream_id).await.is_err() {
                    break;
                }
            }
            frame = read_frame(&mut transport) => {
                match frame {
                    Ok(Some((kind, stream_id, payload))) => {
                        if handle_frame(kind, stream_id, payload, &command_tx, &accept_tx, &mut streams).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        let _ = accept_tx.send(Err(io::Error::new(io::ErrorKind::InvalidData, err.to_string())));
                        break;
                    }
                }
            }
        }
    }
}

async fn handle_command<T>(
    command: Command,
    transport: &mut T,
    command_tx: &mpsc::UnboundedSender<Command>,
    streams: &mut HashMap<u32, StreamEntry>,
    next_stream_id: &mut u32,
) -> Result<()>
where
    T: AsyncWrite + Unpin,
{
    match command {
        Command::Open { reply } => {
            let stream_id = *next_stream_id;
            *next_stream_id = next_stream_id.saturating_add(2);
            let (tx, rx) = mpsc::unbounded_channel();
            streams.insert(stream_id, StreamEntry { tx });
            write_frame(transport, FRAME_OPEN, stream_id, &[]).await?;
            let _ = reply.send(Ok(NativeStream {
                id: stream_id,
                tx: command_tx.clone(),
                rx,
                read_buf: Vec::new(),
                read_offset: 0,
                remote_closed: false,
                local_closed: false,
            }));
        }
        Command::Data { stream_id, payload } => {
            write_frame(transport, FRAME_DATA, stream_id, &payload).await?;
        }
        Command::Fin { stream_id } => {
            write_frame(transport, FRAME_FIN, stream_id, &[]).await?;
        }
        Command::Rst { stream_id } => {
            streams.remove(&stream_id);
            write_frame(transport, FRAME_RST, stream_id, &[]).await?;
        }
        Command::WindowUpdate { stream_id } => {
            write_frame(transport, FRAME_WINDOW_UPDATE, stream_id, &[]).await?;
        }
        Command::Ping => {
            write_frame(transport, FRAME_PING, 0, &[]).await?;
        }
        Command::Goaway => {
            write_frame(transport, FRAME_GOAWAY, 0, &[]).await?;
        }
    }
    Ok(())
}

async fn handle_frame(
    kind: u8,
    stream_id: u32,
    payload: Vec<u8>,
    command_tx: &mpsc::UnboundedSender<Command>,
    accept_tx: &mpsc::UnboundedSender<Result<NativeStream, io::Error>>,
    streams: &mut HashMap<u32, StreamEntry>,
) -> Result<()> {
    match kind {
        FRAME_OPEN => {
            let (tx, rx) = mpsc::unbounded_channel();
            streams.insert(stream_id, StreamEntry { tx });
            let stream = NativeStream {
                id: stream_id,
                tx: command_tx.clone(),
                rx,
                read_buf: Vec::new(),
                read_offset: 0,
                remote_closed: false,
                local_closed: false,
            };
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
                .map_err(|_| anyhow::anyhow!("native mux stream receiver dropped"))?;
            let _ = command_tx.send(Command::WindowUpdate { stream_id });
        }
        FRAME_WINDOW_UPDATE => {}
        FRAME_FIN | FRAME_RST => {
            streams.remove(&stream_id);
        }
        FRAME_PING => {}
        FRAME_GOAWAY => bail!("native mux received goaway"),
        _ => bail!("unknown native mux frame type {kind}"),
    }
    Ok(())
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
    use super::{client_session, server_session};
    use futures::StreamExt;
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn native_mux_opens_stream_and_roundtrips_data() {
        let (client_io, server_io) = duplex(64 * 1024);
        let (mut client_control, mut client_session) = client_session(client_io);
        let (_server_control, mut server_session) = server_session(server_io);

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
}
