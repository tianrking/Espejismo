//! Regression test for the window-update writer deadlock.
//!
//! Root cause: `StreamHandle::poll_write` drained inbound frames with the
//! non-blocking `try_recv_frames()`, which never registers the task's waker
//! on the stream's `frame_receiver`. When `send_window` hit 0 the writer
//! parked, and a `WindowUpdate` arriving afterwards woke nobody:
//! `handle_window_update` never ran, `send_window` stayed 0, and any one-way
//! bulk transfer larger than the stream window stalled forever.
//!
//! This test does a one-way 8 MiB transfer over a 1 MiB window with NO read
//! task on the writer side. Without the fix it never completes; with the
//! fix it finishes quickly on loopback.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_yamux::{Config, Session};

const WINDOW: u32 = 1024 * 1024;
const TOTAL: usize = 8 * 1024 * 1024;

fn cfg() -> Config {
    Config {
        max_stream_window_size: WINDOW,
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_way_bulk_transfer_exceeding_window() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // ---- server (reader) side ----
    let server = tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let mut session = Session::new_server(sock, cfg());
        // Accept the first stream, then keep driving the session in the
        // background so window updates are flushed (required yamux usage).
        let mut stream = {
            use futures::StreamExt;
            loop {
                if let Some(s) = session.next().await {
                    break s.expect("accept stream");
                }
            }
        };
        let _drive = tokio::spawn(async move {
            use futures::StreamExt;
            while session.next().await.is_some() {}
        });
        let mut received: usize = 0;
        let mut buf = vec![0u8; 65536];
        loop {
            let n = stream.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            received += n;
        }
        received
    });

    // ---- client (writer) side: only writes, never reads ----
    let client = tokio::spawn(async move {
        let sock = TcpStream::connect(addr).await.unwrap();
        let mut session = Session::new_client(sock, cfg());
        let mut ctrl = session.control();
        // Drive the session in the background.
        let _drive = tokio::spawn(async move {
            use futures::StreamExt;
            while session.next().await.is_some() {}
        });
        let mut stream = ctrl.open_stream().await.unwrap();
        let chunk = vec![0xABu8; 65536];
        let mut sent: usize = 0;
        while sent < TOTAL {
            let n = std::cmp::min(chunk.len(), TOTAL - sent);
            stream.write_all(&chunk[..n]).await.unwrap();
            sent += n;
        }
        stream.shutdown().await.unwrap();
        sent
    });

    let (received, sent) = tokio::time::timeout(Duration::from_secs(20), async {
        (server.await.unwrap(), client.await.unwrap())
    })
    .await
    .expect("one-way bulk transfer stalled: window-update deadlock");
    assert_eq!(sent, TOTAL);
    assert_eq!(received, TOTAL);
}
