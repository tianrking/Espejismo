use anyhow::{bail, Result};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::ProxyAuth;

#[derive(Clone, Debug)]
pub struct SocksTarget {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug)]
pub enum SocksRequest {
    Connect(SocksTarget),
    UdpAssociate,
}

#[derive(Clone, Debug)]
pub struct UdpPacket {
    pub target: SocksTarget,
    pub payload: Vec<u8>,
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
    match accept_request_with_auth(stream, auth).await? {
        SocksRequest::Connect(target) => Ok(target),
        SocksRequest::UdpAssociate => bail!("SOCKS5 UDP ASSOCIATE is not a CONNECT request"),
    }
}

pub async fn accept_request_with_auth<S>(
    stream: &mut S,
    auth: Option<&ProxyAuth>,
) -> Result<SocksRequest>
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
    if ver != 5 {
        reply(stream, 0x07).await?;
        bail!("unsupported SOCKS request version {ver}");
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
    match cmd {
        1 => {
            reply(stream, 0x00).await?;
            Ok(SocksRequest::Connect(SocksTarget { host, port }))
        }
        3 => Ok(SocksRequest::UdpAssociate),
        _ => {
            reply(stream, 0x07).await?;
            bail!("unsupported SOCKS5 command {cmd}");
        }
    }
}

pub async fn reply_udp_associate<S>(stream: &mut S, bound: SocketAddr) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    match bound {
        SocketAddr::V4(addr) => {
            let mut reply = vec![0x05, 0x00, 0x00, 0x01];
            reply.extend_from_slice(&addr.ip().octets());
            reply.extend_from_slice(&addr.port().to_be_bytes());
            stream.write_all(&reply).await?;
        }
        SocketAddr::V6(addr) => {
            let mut reply = vec![0x05, 0x00, 0x00, 0x04];
            reply.extend_from_slice(&addr.ip().octets());
            reply.extend_from_slice(&addr.port().to_be_bytes());
            stream.write_all(&reply).await?;
        }
    }
    Ok(())
}

pub fn parse_udp_packet(input: &[u8]) -> Result<UdpPacket> {
    if input.len() < 4 {
        bail!("SOCKS UDP packet too short");
    }
    if input[0] != 0 || input[1] != 0 {
        bail!("SOCKS UDP reserved bytes are invalid");
    }
    if input[2] != 0 {
        bail!("SOCKS UDP fragmentation is not supported");
    }
    let atyp = input[3];
    let mut idx = 4;
    let host = match atyp {
        1 => {
            if input.len() < idx + 4 + 2 {
                bail!("SOCKS UDP IPv4 packet too short");
            }
            let ip = Ipv4Addr::new(input[idx], input[idx + 1], input[idx + 2], input[idx + 3]);
            idx += 4;
            ip.to_string()
        }
        3 => {
            if input.len() < idx + 1 {
                bail!("SOCKS UDP domain packet too short");
            }
            let len = input[idx] as usize;
            idx += 1;
            if input.len() < idx + len + 2 {
                bail!("SOCKS UDP domain packet too short");
            }
            let host = String::from_utf8(input[idx..idx + len].to_vec())?;
            idx += len;
            host
        }
        4 => {
            if input.len() < idx + 16 + 2 {
                bail!("SOCKS UDP IPv6 packet too short");
            }
            let mut octets = [0_u8; 16];
            octets.copy_from_slice(&input[idx..idx + 16]);
            idx += 16;
            Ipv6Addr::from(octets).to_string()
        }
        _ => bail!("unsupported SOCKS UDP address type {atyp}"),
    };
    let port = u16::from_be_bytes([input[idx], input[idx + 1]]);
    idx += 2;
    Ok(UdpPacket {
        target: SocksTarget { host, port },
        payload: input[idx..].to_vec(),
    })
}

pub fn build_udp_packet(target: &SocksTarget, payload: &[u8]) -> Result<Vec<u8>> {
    let mut output = vec![0x00, 0x00, 0x00];
    if let Ok(ip) = target.host.parse::<Ipv4Addr>() {
        output.push(0x01);
        output.extend_from_slice(&ip.octets());
    } else if let Ok(ip) = target.host.parse::<Ipv6Addr>() {
        output.push(0x04);
        output.extend_from_slice(&ip.octets());
    } else {
        let host = target.host.as_bytes();
        if host.len() > u8::MAX as usize {
            bail!("SOCKS UDP domain name too long");
        }
        output.push(0x03);
        output.push(host.len() as u8);
        output.extend_from_slice(host);
    }
    output.extend_from_slice(&target.port.to_be_bytes());
    output.extend_from_slice(payload);
    Ok(output)
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

#[cfg(test)]
mod tests {
    use super::{build_udp_packet, parse_udp_packet, SocksTarget};

    #[test]
    fn udp_packet_roundtrips_domain_target() {
        let target = SocksTarget {
            host: "example.com".to_string(),
            port: 443,
        };
        let encoded = build_udp_packet(&target, b"hello").unwrap();
        let decoded = parse_udp_packet(&encoded).unwrap();

        assert_eq!(decoded.target.host, "example.com");
        assert_eq!(decoded.target.port, 443);
        assert_eq!(decoded.payload, b"hello");
    }

    #[test]
    fn udp_packet_roundtrips_ipv6_target() {
        let target = SocksTarget {
            host: "2001:db8::1".to_string(),
            port: 53,
        };
        let encoded = build_udp_packet(&target, b"dns").unwrap();
        let decoded = parse_udp_packet(&encoded).unwrap();

        assert_eq!(decoded.target.host, "2001:db8::1");
        assert_eq!(decoded.target.port, 53);
        assert_eq!(decoded.payload, b"dns");
    }

    #[test]
    fn udp_packet_rejects_fragmentation() {
        let packet = [0x00, 0x00, 0x01, 0x01, 127, 0, 0, 1, 0, 53];
        assert!(parse_udp_packet(&packet).is_err());
    }

    #[test]
    fn udp_packet_rejects_truncated_domain() {
        let packet = [0x00, 0x00, 0x00, 0x03, 10, b'e', b'x'];
        assert!(parse_udp_packet(&packet).is_err());
    }
}
