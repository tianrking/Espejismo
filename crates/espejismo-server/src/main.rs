use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use espejismo_core::{
    accept_handshake_with_replay, parse_psk, spawn_frame_transport, FrameOptions, HandshakeConfig,
    ReplayCache,
};
use futures::StreamExt;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout, Duration};
use tokio_yamux::{Config as YamuxConfig, Session, StreamHandle};
use tracing::{debug, info};

#[derive(Parser, Debug)]
#[command(name = "espejismo-remote")]
struct Args {
    #[arg(long, default_value = "0.0.0.0:8443")]
    listen: SocketAddr,
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
    #[arg(long, default_value_t = 3000)]
    handshake_timeout_ms: u64,
    #[arg(long, default_value_t = 0)]
    reject_delay_ms: u64,
    #[arg(long, default_value_t = 1024)]
    max_handshake_padding: usize,
    #[arg(long, default_value_t = 60)]
    replay_window_secs: i64,
    #[arg(long, default_value_t = 12)]
    puzzle_bits: u8,
    #[arg(long, default_value_t = 1024 * 1024)]
    tunnel_buffer: usize,
    #[arg(long, default_value_t = 35)]
    cold_start_delay_ms: u64,
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
        max_handshake_padding: args.max_handshake_padding,
        puzzle_difficulty_bits: args.puzzle_bits,
    };
    let frame_options = FrameOptions {
        max_padding: args.max_padding,
        jitter_ms: args.jitter_ms,
        padding_chance_percent: args.padding_chance_percent,
        backpressure_threshold_ms: args.backpressure_threshold_ms,
        backpressure_cooldown_ms: args.backpressure_cooldown_ms,
    };

    let listener = TcpListener::bind(args.listen).await?;
    let replay = Arc::new(tokio::sync::Mutex::new(ReplayCache::new(
        args.replay_window_secs,
    )));
    info!(listen = %args.listen, "remote listening with yamux tunnel support");

    loop {
        let (socket, peer) = listener.accept().await?;
        let cfg = cfg.clone();
        let options = frame_options.clone();
        let replay = replay.clone();
        let handshake_timeout = Duration::from_millis(args.handshake_timeout_ms);
        let reject_delay = Duration::from_millis(args.reject_delay_ms.min(10_000));
        let tunnel_buffer = args.tunnel_buffer;
        let cold_start_delay = Duration::from_millis(args.cold_start_delay_ms);
        tokio::spawn(async move {
            if let Err(err) = handle_peer(
                socket,
                cfg,
                options,
                handshake_timeout,
                reject_delay,
                replay,
                tunnel_buffer,
                cold_start_delay,
            )
            .await
            {
                debug!(%peer, error = %err, "remote peer ended");
            }
        });
    }
}

async fn handle_peer(
    mut inbound: TcpStream,
    cfg: HandshakeConfig,
    options: FrameOptions,
    handshake_timeout: Duration,
    reject_delay: Duration,
    replay: Arc<tokio::sync::Mutex<ReplayCache>>,
    tunnel_buffer: usize,
    cold_start_delay: Duration,
) -> Result<()> {
    let keys = match timeout(
        handshake_timeout,
        accept_handshake_with_replay(&mut inbound, &cfg, replay),
    )
    .await
    {
        Ok(Ok(keys)) => keys,
        Ok(Err(err)) => {
            quiet_reject(inbound, reject_delay).await;
            return Err(err);
        }
        Err(err) => {
            quiet_reject(inbound, reject_delay).await;
            return Err(err.into());
        }
    };

    if !cold_start_delay.is_zero() {
        sleep(cold_start_delay).await;
    }

    let transport = spawn_frame_transport(inbound, keys, options, tunnel_buffer);
    let mut session = Session::new_server(transport, YamuxConfig::default());
    while let Some(stream) = session.next().await {
        let stream = stream?;
        tokio::spawn(async move {
            if let Err(err) = handle_mux_stream(stream).await {
                debug!(error = %err, "mux stream ended");
            }
        });
    }
    Ok(())
}

async fn handle_mux_stream(mut stream: StreamHandle) -> Result<()> {
    let target = read_target(&mut stream).await?;
    let mut remote = TcpStream::connect(&target)
        .await
        .with_context(|| format!("connect {target}"))?;
    info!(%target, "mux relay opened");
    copy_bidirectional(&mut stream, &mut remote).await?;
    Ok(())
}

async fn read_target<R>(reader: &mut R) -> Result<String>
where
    R: AsyncReadExt + Unpin,
{
    let len = reader.read_u16().await? as usize;
    anyhow::ensure!(len > 0, "empty target authority");
    let mut bytes = vec![0_u8; len];
    reader.read_exact(&mut bytes).await?;
    Ok(String::from_utf8(bytes)?)
}

async fn quiet_reject(mut stream: TcpStream, delay: Duration) {
    if !delay.is_zero() {
        sleep(delay).await;
    }
    let _ = stream.shutdown().await;
}
