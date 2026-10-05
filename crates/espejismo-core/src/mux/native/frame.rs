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
    use proptest::prelude::*;
    use tokio::io::duplex;

    const KINDS: &[u8] = &[
        FRAME_OPEN,
        FRAME_DATA,
        FRAME_WINDOW_UPDATE,
        FRAME_FIN,
        FRAME_RST,
        FRAME_PING,
        FRAME_GOAWAY,
    ];

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn valid_frames_roundtrip(kind_index in 0usize..KINDS.len(), stream_id in any::<u32>(), payload in prop::collection::vec(any::<u8>(), 0..4097)) {
            let kind = KINDS[kind_index];
            let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
            runtime.block_on(async {
                let (mut tx, mut rx) = duplex(payload.len() + 9);
                write_frame(&mut tx, kind, stream_id, &payload).await.unwrap();
                drop(tx);
                prop_assert_eq!(read_frame(&mut rx).await.unwrap(), Some((kind, stream_id, payload.clone())));
                prop_assert_eq!(read_frame(&mut rx).await.unwrap(), None);
                Ok(())
            })?;
        }

        #[test]
        fn arbitrary_bytes_are_panic_free(bytes in prop::collection::vec(any::<u8>(), 0..(MAX_PAYLOAD + 16))) {
            let result = std::panic::catch_unwind(|| validate_frame_bytes_for_fuzz(&bytes));
            prop_assert!(result.is_ok());
        }

        #[test]
        fn complete_valid_headers_report_exact_payload(kind_index in 0usize..KINDS.len(), stream_id in any::<u32>(), payload in prop::collection::vec(any::<u8>(), 0..4097)) {
            let kind = KINDS[kind_index];
            let mut bytes = vec![kind];
            bytes.extend_from_slice(&stream_id.to_be_bytes());
            bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            bytes.extend_from_slice(&payload);
            prop_assert_eq!(validate_frame_bytes_for_fuzz(&bytes).unwrap(), Some((kind, stream_id, payload.len())));
        }
    }

    #[test]
    fn malformed_headers_reject_unknown_kind_and_oversized_payload() {
        let mut unknown_kind = vec![0, 0, 0, 0, 1];
        unknown_kind.extend_from_slice(&0_u32.to_be_bytes());
        assert!(validate_frame_bytes_for_fuzz(&unknown_kind).is_err());

        let mut oversized = vec![FRAME_DATA, 0, 0, 0, 1];
        oversized.extend_from_slice(&((MAX_PAYLOAD as u32) + 1).to_be_bytes());
        assert!(validate_frame_bytes_for_fuzz(&oversized).is_err());
    }

    #[test]
    fn complete_frame_validation_is_stable_under_truncation_and_trailing_bytes() {
        let kind = FRAME_DATA;
        let stream_id = 0x1234_5678_u32;
        let payload = b"frame payload";
        let mut frame = vec![kind];
        frame.extend_from_slice(&stream_id.to_be_bytes());
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(payload);

        assert_eq!(
            validate_frame_bytes_for_fuzz(&frame).unwrap(),
            Some((kind, stream_id, payload.len()))
        );
        for end in 0..frame.len() {
            assert_eq!(validate_frame_bytes_for_fuzz(&frame[..end]).unwrap(), None);
        }

        frame.extend_from_slice(&[0xaa, 0x55]);
        assert_eq!(
            validate_frame_bytes_for_fuzz(&frame).unwrap(),
            Some((kind, stream_id, payload.len()))
        );
    }
}
