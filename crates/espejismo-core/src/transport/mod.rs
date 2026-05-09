use std::time::Duration;

use tokio::io::{duplex, split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};
use tokio::time::timeout;
use tracing::debug;

use crate::crypto::SessionKeys;
use crate::protocol::framing::{Frame, FrameOptions, FrameReader, FrameType, FrameWriter};

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
        if let Err(err) = download_frames(net_reader, app_writer, keys).await {
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
    let mut buf_a = [0u8; 8192];
    let mut buf_b = [0u8; 8192];
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
                    total_a += n as u64;
                    b.write_all(&buf_a[..n]).await?;
                }
            },
            (None, Some(rb)) => match rb.await {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                Ok(Ok(n)) => {
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
    let mut buf = vec![0_u8; options.normalized_chunk_bounds().1];
    let mut frame_writer = FrameWriter::new(net_writer, keys, options);
    loop {
        let chunk_size = frame_writer.options().next_chunk_size();
        let n = app_reader.read(&mut buf[..chunk_size]).await?;
        if n == 0 {
            frame_writer
                .send(Frame {
                    ty: FrameType::Close,
                    payload: Vec::new(),
                })
                .await?;
            return Ok(());
        }
        frame_writer
            .send(Frame {
                ty: FrameType::Data,
                payload: buf[..n].to_vec(),
            })
            .await?;
    }
}

async fn download_frames<R, W>(
    net_reader: R,
    mut app_writer: W,
    keys: SessionKeys,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut frame_reader = FrameReader::new(net_reader, keys);
    loop {
        match frame_reader.recv().await? {
            Frame {
                ty: FrameType::Data,
                payload,
            } => app_writer.write_all(&payload).await?,
            Frame {
                ty: FrameType::Close,
                ..
            } => {
                app_writer.shutdown().await?;
                return Ok(());
            }
            _ => {}
        }
    }
}
