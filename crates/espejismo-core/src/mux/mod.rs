pub mod native;

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::Result;
use futures::Stream;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_yamux::{Config as YamuxConfig, Control, Session, StreamHandle};

use crate::config::{MuxConfig, MuxMode};

#[derive(Clone, Copy, Debug)]
pub struct MuxRuntimeConfig {
    pub mode: MuxMode,
    pub max_streams: usize,
    pub native_initial_window_bytes: usize,
    pub native_stream_buffer_frames: usize,
    pub native_idle_timeout: Duration,
}

impl MuxRuntimeConfig {
    pub fn from_config(max_streams: u32, config: &MuxConfig) -> Self {
        Self {
            mode: config.mode,
            max_streams: max_streams.max(1) as usize,
            native_initial_window_bytes: config.native_initial_window_bytes.max(1),
            native_stream_buffer_frames: config.native_stream_buffer_frames.max(1),
            native_idle_timeout: Duration::from_secs(config.native_idle_timeout_secs.max(1)),
        }
    }

    fn native(self) -> native::NativeMuxConfig {
        native::NativeMuxConfig {
            max_streams: self.max_streams,
            initial_window_bytes: self.native_initial_window_bytes,
            stream_buffer_frames: self.native_stream_buffer_frames,
            session_idle_timeout: self.native_idle_timeout,
        }
    }
}

pub enum MuxStream {
    Yamux(StreamHandle),
    Native(native::NativeStream),
}

pub enum MuxControl {
    Yamux(Control),
    Native(native::NativeControl),
}

pub enum ClientMuxSession<T> {
    Yamux(Box<Session<T>>),
    Native(native::NativeSession),
}

pub enum ServerMuxSession<T> {
    Yamux(Box<Session<T>>),
    Native(native::NativeSession),
}

impl MuxControl {
    pub async fn open_stream(&mut self) -> Result<MuxStream> {
        match self {
            Self::Yamux(control) => Ok(MuxStream::Yamux(control.open_stream().await?)),
            Self::Native(control) => Ok(MuxStream::Native(control.open_stream().await?)),
        }
    }
}

pub fn client_session<T>(
    transport: T,
    config: MuxRuntimeConfig,
) -> (MuxControl, ClientMuxSession<T>)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    match config.mode {
        MuxMode::Yamux => {
            let session = Session::new_client(transport, YamuxConfig::default());
            let control = MuxControl::Yamux(session.control());
            (control, ClientMuxSession::Yamux(Box::new(session)))
        }
        MuxMode::Native => {
            let (control, session) = native::client_session(transport, config.native());
            (
                MuxControl::Native(control),
                ClientMuxSession::Native(session),
            )
        }
    }
}

pub fn server_session<T>(transport: T, config: MuxRuntimeConfig) -> ServerMuxSession<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    match config.mode {
        MuxMode::Yamux => ServerMuxSession::Yamux(Box::new(Session::new_server(
            transport,
            YamuxConfig::default(),
        ))),
        MuxMode::Native => {
            let (_control, session) = native::server_session(transport, config.native());
            ServerMuxSession::Native(session)
        }
    }
}

impl<T> Stream for ClientMuxSession<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = Result<(), anyhow::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut *self {
            Self::Yamux(session) => match Pin::new(session).poll_next(cx) {
                Poll::Ready(Some(Ok(_stream))) => Poll::Ready(Some(Ok(()))),
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            },
            Self::Native(session) => match Pin::new(session).poll_next(cx) {
                Poll::Ready(Some(Ok(_stream))) => Poll::Ready(Some(Ok(()))),
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

impl<T> Stream for ServerMuxSession<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = Result<MuxStream, anyhow::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut *self {
            Self::Yamux(session) => match Pin::new(session).poll_next(cx) {
                Poll::Ready(Some(Ok(stream))) => Poll::Ready(Some(Ok(MuxStream::Yamux(stream)))),
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            },
            Self::Native(session) => match Pin::new(session).poll_next(cx) {
                Poll::Ready(Some(Ok(stream))) => Poll::Ready(Some(Ok(MuxStream::Native(stream)))),
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

impl AsyncRead for MuxStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Yamux(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Native(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for MuxStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            Self::Yamux(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Native(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Yamux(stream) => Pin::new(stream).poll_flush(cx),
            Self::Native(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Yamux(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Native(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}
