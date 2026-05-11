use std::time::Duration;

use anyhow::{Context, Result};
use espejismo_core::EgressPolicy;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{lookup_host, TcpStream, UdpSocket};
use tokio::time::timeout;

use crate::limits::UserLimitRegistry;
use crate::socks5_chain::{connect_via_socks5_proxy, relay_udp_via_socks5_proxy};

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
    let mut buf_a = [0_u8; 8192];
    let mut buf_b = [0_u8; 8192];
    let mut total_a = 0_u64;
    let mut total_b = 0_u64;
    let mut a_done = false;
    let mut b_done = false;

    loop {
        let read_a = if !a_done {
            Some(timeout(idle, a.read(&mut buf_a)))
        } else {
            None
        };
        let read_b = if !b_done {
            Some(timeout(idle, b.read(&mut buf_b)))
        } else {
            None
        };

        match (read_a, read_b) {
            (Some(ra), Some(rb)) => {
                tokio::select! {
                    r = ra => {
                        match r {
                            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => {
                                a_done = true;
                                let _ = b.shutdown().await;
                                if b_done {
                                    break;
                                }
                            }
                            Ok(Ok(n)) => {
                                limits.account_and_throttle(user, n as u64).await?;
                                b.write_all(&buf_a[..n]).await?;
                                total_a += n as u64;
                            }
                        }
                        continue;
                    }
                    r = rb => {
                        match r {
                            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => {
                                b_done = true;
                                let _ = a.shutdown().await;
                                if a_done {
                                    break;
                                }
                            }
                            Ok(Ok(n)) => {
                                limits.account_and_throttle(user, n as u64).await?;
                                a.write_all(&buf_b[..n]).await?;
                                total_b += n as u64;
                            }
                        }
                        continue;
                    }
                }
            }
            (Some(ra), None) => match ra.await {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                Ok(Ok(n)) => {
                    limits.account_and_throttle(user, n as u64).await?;
                    b.write_all(&buf_a[..n]).await?;
                    total_a += n as u64;
                }
            },
            (None, Some(rb)) => match rb.await {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                Ok(Ok(n)) => {
                    limits.account_and_throttle(user, n as u64).await?;
                    a.write_all(&buf_b[..n]).await?;
                    total_b += n as u64;
                }
            },
            (None, None) => break,
        }
    }

    Ok((total_a, total_b))
}

pub(crate) async fn connect_egress_tcp(
    authority: &str,
    egress: &EgressPolicy,
) -> Result<TcpStream> {
    egress
        .validate_authority(authority)
        .context("egress policy rejected target")?;
    if let Some(proxy) = &egress.socks5_proxy {
        return connect_via_socks5_proxy(proxy, authority).await;
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
            Ok(stream) => return Ok(stream),
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
    egress
        .validate_authority(authority)
        .context("egress policy rejected UDP target")?;
    if let Some(proxy) = &egress.socks5_proxy {
        return relay_udp_via_socks5_proxy(proxy, authority, payload, idle).await;
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
