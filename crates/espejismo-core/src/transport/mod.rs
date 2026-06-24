use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use anyhow::Result;
use rand::Rng;
use tokio::io::{duplex, split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};
use tokio::time::{sleep, timeout};
use tracing::debug;

use crate::crypto::SessionKeys;
use crate::protocol::framing::{Frame, FrameOptions, FrameReader, FrameType, FrameWriter};

const STEALTH_WARMUP_MIN_FRAMES: usize = 2;
const STEALTH_WARMUP_MAX_FRAMES: usize = 5;
const STEALTH_IDLE_DECAY_FRAMES: u64 = 8;
const COPY_BUFFER_SIZE: usize = 128 * 1024;

pub fn spawn_frame_transport<S>(
    stream: S,
    keys: SessionKeys,
    options: FrameOptions,
    buffer_size: usize,
) -> DuplexStream
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let (app_stream, pump_stream) = duplex(buffer_size);
    let (app_reader, app_writer) = split(pump_stream);
    let (net_reader, net_writer) = split(stream);

    let upload_keys = keys.clone();
    let upload_options = options.clone();
    tokio::spawn(async move {
        if let Err(err) = upload_frames(app_reader, net_writer, upload_keys, upload_options).await {
            debug!(error = %err, "encrypted upload pump stopped");
        }
    });

    tokio::spawn(async move {
        if let Err(err) = download_frames(net_reader, app_writer, keys, options).await {
            debug!(error = %err, "encrypted download pump stopped");
        }
    });

    app_stream
}

pub async fn idle_copy_bidirectional<A, B>(
    a: &mut A,
    b: &mut B,
    idle: Duration,
) -> std::io::Result<(u64, u64)>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let mut meter = NoopCopyMeter;
    metered_idle_copy_bidirectional(a, b, idle, &mut meter)
        .await
        .map_err(std::io::Error::other)
}

pub trait CopyMeter {
    fn account<'a>(
        &'a mut self,
        bytes: u64,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}

pub struct NoopCopyMeter;

impl CopyMeter for NoopCopyMeter {
    fn account<'a>(
        &'a mut self,
        _bytes: u64,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }
}

pub async fn metered_idle_copy_bidirectional<A, B, M>(
    a: &mut A,
    b: &mut B,
    idle: Duration,
    meter: &mut M,
) -> Result<(u64, u64)>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
    M: CopyMeter,
{
    let mut buf_a = [0u8; COPY_BUFFER_SIZE];
    let mut buf_b = [0u8; COPY_BUFFER_SIZE];
    let mut total_a = 0u64;
    let mut total_b = 0u64;
    let mut a_done = false;
    let mut b_done = false;

    loop {
        let read_a = if !a_done {
            Some(timeout(idle, a.read(&mut buf_a)))
        } else {
            None
        };
        let read_b = if !b_done {
            Some(timeout(idle, b.read(&mut buf_b)))
        } else {
            None
        };

        match (read_a, read_b) {
            (Some(ra), Some(rb)) => {
                tokio::select! {
                    r = ra => {
                        match r {
                            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => {
                                a_done = true;
                                let _ = b.shutdown().await;
                                if b_done { break; }
                            }
                            Ok(Ok(n)) => {
                                meter.account(n as u64).await?;
                                total_a += n as u64;
                                b.write_all(&buf_a[..n]).await?;
                            }
                        }
                        continue;
                    }
                    r = rb => {
                        match r {
                            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => {
                                b_done = true;
                                let _ = a.shutdown().await;
                                if a_done { break; }
                            }
                            Ok(Ok(n)) => {
                                meter.account(n as u64).await?;
                                total_b += n as u64;
                                a.write_all(&buf_b[..n]).await?;
                            }
                        }
                        continue;
                    }
                }
            }
            (Some(ra), None) => match ra.await {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                Ok(Ok(n)) => {
                    meter.account(n as u64).await?;
                    total_a += n as u64;
                    b.write_all(&buf_a[..n]).await?;
                }
            },
            (None, Some(rb)) => match rb.await {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                Ok(Ok(n)) => {
                    meter.account(n as u64).await?;
                    total_b += n as u64;
                    a.write_all(&buf_b[..n]).await?;
                }
            },
            (None, None) => break,
        }
    }

    Ok((total_a, total_b))
}

async fn upload_frames<R, W>(
    mut app_reader: R,
    net_writer: W,
    keys: SessionKeys,
    options: FrameOptions,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    if options.is_stealth() {
        return upload_stealth_frames(app_reader, net_writer, keys, options).await;
    }
    let mut buf = vec![0_u8; options.normalized_chunk_bounds().1];
    let mut frame_writer = FrameWriter::new(net_writer, keys, options);
    let heartbeat = frame_writer.options().heartbeat_secs;
    let started = Instant::now();
    let mut data_frames = 0_u64;
    let mut padding_frames = 0_u64;
    let mut bytes = 0_u64;
    loop {
        let chunk_size = frame_writer.options().next_chunk_size();
        let n = if heartbeat == 0 {
            app_reader.read(&mut buf[..chunk_size]).await?
        } else {
            tokio::select! {
                read = app_reader.read(&mut buf[..chunk_size]) => read?,
                _ = sleep(Duration::from_secs(heartbeat)) => {
                    frame_writer
                        .send(Frame {
                            ty: FrameType::Padding,
                            payload: Vec::new(),
                        })
                        .await?;
                    padding_frames = padding_frames.saturating_add(1);
                    continue;
                }
            }
        };
        if n == 0 {
            frame_writer
                .send(Frame {
                    ty: FrameType::Close,
                    payload: Vec::new(),
                })
                .await?;
            debug!(
                elapsed_ms = started.elapsed().as_millis(),
                data_frames,
                padding_frames,
                bytes,
                bps = throughput_bps(bytes, started.elapsed()),
                "perf encrypted upload pump completed"
            );
            return Ok(());
        }
        data_frames = data_frames.saturating_add(1);
        bytes = bytes.saturating_add(n as u64);
        frame_writer
            .send(Frame {
                ty: FrameType::Data,
                payload: buf[..n].to_vec(),
            })
            .await?;
    }
}

async fn upload_stealth_frames<R, W>(
    mut app_reader: R,
    net_writer: W,
    keys: SessionKeys,
    options: FrameOptions,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let capacity = options.stealth_payload_capacity()?;
    let mut read_buf = vec![0_u8; capacity];
    let mut pending = VecDeque::with_capacity(capacity * 2);
    let pending_limit = capacity * 8;
    let mut frame_writer = FrameWriter::new(net_writer, keys, options.clone());
    let mut app_closed = false;
    let mut idle_frames = 0_u64;
    let started = Instant::now();
    let mut data_frames = 0_u64;
    let mut padding_frames = 0_u64;
    let mut bytes = 0_u64;

    let warmup_frames =
        rand::thread_rng().gen_range(STEALTH_WARMUP_MIN_FRAMES..=STEALTH_WARMUP_MAX_FRAMES);
    for _ in 0..warmup_frames {
        stealth_pre_write_delay(&options).await;
        frame_writer
            .send(Frame {
                ty: FrameType::Padding,
                payload: Vec::new(),
            })
            .await?;
        padding_frames = padding_frames.saturating_add(1);
        sleep(stealth_tick_delay(&options, idle_frames)).await;
        idle_frames += 1;
    }

    loop {
        tokio::select! {
            read = app_reader.read(&mut read_buf), if !app_closed && pending.len() < pending_limit => {
                match read? {
                    0 => app_closed = true,
                    n => pending.extend(&read_buf[..n]),
                }
            }
            _ = sleep(stealth_tick_delay(&options, idle_frames)) => {
                stealth_pre_write_delay(&options).await;
                if !pending.is_empty() {
                    let len = pending.len().min(capacity);
                    let payload: Vec<u8> = pending.drain(..len).collect();
                    frame_writer
                        .send(Frame {
                            ty: FrameType::Data,
                            payload,
                        })
                        .await?;
                    data_frames = data_frames.saturating_add(1);
                    bytes = bytes.saturating_add(len as u64);
                    idle_frames = 0;
                } else if app_closed {
                    frame_writer
                        .send(Frame {
                            ty: FrameType::Close,
                        payload: Vec::new(),
                    })
                    .await?;
                    debug!(
                        elapsed_ms = started.elapsed().as_millis(),
                        data_frames,
                        padding_frames,
                        bytes,
                        bps = throughput_bps(bytes, started.elapsed()),
                        "perf encrypted stealth upload pump completed"
                    );
                    return Ok(());
                } else {
                    frame_writer
                        .send(Frame {
                            ty: FrameType::Padding,
                        payload: Vec::new(),
                    })
                    .await?;
                    padding_frames = padding_frames.saturating_add(1);
                    idle_frames = idle_frames.saturating_add(1);
                }
            }
        }
    }
}

fn stealth_tick_delay(options: &FrameOptions, idle_frames: u64) -> Duration {
    let base = options.stealth_tick_ms.max(1);
    let multiplier = match idle_frames / STEALTH_IDLE_DECAY_FRAMES {
        0 => 1,
        1 => 4,
        2 => 10,
        _ => 20,
    };
    let target = base.saturating_mul(multiplier).min(1000);
    let lower = target.saturating_mul(3).saturating_div(4).max(1);
    let upper = target.saturating_mul(5).saturating_div(4).max(lower);
    Duration::from_millis(rand::thread_rng().gen_range(lower..=upper))
}

async fn stealth_pre_write_delay(options: &FrameOptions) {
    let upper = (options.stealth_tick_ms / 5).clamp(1, 10);
    let delay = rand::thread_rng().gen_range(0..=upper);
    if delay > 0 {
        sleep(Duration::from_millis(delay)).await;
    }
}

async fn download_frames<R, W>(
    net_reader: R,
    mut app_writer: W,
    keys: SessionKeys,
    options: FrameOptions,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut frame_reader = FrameReader::new(net_reader, keys, options);
    let started = Instant::now();
    let mut data_frames = 0_u64;
    let mut bytes = 0_u64;
    loop {
        match frame_reader.recv().await? {
            Frame {
                ty: FrameType::Data,
                payload,
            } => {
                data_frames = data_frames.saturating_add(1);
                bytes = bytes.saturating_add(payload.len() as u64);
                app_writer.write_all(&payload).await?
            }
            Frame {
                ty: FrameType::Close,
                ..
            } => {
                app_writer.shutdown().await?;
                debug!(
                    elapsed_ms = started.elapsed().as_millis(),
                    data_frames,
                    bytes,
                    bps = throughput_bps(bytes, started.elapsed()),
                    "perf encrypted download pump completed"
                );
                return Ok(());
            }
            _ => {}
        }
    }
}

fn throughput_bps(bytes: u64, elapsed: Duration) -> u64 {
    let nanos = elapsed.as_nanos();
    if nanos == 0 {
        return 0;
    }
    ((bytes as u128)
        .saturating_mul(8)
        .saturating_mul(1_000_000_000)
        / nanos) as u64
}

#[cfg(test)]
mod tests {
    use super::idle_copy_bidirectional;
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};
    use tokio::time::Duration;

    #[tokio::test]
    async fn idle_copy_bidirectional_exits_after_idle_timeout() {
        let (mut left, _left_peer) = duplex(64);
        let (mut right, _right_peer) = duplex(64);

        let copied = idle_copy_bidirectional(&mut left, &mut right, Duration::from_millis(5)).await;

        assert_eq!(copied.unwrap(), (0, 0));
    }

    #[tokio::test]
    async fn idle_copy_bidirectional_copies_one_direction_before_shutdown() {
        let (mut left, mut left_peer) = duplex(64);
        let (mut right, mut right_peer) = duplex(64);

        let task = tokio::spawn(async move {
            idle_copy_bidirectional(&mut left, &mut right, Duration::from_millis(100)).await
        });
        left_peer.write_all(b"ping").await.unwrap();
        left_peer.shutdown().await.unwrap();

        let mut received = [0_u8; 4];
        right_peer.read_exact(&mut received).await.unwrap();
        assert_eq!(&received, b"ping");

        let copied = task.await.unwrap().unwrap();
        assert_eq!(copied.0, 4);
    }

    // Isolates spawn_frame_transport (encrypted duplex + pumps) with NO mux on
    // top. Two endpoints bridged by an in-memory "TCP" duplex. If bytes corrupt
    // here, the bug is inside the encrypted pump itself.
    #[tokio::test]
    async fn spawn_frame_transport_preserves_bulk_bytes() {
        use super::spawn_frame_transport;
        use crate::crypto::{accept_handshake, connect_handshake, HandshakeConfig};
        use crate::protocol::framing::FrameOptions;

        let (mut net_a, mut net_b) = duplex(1024 * 1024);
        let cfg = HandshakeConfig::new(b"test-secret-that-is-long-enough".to_vec(), 30, 128, 4);
        let cfg_c = cfg.clone();
        let client_hs = tokio::spawn(async move {
            let keys = connect_handshake(&mut net_a, &cfg_c).await?;
            Ok::<_, anyhow::Error>((net_a, keys))
        });
        let cfg_s = cfg.clone();
        let server_hs = tokio::spawn(async move {
            let keys = accept_handshake(&mut net_b, &cfg_s).await?;
            Ok::<_, anyhow::Error>((net_b, keys))
        });
        let (net_a, keys_a) = client_hs.await.unwrap().unwrap();
        let (net_b, keys_b) = server_hs.await.unwrap().unwrap();

        let opts = FrameOptions {
            heartbeat_secs: 30,
            ..FrameOptions::default()
        };
        let mut app_a = spawn_frame_transport(net_a, keys_a, opts.clone(), 1024 * 1024);
        let mut app_b = spawn_frame_transport(net_b, keys_b, opts, 1024 * 1024);

        let total: usize = 1024 * 1024;
        let data: Vec<u8> = (0..total).map(|i| (i % 256) as u8).collect();
        let writer = tokio::spawn(async move {
            app_a.write_all(&data).await.unwrap();
            app_a.shutdown().await.unwrap();
        });
        let mut received = Vec::with_capacity(total);
        app_b.read_to_end(&mut received).await.unwrap();
        writer.await.unwrap();
        assert_eq!(
            received.len(),
            total,
            "received {} of {}",
            received.len(),
            total
        );
        for (i, b) in received.iter().enumerate() {
            assert_eq!(*b, (i % 256) as u8, "byte mismatch at offset {i}");
        }
    }

    // Reproduces the real production stack end-to-end:
    //   native mux <-> encrypted frame transport (duplex + pumps) <-> "TCP" (duplex)
    // If this desyncs while the pure-native-mux test passes, the bug is in the
    // composition of spawn_frame_transport with the mux session.
    #[tokio::test]
    async fn encrypted_transport_with_native_mux_preserves_bulk_integrity() {
        use super::spawn_frame_transport;
        use crate::crypto::{accept_handshake, connect_handshake, HandshakeConfig};
        use crate::mux::{client_session, server_session, MuxRuntimeConfig};
        use crate::protocol::framing::FrameOptions;
        use futures::StreamExt;

        let (mut net_a, mut net_b) = duplex(1024 * 1024);
        let cfg = HandshakeConfig::new(b"test-secret-that-is-long-enough".to_vec(), 30, 128, 4);

        let cfg_c = cfg.clone();
        let client_hs = tokio::spawn(async move {
            let keys = connect_handshake(&mut net_a, &cfg_c).await?;
            Ok::<_, anyhow::Error>((net_a, keys))
        });
        let cfg_s = cfg.clone();
        let server_hs = tokio::spawn(async move {
            let keys = accept_handshake(&mut net_b, &cfg_s).await?;
            Ok::<_, anyhow::Error>((net_b, keys))
        });
        let (net_a, keys_a) = client_hs.await.unwrap().unwrap();
        let (net_b, keys_b) = server_hs.await.unwrap().unwrap();

        let opts = FrameOptions {
            heartbeat_secs: 30,
            ..FrameOptions::default()
        };
        let transport_a = spawn_frame_transport(net_a, keys_a, opts.clone(), 1024 * 1024);
        let transport_b = spawn_frame_transport(net_b, keys_b, opts, 1024 * 1024);
        let mux = MuxRuntimeConfig {
            mode: crate::config::MuxMode::Native,
            max_streams: 256,
            native_initial_window_bytes: 1024 * 1024,
            native_stream_buffer_frames: 128,
            native_send_queue_frames: 64,
            native_idle_timeout: Duration::from_secs(300),
            native_drain_timeout: Duration::from_secs(30),
        };
        let (mut ctrl_a, mut session_a) = client_session(transport_a, mux);
        let mut session_b = server_session(transport_b, mux);

        tokio::spawn(async move { while session_a.next().await.is_some() {} });

        let mut client_stream = ctrl_a.open_stream().await.unwrap();
        let mut server_stream = session_b.next().await.unwrap().unwrap();
        tokio::spawn(async move { while session_b.next().await.is_some() {} });

        let total: usize = 1024 * 1024;
        let writer = tokio::spawn(async move {
            let mut sent = 0usize;
            let mut buf = vec![0u8; 8192];
            while sent < total {
                let take = buf.len().min(total - sent);
                for (j, slot) in buf.iter_mut().enumerate().take(take) {
                    *slot = (sent + j) as u8;
                }
                let mut written = 0;
                while written < take {
                    let n = server_stream.write(&buf[written..take]).await.unwrap();
                    assert!(n > 0, "server stream write returned 0");
                    written += n;
                }
                sent += take;
            }
            server_stream.shutdown().await.unwrap();
        });

        let mut received = Vec::with_capacity(total);
        client_stream.read_to_end(&mut received).await.unwrap();
        writer.await.unwrap();
        assert_eq!(received.len(), total, "client received wrong byte count");
        for (i, b) in received.iter().enumerate() {
            assert_eq!(*b, (i % 256) as u8, "byte mismatch at offset {i}");
        }
    }

    // Same full-stack shape as the native test above but exercising the yamux
    // path with the operator-tuned window (P1: config now maps to YamuxConfig).
    #[tokio::test]
    async fn encrypted_transport_with_yamux_mux_preserves_bulk_integrity() {
        use super::spawn_frame_transport;
        use crate::crypto::{accept_handshake, connect_handshake, HandshakeConfig};
        use crate::mux::{client_session, server_session, MuxRuntimeConfig};
        use crate::protocol::framing::FrameOptions;
        use futures::StreamExt;

        let (mut net_a, mut net_b) = duplex(1024 * 1024);
        let cfg = HandshakeConfig::new(b"test-secret-that-is-long-enough".to_vec(), 30, 128, 4);
        let cfg_c = cfg.clone();
        let client_hs = tokio::spawn(async move {
            let keys = connect_handshake(&mut net_a, &cfg_c).await?;
            Ok::<_, anyhow::Error>((net_a, keys))
        });
        let cfg_s = cfg.clone();
        let server_hs = tokio::spawn(async move {
            let keys = accept_handshake(&mut net_b, &cfg_s).await?;
            Ok::<_, anyhow::Error>((net_b, keys))
        });
        let (net_a, keys_a) = client_hs.await.unwrap().unwrap();
        let (net_b, keys_b) = server_hs.await.unwrap().unwrap();

        let opts = FrameOptions {
            heartbeat_secs: 30,
            ..FrameOptions::default()
        };
        let transport_a = spawn_frame_transport(net_a, keys_a, opts.clone(), 1024 * 1024);
        let transport_b = spawn_frame_transport(net_b, keys_b, opts, 1024 * 1024);
        let mux = MuxRuntimeConfig {
            mode: crate::config::MuxMode::Yamux,
            max_streams: 256,
            native_initial_window_bytes: 1024 * 1024,
            native_stream_buffer_frames: 128,
            native_send_queue_frames: 64,
            native_idle_timeout: Duration::from_secs(300),
            native_drain_timeout: Duration::from_secs(30),
        };
        let (mut ctrl_a, mut session_a) = client_session(transport_a, mux);
        let mut session_b = server_session(transport_b, mux);

        tokio::spawn(async move { while session_a.next().await.is_some() {} });

        let mut client_stream = ctrl_a.open_stream().await.unwrap();
        let mut server_stream = session_b.next().await.unwrap().unwrap();

        let total: usize = 1024 * 1024;
        let writer = tokio::spawn(async move {
            let mut sent = 0usize;
            let mut buf = vec![0u8; 8192];
            while sent < total {
                let take = buf.len().min(total - sent);
                for (j, slot) in buf.iter_mut().enumerate().take(take) {
                    *slot = (sent + j) as u8;
                }
                let mut written = 0;
                while written < take {
                    let n = server_stream.write(&buf[written..take]).await.unwrap();
                    assert!(n > 0, "server stream write returned 0");
                    written += n;
                }
                sent += take;
            }
            server_stream.shutdown().await.unwrap();
        });

        let mut received = Vec::with_capacity(total);
        client_stream.read_to_end(&mut received).await.unwrap();
        writer.await.unwrap();
        assert_eq!(received.len(), total, "client received wrong byte count");
        for (i, b) in received.iter().enumerate() {
            assert_eq!(*b, (i % 256) as u8, "byte mismatch at offset {i}");
        }
    }
}
