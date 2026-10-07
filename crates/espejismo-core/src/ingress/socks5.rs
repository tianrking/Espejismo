use anyhow::{bail, Result};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::{Duration, Instant},
};
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

#[derive(Clone, Debug)]
pub struct SocksUdpReassembler {
    target: Option<SocksTarget>,
    payload: Vec<u8>,
    next_fragment: u8,
    expires_at: Option<Instant>,
    peer: Option<IpAddr>,
}

impl Default for SocksUdpReassembler {
    fn default() -> Self {
        Self {
            target: None,
            payload: Vec::new(),
            next_fragment: 1,
            expires_at: None,
            peer: None,
        }
    }
}

impl SocksUdpReassembler {
    /// Accept one SOCKS5 UDP datagram and return a packet only when complete.
    /// The queue is capped at the tunnel's 16-bit UDP payload limit and expires
    /// after the RFC 1928 minimum reassembly interval.
    pub fn push(&mut self, input: &[u8]) -> Result<Option<UdpPacket>> {
        self.push_for_peer(None, input)
    }

    pub fn push_from(&mut self, peer: SocketAddr, input: &[u8]) -> Result<Option<UdpPacket>> {
        self.push_for_peer(Some(peer.ip()), input)
    }

    fn push_for_peer(&mut self, peer: Option<IpAddr>, input: &[u8]) -> Result<Option<UdpPacket>> {
        let parsed = parse_udp_packet_inner(input)?;
        if parsed.frag == 0 {
            self.reset();
            return Ok(Some(parsed.packet));
        }

        let now = Instant::now();
        if self.expires_at.is_some_and(|deadline| now >= deadline) {
            self.reset();
        }
        let sequence = parsed.frag & 0x7f;
        let final_fragment = parsed.frag & 0x80 != 0;
        if sequence == 0 {
            self.reset();
            return Ok(None);
        }
        if sequence == 1 {
            self.reset();
            self.target = Some(parsed.packet.target.clone());
            self.expires_at = Some(now + Duration::from_secs(5));
            self.peer = peer;
        } else if self.target.is_none() {
            return Ok(None);
        }

        if sequence != self.next_fragment
            || self.peer != peer
            || self.target.as_ref().is_none_or(|target| {
                target.host != parsed.packet.target.host || target.port != parsed.packet.target.port
            })
            || self
                .payload
                .len()
                .saturating_add(parsed.packet.payload.len())
                > u16::MAX as usize
        {
            self.reset();
            return Ok(None);
        }
        self.payload.extend_from_slice(&parsed.packet.payload);
        if final_fragment {
            let packet = UdpPacket {
                target: self.target.take().expect("active fragment sequence"),
                payload: std::mem::take(&mut self.payload),
            };
            self.reset();
            return Ok(Some(packet));
        }
        self.next_fragment = self.next_fragment.saturating_add(1);
        if self.next_fragment > 0x7f {
            self.reset();
        }
        Ok(None)
    }

    fn reset(&mut self) {
        self.target = None;
        self.payload.clear();
        self.next_fragment = 1;
        self.expires_at = None;
        self.peer = None;
    }
}

struct ParsedSocksUdpPacket {
    frag: u8,
    packet: UdpPacket,
}

impl SocksTarget {
    pub fn authority(&self) -> String {
        if self.host.parse::<Ipv6Addr>().is_ok() {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
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
    let rsv = stream.read_u8().await?;
    let atyp = stream.read_u8().await?;
    if ver != 5 {
        reply(stream, 0x07).await?;
        bail!("unsupported SOCKS request version {ver}");
    }
    if rsv != 0 {
        reply(stream, 0x01).await?;
        bail!("SOCKS request reserved byte must be zero");
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
            let name = match String::from_utf8(name) {
                Ok(name) if !name.is_empty() && !name.as_bytes().contains(&0) => name,
                _ => {
                    reply(stream, 0x08).await?;
                    bail!("invalid SOCKS5 domain name");
                }
            };
            name
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
    let parsed = parse_udp_packet_inner(input)?;
    if parsed.frag != 0 {
        bail!("SOCKS UDP fragmentation is not supported by packet parser");
    }
    Ok(parsed.packet)
}

fn parse_udp_packet_inner(input: &[u8]) -> Result<ParsedSocksUdpPacket> {
    if input.len() < 4 {
        bail!("SOCKS UDP packet too short");
    }
    if input[0] != 0 || input[1] != 0 {
        bail!("SOCKS UDP reserved bytes are invalid");
    }
    let frag = input[2];
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
    Ok(ParsedSocksUdpPacket {
        frag,
        packet: UdpPacket {
            target: SocksTarget { host, port },
            payload: input[idx..].to_vec(),
        },
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
    if username_len == 0 {
        stream.write_all(&[0x01, 0x01]).await?;
        bail!("SOCKS username must not be empty");
    }
    let mut username = vec![0_u8; username_len];
    stream.read_exact(&mut username).await?;
    let password_len = stream.read_u8().await? as usize;
    if password_len == 0 {
        stream.write_all(&[0x01, 0x01]).await?;
        bail!("SOCKS password must not be empty");
    }
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
    use super::{
        accept_request_with_auth, build_udp_packet, parse_udp_packet, reply_udp_associate,
        SocksRequest, SocksTarget, SocksUdpReassembler,
    };
    use crate::ingress::ProxyAuth;

    fn auth() -> ProxyAuth {
        ProxyAuth {
            username: "user".into(),
            password: "pass".into(),
        }
    }

    async fn exchange(
        input: Vec<u8>,
        auth: Option<ProxyAuth>,
    ) -> (anyhow::Result<SocksRequest>, Vec<u8>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut client, mut server) = tokio::io::duplex(256);
        let server_task =
            tokio::spawn(async move { accept_request_with_auth(&mut server, auth.as_ref()).await });
        client.write_all(&input).await.unwrap();
        client.shutdown().await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        (server_task.await.unwrap(), response)
    }

    #[tokio::test]
    async fn no_auth_accepts_method_zero_and_connects() {
        let (result, response) =
            exchange(vec![5, 1, 0, 5, 1, 0, 1, 127, 0, 0, 1, 0, 80], None).await;
        assert!(
            matches!(result.unwrap(), SocksRequest::Connect(target) if target.authority() == "127.0.0.1:80")
        );
        assert_eq!(&response[..2], &[5, 0]);
        assert_eq!(&response[2..4], &[5, 0]);
    }

    #[tokio::test]
    async fn connect_preserves_domain_for_remote_resolution() {
        let domain = b"remote.test.invalid";
        let mut request = vec![5, 1, 0, 5, 1, 0, 3, domain.len() as u8];
        request.extend_from_slice(domain);
        request.extend_from_slice(&[0x01, 0xbb]);
        let (result, response) = exchange(request, None).await;
        assert!(matches!(result.unwrap(), SocksRequest::Connect(target)
            if target.host == "remote.test.invalid" && target.port == 443));
        assert_eq!(&response[2..4], &[5, 0]);
    }

    #[tokio::test]
    async fn connect_rejects_empty_non_utf8_and_nul_domain_names() {
        for domain in [vec![], vec![0xff], vec![b'a', 0, b'b']] {
            let mut request = vec![5, 1, 0, 5, 1, 0, 3, domain.len() as u8];
            request.extend_from_slice(&domain);
            request.extend_from_slice(&[0, 80]);
            let (result, response) = exchange(request, None).await;
            assert!(result.is_err());
            assert_eq!(&response[2..4], &[5, 8]);
        }
    }

    #[tokio::test]
    async fn concurrent_ingress_burst_keeps_socks_requests_isolated() {
        let burst = (1..=128).map(|port| {
            let request = vec![
                5,
                1,
                0,
                5,
                1,
                0,
                3,
                12,
                b'e',
                b'x',
                b'a',
                b'm',
                b'p',
                b'l',
                b'e',
                b'.',
                b't',
                b'e',
                b's',
                b't',
                (port >> 8) as u8,
                port as u8,
            ];
            exchange(request, None)
        });

        let results = futures::future::join_all(burst).await;
        assert_eq!(results.len(), 128);
        for (index, (result, response)) in results.into_iter().enumerate() {
            let port = index as u16 + 1;
            assert!(matches!(result.unwrap(), SocksRequest::Connect(target)
                if target.host == "example.test" && target.port == port));
            assert_eq!(&response[..4], [5, 0, 5, 0]);
        }
    }

    #[tokio::test]
    async fn udp_associate_accepts_unspecified_ipv4_client_endpoint() {
        let request = vec![5, 1, 0, 5, 3, 0, 1, 0, 0, 0, 0, 0, 0];
        let (result, response) = exchange(request, None).await;
        assert!(matches!(result.unwrap(), SocksRequest::UdpAssociate));
        assert_eq!(response, [5, 0]);
    }

    #[tokio::test]
    async fn udp_associate_reply_encodes_ipv4_and_ipv6_bound_endpoints() {
        use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
        use tokio::io::AsyncReadExt;

        for (addr, expected) in [
            (
                SocketAddr::from((Ipv4Addr::LOCALHOST, 1234)),
                vec![5, 0, 0, 1, 127, 0, 0, 1, 4, 210],
            ),
            (
                SocketAddr::from((Ipv6Addr::LOCALHOST, 53)),
                vec![
                    5, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 53,
                ],
            ),
        ] {
            let (mut client, mut server) = tokio::io::duplex(64);
            reply_udp_associate(&mut server, addr).await.unwrap();
            let mut actual = vec![0; expected.len()];
            client.read_exact(&mut actual).await.unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[tokio::test]
    async fn request_rejects_nonzero_reserved_byte() {
        let request = vec![5, 1, 0, 5, 3, 1, 1, 0, 0, 0, 0, 0, 0];
        let (result, response) = exchange(request, None).await;
        assert!(result.is_err());
        assert_eq!(&response[2..], &[5, 1, 0, 1, 0, 0, 0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn configured_auth_rejects_no_auth_only_offer() {
        let (result, response) = exchange(vec![5, 1, 0], Some(auth())).await;
        assert!(result.is_err());
        assert_eq!(response, [5, 0xff]);
    }

    #[tokio::test]
    async fn no_auth_rejects_password_only_offer() {
        let (result, response) = exchange(vec![5, 1, 2], None).await;
        assert!(result.is_err());
        assert_eq!(response, [5, 0xff]);
    }

    #[tokio::test]
    async fn password_auth_checks_credentials_and_nonempty_fields() {
        let valid = vec![
            5, 1, 2, 1, 4, b'u', b's', b'e', b'r', 4, b'p', b'a', b's', b's', 5, 1, 0, 1, 127, 0,
            0, 1, 0, 80,
        ];
        let (result, response) = exchange(valid, Some(auth())).await;
        assert!(matches!(result.unwrap(), SocksRequest::Connect(_)));
        assert_eq!(&response[..4], &[5, 2, 1, 0]);

        let bad = vec![
            5, 1, 2, 1, 4, b'u', b's', b'e', b'r', 4, b'n', b'o', b'p', b'e',
        ];
        let (result, response) = exchange(bad, Some(auth())).await;
        assert!(result.is_err());
        assert_eq!(response, [5, 2, 1, 1]);

        let empty_user = vec![5, 1, 2, 1, 0];
        let (result, response) = exchange(empty_user, Some(auth())).await;
        assert!(result.is_err());
        assert_eq!(response, [5, 2, 1, 1]);

        let empty_password = vec![5, 1, 2, 1, 1, b'u', 0];
        let (result, response) = exchange(empty_password, Some(auth())).await;
        assert!(result.is_err());
        assert_eq!(response, [5, 2, 1, 1]);
    }

    #[test]
    fn ipv6_target_authority_is_bracketed() {
        let target = SocksTarget {
            host: "2001:db8::1".to_string(),
            port: 443,
        };
        assert_eq!(target.authority(), "[2001:db8::1]:443");
        assert_eq!(
            crate::egress::split_authority(&target.authority()).unwrap(),
            ("2001:db8::1".to_string(), 443)
        );
    }

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
    fn udp_packet_parser_rejects_fragments_without_reassembler() {
        for frag in 1..=u8::MAX {
            let packet = [0x00, 0x00, frag, 0x01, 127, 0, 0, 1, 0, 53];
            assert!(parse_udp_packet(&packet).is_err(), "FRAG={frag:#04x}");
        }
        // Reject FRAG before parsing the address, even when the rest is absent.
        assert!(parse_udp_packet(&[0, 0, 0x80, 0x01]).is_err());
    }

    #[test]
    fn socks_udp_reassembler_joins_ordered_fragments_and_final_marker() {
        let mut reassembler = SocksUdpReassembler::default();
        let first = [0, 0, 1, 1, 127, 0, 0, 1, 0, 53, b'h', b'e'];
        let last = [0, 0, 0x82, 1, 127, 0, 0, 1, 0, 53, b'l', b'l', b'o'];
        assert!(reassembler.push(&first).unwrap().is_none());
        let packet = reassembler.push(&last).unwrap().unwrap();
        assert_eq!(packet.target.authority(), "127.0.0.1:53");
        assert_eq!(packet.payload, b"hello");
    }

    #[test]
    fn socks_udp_reassembler_discards_gaps_and_changed_targets() {
        let mut reassembler = SocksUdpReassembler::default();
        let first = [0, 0, 1, 1, 127, 0, 0, 1, 0, 53, b'a'];
        let gap = [0, 0, 0x83, 1, 127, 0, 0, 1, 0, 53, b'c'];
        let changed = [0, 0, 0x82, 1, 127, 0, 0, 2, 0, 53, b'b'];
        assert!(reassembler.push(&first).unwrap().is_none());
        assert!(reassembler.push(&gap).unwrap().is_none());
        assert!(reassembler.push(&changed).unwrap().is_none());
        assert!(reassembler.target.is_none());
        assert!(reassembler.payload.is_empty());
    }

    #[test]
    fn socks_udp_reassembler_does_not_mix_fragment_sources() {
        let mut reassembler = SocksUdpReassembler::default();
        let first = [0, 0, 1, 1, 127, 0, 0, 1, 0, 53, b'a'];
        let last = [0, 0, 0x82, 1, 127, 0, 0, 1, 0, 53, b'b'];
        let peer_a = "127.0.0.1:1000".parse().unwrap();
        let peer_b = "127.0.0.2:1001".parse().unwrap();
        assert!(reassembler.push_from(peer_a, &first).unwrap().is_none());
        assert!(reassembler.push_from(peer_b, &last).unwrap().is_none());
        assert!(reassembler.target.is_none());
    }

    #[test]
    fn socks_udp_reassembler_drops_sequences_over_wire_payload_limit() {
        let mut reassembler = SocksUdpReassembler::default();
        let mut first = vec![0, 0, 1, 1, 127, 0, 0, 1, 0, 53];
        first.extend(std::iter::repeat_n(b'a', u16::MAX as usize));
        let last = [0, 0, 0x82, 1, 127, 0, 0, 1, 0, 53, b'b'];
        assert!(reassembler.push(&first).unwrap().is_none());
        assert!(reassembler.push(&last).unwrap().is_none());
        assert!(reassembler.target.is_none());
    }

    #[test]
    fn udp_packet_checks_address_header_boundaries() {
        let valid_ipv4_header = [0, 0, 0, 1, 127, 0, 0, 1, 0, 53];
        let valid_ipv6_header = [
            0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 53,
        ];
        let valid_domain_header = [0, 0, 0, 3, 1, b'a', 0, 53];

        for (name, packet) in [
            ("ipv4", valid_ipv4_header.as_slice()),
            ("ipv6", valid_ipv6_header.as_slice()),
            ("domain", valid_domain_header.as_slice()),
        ] {
            for end in 0..packet.len() {
                assert!(
                    parse_udp_packet(&packet[..end]).is_err(),
                    "{name} len={end}"
                );
            }
            let parsed = parse_udp_packet(packet).unwrap();
            assert!(parsed.payload.is_empty(), "{name} empty payload");
        }
    }

    #[test]
    fn udp_packet_rejects_truncated_domain() {
        let packet = [0x00, 0x00, 0x00, 0x03, 10, b'e', b'x'];
        assert!(parse_udp_packet(&packet).is_err());
    }

    #[test]
    fn udp_parser_never_panics_on_bounded_random_inputs() {
        use rand::{Rng, SeedableRng};

        let mut rng = rand::rngs::StdRng::seed_from_u64(0x534f_434b_5355_4450);
        for len in 0..=1024 {
            let mut bytes = vec![0; len];
            rng.fill(bytes.as_mut_slice());
            assert!(std::panic::catch_unwind(|| parse_udp_packet(&bytes)).is_ok());
        }

        for atyp in [1, 3, 4] {
            for len in 0..=24 {
                let mut packet = vec![0, 0, 0, atyp];
                packet.resize(len, 0);
                assert!(std::panic::catch_unwind(|| parse_udp_packet(&packet)).is_ok());
            }
        }
    }
}
