use std::time::Duration;
use std::{future::Future, pin::Pin};

use anyhow::{Context, Result};
use espejismo_core::{
    metered_idle_copy_bidirectional, CopyMeter, EgressPolicy, EgressProxyKind, EgressRequest,
    OutboundConnector, StreamPriority, TransportStream,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;

const EGRESS_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

use crate::http_chain::connect_via_http_proxy;
use crate::limits::UserLimitRegistry;
use crate::socks5_chain::{
    connect_via_socks4_proxy, connect_via_socks5_proxy, relay_udp_via_socks5_proxy,
};

pub(crate) type EgressStream = Box<dyn TransportStream>;

#[derive(Clone, Debug, Default)]
pub(crate) struct DefaultOutboundConnector;

impl OutboundConnector for DefaultOutboundConnector {
    fn connect_tcp<'a>(
        &'a self,
        request: EgressRequest,
    ) -> espejismo_core::extension::BoxFutureResult<'a, EgressStream> {
        Box::pin(async move { connect_egress_tcp_inner(&request.authority, &request.policy).await })
    }

    fn relay_udp<'a>(
        &'a self,
        request: EgressRequest,
        payload: &'a [u8],
    ) -> espejismo_core::extension::BoxFutureResult<'a, Vec<u8>> {
        Box::pin(async move {
            relay_udp_datagram_inner(
                &request.authority,
                payload,
                &request.policy,
                // Bound one UDP reply independently of the longer TCP stream idle timeout.
                Duration::from_secs(10),
            )
            .await
        })
    }
}

pub(crate) async fn limited_copy_bidirectional<A, B>(
    a: &mut A,
    b: &mut B,
    idle: Duration,
    limits: &UserLimitRegistry,
    user: &str,
) -> Result<(u64, u64)>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let mut meter = UserLimitMeter { limits, user };
    metered_idle_copy_bidirectional(a, b, idle, &mut meter).await
}

struct UserLimitMeter<'a> {
    limits: &'a UserLimitRegistry,
    user: &'a str,
}

impl CopyMeter for UserLimitMeter<'_> {
    fn account<'a>(
        &'a mut self,
        bytes: u64,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { self.limits.account_and_throttle(self.user, bytes).await })
    }
}

pub(crate) async fn connect_egress_tcp(
    authority: &str,
    egress: &EgressPolicy,
) -> Result<EgressStream> {
    let connector = DefaultOutboundConnector;
    connector
        .connect_tcp(EgressRequest {
            user: "default".to_string(),
            authority: authority.to_string(),
            priority: StreamPriority::Interactive,
            policy: egress.clone(),
        })
        .await
}

async fn connect_egress_tcp_inner(authority: &str, egress: &EgressPolicy) -> Result<EgressStream> {
    egress
        .validate_authority(authority)
        .context("egress policy rejected target")?;
    if let Some(proxy) = egress.upstream_proxy()? {
        return match proxy.kind {
            EgressProxyKind::Socks4 | EgressProxyKind::Socks4a => {
                connect_via_socks4_proxy(&proxy, authority).await
            }
            EgressProxyKind::Socks5 => connect_via_socks5_proxy(&proxy, authority).await,
            EgressProxyKind::Http | EgressProxyKind::Https => {
                connect_via_http_proxy(&proxy, authority).await
            }
        };
    }
    connect_first_allowed(
        espejismo_core::resolve_socket_addrs(authority).await?,
        egress,
        EGRESS_CONNECT_TIMEOUT,
        |addr| async move { TcpStream::connect(addr).await.map_err(Into::into) },
    )
    .await
    .map(|stream| Box::new(stream) as EgressStream)
    .with_context(|| format!("connect {authority}"))
}

// Each DNS result gets a bounded attempt so an unresponsive first address cannot
// prevent failover to later IPv4/IPv6 results.
async fn connect_first_allowed<T, F, Fut>(
    addrs: Vec<std::net::SocketAddr>,
    egress: &EgressPolicy,
    attempt_timeout: Duration,
    mut connect: F,
) -> Result<T>
where
    F: FnMut(std::net::SocketAddr) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error = None;
    for addr in addrs {
        if let Err(err) = egress.validate_resolved_addr(addr) {
            last_error = Some(err);
            continue;
        }
        match timeout(attempt_timeout, connect(addr)).await {
            Err(_) => last_error = Some(anyhow::anyhow!("connection to {addr} timed out")),
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(err)) => last_error = Some(err),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no resolved egress address")))
}

pub(crate) async fn relay_udp_datagram(
    authority: &str,
    payload: &[u8],
    egress: &EgressPolicy,
    idle: Duration,
) -> Result<Vec<u8>> {
    relay_udp_datagram_inner(authority, payload, egress, idle).await
}

async fn relay_udp_datagram_inner(
    authority: &str,
    payload: &[u8],
    egress: &EgressPolicy,
    idle: Duration,
) -> Result<Vec<u8>> {
    egress
        .validate_authority(authority)
        .context("egress policy rejected UDP target")?;
    if let Some(proxy) = egress.upstream_proxy()? {
        return match proxy.kind {
            EgressProxyKind::Socks4 | EgressProxyKind::Socks4a => anyhow::bail!(
                "SOCKS4/SOCKS4a egress proxy does not support UDP; use socks5:// for UDP relay"
            ),
            EgressProxyKind::Socks5 => {
                relay_udp_via_socks5_proxy(&proxy, authority, payload, idle).await
            }
            EgressProxyKind::Http | EgressProxyKind::Https => {
                anyhow::bail!(
                    "HTTP/HTTPS egress proxy does not support UDP; use socks5:// for UDP relay"
                )
            }
        };
    }
    let mut selected = None;
    for addr in espejismo_core::resolve_socket_addrs(authority).await? {
        if egress.validate_resolved_addr(addr).is_ok() {
            selected = Some(addr);
            break;
        }
    }
    let target = selected.context("no allowed UDP egress address")?;
    let bind = if target.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind).await?;
    socket.connect(target).await?;
    socket.send(payload).await?;
    let mut response = vec![0_u8; 65_535];
    // UDP replies are datagram transactions and should not inherit a long-lived stream timeout.
    let n = timeout(
        idle.min(Duration::from_secs(10)),
        socket.recv(&mut response),
    )
    .await??;
    response.truncate(n);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };

    use std::net::SocketAddr;

    use super::{connect_egress_tcp_inner, connect_first_allowed};
    use espejismo_core::EgressPolicy;
    use std::time::Duration;

    #[tokio::test]
    async fn egress_connect_failure_advances_to_next_resolved_address() {
        let first: SocketAddr = "192.0.2.10:443".parse().unwrap();
        let second: SocketAddr = "192.0.2.11:443".parse().unwrap();
        let mut attempted = Vec::new();
        let result = connect_first_allowed(
            vec![first, second],
            &EgressPolicy::default(),
            Duration::from_secs(1),
            |addr| {
                attempted.push(addr);
                async move {
                    if addr == first {
                        Err(anyhow::anyhow!("simulated unreachable egress"))
                    } else {
                        Ok(addr)
                    }
                }
            },
        )
        .await
        .unwrap();

        assert_eq!(result, second);
        assert_eq!(attempted, vec![first, second]);
    }

    #[tokio::test]
    async fn rejected_address_is_skipped_before_connect_attempt() {
        let denied: SocketAddr = "127.0.0.1:443".parse().unwrap();
        let allowed: SocketAddr = "8.8.8.8:443".parse().unwrap();
        let mut attempted = Vec::new();
        let policy = EgressPolicy {
            deny_private_ips: true,
            ..EgressPolicy::default()
        };
        let result = connect_first_allowed(
            vec![denied, allowed],
            &policy,
            Duration::from_secs(1),
            |addr| {
                attempted.push(addr);
                std::future::ready(Ok(addr))
            },
        )
        .await
        .unwrap();
        assert_eq!(result, allowed);
        assert_eq!(attempted, vec![allowed]);
    }

    #[tokio::test]
    async fn timed_out_address_advances_and_later_address_can_succeed() {
        let first: SocketAddr = "192.0.2.20:443".parse().unwrap();
        let second: SocketAddr = "192.0.2.21:443".parse().unwrap();
        let mut attempted = Vec::new();
        let result = connect_first_allowed(
            vec![first, second],
            &EgressPolicy::default(),
            Duration::from_millis(5),
            |addr| {
                attempted.push(addr);
                async move {
                    if addr == first {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Ok(addr)
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(result, second);
        assert_eq!(attempted, vec![first, second]);
    }

    #[tokio::test]
    async fn exhausted_addresses_report_last_failure_and_empty_list_is_explicit() {
        let first: SocketAddr = "192.0.2.30:443".parse().unwrap();
        let second: SocketAddr = "192.0.2.31:443".parse().unwrap();
        let error = connect_first_allowed(
            vec![first, second],
            &EgressPolicy::default(),
            Duration::from_secs(1),
            |addr| async move { Err::<(), _>(anyhow::anyhow!("failed at {addr}")) },
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains(&second.to_string()));

        let error = connect_first_allowed::<(), _, _>(
            Vec::new(),
            &EgressPolicy::default(),
            Duration::from_secs(1),
            |_| std::future::ready(Ok(())),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("no resolved egress address"));
    }

    async fn socks5_hop(listener: TcpListener) {
        loop {
            let (mut client, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut greeting = [0; 3];
                client.read_exact(&mut greeting).await.unwrap();
                assert_eq!(greeting, [5, 1, 0]);
                client.write_all(&[5, 0]).await.unwrap();

                let mut header = [0; 4];
                client.read_exact(&mut header).await.unwrap();
                assert_eq!(header, [5, 1, 0, 3]);
                let host_len = client.read_u8().await.unwrap() as usize;
                let mut host = vec![0; host_len];
                client.read_exact(&mut host).await.unwrap();
                let port = client.read_u16().await.unwrap();
                let authority = format!("{}:{port}", String::from_utf8(host).unwrap());
                let upstream = TcpStream::connect(authority).await.unwrap();
                let mut upstream = upstream;
                client
                    .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
                    .await
                    .unwrap();
                let _ = copy_bidirectional(&mut client, &mut upstream).await;
            });
        }
    }

    // Requires loopback TCP bind, unavailable in the Codex sandbox; the gate
    // skips it there. Run outside the sandbox with
    // `cargo test -p espejismo-server -- --ignored` to execute it.
    #[tokio::test]
    #[ignore = "requires loopback bind"]
    async fn relays_tcp_through_two_socks5_hops() {
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = target.local_addr().unwrap();
        let echo = tokio::spawn(async move {
            let (mut stream, _) = target.accept().await.unwrap();
            let mut bytes = [0; 32];
            let n = stream.read(&mut bytes).await.unwrap();
            stream.write_all(&bytes[..n]).await.unwrap();
        });

        let second = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let second_addr = second.local_addr().unwrap();
        let target_for_second = target_addr;
        tokio::spawn(async move {
            let (mut incoming, _) = second.accept().await.unwrap();
            let mut outgoing = TcpStream::connect(target_for_second).await.unwrap();
            let _ = copy_bidirectional(&mut incoming, &mut outgoing).await;
        });
        let first = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let first_addr = first.local_addr().unwrap();
        tokio::spawn(socks5_hop(first));

        // The SOCKS5 CONNECT tunnel crosses the first proxy into a second
        // local forwarding hop, then reaches the echo service.
        let policy = EgressPolicy {
            proxy: Some(format!("socks5://{first_addr}")),
            ..EgressPolicy::default()
        };
        let mut stream = connect_egress_tcp_inner(&format!("{second_addr}"), &policy)
            .await
            .unwrap();
        stream.write_all(b"two-hop-echo").await.unwrap();
        let mut echoed = [0; 12];
        stream.read_exact(&mut echoed).await.unwrap();
        assert_eq!(&echoed, b"two-hop-echo");
        drop(stream);
        echo.await.unwrap();
    }
}
