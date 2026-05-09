pub mod admin;
pub mod config;
pub mod crypto;
pub mod egress;
pub mod ingress;
pub mod logging;
pub mod metrics;
pub mod protocol;
pub mod transport;

pub use admin::{spawn_admin_server, AdminState};
pub use config::{
    load_config, AdminConfig, ConfigInput, EgressConfig, EspejismoConfig, LogConfig, LogFormat,
};
pub use crypto::{
    accept_handshake, accept_handshake_with_replay, connect_handshake, parse_psk, HandshakeConfig,
    SessionKeys,
};
pub use egress::{split_authority, EgressPolicy};
pub use ingress::{http_proxy, socks5, ProxyAuth};
pub use logging::{init_logging, LogGuard};
pub use metrics::{Metrics, MetricsSnapshot};
pub use protocol::framing::{
    copy_encrypted, read_frame, send_frame, Frame, FrameCodec, FrameOptions, FrameReader,
    FrameType, FrameWriter,
};
pub use protocol::puzzle::PuzzleConfig;
pub use protocol::replay::ReplayCache;
pub use transport::spawn_frame_transport;
