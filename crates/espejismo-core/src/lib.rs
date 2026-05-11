pub mod admin;
pub mod config;
pub mod crypto;
pub mod egress;
pub mod ingress;
pub mod logging;
pub mod metrics;
pub mod profile;
pub mod protocol;
pub mod transport;
pub mod updater;

pub use admin::{spawn_admin_server, AdminAction, AdminState};
pub use config::{
    load_config, parse_config, AdminConfig, ConfigInput, EgressConfig, EspejismoConfig, LogConfig,
    LogFormat, ObfuscationConfig, ProbeDefenseMode,
};
pub use crypto::{
    accept_handshake, accept_handshake_with_replay, accept_handshake_with_users, connect_handshake,
    parse_psk, AuthenticatedSession, HandshakeConfig, HandshakeUser, SessionKeys,
};
pub use egress::{split_authority, EgressPolicy};
pub use ingress::{http_proxy, socks5, ProxyAuth};
pub use logging::{init_logging, LogGuard};
pub use metrics::{Metrics, MetricsSnapshot};
pub use profile::{decode_profile_url, encode_profile_url, ClientProfile};
pub use protocol::framing::{
    copy_encrypted, read_frame, send_frame, Frame, FrameCodec, FrameOptions, FrameReader,
    FrameType, FrameWriter, ObfuscationProfile,
};
pub use protocol::puzzle::PuzzleConfig;
pub use protocol::replay::ReplayCache;
pub use protocol::request::{
    read_tunnel_request, write_tcp_connect, write_udp_datagram, TunnelRequest, CMD_TCP_CONNECT,
    CMD_UDP_DATAGRAM,
};
pub use transport::{idle_copy_bidirectional, spawn_frame_transport};
pub use updater::{check_for_update, default_release_url, UpdateInfo};
