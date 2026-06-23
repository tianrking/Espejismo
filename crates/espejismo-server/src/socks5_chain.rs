use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::{Context, Result};
use espejismo_core::{EgressProxy, EgressProxyKind, TransportStream};
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;

pub(crate) async fn connect_via_socks4_proxy(
    proxy: &EgressProxy,
    authority: &str,
) -> Result<Box<dyn TransportStream>> {
    let (host, port) = espejismo_core::split_authority(authority)?;
    let mut stream = TcpStream::connect(&proxy.endpoint)
        .await
        .with_context(|| format!("connect SOCKS4 proxy {}", proxy.endpoint))?;
    write_socks4_connect(&mut stream, proxy, &host, port).await?;
    read_socks4_connect_reply(&mut stream).await?;
    Ok(Box::new(stream))
}

pub(crate) async fn connect_via_socks5_proxy(
    proxy: &EgressProxy,
    authority: &str,
) -> Result<Box<dyn TransportStream>> {
    let (host, port) = espejismo_core::split_authority(authority)?;
    let mut stream = TcpStream::connect(&proxy.endpoint)
        .await
        .with_context(|| format!("connect SOCKS5 proxy {}", proxy.endpoint))?;
    negotiate(&mut stream, proxy).await?;
    write_socks5_connect(&mut stream, &host, port).await?;
    read_socks5_connect_reply(&mut stream).await?;
    Ok(Box::new(stream))
}

pub(crate) async fn relay_udp_via_socks5_proxy(
    proxy: &EgressProxy,
    authority: &str,
    payload: &[u8],
    idle: Duration,
) -> Result<Vec<u8>> {
    let (host, port) = espejismo_core::split_authority(authority)?;
    let mut control = TcpStream::connect(&proxy.endpoint)
        .await
        .with_context(|| format!("connect SOCKS5 proxy {}", proxy.endpoint))?;
    negotiate(&mut control, proxy).await?;

    control
        .write_all(&[0x05, 0x03, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;
    let relay = read_socks5_reply_addr(&mut control).await?;
    let bind = if relay.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind).await?;
    socket.connect(relay).await?;

    let request = encode_socks5_udp_datagram(&host, port, payload)?;
    socket.send(&request).await?;
    let mut response = vec![0_u8; 65_535];
    let n = timeout(
        idle.min(Duration::from_secs(10)),
        socket.recv(&mut response),
    )
    .await??;
    decode_socks5_udp_datagram(&response[..n])
}

async fn negotiate(stream: &mut TcpStream, proxy: &EgressProxy) -> Result<()> {
    if proxy.username.is_some() {
        stream.write_all(&[0x05, 0x02, 0x00, 0x02]).await?;
    } else {
        stream.write_all(&[0x05, 0x01, 0x00]).await?;
    }
    let mut method = [0_u8; 2];
    stream.read_exact(&mut method).await?;
    anyhow::ensure!(method[0] == 0x05, "SOCKS5 proxy returned invalid version");
    match method[1] {
        0x00 => Ok(()),
        0x02 => authenticate_username_password(stream, proxy).await,
        0xff => anyhow::bail!("SOCKS5 proxy rejected offered auth methods"),
        method => anyhow::bail!("SOCKS5 proxy selected unsupported auth method {method}"),
    }
}

async fn authenticate_username_password(stream: &mut TcpStream, proxy: &EgressProxy) -> Result<()> {
    let username = proxy
        .username
        .as_deref()
        .context("SOCKS5 proxy requested username/password but username is missing")?;
    let password = proxy.password.as_deref().unwrap_or("");
    anyhow::ensure!(
        username.len() <= u8::MAX as usize,
        "SOCKS5 proxy username too long"
    );
    anyhow::ensure!(
        password.len() <= u8::MAX as usize,
        "SOCKS5 proxy password too long"
    );
    let mut request = vec![0x01, username.len() as u8];
    request.extend_from_slice(username.as_bytes());
    request.push(password.len() as u8);
    request.extend_from_slice(password.as_bytes());
    stream.write_all(&request).await?;
    let mut response = [0_u8; 2];
    stream.read_exact(&mut response).await?;
    anyhow::ensure!(
        response == [0x01, 0x00],
        "SOCKS5 proxy username/password authentication failed"
    );
    Ok(())
}

async fn write_socks4_connect<W>(
    stream: &mut W,
    proxy: &EgressProxy,
    host: &str,
    port: u16,
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let user = proxy.username.as_deref().unwrap_or("");
    anyhow::ensure!(
        !user.as_bytes().contains(&0),
        "SOCKS4 proxy username must not contain NUL"
    );
    let mut request = vec![0x04, 0x01];
    request.extend_from_slice(&port.to_be_bytes());
    match proxy.kind {
        EgressProxyKind::Socks4 => {
            let ip = host.parse::<Ipv4Addr>().context(
                "SOCKS4 proxy requires an IPv4 literal target; use socks4a:// for domain targets",
            )?;
            request.extend_from_slice(&ip.octets());
            request.extend_from_slice(user.as_bytes());
            request.push(0);
        }
        EgressProxyKind::Socks4a => {
            anyhow::ensure!(
                !host.as_bytes().contains(&0),
                "SOCKS4a target host must not contain NUL"
            );
            request.extend_from_slice(&[0, 0, 0, 1]);
            request.extend_from_slice(user.as_bytes());
            request.push(0);
            request.extend_from_slice(host.as_bytes());
            request.push(0);
        }
        _ => anyhow::bail!("invalid proxy kind for SOCKS4 CONNECT"),
    }
    stream.write_all(&request).await?;
    Ok(())
}

async fn read_socks4_connect_reply(stream: &mut TcpStream) -> Result<()> {
    let mut reply = [0_u8; 8];
    stream.read_exact(&mut reply).await?;
    anyhow::ensure!(
        reply[0] == 0x00 && reply[1] == 0x5a,
        "SOCKS4 proxy CONNECT failed with status {}",
        reply[1]
    );
    Ok(())
}

async fn write_socks5_connect(stream: &mut TcpStream, host: &str, port: u16) -> Result<()> {
    let host_bytes = host.as_bytes();
    anyhow::ensure!(
        host_bytes.len() <= u8::MAX as usize,
        "SOCKS5 proxy target host too long"
    );
    let mut request = vec![0x05, 0x01, 0x00, 0x03, host_bytes.len() as u8];
    request.extend_from_slice(host_bytes);
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).await?;
    Ok(())
}

async fn read_socks5_connect_reply(stream: &mut TcpStream) -> Result<()> {
    let mut head = [0_u8; 4];
    stream.read_exact(&mut head).await?;
    anyhow::ensure!(
        head[0] == 0x05 && head[1] == 0x00,
        "SOCKS5 proxy CONNECT failed"
    );
    match head[3] {
        0x01 => {
            let mut skip = [0_u8; 6];
            stream.read_exact(&mut skip).await?;
        }
        0x03 => {
            let len = stream.read_u8().await? as usize;
            let mut skip = vec![0_u8; len + 2];
            stream.read_exact(&mut skip).await?;
        }
        0x04 => {
            let mut skip = [0_u8; 18];
            stream.read_exact(&mut skip).await?;
        }
        atyp => anyhow::bail!("SOCKS5 proxy returned unsupported address type {atyp}"),
    }
    Ok(())
}

async fn read_socks5_reply_addr(stream: &mut TcpStream) -> Result<SocketAddr> {
    let mut head = [0_u8; 4];
    stream.read_exact(&mut head).await?;
    anyhow::ensure!(
        head[0] == 0x05 && head[1] == 0x00,
        "SOCKS5 UDP ASSOCIATE failed"
    );
    let mut host = match head[3] {
        0x01 => {
            let mut ip = [0_u8; 4];
            stream.read_exact(&mut ip).await?;
            std::net::IpAddr::from(ip)
        }
        0x04 => {
            let mut ip = [0_u8; 16];
            stream.read_exact(&mut ip).await?;
            std::net::IpAddr::from(ip)
        }
        atyp => anyhow::bail!("SOCKS5 proxy returned unsupported UDP relay address type {atyp}"),
    };
    let port = stream.read_u16().await?;
    if host.is_unspecified() {
        host = stream.peer_addr()?.ip();
    }
    Ok(SocketAddr::new(host, port))
}

pub(crate) fn encode_socks5_udp_datagram(host: &str, port: u16, payload: &[u8]) -> Result<Vec<u8>> {
    let host_bytes = host.as_bytes();
    anyhow::ensure!(
        host_bytes.len() <= u8::MAX as usize,
        "SOCKS5 UDP target host too long"
    );
    let mut out = Vec::with_capacity(6 + host_bytes.len() + payload.len());
    out.extend_from_slice(&[0x00, 0x00, 0x00, 0x03, host_bytes.len() as u8]);
    out.extend_from_slice(host_bytes);
    out.extend_from_slice(&port.to_be_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

pub(crate) fn decode_socks5_udp_datagram(input: &[u8]) -> Result<Vec<u8>> {
    anyhow::ensure!(input.len() >= 6, "SOCKS5 UDP response too short");
    anyhow::ensure!(
        input[0] == 0 && input[1] == 0 && input[2] == 0,
        "SOCKS5 UDP response has unsupported fragmentation"
    );
    let mut offset = 4;
    match input[3] {
        0x01 => offset += 4,
        0x03 => {
            anyhow::ensure!(input.len() > offset, "SOCKS5 UDP domain length missing");
            offset += 1 + input[offset] as usize;
        }
        0x04 => offset += 16,
        atyp => anyhow::bail!("SOCKS5 UDP response has unsupported address type {atyp}"),
    }
    offset += 2;
    anyhow::ensure!(input.len() >= offset, "SOCKS5 UDP response truncated");
    Ok(input[offset..].to_vec())
}

#[cfg(test)]
mod tests {
    use espejismo_core::{EgressProxy, EgressProxyKind};
    use tokio::io::AsyncReadExt;

    use super::{decode_socks5_udp_datagram, encode_socks5_udp_datagram, write_socks4_connect};

    #[test]
    fn socks5_udp_datagram_codec_roundtrips_payload() {
        let encoded = encode_socks5_udp_datagram("example.com", 443, b"payload").unwrap();
        assert_eq!(&encoded[..5], &[0, 0, 0, 3, 11]);
        assert_eq!(decode_socks5_udp_datagram(&encoded).unwrap(), b"payload");
    }

    #[test]
    fn socks5_udp_datagram_rejects_fragmented_packets() {
        let mut encoded = encode_socks5_udp_datagram("example.com", 443, b"payload").unwrap();
        encoded[2] = 1;
        assert!(decode_socks5_udp_datagram(&encoded).is_err());
    }

    #[tokio::test]
    async fn socks4a_connect_request_encodes_domain_target() {
        let proxy = EgressProxy {
            kind: EgressProxyKind::Socks4a,
            endpoint: "127.0.0.1:1080".to_string(),
            username: Some("user".to_string()),
            password: None,
        };
        let (mut client, mut server) = tokio::io::duplex(128);
        let write = write_socks4_connect(&mut client, &proxy, "example.com", 443);
        let read = async {
            let mut buf = vec![0_u8; 32];
            let n = server.read(&mut buf).await.unwrap();
            buf.truncate(n);
            buf
        };
        let (_, buf) = tokio::join!(write, read);
        assert_eq!(
            buf,
            b"\x04\x01\x01\xbb\x00\x00\x00\x01user\x00example.com\x00"
        );
    }
}
