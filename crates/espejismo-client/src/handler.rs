use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use espejismo_core::{
    http_proxy, idle_copy_bidirectional, socks5, write_tcp_connect, write_udp_datagram, Metrics,
    ProxyAuth,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;

use crate::tunnel::TunnelManager;

pub(crate) async fn handle_socks5_client(
    mut local: TcpStream,
    tunnel: Arc<TunnelManager>,
    auth: Option<ProxyAuth>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    metrics.inc_active_stream();
    metrics.inc_stream_opened();
    let result = handle_socks5_client_inner(&mut local, tunnel, auth, metrics.clone(), idle).await;
    if result.is_err() {
        metrics.inc_stream_failed();
    }
    metrics.dec_active_stream();
    result
}

async fn handle_socks5_client_inner(
    local: &mut TcpStream,
    tunnel: Arc<TunnelManager>,
    auth: Option<ProxyAuth>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    match socks5::accept_request_with_auth(local, auth.as_ref()).await? {
        socks5::SocksRequest::Connect(target) => {
            let mut stream = tunnel.open_stream().await?;
            write_tcp_connect(&mut stream, &target.authority()).await?;
            let (client_to_remote, remote_to_client) =
                idle_copy_bidirectional(local, &mut stream, idle).await?;
            metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
            Ok(())
        }
        socks5::SocksRequest::UdpAssociate => {
            handle_udp_associate(local, tunnel, metrics, idle).await
        }
    }
}

pub(crate) async fn handle_http_client(
    mut local: TcpStream,
    tunnel: Arc<TunnelManager>,
    auth: Option<ProxyAuth>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    metrics.inc_active_stream();
    metrics.inc_stream_opened();
    let result = handle_http_client_inner(&mut local, tunnel, auth, metrics.clone(), idle).await;
    if result.is_err() {
        metrics.inc_stream_failed();
    }
    metrics.dec_active_stream();
    result
}

async fn handle_http_client_inner(
    local: &mut TcpStream,
    tunnel: Arc<TunnelManager>,
    auth: Option<ProxyAuth>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    let target = http_proxy::accept_http_proxy_with_auth(local, auth.as_ref()).await?;
    let mut stream = tunnel.open_stream().await?;
    write_tcp_connect(&mut stream, &target.authority).await?;
    if !target.prebuffer.is_empty() {
        stream.write_all(&target.prebuffer).await?;
    }
    let (client_to_remote, remote_to_client) =
        idle_copy_bidirectional(local, &mut stream, idle).await?;
    metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
    Ok(())
}

async fn handle_udp_associate(
    control_stream: &mut TcpStream,
    tunnel: Arc<TunnelManager>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    let bind_addr = if control_stream.local_addr()?.is_ipv4() {
        "127.0.0.1:0"
    } else {
        "[::1]:0"
    };
    let udp = UdpSocket::bind(bind_addr).await?;
    let udp_addr = udp.local_addr()?;
    socks5::reply_udp_associate(control_stream, udp_addr).await?;
    let mut buf = vec![0_u8; 65_535];
    loop {
        let (n, peer) = timeout(idle, udp.recv_from(&mut buf)).await??;
        let packet = socks5::parse_udp_packet(&buf[..n])?;
        let response = relay_udp_packet(tunnel.clone(), &packet.target, &packet.payload).await?;
        let wrapped = socks5::build_udp_packet(&packet.target, &response)?;
        udp.send_to(&wrapped, peer).await?;
        metrics.add_tunnel_bytes(packet.payload.len() as u64, response.len() as u64);
    }
}

async fn relay_udp_packet(
    tunnel: Arc<TunnelManager>,
    target: &socks5::SocksTarget,
    payload: &[u8],
) -> Result<Vec<u8>> {
    let mut stream = tunnel.open_stream().await?;
    write_udp_datagram(&mut stream, &target.authority(), payload).await?;
    let len = timeout(Duration::from_secs(15), stream.read_u16()).await?? as usize;
    let mut response = vec![0_u8; len];
    timeout(Duration::from_secs(15), stream.read_exact(&mut response)).await??;
    Ok(response)
}
