use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const CMD_TCP_CONNECT: u8 = 1;
pub const CMD_UDP_DATAGRAM: u8 = 2;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum StreamPriority {
    #[default]
    Interactive = 1,
    Bulk = 2,
}

impl TryFrom<u8> for StreamPriority {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Interactive),
            2 => Ok(Self::Bulk),
            _ => bail!("unsupported stream priority {value}"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TunnelRequest {
    TcpConnect {
        authority: String,
        priority: StreamPriority,
    },
    UdpDatagram {
        authority: String,
        priority: StreamPriority,
        payload: Vec<u8>,
    },
}

pub async fn write_tcp_connect<W>(writer: &mut W, authority: &str) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    write_tcp_connect_with_priority(writer, authority, StreamPriority::Interactive).await
}

pub async fn write_tcp_connect_with_priority<W>(
    writer: &mut W,
    authority: &str,
    priority: StreamPriority,
) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    writer.write_u8(CMD_TCP_CONNECT).await?;
    writer.write_u8(priority as u8).await?;
    write_authority(writer, authority).await?;
    Ok(())
}

pub async fn write_udp_datagram<W>(writer: &mut W, authority: &str, payload: &[u8]) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    write_udp_datagram_with_priority(writer, authority, StreamPriority::Interactive, payload).await
}

pub async fn write_udp_datagram_with_priority<W>(
    writer: &mut W,
    authority: &str,
    priority: StreamPriority,
    payload: &[u8],
) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    if payload.len() > u16::MAX as usize {
        bail!("UDP payload too large");
    }
    writer.write_u8(CMD_UDP_DATAGRAM).await?;
    writer.write_u8(priority as u8).await?;
    write_authority(writer, authority).await?;
    writer.write_u16(payload.len() as u16).await?;
    writer.write_all(payload).await?;
    Ok(())
}

pub async fn read_tunnel_request<R>(reader: &mut R) -> Result<TunnelRequest>
where
    R: AsyncReadExt + Unpin,
{
    let cmd = reader.read_u8().await?;
    let priority = StreamPriority::try_from(reader.read_u8().await?)?;
    let authority = read_authority(reader).await?;
    match cmd {
        CMD_TCP_CONNECT => Ok(TunnelRequest::TcpConnect {
            authority,
            priority,
        }),
        CMD_UDP_DATAGRAM => {
            let payload_len = reader.read_u16().await? as usize;
            let mut payload = vec![0_u8; payload_len];
            reader.read_exact(&mut payload).await?;
            Ok(TunnelRequest::UdpDatagram {
                authority,
                priority,
                payload,
            })
        }
        _ => bail!("unsupported tunnel request command {cmd}"),
    }
}

async fn write_authority<W>(writer: &mut W, authority: &str) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let bytes = authority.as_bytes();
    if bytes.len() > u16::MAX as usize {
        bail!("target authority too long");
    }
    writer.write_u16(bytes.len() as u16).await?;
    writer.write_all(bytes).await?;
    Ok(())
}

async fn read_authority<R>(reader: &mut R) -> Result<String>
where
    R: AsyncReadExt + Unpin,
{
    let len = reader.read_u16().await? as usize;
    if len == 0 {
        bail!("empty target authority");
    }
    let mut bytes = vec![0_u8; len];
    reader.read_exact(&mut bytes).await?;
    Ok(String::from_utf8(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::{
        read_tunnel_request, write_tcp_connect_with_priority, write_udp_datagram_with_priority,
        StreamPriority, TunnelRequest, CMD_TCP_CONNECT, CMD_UDP_DATAGRAM,
    };

    #[tokio::test]
    async fn tcp_connect_wire_format_matches_protocol_doc() {
        let mut wire = Vec::new();
        write_tcp_connect_with_priority(&mut wire, "example.com:443", StreamPriority::Bulk)
            .await
            .unwrap();

        assert_eq!(wire[0], CMD_TCP_CONNECT);
        assert_eq!(wire[1], StreamPriority::Bulk as u8);
        assert_eq!(u16::from_be_bytes([wire[2], wire[3]]), 15);

        let request = read_tunnel_request(&mut &wire[..]).await.unwrap();
        match request {
            TunnelRequest::TcpConnect {
                authority,
                priority,
            } => {
                assert_eq!(authority, "example.com:443");
                assert_eq!(priority, StreamPriority::Bulk);
            }
            _ => panic!("expected TCP connect request"),
        }
    }

    #[tokio::test]
    async fn udp_datagram_wire_format_matches_protocol_doc() {
        let mut wire = Vec::new();
        write_udp_datagram_with_priority(
            &mut wire,
            "dns.example:53",
            StreamPriority::Interactive,
            b"payload",
        )
        .await
        .unwrap();

        assert_eq!(wire[0], CMD_UDP_DATAGRAM);
        assert_eq!(wire[1], StreamPriority::Interactive as u8);
        assert_eq!(u16::from_be_bytes([wire[2], wire[3]]), 14);

        let request = read_tunnel_request(&mut &wire[..]).await.unwrap();
        match request {
            TunnelRequest::UdpDatagram {
                authority,
                priority,
                payload,
            } => {
                assert_eq!(authority, "dns.example:53");
                assert_eq!(priority, StreamPriority::Interactive);
                assert_eq!(payload, b"payload");
            }
            _ => panic!("expected UDP datagram request"),
        }
    }

    #[tokio::test]
    async fn udp_datagram_rejects_payload_larger_than_wire_length_limit() {
        let payload = vec![0_u8; u16::MAX as usize + 1];
        let mut wire = Vec::new();

        assert!(write_udp_datagram_with_priority(
            &mut wire,
            "example.com:53",
            StreamPriority::Interactive,
            &payload,
        )
        .await
        .is_err());
        assert!(wire.is_empty());
    }

    #[tokio::test]
    async fn maximum_udp_datagram_survives_fragmented_stream_io() {
        // A small duplex buffer forces the length-prefixed datagram across many
        // underlying reads and writes, as happens when a large request is split
        // into transport frames or TCP segments.
        let payload: Vec<u8> = (0..=u8::MAX).cycle().take(u16::MAX as usize).collect();
        let (mut tx, mut rx) = tokio::io::duplex(31);
        let expected = payload.clone();
        let writer = tokio::spawn(async move {
            write_udp_datagram_with_priority(
                &mut tx,
                "example.com:53",
                StreamPriority::Bulk,
                &payload,
            )
            .await
            .unwrap();
        });

        match read_tunnel_request(&mut rx).await.unwrap() {
            TunnelRequest::UdpDatagram {
                authority,
                priority,
                payload,
            } => {
                assert_eq!(authority, "example.com:53");
                assert_eq!(priority, StreamPriority::Bulk);
                assert_eq!(payload, expected);
            }
            _ => panic!("expected UDP datagram request"),
        }
        writer.await.unwrap();
    }
}
