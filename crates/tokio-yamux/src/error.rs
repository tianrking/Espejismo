//! The error types

use std::{error, fmt};

/// The error types
#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    /// InvalidVersion means we received a frame with an
    /// invalid version
    InvalidVersion,

    /// InvalidMsgType means we received a frame with an
    /// invalid message type
    InvalidMsgType,

    /// SessionShutdown is used if there is a shutdown during
    /// an operation
    SessionShutdown,

    /// StreamsExhausted is returned if we have no more
    /// stream ids to issue
    StreamsExhausted,

    /// DuplicateStream is used if a duplicate stream is
    /// opened inbound
    DuplicateStream,

    /// ReceiveWindowExceeded indicates the window was exceeded
    RecvWindowExceeded,

    /// Timeout is used when we reach an IO deadline
    Timeout,

    /// StreamClosed is returned when using a closed stream
    StreamClosed,

    /// UnexpectedFlag is set when we get an unexpected flag
    UnexpectedFlag,

    /// RemoteGoAway is used when we get a go away from the other side
    RemoteGoAway,

    /// ConnectionReset is sent if a stream is reset. This can happen
    /// if the backlog is exceeded, or if there was a remote GoAway.
    ConnectionReset,

    /// ConnectionWriteTimeout indicates that we hit the "safety valve"
    /// timeout writing to the underlying stream connection.
    ConnectionWriteTimeout,

    /// KeepAliveTimeout is sent if a missed keepalive caused the stream close
    KeepAliveTimeout,

    /// Remote sub stream is closed, but local can still send data to remote
    SubStreamRemoteClosing,

    /// Sub stream send event channel full, block to complete
    WouldBlock,
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::InvalidVersion => write!(f, "yamux: received a frame with an invalid version"),
            Error::InvalidMsgType => {
                write!(f, "yamux: received a frame with an invalid message type")
            }
            Error::SessionShutdown => write!(f, "yamux: session shut down"),
            Error::StreamsExhausted => write!(f, "yamux: no stream IDs remain"),
            Error::DuplicateStream => write!(f, "yamux: received a duplicate inbound stream"),
            Error::RecvWindowExceeded => write!(f, "yamux: receive window was exceeded"),
            Error::Timeout => write!(f, "yamux: I/O deadline reached"),
            Error::StreamClosed => write!(f, "yamux: stream is closed"),
            Error::UnexpectedFlag => write!(f, "yamux: received an unexpected flag"),
            Error::RemoteGoAway => {
                write!(f, "yamux: received a go-away message from the remote peer")
            }
            Error::ConnectionReset => write!(f, "yamux: stream was reset"),
            Error::ConnectionWriteTimeout => {
                write!(f, "yamux: timed out writing to the underlying connection")
            }
            Error::KeepAliveTimeout => write!(f, "yamux: keepalive timed out"),
            Error::SubStreamRemoteClosing => write!(f, "yamux: remote substream is closed"),
            Error::WouldBlock => write!(f, "yamux: substream send channel is full"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn display_messages_have_consistent_prefix_and_text() {
        let cases = [
            (
                Error::InvalidVersion,
                "yamux: received a frame with an invalid version",
            ),
            (
                Error::InvalidMsgType,
                "yamux: received a frame with an invalid message type",
            ),
            (Error::SessionShutdown, "yamux: session shut down"),
            (Error::StreamsExhausted, "yamux: no stream IDs remain"),
            (
                Error::DuplicateStream,
                "yamux: received a duplicate inbound stream",
            ),
            (
                Error::RecvWindowExceeded,
                "yamux: receive window was exceeded",
            ),
            (Error::Timeout, "yamux: I/O deadline reached"),
            (Error::StreamClosed, "yamux: stream is closed"),
            (Error::UnexpectedFlag, "yamux: received an unexpected flag"),
            (
                Error::RemoteGoAway,
                "yamux: received a go-away message from the remote peer",
            ),
            (Error::ConnectionReset, "yamux: stream was reset"),
            (
                Error::ConnectionWriteTimeout,
                "yamux: timed out writing to the underlying connection",
            ),
            (Error::KeepAliveTimeout, "yamux: keepalive timed out"),
            (
                Error::SubStreamRemoteClosing,
                "yamux: remote substream is closed",
            ),
            (Error::WouldBlock, "yamux: substream send channel is full"),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }
}
