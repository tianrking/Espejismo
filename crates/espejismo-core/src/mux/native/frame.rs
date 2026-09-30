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
pub(super) const MAX_PAYLOAD: usize = 256 * 1024;

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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use tokio::io::duplex;

    #[tokio::test]
    async fn random_frames_roundtrip_through_async_codec() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0x4652_414d_4553);
        let kinds = [
            FRAME_OPEN,
            FRAME_DATA,
            FRAME_WINDOW_UPDATE,
            FRAME_FIN,
            FRAME_RST,
            FRAME_PING,
            FRAME_GOAWAY,
        ];

        for _ in 0..256 {
            let kind = kinds[rng.gen_range(0..kinds.len())];
            let stream_id = rng.gen::<u32>();
            let len = rng.gen_range(0..=4096);
            let mut payload = vec![0; len];
            rng.fill(payload.as_mut_slice());

            let (mut tx, mut rx) = duplex(len + 9);
            write_frame(&mut tx, kind, stream_id, &payload).await.unwrap();
            drop(tx);
            assert_eq!(
                read_frame(&mut rx).await.unwrap(),
                Some((kind, stream_id, payload))
            );
            assert_eq!(read_frame(&mut rx).await.unwrap(), None);
        }
    }

    #[test]
    fn random_byte_sequences_never_panic_and_invalid_complete_frames_error() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0x494e_5055_5453);

        for len in 0..=1024 {
            let mut bytes = vec![0; len];
            rng.fill(bytes.as_mut_slice());
            assert!(std::panic::catch_unwind(|| validate_frame_bytes_for_fuzz(&bytes)).is_ok());
        }

        for kind in 0..=u8::MAX {
            if matches!(
                kind,
                FRAME_OPEN
                    | FRAME_DATA
                    | FRAME_WINDOW_UPDATE
                    | FRAME_FIN
                    | FRAME_RST
                    | FRAME_PING
                    | FRAME_GOAWAY
            ) {
                continue;
            }
            let mut bytes = vec![kind, 0, 0, 0, 0];
            bytes.extend_from_slice(&0_u32.to_be_bytes());
            assert!(validate_frame_bytes_for_fuzz(&bytes).is_err());
        }

        let mut oversized = vec![FRAME_DATA, 0, 0, 0, 1];
        oversized.extend_from_slice(&((MAX_PAYLOAD as u32) + 1).to_be_bytes());
        assert!(validate_frame_bytes_for_fuzz(&oversized).is_err());
    }
}
