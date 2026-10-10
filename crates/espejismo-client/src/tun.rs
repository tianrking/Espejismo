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
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{debug, info, trace, warn};
use tun_rs::DeviceBuilder;

use crate::route;
use crate::tunnel::{MeteredTunnelStream, TunnelService, TunnelStream};

const MAX_TUN_UDP_TASKS: usize = 1024;
// TUN packets should not queue behind an unavailable tunnel stream for long.
const TUN_STREAM_OPEN_TIMEOUT: Duration = Duration::from_secs(10);

// Admission is deliberately non-blocking: when every UDP task slot is occupied,
// the caller drops the new datagram instead of building an unbounded task backlog.
fn try_acquire_udp_task(limit: &Arc<Semaphore>) -> Option<tokio::sync::OwnedSemaphorePermit> {
    limit.clone().try_acquire_owned().ok()
}

fn tun_udp_response_queue() -> (
    tokio::sync::mpsc::Sender<(Vec<u8>, SocketAddr, SocketAddr)>,
    tokio::sync::mpsc::Receiver<(Vec<u8>, SocketAddr, SocketAddr)>,
) {
    tokio::sync::mpsc::channel(MAX_TUN_UDP_TASKS)
}

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

    // smoltcp emits complete IP packets with checksums already filled. Do not
    // enable tun-rs Linux offload here: it changes the device ABI to include a
    // virtio-net header and may deliver GRO/GSO super-packets, which this
    // packet-at-a-time stack bridge does not handle.
    let (stack, runner, udp_socket, tcp_listener) = StackBuilder::default()
        .enable_tcp(true)
        .enable_udp(true)
        .enable_icmp(false)
        .mtu(usize::from(config.mtu))
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
                    let packet = buf[..n].to_vec();
                    if !tun_udp_checksums_valid(&packet) {
                        trace!("TUN packet dropped for invalid UDP checksum");
                        continue;
                    }
                    if let Err(err) = stack_sink.send(packet).await {
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
                debug!(%local, %remote, authority = %authority, "TUN TCP flow accepted");
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
    // Bound completed responses too: a stalled netstack writer must apply
    // backpressure to relay tasks instead of accumulating datagrams forever.
    let (tx, mut rx) = tun_udp_response_queue();
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
            trace!(
                %local,
                %remote,
                "TUN UDP datagram dropped by local UDP port policy"
            );
            continue;
        }
        let Some(permit) = try_acquire_udp_task(&task_limit) else {
            metrics.inc_stream_failed();
            trace!(
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
            trace!(
                %local,
                %remote,
                authority = %authority,
                bytes = payload.len(),
                "TUN UDP datagram accepted"
            );
            match relay_udp_authority(tunnel, &authority, &payload, policy.timeout).await {
                Ok(response) => {
                    metrics.add_tunnel_bytes(payload.len() as u64, response.len() as u64);
                    if tx.send((response, local, remote)).await.is_err() {
                        trace!(%local, %remote, "TUN UDP response writer stopped");
                    }
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
        response = read_udp_response(&mut stream, response_timeout).await?;
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

// UDP traffic is relayed as independent datagrams. Start a fresh inactivity
// window for each response so later packets on the same NAT flow stay usable.
async fn read_udp_response<R>(reader: &mut R, response_timeout: Duration) -> Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    timeout(response_timeout, async {
        let len = reader.read_u16().await? as usize;
        let mut response = vec![0_u8; len];
        reader.read_exact(&mut response).await?;
        Ok(response)
    })
    .await?
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

// UDP checksum zero is the IPv4 "not supplied" sentinel. IPv6 requires a
// checksum, so do not let smoltcp's shared UDP verifier accept zero there.
fn udp_checksum_valid(
    packet: &netstack_smoltcp::smoltcp::wire::UdpPacket<&[u8]>,
    src_addr: &netstack_smoltcp::smoltcp::wire::IpAddress,
    dst_addr: &netstack_smoltcp::smoltcp::wire::IpAddress,
) -> bool {
    let is_ipv6 = matches!(src_addr, netstack_smoltcp::smoltcp::wire::IpAddress::Ipv6(_));
    (!is_ipv6 || packet.checksum() != 0) && packet.verify_checksum(src_addr, dst_addr)
}

fn tun_udp_checksums_valid(packet: &[u8]) -> bool {
    use netstack_smoltcp::smoltcp::wire::{
        IpAddress, Ipv4Packet, Ipv6Packet, UdpPacket,
    };

    match packet.first().map(|byte| byte >> 4) {
        Some(4) => {
            let Ok(ip) = Ipv4Packet::new_checked(packet) else {
                return false;
            };
            if ip.next_header() != netstack_smoltcp::smoltcp::wire::IpProtocol::Udp {
                return true;
            }
            let Ok(udp) = UdpPacket::new_checked(ip.payload()) else {
                return false;
            };
            udp_checksum_valid(
                &udp,
                &IpAddress::Ipv4(ip.src_addr()),
                &IpAddress::Ipv4(ip.dst_addr()),
            )
        }
        Some(6) => {
            let Ok(ip) = Ipv6Packet::new_checked(packet) else {
                return false;
            };
            if ip.next_header() != netstack_smoltcp::smoltcp::wire::IpProtocol::Udp {
                return true;
            }
            let Ok(udp) = UdpPacket::new_checked(ip.payload()) else {
                return false;
            };
            udp_checksum_valid(
                &udp,
                &IpAddress::Ipv6(ip.src_addr()),
                &IpAddress::Ipv6(ip.dst_addr()),
            )
        }
        // Leave unsupported IP versions and non-UDP parsing to smoltcp.
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tun_udp_task_limit_accepts_capacity_then_drops_new_work() {
        let limit = Arc::new(Semaphore::new(MAX_TUN_UDP_TASKS));
        let mut permits: Vec<_> = (0..MAX_TUN_UDP_TASKS)
            .map(|_| try_acquire_udp_task(&limit).expect("within task bound"))
            .collect();

        assert_eq!(limit.available_permits(), 0);
        assert!(try_acquire_udp_task(&limit).is_none());

        drop(permits.pop());
        assert!(try_acquire_udp_task(&limit).is_some());
    }

    #[tokio::test]
    async fn tun_udp_response_queue_holds_exactly_the_configured_bound() {
        let (tx, mut rx) = tun_udp_response_queue();
        let addr: SocketAddr = "127.0.0.1:53".parse().unwrap();

        for index in 0..MAX_TUN_UDP_TASKS {
            tx.try_send((vec![index as u8], addr, addr)).unwrap();
        }
        assert!(matches!(
            tx.try_send((vec![0], addr, addr)),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_))
        ));
        assert_eq!(rx.recv().await.unwrap().0, vec![0]);
        tx.try_send((vec![255], addr, addr)).unwrap();
        assert_eq!(rx.len(), MAX_TUN_UDP_TASKS);
    }

    #[tokio::test]
    async fn udp_response_queue_applies_backpressure() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        tx.send((vec![1], "local", "remote")).await.unwrap();

        let blocked_send = tokio::spawn(async move { tx.send((vec![2], "local", "remote")).await });
        tokio::task::yield_now().await;
        assert!(!blocked_send.is_finished());

        assert_eq!(rx.recv().await.unwrap().0, vec![1]);
        blocked_send.await.unwrap().unwrap();
        assert_eq!(rx.recv().await.unwrap().0, vec![2]);
    }

    #[tokio::test]
    async fn udp_response_queue_reports_closed_receiver() {
        let (tx, rx) = tokio::sync::mpsc::channel::<(Vec<u8>, &str, &str)>(1);
        drop(rx);

        assert!(tx.send((vec![3], "local", "remote")).await.is_err());
    }

    #[tokio::test]
    async fn udp_response_timeout_expires_and_next_datagram_gets_a_fresh_window() {
        let (mut writer, mut reader) = tokio::io::duplex(16);
        let timeout = Duration::from_millis(40);

        // The per-datagram response wait expires when no response arrives.
        assert!(read_udp_response(&mut reader, timeout).await.is_err());

        // A subsequent datagram starts a new window; delayed data is still accepted.
        let response = async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            tokio::io::AsyncWriteExt::write_all(&mut writer, &[0, 3, b'o', b'k', b'!'])
                .await
                .unwrap();
        };
        let (result, ()) = tokio::join!(read_udp_response(&mut reader, timeout), response);
        assert_eq!(result.unwrap(), b"ok!");
    }

    #[tokio::test]
    async fn udp_response_timeout_bounds_partial_header_and_payload() {
        let timeout = Duration::from_millis(25);

        // A partial length prefix must not wait indefinitely.
        let (mut writer, mut reader) = tokio::io::duplex(16);
        tokio::io::AsyncWriteExt::write_all(&mut writer, &[0])
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(
                Duration::from_millis(100),
                read_udp_response(&mut reader, timeout),
            )
            .await
            .unwrap()
            .is_err()
        );

        // Once the length is known, a partial payload shares the same response budget.
        let (mut writer, mut reader) = tokio::io::duplex(16);
        tokio::io::AsyncWriteExt::write_all(&mut writer, &[0, 3, b'x'])
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(
                Duration::from_millis(100),
                read_udp_response(&mut reader, timeout),
            )
            .await
            .unwrap()
            .is_err()
        );
    }

    #[tokio::test]
    async fn udp_response_zero_timeout_expires_at_entry() {
        let (_writer, mut reader) = tokio::io::duplex(16);
        assert!(
            read_udp_response(&mut reader, Duration::ZERO)
                .await
                .is_err()
        );
    }

    #[test]
    fn userspace_netstack_builds_with_common_tun_mtu_values() {
        // Match the values commonly used for direct Ethernet, IPv6-minimum,
        // and conservative tunnel paths. This verifies MTU plumbing/building;
        // netstack-smoltcp does not enable IP fragmentation.
        for mtu in [576_u16, 1280, 1400, 1500] {
            let result = StackBuilder::default()
                .enable_tcp(true)
                .enable_udp(true)
                .enable_icmp(false)
                .mtu(usize::from(mtu))
                .build();

            assert!(result.is_ok(), "netstack failed to build at MTU {mtu}");
        }
    }

    #[test]
    fn ipv4_checksum_is_filled_and_detects_corruption() {
        use netstack_smoltcp::smoltcp::wire::{IpProtocol, Ipv4Address, Ipv4Packet, Ipv4Repr};

        let repr = Ipv4Repr {
            src_addr: Ipv4Address::new(10, 0, 0, 1),
            dst_addr: Ipv4Address::new(10, 0, 0, 2),
            next_header: IpProtocol::Udp,
            payload_len: 8,
            hop_limit: 64,
        };
        let mut bytes = vec![0; repr.buffer_len()];
        let mut packet = Ipv4Packet::new_unchecked(&mut bytes);
        repr.emit(&mut packet, &Default::default());

        assert!(
            packet.verify_checksum(),
            "emitted IPv4 header checksum must be valid"
        );
        packet.set_hop_limit(63);
        assert!(
            !packet.verify_checksum(),
            "mutating a checksummed header must be detected"
        );
    }

    #[test]
    fn udp_checksum_rules_cover_ipv4_ipv6_zero_and_corruption() {
        use netstack_smoltcp::smoltcp::wire::{
            IpAddress, Ipv4Address, Ipv6Address, UdpPacket,
        };

        let v4_src = IpAddress::Ipv4(Ipv4Address::new(192, 0, 2, 1));
        let v4_dst = IpAddress::Ipv4(Ipv4Address::new(198, 51, 100, 2));
        let v6_src = IpAddress::Ipv6(Ipv6Address::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        let v6_dst = IpAddress::Ipv6(Ipv6Address::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2));

        // A zero checksum is accepted only for IPv4's optional checksum.
        let mut zero = [0_u8; 8];
        zero[4..6].copy_from_slice(&8_u16.to_be_bytes());
        let packet = UdpPacket::new_checked(&zero[..]).unwrap();
        assert!(udp_checksum_valid(&packet, &v4_src, &v4_dst));
        assert!(!udp_checksum_valid(&packet, &v6_src, &v6_dst));

        // A non-zero checksum must match the address-family pseudo-header.
        let mut valid_v4 = zero;
        UdpPacket::new_unchecked(&mut valid_v4[..]).fill_checksum(&v4_src, &v4_dst);
        let packet = UdpPacket::new_checked(&valid_v4[..]).unwrap();
        assert!(udp_checksum_valid(&packet, &v4_src, &v4_dst));
        let mut corrupt = valid_v4;
        corrupt[7] ^= 1;
        let packet = UdpPacket::new_checked(&corrupt[..]).unwrap();
        assert!(!udp_checksum_valid(&packet, &v4_src, &v4_dst));

        let mut valid_v6 = zero;
        UdpPacket::new_unchecked(&mut valid_v6[..]).fill_checksum(&v6_src, &v6_dst);
        let packet = UdpPacket::new_checked(&valid_v6[..]).unwrap();
        assert_ne!(packet.checksum(), 0);
        assert!(udp_checksum_valid(&packet, &v6_src, &v6_dst));
        let mut corrupt = valid_v6;
        corrupt[6] ^= 1;
        let packet = UdpPacket::new_checked(&corrupt[..]).unwrap();
        assert!(!udp_checksum_valid(&packet, &v6_src, &v6_dst));
    }
}
