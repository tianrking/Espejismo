use anyhow::{bail, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::ProxyAuth;

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
    accept_connect_with_auth(stream, None).await
}

pub async fn accept_connect_with_auth<S>(
    stream: &mut S,
    auth: Option<&ProxyAuth>,
) -> Result<SocksTarget>
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
    negotiate_auth(stream, &methods, auth).await?;

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

async fn negotiate_auth<S>(stream: &mut S, methods: &[u8], auth: Option<&ProxyAuth>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    match auth {
        Some(auth) => {
            if !methods.contains(&0x02) {
                stream.write_all(&[0x05, 0xff]).await?;
                bail!("SOCKS client did not offer username/password auth");
            }
            stream.write_all(&[0x05, 0x02]).await?;
            verify_password_auth(stream, auth).await
        }
        None => {
            if !methods.contains(&0x00) {
                stream.write_all(&[0x05, 0xff]).await?;
                bail!("SOCKS client did not offer no-auth method");
            }
            stream.write_all(&[0x05, 0x00]).await?;
            Ok(())
        }
    }
}

async fn verify_password_auth<S>(stream: &mut S, auth: &ProxyAuth) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let ver = stream.read_u8().await?;
    if ver != 1 {
        stream.write_all(&[0x01, 0x01]).await?;
        bail!("unsupported SOCKS username/password auth version {ver}");
    }
    let username_len = stream.read_u8().await? as usize;
    let mut username = vec![0_u8; username_len];
    stream.read_exact(&mut username).await?;
    let password_len = stream.read_u8().await? as usize;
    let mut password = vec![0_u8; password_len];
    stream.read_exact(&mut password).await?;
    if !auth.matches(&username, &password) {
        stream.write_all(&[0x01, 0x01]).await?;
        bail!("SOCKS username/password auth failed");
    }
    stream.write_all(&[0x01, 0x00]).await?;
    Ok(())
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
