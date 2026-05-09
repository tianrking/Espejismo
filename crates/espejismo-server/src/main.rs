use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use espejismo_core::config::{encode_config_base64, example_config};
use espejismo_core::{
    accept_handshake_with_replay, load_config, parse_psk, spawn_frame_transport, ConfigInput,
    EspejismoConfig, FrameOptions, HandshakeConfig, ReplayCache,
};
use futures::StreamExt;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout, Duration};
use tokio_yamux::{Config as YamuxConfig, Session, StreamHandle};
use tracing::{debug, info};

mod tarpit;

#[derive(Parser, Debug)]
#[command(name = "espejismo-remote")]
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
    listen: Option<SocketAddr>,
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
    handshake_timeout_ms: Option<u64>,
    #[arg(long)]
    reject_delay_ms: Option<u64>,
    #[arg(long)]
    max_handshake_padding: Option<usize>,
    #[arg(long)]
    replay_window_secs: Option<i64>,
    #[arg(long)]
    puzzle_bits: Option<u8>,
    #[arg(long)]
    tunnel_buffer: Option<usize>,
    #[arg(long)]
    cold_start_delay_ms: Option<u64>,
    #[arg(long)]
    tarpit_max: Option<usize>,
    #[arg(long)]
    tarpit_hold_secs: Option<u64>,
}

#[derive(Clone)]
struct RemoteRuntime {
    listen: SocketAddr,
    handshake: HandshakeConfig,
    frames: FrameOptions,
    handshake_timeout: Duration,
    reject_delay: Duration,
    replay_window_secs: i64,
    tunnel_buffer: usize,
    cold_start_delay: Duration,
    tarpit_max: usize,
    tarpit_hold: Duration,
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

    let listener = TcpListener::bind(runtime.listen).await?;
    let tarpit = tarpit::TarpitManager::spawn(runtime.tarpit_max, runtime.tarpit_hold);
    let replay = Arc::new(tokio::sync::Mutex::new(ReplayCache::new(
        runtime.replay_window_secs,
    )));
    info!(listen = %runtime.listen, "remote listening with yamux tunnel support");

    loop {
        let (socket, peer) = listener.accept().await?;
        let replay = replay.clone();
        let runtime = runtime.clone();
        let tarpit = tarpit.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_peer(socket, runtime, replay, tarpit).await {
                debug!(%peer, error = %err, "remote peer ended");
            }
        });
    }
}

fn build_runtime(config: EspejismoConfig, args: &Args) -> Result<RemoteRuntime> {
    let psk = args
        .psk
        .clone()
        .or(config.shared.psk)
        .context("provide psk in config, --psk, or ESPEJISMO_PSK")?;

    Ok(RemoteRuntime {
        listen: args.listen.unwrap_or(config.remote.listen),
        handshake: HandshakeConfig {
            psk: parse_psk(&psk)?,
            clock_skew_secs: args
                .clock_skew_secs
                .unwrap_or(config.shared.clock_skew_secs),
            max_handshake_padding: args
                .max_handshake_padding
                .unwrap_or(config.remote.max_handshake_padding),
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
        handshake_timeout: Duration::from_millis(
            args.handshake_timeout_ms
                .unwrap_or(config.remote.handshake_timeout_ms),
        ),
        reject_delay: Duration::from_millis(
            args.reject_delay_ms
                .unwrap_or(config.remote.reject_delay_ms)
                .min(10_000),
        ),
        replay_window_secs: args
            .replay_window_secs
            .unwrap_or(config.remote.replay_window_secs),
        tunnel_buffer: args.tunnel_buffer.unwrap_or(config.shared.tunnel_buffer),
        cold_start_delay: Duration::from_millis(
            args.cold_start_delay_ms
                .unwrap_or(config.remote.cold_start_delay_ms),
        ),
        tarpit_max: args.tarpit_max.unwrap_or(config.remote.tarpit_max),
        tarpit_hold: Duration::from_secs(
            args.tarpit_hold_secs
                .unwrap_or(config.remote.tarpit_hold_secs),
        ),
    })
}

async fn handle_peer(
    mut inbound: TcpStream,
    runtime: RemoteRuntime,
    replay: Arc<tokio::sync::Mutex<ReplayCache>>,
    tarpit: tarpit::TarpitManager,
) -> Result<()> {
    let keys = match timeout(
        runtime.handshake_timeout,
        accept_handshake_with_replay(&mut inbound, &runtime.handshake, replay),
    )
    .await
    {
        Ok(Ok(keys)) => keys,
        Ok(Err(err)) => {
            reject_or_quarantine(inbound, runtime.reject_delay, &tarpit).await;
            return Err(err);
        }
        Err(err) => {
            reject_or_quarantine(inbound, runtime.reject_delay, &tarpit).await;
            return Err(err.into());
        }
    };

    if !runtime.cold_start_delay.is_zero() {
        sleep(runtime.cold_start_delay).await;
    }

    let transport = spawn_frame_transport(inbound, keys, runtime.frames, runtime.tunnel_buffer);
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

async fn reject_or_quarantine(
    stream: TcpStream,
    reject_delay: Duration,
    tarpit: &tarpit::TarpitManager,
) {
    if reject_delay.is_zero() {
        tarpit.quarantine(stream).await;
    } else {
        quiet_reject(stream, reject_delay).await;
    }
}

async fn quiet_reject(mut stream: TcpStream, delay: Duration) {
    if !delay.is_zero() {
        sleep(delay).await;
    }
    let _ = stream.shutdown().await;
}
