pub mod admin;
pub mod cli_support;
pub mod config;
pub mod crypto;
pub mod egress;
pub mod extension;
pub mod ingress;
pub mod logging;
pub mod metrics;
pub mod mux;
pub mod profile;
pub mod protocol;
pub mod runtime_state;
pub mod tcp;
pub mod transport;
pub mod updater;

pub use admin::{spawn_admin_server, AdminAction, AdminState};
pub use cli_support::{apply_log_overrides, print_update_check, report_config_check, LogOverrides};
pub use config::{
    apply_named_profile, config_to_toml, encode_config_base64, load_config, load_config_base64,
    parse_config, AdminConfig, ConfigInput, EgressConfig, EspejismoConfig, FrameOptionOverrides,
    LogConfig, LogFormat, MuxConfig, MuxMode, ObfuscationConfig, PacingConfig, ProbeDefenseMode,
    TcpConfig, TunnelPoolConfig,
};
pub use crypto::{
    accept_handshake, accept_handshake_with_replay, accept_handshake_with_users, connect_handshake,
    parse_psk, AuthenticatedSession, HandshakeConfig, HandshakeUser, SessionKeys,
};
pub use egress::{split_authority, EgressPolicy, EgressProxy, EgressProxyKind};
pub use extension::{
    AuthDecision, AuthRequest, Authenticator, CommandAuthenticator, EgressRequest,
    HttpJsonAuthenticator, NoopTrafficObserver, OutboundConnector, RequestContext, RequestPolicy,
    TrafficEvent, TrafficObserver, TransportConnector, TransportStream, TransportTarget,
};
pub use ingress::{http_proxy, socks5, ProxyAuth};
pub use logging::{init_logging, LogGuard};
pub use metrics::{Metrics, MetricsSnapshot};
pub use mux::MuxRuntimeConfig;
pub use profile::{decode_profile_url, encode_profile_url, ClientProfile};
pub use protocol::framing::{
    copy_encrypted, read_frame, send_frame, ChunkPolicy, Frame, FrameCodec, FrameOptions,
    FrameReader, FrameType, FrameWriter, ObfuscationProfile,
};
pub use protocol::puzzle::PuzzleConfig;
pub use protocol::replay::ReplayCache;
pub use protocol::request::{
    read_tunnel_request, write_tcp_connect, write_tcp_connect_with_priority, write_udp_datagram,
    write_udp_datagram_with_priority, StreamPriority, TunnelRequest, CMD_TCP_CONNECT,
    CMD_UDP_DATAGRAM,
};
pub use protocol::udp::{
    DeliveredDatagram, UdpCongestionController, UdpPacket, UdpPacketKind, UdpReliability,
};
pub use runtime_state::{RuntimeState, RuntimeStateSnapshot, TunnelLaneSnapshot};
pub use tcp::{apply_tcp_options, bind_tcp_listener, connect_tcp_stream};
pub use transport::{
    idle_copy_bidirectional, metered_idle_copy_bidirectional, spawn_frame_transport, CopyMeter,
    NoopCopyMeter,
};
pub use updater::{check_for_update, default_release_url, UpdateInfo};
