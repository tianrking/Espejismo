use anyhow::{bail, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Clone, Debug)]
pub struct SocksTarget {
    pub host: String,
    pub port: u16,
}

impl SocksTarget {
    pub fn authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

pub async fn accept_connect<S>(stream: &mut S) -> Result<SocksTarget>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let ver = stream.read_u8().await?;
    if ver != 5 {
        bail!("unsupported SOCKS version {ver}");
    }
    let methods_len = stream.read_u8().await? as usize;
    let mut methods = vec![0_u8; methods_len];
    stream.read_exact(&mut methods).await?;
    if !methods.contains(&0x00) {
        stream.write_all(&[0x05, 0xff]).await?;
        bail!("SOCKS client did not offer no-auth method");
    }
    stream.write_all(&[0x05, 0x00]).await?;

    let ver = stream.read_u8().await?;
    let cmd = stream.read_u8().await?;
    let _rsv = stream.read_u8().await?;
    let atyp = stream.read_u8().await?;
    if ver != 5 || cmd != 1 {
        reply(stream, 0x07).await?;
        bail!("only SOCKS5 CONNECT is supported");
    }

    let host = match atyp {
        1 => {
            let mut ip = [0_u8; 4];
            stream.read_exact(&mut ip).await?;
            std::net::Ipv4Addr::from(ip).to_string()
        }
        3 => {
            let len = stream.read_u8().await? as usize;
            let mut name = vec![0_u8; len];
            stream.read_exact(&mut name).await?;
            String::from_utf8(name)?
        }
        4 => {
            let mut ip = [0_u8; 16];
            stream.read_exact(&mut ip).await?;
            std::net::Ipv6Addr::from(ip).to_string()
        }
        _ => {
            reply(stream, 0x08).await?;
            bail!("unsupported address type {atyp}");
        }
    };
    let port = stream.read_u16().await?;
    reply(stream, 0x00).await?;
    Ok(SocksTarget { host, port })
}

async fn reply<S>(stream: &mut S, code: u8) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    stream
        .write_all(&[0x05, code, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;
    Ok(())
}
