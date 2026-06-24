use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use espejismo_core::config::LocalTunConfig;
use espejismo_core::{
    idle_copy_bidirectional, write_tcp_connect_with_priority, write_udp_datagram_with_priority,
    Metrics, StreamPriority,
};
use futures::{SinkExt, StreamExt};
use netstack_smoltcp::{StackBuilder, TcpListener, UdpSocket};
use tokio::io::AsyncReadExt;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{debug, info, warn};
use tun_rs::DeviceBuilder;

use crate::route;
use crate::tunnel::{MeteredTunnelStream, TunnelService, TunnelStream};

const MAX_TUN_UDP_TASKS: usize = 1024;
const TUN_STREAM_OPEN_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn run_tun_ingress(
    config: LocalTunConfig,
    server: String,
    tunnel: Arc<TunnelService>,
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

    if config.route.enabled {
        warm_up_tunnel(tunnel.clone()).await?;
    }

    let _route_guard = if config.route.enabled {
        Some(route::install_tun_routes(&config, &server).await?)
    } else {
        None
    };

    let (stack, runner, udp_socket, tcp_listener) = StackBuilder::default()
        .enable_tcp(true)
        .enable_udp(true)
        .enable_icmp(false)
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
    if config.udp_enabled {
        tokio::spawn(handle_tun_udp(
            udp_socket,
            tunnel,
            metrics,
            UdpTunPolicy::from(&config),
        ));
    } else {
        info!("TUN UDP relay disabled");
    }

    futures::future::pending::<Result<()>>().await
}

async fn handle_tun_tcp(
    mut tcp_listener: TcpListener,
    tunnel: Arc<TunnelService>,
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
                info!(%local, %remote, authority = %authority, "TUN TCP flow accepted");
                let (tunnel_stream, priority) =
                    open_tun_stream(tunnel.clone(), StreamPriority::Interactive).await?;
                let mut tunnel_stream = MeteredTunnelStream::new(tunnel_stream);
                let lane_id = tunnel_stream.lane_id();
                debug!(lane_id, %local, %remote, priority = ?priority, "TUN TCP flow opened tunnel stream");
                let mut copy_elapsed = Duration::ZERO;
                let result = async {
                    write_tcp_connect_with_priority(&mut tunnel_stream, &authority, priority)
                        .await?;
                    let copy_started = Instant::now();
                    let copy_result =
                        idle_copy_bidirectional(&mut local_stream, &mut tunnel_stream, idle).await;
                    copy_elapsed = copy_started.elapsed();
                    copy_result?;
                    anyhow::Ok(())
                }
                .await;
                let (client_to_remote, remote_to_client) = tunnel_stream.byte_counts();
                metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
                tunnel
                    .record_stream_bytes(lane_id, client_to_remote, remote_to_client, copy_elapsed)
                    .await;
                result
            }
            .await;
            if let Err(err) = result {
                metrics.inc_stream_failed();
                warn!(%local, %remote, error = %err, "TUN TCP flow failed");
            }
            metrics.dec_active_stream();
        });
    }
}

#[derive(Clone, Debug)]
struct UdpTunPolicy {
    timeout: Duration,
    blocked_ports: BTreeSet<u16>,
}

impl UdpTunPolicy {
    fn from(config: &LocalTunConfig) -> Self {
        Self {
            timeout: Duration::from_secs(config.udp_timeout_secs.max(1)),
            blocked_ports: config.udp_block_ports.iter().copied().collect(),
        }
    }

    fn blocks(&self, remote: SocketAddr) -> bool {
        self.blocked_ports.contains(&remote.port())
    }
}

async fn handle_tun_udp(
    udp_socket: UdpSocket,
    tunnel: Arc<TunnelService>,
    metrics: Metrics,
    policy: UdpTunPolicy,
) {
    let task_limit = Arc::new(Semaphore::new(MAX_TUN_UDP_TASKS));
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
        if policy.blocks(remote) {
            debug!(
                %local,
                %remote,
                "TUN UDP datagram dropped by local UDP port policy"
            );
            continue;
        }
        let Ok(permit) = task_limit.clone().try_acquire_owned() else {
            metrics.inc_stream_failed();
            debug!(
                %local,
                %remote,
                max = MAX_TUN_UDP_TASKS,
                "TUN UDP datagram dropped because relay task limit is full"
            );
            continue;
        };
        let tx = tx.clone();
        let tunnel = tunnel.clone();
        let metrics = metrics.clone();
        let policy = policy.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let authority = authority_from_socket(remote);
            debug!(
                %local,
                %remote,
                authority = %authority,
                bytes = payload.len(),
                "TUN UDP datagram accepted"
            );
            match relay_udp_authority(tunnel, &authority, &payload, policy.timeout).await {
                Ok(response) => {
                    metrics.add_tunnel_bytes(payload.len() as u64, response.len() as u64);
                    let _ = tx.send((response, local, remote));
                }
                Err(err) => {
                    metrics.inc_stream_failed();
                    warn!(%local, %remote, error = %err, "TUN UDP datagram failed");
                }
            }
        });
    }
}

async fn relay_udp_authority(
    tunnel: Arc<TunnelService>,
    authority: &str,
    payload: &[u8],
    response_timeout: Duration,
) -> Result<Vec<u8>> {
    let (stream, priority) = open_tun_stream(tunnel.clone(), StreamPriority::Interactive).await?;
    let mut stream = MeteredTunnelStream::new(stream);
    let lane_id = stream.lane_id();
    let mut response = Vec::new();
    let started = Instant::now();
    let result = async {
        write_udp_datagram_with_priority(&mut stream, authority, priority, payload).await?;
        let len = timeout(response_timeout, stream.read_u16()).await?? as usize;
        response = vec![0_u8; len];
        timeout(response_timeout, stream.read_exact(&mut response)).await??;
        anyhow::Ok(())
    }
    .await;
    let (client_to_remote, remote_to_client) = stream.byte_counts();
    tunnel
        .record_stream_bytes(
            lane_id,
            client_to_remote,
            remote_to_client,
            started.elapsed(),
        )
        .await;
    result?;
    Ok(response)
}

async fn open_tun_stream(
    tunnel: Arc<TunnelService>,
    priority: StreamPriority,
) -> Result<(TunnelStream, StreamPriority)> {
    match timeout(TUN_STREAM_OPEN_TIMEOUT, tunnel.open_stream(priority)).await {
        Ok(Ok(stream)) => Ok((stream, priority)),
        Ok(Err(err)) => {
            warn!(error = %err, ?priority, "TUN lane open failed");
            if priority == StreamPriority::Interactive {
                return Err(err);
            }
            let stream = timeout(
                TUN_STREAM_OPEN_TIMEOUT,
                tunnel.open_stream(StreamPriority::Interactive),
            )
            .await
            .map_err(|_| anyhow::anyhow!("TUN interactive lane open timed out"))??;
            Ok((stream, StreamPriority::Interactive))
        }
        Err(_) => {
            warn!(?priority, "TUN lane open timed out");
            if priority == StreamPriority::Interactive {
                anyhow::bail!("TUN interactive lane open timed out");
            }
            let stream = timeout(
                TUN_STREAM_OPEN_TIMEOUT,
                tunnel.open_stream(StreamPriority::Interactive),
            )
            .await
            .map_err(|_| anyhow::anyhow!("TUN interactive lane open timed out"))??;
            Ok((stream, StreamPriority::Interactive))
        }
    }
}

async fn warm_up_tunnel(tunnel: Arc<TunnelService>) -> Result<()> {
    info!("warming up tunnel before TUN route takeover");
    let stream = timeout(
        TUN_STREAM_OPEN_TIMEOUT,
        tunnel.open_stream(StreamPriority::Interactive),
    )
    .await
    .map_err(|_| anyhow::anyhow!("TUN warm-up stream open timed out"))??;
    let lane_id = stream.lane_id();
    drop(stream);
    tunnel
        .record_stream_bytes(lane_id, 0, 0, Duration::ZERO)
        .await;
    info!(lane_id, "TUN warm-up stream opened");
    Ok(())
}

fn authority_from_socket(addr: SocketAddr) -> String {
    addr.to_string()
}
