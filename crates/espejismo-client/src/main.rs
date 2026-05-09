use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use espejismo_core::config::{encode_config_base64, example_config};
use espejismo_core::{
    connect_handshake, decode_profile_url, encode_profile_url, http_proxy, idle_copy_bidirectional,
    init_logging, load_config, parse_psk, socks5, spawn_admin_server, spawn_frame_transport,
    write_tcp_connect, write_udp_datagram, AdminState, ClientProfile, ConfigInput, EspejismoConfig,
    FrameOptions, HandshakeConfig, LogConfig, LogFormat, Metrics, ProxyAuth,
};
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_yamux::{Config as YamuxConfig, Control, Session, StreamHandle};
use tracing::{debug, info};

#[derive(Parser, Debug)]
#[command(name = "espejismo-local")]
struct Args {
    #[arg(long)]
    config: Option<String>,
    #[arg(long)]
    config_base64: Option<String>,
    #[arg(long)]
    print_example_config: bool,
    #[arg(long)]
    print_example_config_base64: bool,
    #[arg(long)]
    print_client_profile: bool,
    #[arg(long, default_value = "default")]
    profile_name: String,
    #[arg(long)]
    import_profile: Option<String>,
    #[arg(long)]
    socks5_listen: Option<SocketAddr>,
    #[arg(long)]
    http_listen: Option<SocketAddr>,
    #[arg(long)]
    server: Option<SocketAddr>,
    #[arg(long, env = "ESPEJISMO_PSK")]
    psk: Option<String>,
    #[arg(long)]
    clock_skew_secs: Option<i64>,
    #[arg(long)]
    max_padding: Option<usize>,
    #[arg(long)]
    jitter_ms: Option<u64>,
    #[arg(long)]
    padding_chance_percent: Option<u8>,
    #[arg(long)]
    backpressure_threshold_ms: Option<u64>,
    #[arg(long)]
    backpressure_cooldown_ms: Option<u64>,
    #[arg(long)]
    handshake_padding: Option<usize>,
    #[arg(long)]
    puzzle_bits: Option<u8>,
    #[arg(long)]
    tunnel_buffer: Option<usize>,
    #[arg(long)]
    log_level: Option<String>,
    #[arg(long)]
    log_format: Option<String>,
    #[arg(long)]
    log_file: Option<PathBuf>,
    #[arg(long)]
    no_log_ansi: bool,
    #[arg(long)]
    admin_listen: Option<SocketAddr>,
    #[arg(long)]
    admin_token: Option<String>,
}

#[derive(Clone)]
struct LocalRuntime {
    server: SocketAddr,
    socks5_listen: Option<SocketAddr>,
    http_listen: Option<SocketAddr>,
    handshake: HandshakeConfig,
    frames: FrameOptions,
    tunnel_buffer: usize,
    auth: Option<ProxyAuth>,
    admin_listen: Option<SocketAddr>,
    admin_token: Option<String>,
    idle_timeout: Duration,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.print_example_config || args.print_example_config_base64 {
        let example = example_config();
        if args.print_example_config_base64 {
            println!("{}", encode_config_base64(&example));
        } else {
            print!("{example}");
        }
        return Ok(());
    }

    let mut config = load_config(ConfigInput {
        path: args.config.clone(),
        base64: args.config_base64.clone(),
    })?;
    if let Some(profile_url) = &args.import_profile {
        decode_profile_url(profile_url)?.apply_to_config(&mut config);
    }
    if args.print_client_profile {
        let profile = ClientProfile::from_config(args.profile_name.clone(), &config)?;
        println!("{}", encode_profile_url(&profile)?);
        return Ok(());
    }
    apply_log_overrides(&mut config.logging, &args)?;
    let _log_guard = init_logging(&config.logging)?;
    let runtime = build_runtime(config, &args)?;
    let metrics = Metrics::default();
    if let Some(addr) = runtime.admin_listen {
        spawn_admin_server(
            addr,
            AdminState {
                role: "local".to_string(),
                metrics: metrics.clone(),
                token: runtime.admin_token.clone(),
            },
        );
    }

    let tunnel = Arc::new(TunnelManager::new(
        runtime.server,
        runtime.handshake,
        runtime.frames,
        runtime.tunnel_buffer,
        metrics.clone(),
    ));

    let mut listeners = Vec::new();
    if let Some(addr) = runtime.socks5_listen {
        let listener = TcpListener::bind(addr).await?;
        let tunnel = tunnel.clone();
        let auth = runtime.auth.clone();
        let metrics = metrics.clone();
        let idle = runtime.idle_timeout;
        listeners.push(tokio::spawn(async move {
            info!(listen = %addr, "SOCKS5 proxy listening");
            loop {
                let (socket, peer) = listener.accept().await?;
                let tunnel = tunnel.clone();
                let auth = auth.clone();
                let metrics = metrics.clone();
                metrics.inc_accepted();
                tokio::spawn(async move {
                    if let Err(err) =
                        handle_socks5_client(socket, tunnel, auth, metrics, idle).await
                    {
                        debug!(%peer, error = %err, "SOCKS5 connection ended");
                    }
                });
            }
            #[allow(unreachable_code)]
            Ok::<_, anyhow::Error>(())
        }));
    }

    if let Some(addr) = runtime.http_listen {
        let listener = TcpListener::bind(addr).await?;
        let tunnel = tunnel.clone();
        let auth = runtime.auth.clone();
        let metrics = metrics.clone();
        let idle = runtime.idle_timeout;
        listeners.push(tokio::spawn(async move {
            info!(listen = %addr, "HTTP proxy listening");
            loop {
                let (socket, peer) = listener.accept().await?;
                let tunnel = tunnel.clone();
                let auth = auth.clone();
                let metrics = metrics.clone();
                metrics.inc_accepted();
                tokio::spawn(async move {
                    if let Err(err) = handle_http_client(socket, tunnel, auth, metrics, idle).await
                    {
                        debug!(%peer, error = %err, "HTTP proxy connection ended");
                    }
                });
            }
            #[allow(unreachable_code)]
            Ok::<_, anyhow::Error>(())
        }));
    }

    anyhow::ensure!(
        !listeners.is_empty(),
        "enable at least one local listener: socks5_listen or http_listen"
    );
    info!(server = %runtime.server, "local proxy ready with reconnecting yamux tunnel manager");

    futures::future::try_join_all(listeners).await?;
    Ok(())
}

fn apply_log_overrides(config: &mut LogConfig, args: &Args) -> Result<()> {
    if let Some(level) = &args.log_level {
        config.level = level.clone();
    }
    if let Some(format) = &args.log_format {
        config.format = parse_log_format(format)?;
    }
    if let Some(file) = &args.log_file {
        config.file = Some(file.clone());
    }
    if args.no_log_ansi {
        config.ansi = false;
    }
    Ok(())
}

fn parse_log_format(format: &str) -> Result<LogFormat> {
    match format {
        "compact" => Ok(LogFormat::Compact),
        "pretty" => Ok(LogFormat::Pretty),
        "json" => Ok(LogFormat::Json),
        _ => anyhow::bail!("log format must be compact, pretty, or json"),
    }
}

fn build_runtime(config: EspejismoConfig, args: &Args) -> Result<LocalRuntime> {
    let psk = args
        .psk
        .clone()
        .or(config.shared.psk)
        .context("provide psk in config, --psk, or ESPEJISMO_PSK")?;
    let server = args
        .server
        .or(config.local.server)
        .context("provide local.server in config or --server")?;

    Ok(LocalRuntime {
        server,
        socks5_listen: args.socks5_listen.or(config.local.socks5_listen),
        http_listen: args.http_listen.or(config.local.http_listen),
        handshake: HandshakeConfig::new(
            parse_psk(&psk)?,
            args.clock_skew_secs
                .unwrap_or(config.shared.clock_skew_secs),
            args.handshake_padding
                .unwrap_or(config.local.handshake_padding),
            args.puzzle_bits.unwrap_or(config.shared.puzzle_bits),
        ),
        frames: FrameOptions {
            max_padding: args.max_padding.unwrap_or(config.shared.max_padding),
            jitter_ms: args.jitter_ms.unwrap_or(config.shared.jitter_ms),
            padding_chance_percent: args
                .padding_chance_percent
                .unwrap_or(config.shared.padding_chance_percent),
            backpressure_threshold_ms: args
                .backpressure_threshold_ms
                .unwrap_or(config.shared.backpressure_threshold_ms),
            backpressure_cooldown_ms: args
                .backpressure_cooldown_ms
                .unwrap_or(config.shared.backpressure_cooldown_ms),
        },
        tunnel_buffer: args.tunnel_buffer.unwrap_or(config.shared.tunnel_buffer),
        auth: config.local.auth,
        admin_listen: args.admin_listen.or(config.admin.listen),
        admin_token: args.admin_token.clone().or(config.admin.token),
        idle_timeout: Duration::from_secs(config.shared.idle_timeout_secs),
    })
}

#[derive(Clone)]
struct TunnelManager {
    server: SocketAddr,
    handshake: HandshakeConfig,
    frames: FrameOptions,
    tunnel_buffer: usize,
    metrics: Metrics,
    control: Arc<Mutex<Option<Control>>>,
}

impl TunnelManager {
    fn new(
        server: SocketAddr,
        handshake: HandshakeConfig,
        frames: FrameOptions,
        tunnel_buffer: usize,
        metrics: Metrics,
    ) -> Self {
        Self {
            server,
            handshake,
            frames,
            tunnel_buffer,
            metrics,
            control: Arc::new(Mutex::new(None)),
        }
    }

    async fn open_stream(&self) -> Result<StreamHandle> {
        let mut guard = self.control.lock().await;
        if guard.is_none() {
            *guard = Some(
                connect_mux(
                    self.server,
                    self.handshake.clone(),
                    self.frames.clone(),
                    self.tunnel_buffer,
                    self.metrics.clone(),
                )
                .await?,
            );
        }
        if let Some(control) = guard.as_mut() {
            match control.open_stream().await {
                Ok(stream) => return Ok(stream),
                Err(err) => {
                    debug!(error = %err, "yamux stream open failed; reconnecting tunnel");
                    *guard = None;
                }
            }
        }
        *guard = Some(
            connect_mux(
                self.server,
                self.handshake.clone(),
                self.frames.clone(),
                self.tunnel_buffer,
                self.metrics.clone(),
            )
            .await?,
        );
        guard
            .as_mut()
            .context("tunnel reconnect did not install control")?
            .open_stream()
            .await
            .context("open yamux stream after reconnect")
    }
}

async fn connect_mux(
    server: SocketAddr,
    cfg: HandshakeConfig,
    options: FrameOptions,
    tunnel_buffer: usize,
    metrics: Metrics,
) -> Result<Control> {
    let mut upstream = TcpStream::connect(server).await?;
    metrics.inc_active_physical();
    let keys = match connect_handshake(&mut upstream, &cfg).await {
        Ok(keys) => {
            metrics.inc_handshake_success();
            keys
        }
        Err(err) => {
            metrics.inc_handshake_failure();
            metrics.dec_active_physical();
            return Err(err);
        }
    };
    let transport = spawn_frame_transport(upstream, keys, options, tunnel_buffer);
    let mut session = Session::new_client(transport, YamuxConfig::default());
    let control = session.control();

    tokio::spawn(async move {
        while let Some(event) = session.next().await {
            if let Err(err) = event {
                debug!(error = %err, "yamux client session stopped");
                break;
            }
        }
        metrics.dec_active_physical();
    });

    Ok(control)
}

async fn handle_socks5_client(
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

async fn handle_http_client(
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
