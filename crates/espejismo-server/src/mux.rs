use tokio::io::{AsyncRead, AsyncWrite};
use tokio_yamux::{Config as YamuxConfig, Session, StreamHandle};

pub(crate) type MuxStream = StreamHandle;

pub(crate) fn server_session<T>(transport: T) -> Session<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    Session::new_server(transport, YamuxConfig::default())
}
