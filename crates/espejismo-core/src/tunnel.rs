use tokio::io::{duplex, split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};
use tracing::debug;

use crate::crypto::SessionKeys;
use crate::framing::{Frame, FrameOptions, FrameReader, FrameType, FrameWriter};

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
    let mut frame_writer = FrameWriter::new(net_writer, keys, options);
    let mut buf = vec![0_u8; 16 * 1024];
    loop {
        let n = app_reader.read(&mut buf).await?;
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
