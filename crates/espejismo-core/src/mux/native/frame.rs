use std::io;

use anyhow::{bail, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub(super) const FRAME_OPEN: u8 = 1;
pub(super) const FRAME_DATA: u8 = 2;
pub(super) const FRAME_WINDOW_UPDATE: u8 = 3;
pub(super) const FRAME_FIN: u8 = 4;
pub(super) const FRAME_RST: u8 = 5;
pub(super) const FRAME_PING: u8 = 6;
pub(super) const FRAME_GOAWAY: u8 = 7;
pub(super) const MAX_PAYLOAD: usize = 64 * 1024;

pub(super) async fn write_frame<W>(
    writer: &mut W,
    kind: u8,
    stream_id: u32,
    payload: &[u8],
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    if payload.len() > MAX_PAYLOAD {
        bail!("native mux payload too large");
    }
    writer.write_u8(kind).await?;
    writer.write_u32(stream_id).await?;
    writer.write_u32(payload.len() as u32).await?;
    writer.write_all(payload).await?;
    Ok(())
}

pub(super) async fn read_frame<R>(reader: &mut R) -> Result<Option<(u8, u32, Vec<u8>)>>
where
    R: AsyncRead + Unpin,
{
    let kind = match reader.read_u8().await {
        Ok(kind) => kind,
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let stream_id = reader.read_u32().await?;
    let len = reader.read_u32().await? as usize;
    if len > MAX_PAYLOAD {
        bail!("native mux payload too large");
    }
    let mut payload = vec![0_u8; len];
    reader.read_exact(&mut payload).await?;
    Ok(Some((kind, stream_id, payload)))
}

pub fn validate_frame_bytes_for_fuzz(input: &[u8]) -> Result<Option<(u8, u32, usize)>> {
    if input.len() < 9 {
        return Ok(None);
    }
    let kind = input[0];
    match kind {
        FRAME_OPEN | FRAME_DATA | FRAME_WINDOW_UPDATE | FRAME_FIN | FRAME_RST | FRAME_PING
        | FRAME_GOAWAY => {}
        _ => bail!("unknown native mux frame type {kind}"),
    }
    let stream_id = u32::from_be_bytes([input[1], input[2], input[3], input[4]]);
    let len = u32::from_be_bytes([input[5], input[6], input[7], input[8]]) as usize;
    if len > MAX_PAYLOAD {
        bail!("native mux payload too large");
    }
    if input.len() < 9 + len {
        return Ok(None);
    }
    Ok(Some((kind, stream_id, len)))
}
