//! Parsers for the HAProxy PROXY protocol preamble used by trusted front proxies.
//! The parser is deliberately independent of sockets so malformed and partial
//! headers can be exercised without network permissions.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProxyHeader {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub consumed: usize,
}

pub(crate) fn parse_v1(input: &[u8]) -> Result<ProxyHeader, &'static str> {
    // HAProxy's v1 limit is 107 bytes including the terminating CRLF.
    let end = input
        .windows(2)
        .position(|w| w == b"\r\n")
        .ok_or_else(|| {
            if input.len() >= 106 {
                "v1 line exceeds 107 bytes"
            } else {
                "incomplete v1 line"
            }
        })?;
    if end + 2 > 107 {
        return Err("v1 line exceeds 107 bytes");
    }
    let line = std::str::from_utf8(&input[..end]).map_err(|_| "v1 line is not ASCII")?;
    if !line.is_ascii() {
        return Err("v1 line is not ASCII");
    }
    let fields: Vec<_> = line.split(' ').collect();
    if fields.len() == 2 && fields[0] == "PROXY" && fields[1] == "UNKNOWN" {
        return Err("UNKNOWN address family has no socket addresses");
    }
    if fields.len() != 6 || fields[0] != "PROXY" {
        return Err("invalid v1 fields");
    }
    let (src, dst): (IpAddr, IpAddr) = match fields[1] {
        "TCP4" => (
            fields[2]
                .parse::<Ipv4Addr>()
                .map_err(|_| "invalid source IPv4")?
                .into(),
            fields[3]
                .parse::<Ipv4Addr>()
                .map_err(|_| "invalid destination IPv4")?
                .into(),
        ),
        "TCP6" => (
            fields[2]
                .parse::<Ipv6Addr>()
                .map_err(|_| "invalid source IPv6")?
                .into(),
            fields[3]
                .parse::<Ipv6Addr>()
                .map_err(|_| "invalid destination IPv6")?
                .into(),
        ),
        _ => return Err("unsupported v1 transport"),
    };
    let source_port = parse_port(fields[4])?;
    let destination_port = parse_port(fields[5])?;
    Ok(ProxyHeader {
        source: SocketAddr::new(src, source_port),
        destination: SocketAddr::new(dst, destination_port),
        consumed: end + 2,
    })
}

pub(crate) fn parse_v2(input: &[u8]) -> Result<ProxyHeader, &'static str> {
    const SIG: &[u8; 12] = b"\r\n\r\n\0\r\nQUIT\n";
    if input.len() < 16 {
        return Err("incomplete v2 header");
    }
    if &input[..12] != SIG {
        return Err("invalid v2 signature");
    }
    if input[12] != 0x21 {
        return Err("unsupported v2 version or command");
    }
    let len = u16::from_be_bytes([input[14], input[15]]) as usize;
    let family = input[13];
    let (source, destination, addr_len) = match family {
        0x11 => {
            if len < 12 {
                return Err("short v2 IPv4 address block");
            }
            if input.len() < 28 {
                return Err("incomplete v2 IPv4 address block");
            }
            (
                SocketAddr::new(
                    Ipv4Addr::new(input[16], input[17], input[18], input[19]).into(),
                    u16::from_be_bytes([input[24], input[25]]),
                ),
                SocketAddr::new(
                    Ipv4Addr::new(input[20], input[21], input[22], input[23]).into(),
                    u16::from_be_bytes([input[26], input[27]]),
                ),
                12,
            )
        }
        0x21 => {
            if len < 36 {
                return Err("short v2 IPv6 address block");
            }
            if input.len() < 52 {
                return Err("incomplete v2 IPv6 address block");
            }
            let mut src = [0; 16];
            src.copy_from_slice(&input[16..32]);
            let mut dst = [0; 16];
            dst.copy_from_slice(&input[32..48]);
            (
                SocketAddr::new(
                    Ipv6Addr::from(src).into(),
                    u16::from_be_bytes([input[48], input[49]]),
                ),
                SocketAddr::new(
                    Ipv6Addr::from(dst).into(),
                    u16::from_be_bytes([input[50], input[51]]),
                ),
                36,
            )
        }
        _ => return Err("unsupported v2 address family"),
    };
    let consumed = 16 + len;
    if input.len() < consumed {
        return Err("incomplete v2 address block or TLVs");
    }
    // TLVs are opaque to this parser, but their framing still has to fit the
    // declared address block so malformed lengths cannot be silently accepted.
    let mut offset = 16 + addr_len;
    while offset < consumed {
        if consumed - offset < 3 {
            return Err("truncated v2 TLV header");
        }
        let value_len = u16::from_be_bytes([input[offset + 1], input[offset + 2]]) as usize;
        offset += 3;
        if value_len > consumed - offset {
            return Err("truncated v2 TLV value");
        }
        offset += value_len;
    }
    Ok(ProxyHeader {
        source,
        destination,
        consumed,
    })
}

fn parse_port(value: &str) -> Result<u16, &'static str> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid port");
    }
    value.parse().map_err(|_| "port out of range")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v1_ipv4_ipv6_and_leaves_payload_length() {
        let input = b"PROXY TCP4 192.0.2.1 198.51.100.2 1234 443\r\nDATA";
        let parsed = parse_v1(input).unwrap();
        assert_eq!(parsed.source, "192.0.2.1:1234".parse().unwrap());
        assert_eq!(parsed.destination, "198.51.100.2:443".parse().unwrap());
        assert_eq!(&input[parsed.consumed..], b"DATA");
        let parsed = parse_v1(b"PROXY TCP6 2001:db8::1 ::1 80 443\r\n").unwrap();
        assert_eq!(parsed.source, "[2001:db8::1]:80".parse().unwrap());
    }

    #[test]
    fn rejects_malformed_or_oversized_v1() {
        for input in [
            b"PROXY TCP4 999.2.3.4 5.6.7.8 1 2\r\n".as_slice(),
            b"PROXY UNKNOWN\r\n",
            b"PROXY TCP4 1.2.3.4 5.6.7.8 +1 2\r\n",
            b"PROXY TCP4 1.2.3.4 5.6.7.8 65536 2\r\n",
        ] {
            assert!(parse_v1(input).is_err(), "accepted {input:?}");
        }
        assert!(parse_v1(&[b"PROXY ".as_slice(), &[b'a'; 103], b"\r\n"].concat()).is_err());
        assert!(parse_v1(b"PROXY TCP4").is_err());
    }

    #[test]
    fn enforces_v1_limit_including_crlf_and_payload_boundary() {
        // 105 bytes before CRLF is the largest permitted line body.
        let mut max_line = b"PROXY ".to_vec();
        max_line.extend(std::iter::repeat_n(b'a', 99));
        max_line.extend_from_slice(b"\r\nPAYLOAD");
        assert_eq!(max_line.len(), 107 + b"PAYLOAD".len());
        assert_eq!(parse_v1(&max_line), Err("invalid v1 fields"));

        let mut too_long = b"PROXY ".to_vec();
        too_long.extend(std::iter::repeat_n(b'a', 100));
        too_long.extend_from_slice(b"\r\n");
        assert_eq!(parse_v1(&too_long), Err("v1 line exceeds 107 bytes"));

        let mut unterminated = b"PROXY ".to_vec();
        unterminated.extend(std::iter::repeat_n(b'a', 101));
        assert_eq!(parse_v1(&unterminated), Err("v1 line exceeds 107 bytes"));
    }

    fn v2_header(family: u8, address: &[u8]) -> Vec<u8> {
        let mut out = b"\r\n\r\n\0\r\nQUIT\n\x21".to_vec();
        out.push(family);
        out.extend_from_slice(&(address.len() as u16).to_be_bytes());
        out.extend_from_slice(address);
        out
    }

    #[test]
    fn parses_v2_ipv4_ipv6_and_tlvs() {
        let v4 = v2_header(
            0x11,
            &[
                192, 0, 2, 1, 198, 51, 100, 2, 0x04, 0xd2, 0x01, 0xbb, 0xea, 0, 1, 7,
            ],
        );
        let parsed = parse_v2(&v4).unwrap();
        assert_eq!(parsed.source, "192.0.2.1:1234".parse().unwrap());
        assert_eq!(parsed.destination, "198.51.100.2:443".parse().unwrap());
        assert_eq!(parsed.consumed, v4.len());
        let mut v6 = vec![0; 36];
        v6[..16].copy_from_slice(&Ipv6Addr::LOCALHOST.octets());
        v6[16..32].copy_from_slice(&Ipv6Addr::LOCALHOST.octets());
        v6[32..34].copy_from_slice(&[0, 80]);
        v6[34..36].copy_from_slice(&[1, 187]);
        assert_eq!(
            parse_v2(&v2_header(0x21, &v6)).unwrap().destination,
            "[::1]:443".parse().unwrap()
        );
    }

    #[test]
    fn rejects_malformed_or_truncated_v2() {
        assert!(parse_v2(b"short").is_err());
        assert!(parse_v2(b"not a proxy v2!!").is_err());
        assert!(parse_v2(&v2_header(0x11, &[0; 11])).is_err());
        assert!(parse_v2(&v2_header(0x31, &[0; 12])).is_err());
        let mut bad_version = v2_header(0x11, &[0; 12]);
        bad_version[12] = 0x11;
        assert!(parse_v2(&bad_version).is_err());
        let mut truncated = v2_header(0x11, &[0; 12]);
        truncated.pop();
        assert!(parse_v2(&truncated).is_err());
    }

    #[test]
    fn rejects_every_truncated_v1_and_v2_prefix() {
        let v1 = b"PROXY TCP4 192.0.2.1 198.51.100.2 1234 443\r\n";
        for end in 0..v1.len() {
            assert!(parse_v1(&v1[..end]).is_err(), "accepted v1 prefix of {end} bytes");
        }

        let v2 = v2_header(0x11, &[192, 0, 2, 1, 198, 51, 100, 2, 0, 80, 1, 187]);
        for end in 0..v2.len() {
            assert!(parse_v2(&v2[..end]).is_err(), "accepted v2 prefix of {end} bytes");
        }
    }

    #[test]
    fn rejects_invalid_v2_command_family_and_declared_oversize_block() {
        let mut invalid_command = v2_header(0x11, &[0; 12]);
        invalid_command[12] = 0x20; // Version 2 with LOCAL command is unsupported here.
        assert_eq!(parse_v2(&invalid_command), Err("unsupported v2 version or command"));

        let invalid_family = v2_header(0x99, &[0; 12]);
        assert_eq!(parse_v2(&invalid_family), Err("unsupported v2 address family"));

        let mut oversized = v2_header(0x11, &[0; 12]);
        oversized[14..16].copy_from_slice(&u16::MAX.to_be_bytes());
        assert_eq!(parse_v2(&oversized), Err("incomplete v2 address block or TLVs"));
    }

    #[test]
    fn validates_v2_tlv_framing_and_preserves_following_payload() {
        let mut valid = v2_header(0x11, &[192, 0, 2, 1, 198, 51, 100, 2, 0, 80, 1, 187]);
        valid.extend_from_slice(&[0xea, 0, 2, 0xaa, 0xbb]);
        valid[14..16].copy_from_slice(&17u16.to_be_bytes());
        valid.extend_from_slice(b"DATA");
        let parsed = parse_v2(&valid).unwrap();
        assert_eq!(&valid[parsed.consumed..], b"DATA");

        let mut partial_tlv_header = v2_header(0x11, &[0; 12]);
        partial_tlv_header.extend_from_slice(&[0xea, 0]);
        partial_tlv_header[14..16].copy_from_slice(&14u16.to_be_bytes());
        assert_eq!(parse_v2(&partial_tlv_header), Err("truncated v2 TLV header"));

        let mut partial_tlv_value = v2_header(0x11, &[0; 12]);
        partial_tlv_value.extend_from_slice(&[0xea, 0, 2, 0xaa]);
        partial_tlv_value[14..16].copy_from_slice(&16u16.to_be_bytes());
        assert_eq!(parse_v2(&partial_tlv_value), Err("truncated v2 TLV value"));
    }
}
