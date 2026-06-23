use std::time::Duration;
use std::{future::Future, pin::Pin};

use anyhow::{Context, Result};
use espejismo_core::{
    metered_idle_copy_bidirectional, CopyMeter, EgressPolicy, EgressProxyKind, EgressRequest,
    OutboundConnector, StreamPriority, TransportStream,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{lookup_host, TcpStream, UdpSocket};
use tokio::time::timeout;

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
    let mut last_error = None;
    for addr in lookup_host(authority)
        .await
        .with_context(|| format!("resolve {authority}"))?
    {
        if let Err(err) = egress.validate_resolved_addr(addr) {
            last_error = Some(err);
            continue;
        }
        match TcpStream::connect(addr).await {
            Ok(stream) => return Ok(Box::new(stream)),
            Err(err) => last_error = Some(err.into()),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no resolved egress address")))
        .with_context(|| format!("connect {authority}"))
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
    for addr in lookup_host(authority)
        .await
        .with_context(|| format!("resolve {authority}"))?
    {
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
    let n = timeout(
        idle.min(Duration::from_secs(10)),
        socket.recv(&mut response),
    )
    .await??;
    response.truncate(n);
    Ok(response)
}
