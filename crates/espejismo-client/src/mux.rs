use anyhow::Result;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_yamux::{Config as YamuxConfig, Control, Session, StreamHandle};

pub(crate) type MuxStream = StreamHandle;

pub(crate) struct MuxControl {
    inner: Control,
}

impl MuxControl {
    pub(crate) fn new(inner: Control) -> Self {
        Self { inner }
    }

    pub(crate) async fn open_stream(&mut self) -> Result<MuxStream> {
        Ok(self.inner.open_stream().await?)
    }
}

pub(crate) fn client_session<T>(transport: T) -> Session<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    Session::new_client(transport, YamuxConfig::default())
}
