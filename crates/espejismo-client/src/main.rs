use std::env;
use std::net::SocketAddr;

use anyhow::{Context, Result};
use clap::Parser;
use espejismo_core::{
    connect_handshake, parse_psk, socks5, spawn_frame_transport, FrameOptions, HandshakeConfig,
};
use futures::StreamExt;
use tokio::io::{copy_bidirectional, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_yamux::{Config as YamuxConfig, Control, Session};
use tracing::{debug, info};

#[derive(Parser, Debug)]
#[command(name = "espejismo-local")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:1080")]
    listen: SocketAddr,
    #[arg(long)]
    server: SocketAddr,
    #[arg(long, env = "ESPEJISMO_PSK")]
    psk: Option<String>,
    #[arg(long, default_value_t = 30)]
    clock_skew_secs: i64,
    #[arg(long, default_value_t = 64)]
    max_padding: usize,
    #[arg(long, default_value_t = 0)]
    jitter_ms: u64,
    #[arg(long, default_value_t = 35)]
    padding_chance_percent: u8,
    #[arg(long, default_value_t = 40)]
    backpressure_threshold_ms: u64,
    #[arg(long, default_value_t = 1000)]
    backpressure_cooldown_ms: u64,
    #[arg(long, default_value_t = 256)]
    handshake_padding: usize,
    #[arg(long, default_value_t = 12)]
    puzzle_bits: u8,
    #[arg(long, default_value_t = 1024 * 1024)]
    tunnel_buffer: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let args = Args::parse();
    let psk = args
        .psk
        .as_deref()
        .context("provide --psk or ESPEJISMO_PSK")?;
    let cfg = HandshakeConfig {
        psk: parse_psk(psk)?,
        clock_skew_secs: args.clock_skew_secs,
        max_handshake_padding: args.handshake_padding,
        puzzle_difficulty_bits: args.puzzle_bits,
    };
    let frame_options = FrameOptions {
        max_padding: args.max_padding,
        jitter_ms: args.jitter_ms,
        padding_chance_percent: args.padding_chance_percent,
        backpressure_threshold_ms: args.backpressure_threshold_ms,
        backpressure_cooldown_ms: args.backpressure_cooldown_ms,
    };

    let control = connect_mux(args.server, cfg, frame_options, args.tunnel_buffer).await?;
    let listener = TcpListener::bind(args.listen).await?;
    info!(listen = %args.listen, server = %args.server, "local listening with yamux tunnel");

    loop {
        let (socket, peer) = listener.accept().await?;
        let control = control.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_client(socket, control).await {
                debug!(%peer, error = %err, "local connection ended");
            }
        });
    }
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

async fn handle_client(mut local: TcpStream, mut control: Control) -> Result<()> {
    let target = socks5::accept_connect(&mut local).await?;
    let mut stream = control.open_stream().await?;
    write_target(&mut stream, &target.authority()).await?;
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
