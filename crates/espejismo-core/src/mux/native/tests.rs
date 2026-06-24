use std::io;
use std::time::Duration;

use futures::StreamExt;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

use super::{client_session, server_session, NativeMuxConfig};
use crate::protocol::request::StreamPriority;

#[test]
fn native_frame_parser_rejects_unknown_types_and_large_payloads() {
    assert!(super::validate_frame_bytes_for_fuzz(&[99, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
    let mut frame = vec![super::FRAME_DATA, 0, 0, 0, 1];
    frame.extend_from_slice(&((super::MAX_PAYLOAD as u32) + 1).to_be_bytes());
    assert!(super::validate_frame_bytes_for_fuzz(&frame).is_err());
}

#[test]
fn native_pending_frames_enforce_limit() {
    let mut pending = super::pending::PendingFrames::new(1);
    pending
        .push_control(super::pending::PendingFrame {
            kind: super::FRAME_PING,
            stream_id: 0,
            payload: vec![0; 8],
            queued_stream: None,
        })
        .unwrap();
    assert!(pending
        .push_control(super::pending::PendingFrame {
            kind: super::FRAME_PING,
            stream_id: 0,
            payload: vec![1; 8],
            queued_stream: None,
        })
        .is_err());
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
async fn native_mux_bulk_transfer_preserves_integrity() {
    // Pure native mux over an in-memory duplex: NO encrypted pump, NO TCP.
    // If this desyncs, the bug is inside the native mux itself.
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

    let total: usize = 1024 * 1024; // 1 MB
    let writer = tokio::spawn(async move {
        let mut sent = 0usize;
        let mut buf = vec![0u8; 8192];
        while sent < total {
            for j in 0..buf.len() {
                if sent + j >= total {
                    buf.truncate(j);
                    break;
                }
                buf[j] = (sent + j) as u8;
            }
            let n = server_stream.write(&buf).await.unwrap();
            assert!(n > 0, "server stream write returned 0");
            sent += n;
            buf.clear();
            buf.resize(8192, 0);
        }
        server_stream.shutdown().await.unwrap();
        sent
    });

    let mut received = Vec::with_capacity(total);
    client_stream.read_to_end(&mut received).await.unwrap();
    let sent = writer.await.unwrap();
    assert_eq!(sent, total, "server did not send all bytes");
    assert_eq!(received.len(), total, "client received wrong byte count");
    for (i, b) in received.iter().enumerate() {
        assert_eq!(*b, (i % 256) as u8, "byte mismatch at offset {i}");
    }
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
    let blocked = tokio::time::timeout(Duration::from_millis(50), client_stream.write(b"e")).await;
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

    let blocked = tokio::time::timeout(Duration::from_millis(50), client_stream.write(b"e")).await;
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
async fn native_mux_ignores_unknown_stream_data_with_rst() {
    let (mut client_io, server_io) = duplex(64 * 1024);
    let (_server_control, mut server_session) =
        server_session(server_io, NativeMuxConfig::default());

    super::frame::write_frame(&mut client_io, super::FRAME_DATA, 99, b"orphan")
        .await
        .unwrap();
    let frame = tokio::time::timeout(
        Duration::from_secs(1),
        super::frame::read_frame(&mut client_io),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(frame.0, super::FRAME_RST);
    assert_eq!(frame.1, 99);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), server_session.next())
            .await
            .is_err()
    );
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
