use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::protocol::framing::{DEFAULT_STEALTH_FRAME_SIZE, DEFAULT_STEALTH_TICK_MS};

pub(super) fn default_clock_skew_secs() -> i64 {
    30
}

pub(super) fn default_puzzle_bits() -> u8 {
    12
}

pub(super) fn default_max_padding() -> usize {
    64
}

pub(super) fn default_padding_chance_percent() -> u8 {
    35
}

pub(super) fn default_backpressure_threshold_ms() -> u64 {
    40
}

pub(super) fn default_backpressure_cooldown_ms() -> u64 {
    1000
}

pub(super) fn default_tunnel_buffer() -> usize {
    1024 * 1024
}

pub(super) fn default_tunnel_pool_min_connections() -> usize {
    1
}

pub(super) fn default_tunnel_pool_max_connections() -> usize {
    4
}

pub(super) fn default_tunnel_pool_interactive_lanes() -> usize {
    1
}

pub(super) fn default_tunnel_pool_bulk_lanes() -> usize {
    2
}

pub(super) fn default_user_quota_window_secs() -> u64 {
    24 * 60 * 60
}

pub(super) fn default_tcp_nodelay() -> bool {
    true
}

pub(super) fn default_tcp_keepalive_secs() -> u64 {
    30
}

pub(super) fn default_tcp_heartbeat_secs() -> u64 {
    30
}

pub(super) fn default_pacing_enabled() -> bool {
    true
}

pub(super) fn default_pacing_burst_bytes() -> usize {
    64 * 1024
}

pub(super) fn default_pacing_min_write_bytes() -> usize {
    1024
}

pub(super) fn default_socks5_listen() -> Option<SocketAddr> {
    Some("127.0.0.1:6680".parse().expect("valid address"))
}

pub(super) fn default_http_listen() -> Option<SocketAddr> {
    Some("127.0.0.1:6681".parse().expect("valid address"))
}

pub(super) fn default_handshake_padding() -> usize {
    256
}

pub(super) fn default_tun_name() -> String {
    "esptun0".to_string()
}

pub(super) fn default_tun_address() -> Ipv4Addr {
    Ipv4Addr::new(10, 255, 0, 2)
}

pub(super) fn default_tun_destination() -> Ipv4Addr {
    Ipv4Addr::new(10, 255, 0, 1)
}

pub(super) fn default_tun_prefix() -> u8 {
    24
}

pub(super) fn default_tun_mtu() -> u16 {
    1500
}

pub(super) fn default_tun_protect_server_route() -> bool {
    true
}

pub(super) fn default_tun_dns_servers() -> Vec<IpAddr> {
    vec![
        IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
    ]
}

pub(super) fn default_remote_listen() -> SocketAddr {
    "0.0.0.0:6690".parse().expect("valid address")
}

pub(super) fn default_handshake_timeout_ms() -> u64 {
    3000
}

pub(super) fn default_max_handshake_padding() -> usize {
    1024
}

pub(super) fn default_replay_window_secs() -> i64 {
    60
}

pub(super) fn default_cold_start_delay_ms() -> u64 {
    35
}

pub(super) fn default_tarpit_max() -> usize {
    1024
}

pub(super) fn default_tarpit_hold_secs() -> u64 {
    300
}

pub(super) fn default_fallback_probe_timeout_ms() -> u64 {
    250
}

pub(super) fn default_fallback_server() -> String {
    "nginx".to_string()
}

pub(super) fn default_fallback_body() -> String {
    "<html><head><title>It works</title></head><body><h1>It works</h1></body></html>".to_string()
}

pub(super) fn default_log_level() -> String {
    "info".to_string()
}

pub(super) fn default_log_ansi() -> bool {
    true
}

pub(super) fn default_idle_timeout_secs() -> u64 {
    300
}

pub(super) fn default_max_streams() -> u32 {
    256
}

pub(super) fn default_native_mux_initial_window_bytes() -> usize {
    1024 * 1024
}

pub(super) fn default_native_mux_stream_buffer_frames() -> usize {
    128
}

pub(super) fn default_native_mux_idle_timeout_secs() -> u64 {
    300
}

pub(super) fn default_tunnel_pool_max_reconnect_attempts() -> u32 {
    3
}

pub(super) fn default_randomize_chunks() -> bool {
    true
}

pub(super) fn default_min_chunk() -> usize {
    4 * 1024
}

pub(super) fn default_max_chunk() -> usize {
    16 * 1024
}

pub(super) fn default_stealth_frame_size() -> usize {
    DEFAULT_STEALTH_FRAME_SIZE
}

pub(super) fn default_stealth_tick_ms() -> u64 {
    DEFAULT_STEALTH_TICK_MS
}
