use std::pin::Pin;
use std::task::{Context, Poll};

use espejismo_core::{mux::native, MuxMode};
use futures::Stream;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_yamux::{Config as YamuxConfig, Session, StreamHandle};

pub(crate) enum MuxStream {
    Yamux(StreamHandle),
    Native(native::NativeStream),
}

pub(crate) enum ServerMuxSession<T> {
    Yamux(Box<Session<T>>),
    Native(native::NativeSession),
}

pub(crate) fn server_session<T>(transport: T, mode: MuxMode) -> ServerMuxSession<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    match mode {
        MuxMode::Yamux => ServerMuxSession::Yamux(Box::new(Session::new_server(
            transport,
            YamuxConfig::default(),
        ))),
        MuxMode::Native => {
            let (_control, session) = native::server_session(transport);
            ServerMuxSession::Native(session)
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
