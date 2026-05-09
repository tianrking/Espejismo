use anyhow::{bail, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const CMD_TCP_CONNECT: u8 = 1;
pub const CMD_UDP_DATAGRAM: u8 = 2;

#[derive(Clone, Debug)]
pub enum TunnelRequest {
    TcpConnect { authority: String },
    UdpDatagram { authority: String, payload: Vec<u8> },
}

pub async fn write_tcp_connect<W>(writer: &mut W, authority: &str) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    writer.write_u8(CMD_TCP_CONNECT).await?;
    write_authority(writer, authority).await
}

pub async fn write_udp_datagram<W>(writer: &mut W, authority: &str, payload: &[u8]) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    writer.write_u8(CMD_UDP_DATAGRAM).await?;
    write_authority(writer, authority).await?;
    if payload.len() > u16::MAX as usize {
        bail!("UDP payload too large");
    }
    writer.write_u16(payload.len() as u16).await?;
    writer.write_all(payload).await?;
    Ok(())
}

pub async fn read_tunnel_request<R>(reader: &mut R) -> Result<TunnelRequest>
where
    R: AsyncReadExt + Unpin,
{
    let cmd = reader.read_u8().await?;
    let authority = read_authority(reader).await?;
    match cmd {
        CMD_TCP_CONNECT => Ok(TunnelRequest::TcpConnect { authority }),
        CMD_UDP_DATAGRAM => {
            let payload_len = reader.read_u16().await? as usize;
            let mut payload = vec![0_u8; payload_len];
            reader.read_exact(&mut payload).await?;
            Ok(TunnelRequest::UdpDatagram { authority, payload })
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
