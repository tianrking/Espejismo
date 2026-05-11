use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use espejismo_core::{
    http_proxy, idle_copy_bidirectional, socks5, write_tcp_connect_with_priority,
    write_udp_datagram_with_priority, Metrics, ProxyAuth, StreamPriority,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;

use crate::tunnel::TunnelService;

pub(crate) async fn handle_socks5_client(
    mut local: TcpStream,
    tunnel: Arc<TunnelService>,
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
    tunnel: Arc<TunnelService>,
    auth: Option<ProxyAuth>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    match socks5::accept_request_with_auth(local, auth.as_ref()).await? {
        socks5::SocksRequest::Connect(target) => {
            let priority = StreamPriority::Interactive;
            let mut stream = tunnel.open_stream(priority).await?;
            let lane_id = stream.lane_id();
            let mut client_to_remote = 0;
            let mut remote_to_client = 0;
            let result = async {
                write_tcp_connect_with_priority(&mut stream, &target.authority(), priority).await?;
                (client_to_remote, remote_to_client) =
                    idle_copy_bidirectional(local, &mut stream, idle).await?;
                anyhow::Ok(())
            }
            .await;
            metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
            tunnel
                .record_stream_bytes(lane_id, client_to_remote, remote_to_client)
                .await;
            result
        }
        socks5::SocksRequest::UdpAssociate => {
            handle_udp_associate(local, tunnel, metrics, idle).await
        }
    }
}

pub(crate) async fn handle_http_client(
    mut local: TcpStream,
    tunnel: Arc<TunnelService>,
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
    tunnel: Arc<TunnelService>,
    auth: Option<ProxyAuth>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    let target = http_proxy::accept_http_proxy_with_auth(local, auth.as_ref()).await?;
    let priority = StreamPriority::Interactive;
    let mut stream = tunnel.open_stream(priority).await?;
    let lane_id = stream.lane_id();
    let mut client_to_remote = 0;
    let mut remote_to_client = 0;
    let result = async {
        write_tcp_connect_with_priority(&mut stream, &target.authority, priority).await?;
        if !target.prebuffer.is_empty() {
            stream.write_all(&target.prebuffer).await?;
        }
        (client_to_remote, remote_to_client) =
            idle_copy_bidirectional(local, &mut stream, idle).await?;
        anyhow::Ok(())
    }
    .await;
    metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
    tunnel
        .record_stream_bytes(lane_id, client_to_remote, remote_to_client)
        .await;
    result
}

async fn handle_udp_associate(
    control_stream: &mut TcpStream,
    tunnel: Arc<TunnelService>,
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
    tunnel: Arc<TunnelService>,
    target: &socks5::SocksTarget,
    payload: &[u8],
) -> Result<Vec<u8>> {
    let priority = StreamPriority::Interactive;
    let mut stream = tunnel.open_stream(priority).await?;
    let lane_id = stream.lane_id();
    let mut response = Vec::new();
    let result = async {
        write_udp_datagram_with_priority(&mut stream, &target.authority(), priority, payload)
            .await?;
        let len = timeout(Duration::from_secs(15), stream.read_u16()).await?? as usize;
        response = vec![0_u8; len];
        timeout(Duration::from_secs(15), stream.read_exact(&mut response)).await??;
        anyhow::Ok(())
    }
    .await;
    tunnel
        .record_stream_bytes(lane_id, payload.len() as u64, response.len() as u64)
        .await;
    result?;
    Ok(response)
}
