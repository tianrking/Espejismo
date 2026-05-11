use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Result;
use espejismo_core::{mux::native, MuxMode};
use futures::Stream;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_yamux::{Config as YamuxConfig, Control, Session, StreamHandle};

pub(crate) enum MuxStream {
    Yamux(StreamHandle),
    Native(native::NativeStream),
}

pub(crate) enum MuxControl {
    Yamux(Control),
    Native(native::NativeControl),
}

pub(crate) enum ClientMuxSession<T> {
    Yamux(Box<Session<T>>),
    Native(native::NativeSession),
}

impl MuxControl {
    pub(crate) async fn open_stream(&mut self) -> Result<MuxStream> {
        match self {
            Self::Yamux(control) => Ok(MuxStream::Yamux(control.open_stream().await?)),
            Self::Native(control) => Ok(MuxStream::Native(control.open_stream().await?)),
        }
    }
}

pub(crate) fn client_session<T>(transport: T, mode: MuxMode) -> (MuxControl, ClientMuxSession<T>)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    match mode {
        MuxMode::Yamux => {
            let session = Session::new_client(transport, YamuxConfig::default());
            let control = MuxControl::Yamux(session.control());
            (control, ClientMuxSession::Yamux(Box::new(session)))
        }
        MuxMode::Native => {
            let (control, session) = native::client_session(transport);
            (
                MuxControl::Native(control),
                ClientMuxSession::Native(session),
            )
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
