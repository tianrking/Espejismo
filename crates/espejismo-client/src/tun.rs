use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use espejismo_core::config::LocalTunConfig;
use espejismo_core::{idle_copy_bidirectional, write_tcp_connect, write_udp_datagram, Metrics};
use futures::{SinkExt, StreamExt};
use netstack_smoltcp::{StackBuilder, TcpListener, UdpSocket};
use tokio::io::AsyncReadExt;
use tokio::time::timeout;
use tracing::{debug, info, warn};
use tun_rs::DeviceBuilder;

use crate::route;
use crate::TunnelManager;

pub async fn run_tun_ingress(
    config: LocalTunConfig,
    server: String,
    tunnel: Arc<TunnelManager>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    let device = Arc::new(
        DeviceBuilder::new()
            .name(config.name.clone())
            .ipv4(config.address, config.prefix, Some(config.destination))
            .mtu(config.mtu)
            .build_async()
            .with_context(|| format!("create TUN device {}", config.name))?,
    );
    info!(
        name = %config.name,
        address = %config.address,
        prefix = config.prefix,
        destination = %config.destination,
        mtu = config.mtu,
        "TUN ingress enabled"
    );

    let _route_guard = if config.route.enabled {
        Some(route::install_tun_routes(&config, &server).await?)
    } else {
        None
    };

    let (stack, runner, udp_socket, tcp_listener) = StackBuilder::default()
        .enable_tcp(true)
        .enable_udp(true)
        .enable_icmp(true)
        .mtu(config.mtu as usize)
        .build()
        .context("create userspace netstack")?;

    if let Some(runner) = runner {
        tokio::spawn(runner);
    }

    let udp_socket = udp_socket.context("netstack UDP socket unavailable")?;
    let tcp_listener = tcp_listener.context("netstack TCP listener unavailable")?;
    let (mut stack_sink, mut stack_stream) = stack.split();

    let device_to_stack = device.clone();
    tokio::spawn(async move {
        let mut buf = vec![0_u8; 65_535];
        loop {
            match device_to_stack.recv(&mut buf).await {
                Ok(0) => continue,
                Ok(n) => {
                    if let Err(err) = stack_sink.send(buf[..n].to_vec()).await {
                        warn!(error = %err, "TUN packet could not enter netstack");
                        break;
                    }
                }
                Err(err) => {
                    warn!(error = %err, "TUN receive stopped");
                    break;
                }
            }
        }
    });

    let stack_to_device = device.clone();
    tokio::spawn(async move {
        while let Some(packet) = stack_stream.next().await {
            match packet {
                Ok(packet) => {
                    if let Err(err) = stack_to_device.send(&packet).await {
                        warn!(error = %err, "netstack packet could not leave TUN");
                        break;
                    }
                }
                Err(err) => warn!(error = %err, "netstack emitted invalid packet"),
            }
        }
    });

    tokio::spawn(handle_tun_tcp(
        tcp_listener,
        tunnel.clone(),
        metrics.clone(),
        idle,
    ));
    tokio::spawn(handle_tun_udp(udp_socket, tunnel, metrics));

    futures::future::pending::<Result<()>>().await
}

async fn handle_tun_tcp(
    mut tcp_listener: TcpListener,
    tunnel: Arc<TunnelManager>,
    metrics: Metrics,
    idle: Duration,
) {
    while let Some((mut local_stream, local, remote)) = tcp_listener.next().await {
        let tunnel = tunnel.clone();
        let metrics = metrics.clone();
        tokio::spawn(async move {
            metrics.inc_active_stream();
            metrics.inc_stream_opened();
            let result = async {
                let authority = authority_from_socket(remote);
                let mut tunnel_stream = tunnel.open_stream().await?;
                write_tcp_connect(&mut tunnel_stream, &authority).await?;
                let (client_to_remote, remote_to_client) =
                    idle_copy_bidirectional(&mut local_stream, &mut tunnel_stream, idle).await?;
                metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
                anyhow::Ok(())
            }
            .await;
            if let Err(err) = result {
                metrics.inc_stream_failed();
                debug!(%local, %remote, error = %err, "TUN TCP flow ended");
            }
            metrics.dec_active_stream();
        });
    }
}

async fn handle_tun_udp(udp_socket: UdpSocket, tunnel: Arc<TunnelManager>, metrics: Metrics) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let (mut read_half, mut write_half) = udp_socket.split();
    tokio::spawn(async move {
        while let Some((payload, local, remote)) = rx.recv().await {
            if let Err(err) = write_half.send((payload, remote, local)).await {
                warn!(error = %err, "TUN UDP response could not enter netstack");
            }
        }
    });

    while let Some((payload, local, remote)) = read_half.next().await {
        let tx = tx.clone();
        let tunnel = tunnel.clone();
        let metrics = metrics.clone();
        tokio::spawn(async move {
            let authority = authority_from_socket(remote);
            match relay_udp_authority(tunnel, &authority, &payload).await {
                Ok(response) => {
                    metrics.add_tunnel_bytes(payload.len() as u64, response.len() as u64);
                    let _ = tx.send((response, local, remote));
                }
                Err(err) => {
                    metrics.inc_stream_failed();
                    debug!(%local, %remote, error = %err, "TUN UDP datagram ended");
                }
            }
        });
    }
}

async fn relay_udp_authority(
    tunnel: Arc<TunnelManager>,
    authority: &str,
    payload: &[u8],
) -> Result<Vec<u8>> {
    let mut stream = tunnel.open_stream().await?;
    write_udp_datagram(&mut stream, authority, payload).await?;
    let len = timeout(Duration::from_secs(15), stream.read_u16()).await?? as usize;
    let mut response = vec![0_u8; len];
    timeout(Duration::from_secs(15), stream.read_exact(&mut response)).await??;
    Ok(response)
}

fn authority_from_socket(addr: SocketAddr) -> String {
    addr.to_string()
}
