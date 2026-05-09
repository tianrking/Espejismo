use std::env;
use std::net::SocketAddr;

use anyhow::{Context, Result};
use clap::Parser;
use espejismo_core::config::{encode_config_base64, example_config};
use espejismo_core::{
    connect_handshake, http_proxy, load_config, parse_psk, socks5, spawn_frame_transport,
    ConfigInput, EspejismoConfig, FrameOptions, HandshakeConfig, ProxyAuth,
};
use futures::StreamExt;
use tokio::io::{copy_bidirectional, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_yamux::{Config as YamuxConfig, Control, Session};
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
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

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

    let config = load_config(ConfigInput {
        path: args.config.clone(),
        base64: args.config_base64.clone(),
    })?;
    let runtime = build_runtime(config, &args)?;

    let control = connect_mux(
        runtime.server,
        runtime.handshake,
        runtime.frames,
        runtime.tunnel_buffer,
    )
    .await?;

    let mut listeners = Vec::new();
    if let Some(addr) = runtime.socks5_listen {
        let listener = TcpListener::bind(addr).await?;
        let control = control.clone();
        let auth = runtime.auth.clone();
        listeners.push(tokio::spawn(async move {
            info!(listen = %addr, "SOCKS5 proxy listening");
            loop {
                let (socket, peer) = listener.accept().await?;
                let control = control.clone();
                let auth = auth.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_socks5_client(socket, control, auth).await {
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
        let control = control.clone();
        let auth = runtime.auth.clone();
        listeners.push(tokio::spawn(async move {
            info!(listen = %addr, "HTTP proxy listening");
            loop {
                let (socket, peer) = listener.accept().await?;
                let control = control.clone();
                let auth = auth.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_http_client(socket, control, auth).await {
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
    info!(server = %runtime.server, "local connected with yamux tunnel");

    futures::future::try_join_all(listeners).await?;
    Ok(())
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
        handshake: HandshakeConfig {
            psk: parse_psk(&psk)?,
            clock_skew_secs: args
                .clock_skew_secs
                .unwrap_or(config.shared.clock_skew_secs),
            max_handshake_padding: args
                .handshake_padding
                .unwrap_or(config.local.handshake_padding),
            puzzle_difficulty_bits: args.puzzle_bits.unwrap_or(config.shared.puzzle_bits),
        },
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
    })
}

async fn connect_mux(
    server: SocketAddr,
    cfg: HandshakeConfig,
    options: FrameOptions,
    tunnel_buffer: usize,
) -> Result<Control> {
    let mut upstream = TcpStream::connect(server).await?;
    let keys = connect_handshake(&mut upstream, &cfg).await?;
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
    });

    Ok(control)
}

async fn handle_socks5_client(
    mut local: TcpStream,
    mut control: Control,
    auth: Option<ProxyAuth>,
) -> Result<()> {
    let target = socks5::accept_connect_with_auth(&mut local, auth.as_ref()).await?;
    let mut stream = control.open_stream().await?;
    write_target(&mut stream, &target.authority()).await?;
    copy_bidirectional(&mut local, &mut stream).await?;
    Ok(())
}

async fn handle_http_client(
    mut local: TcpStream,
    mut control: Control,
    auth: Option<ProxyAuth>,
) -> Result<()> {
    let target = http_proxy::accept_http_proxy_with_auth(&mut local, auth.as_ref()).await?;
    let mut stream = control.open_stream().await?;
    write_target(&mut stream, &target.authority).await?;
    if !target.prebuffer.is_empty() {
        stream.write_all(&target.prebuffer).await?;
    }
    copy_bidirectional(&mut local, &mut stream).await?;
    Ok(())
}

async fn write_target<W>(writer: &mut W, target: &str) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let bytes = target.as_bytes();
    anyhow::ensure!(
        bytes.len() <= u16::MAX as usize,
        "target authority too long"
    );
    writer.write_u16(bytes.len() as u16).await?;
    writer.write_all(bytes).await?;
    Ok(())
}
