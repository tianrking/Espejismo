use std::net::SocketAddr;

use anyhow::{bail, Context, Result};
use clap::Parser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, warn};

const HEADER_LIMIT: usize = 64 * 1024;
const DEFAULT_CHUNK_BYTES: usize = 256 * 1024;

#[derive(Debug, Parser)]
#[command(about = "Small Rust HTTP source/sink for Espejismo throughput tests")]
struct Args {
    #[arg(long, default_value = "0.0.0.0:18082")]
    listen: SocketAddr,
    #[arg(long, default_value_t = 256)]
    default_download_mib: u64,
    #[arg(long, default_value_t = 4096)]
    max_upload_mib: u64,
    #[arg(long, default_value_t = DEFAULT_CHUNK_BYTES)]
    chunk_bytes: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();
    let listener = TcpListener::bind(args.listen)
        .await
        .with_context(|| format!("bind {}", args.listen))?;
    info!(
        listen = %args.listen,
        default_download_mib = args.default_download_mib,
        max_upload_mib = args.max_upload_mib,
        "benchmark HTTP source/sink listening"
    );

    loop {
        let (stream, peer) = listener.accept().await?;
        let settings = BenchSettings {
            default_download_bytes: args.default_download_mib.saturating_mul(1024 * 1024),
            max_upload_bytes: args.max_upload_mib.saturating_mul(1024 * 1024),
            chunk_bytes: args.chunk_bytes.clamp(1024, 1024 * 1024),
        };
        tokio::spawn(async move {
            if let Err(err) = handle_peer(stream, settings).await {
                debug!(%peer, error = %err, "benchmark HTTP request ended");
            }
        });
    }
}

#[derive(Clone, Copy)]
struct BenchSettings {
    default_download_bytes: u64,
    max_upload_bytes: u64,
    chunk_bytes: usize,
}

struct Request {
    method: String,
    path: String,
    content_length: Option<u64>,
    prebuffer: Vec<u8>,
}

async fn handle_peer(mut stream: TcpStream, settings: BenchSettings) -> Result<()> {
    stream.set_nodelay(true)?;
    let request = read_request(&mut stream).await?;
    match request.method.as_str() {
        "GET" | "HEAD" => {
            let bytes = download_size(&request.path).unwrap_or(settings.default_download_bytes);
            send_download(&mut stream, &request.method, bytes, settings.chunk_bytes).await
        }
        "POST" | "PUT" => receive_upload(&mut stream, request, settings).await,
        _ => {
            send_response(
                &mut stream,
                "405 Method Not Allowed",
                "text/plain",
                b"method not allowed\n",
            )
            .await
        }
    }
}

async fn read_request(stream: &mut TcpStream) -> Result<Request> {
    let mut buf = Vec::with_capacity(4096);
    let header_end = loop {
        if buf.len() > HEADER_LIMIT {
            bail!("HTTP header too large");
        }
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        let mut chunk = [0_u8; 4096];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            bail!("connection closed before HTTP headers");
        }
        buf.extend_from_slice(&chunk[..n]);
    };

    let header = std::str::from_utf8(&buf[..header_end]).context("HTTP header is not UTF-8")?;
    let mut lines = header.split("\r\n");
    let request_line = lines.next().context("missing request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().context("missing method")?.to_ascii_uppercase();
    let path = parts.next().context("missing path")?.to_string();
    let mut content_length = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            content_length = Some(value.trim().parse().context("parse Content-Length")?);
        }
    }

    Ok(Request {
        method,
        path,
        content_length,
        prebuffer: buf[header_end + 4..].to_vec(),
    })
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|window| window == b"\r\n\r\n")
}

fn download_size(path: &str) -> Option<u64> {
    if let Some(bytes) = path.strip_prefix("/bytes/") {
        return bytes.parse().ok();
    }
    let name = path
        .trim_start_matches('/')
        .split('?')
        .next()
        .unwrap_or(path);
    let mib = name.strip_suffix("m.bin")?;
    mib.parse::<u64>().ok()?.checked_mul(1024 * 1024)
}

async fn send_download(
    stream: &mut TcpStream,
    method: &str,
    bytes: u64,
    chunk_bytes: usize,
) -> Result<()> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {bytes}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(headers.as_bytes()).await?;
    if method == "HEAD" {
        stream.shutdown().await?;
        return Ok(());
    }

    let zeros = vec![0_u8; chunk_bytes];
    let mut remaining = bytes;
    while remaining > 0 {
        let take = remaining.min(zeros.len() as u64) as usize;
        stream.write_all(&zeros[..take]).await?;
        remaining -= take as u64;
    }
    stream.shutdown().await?;
    Ok(())
}

async fn receive_upload(
    stream: &mut TcpStream,
    request: Request,
    settings: BenchSettings,
) -> Result<()> {
    let expected = request
        .content_length
        .context("upload requires Content-Length")?;
    if expected > settings.max_upload_bytes {
        warn!(
            expected,
            max_upload_bytes = settings.max_upload_bytes,
            "benchmark upload rejected"
        );
        return send_response(
            stream,
            "413 Payload Too Large",
            "text/plain",
            b"payload too large\n",
        )
        .await;
    }

    let mut received = request.prebuffer.len().min(expected as usize) as u64;
    let mut buf = vec![0_u8; settings.chunk_bytes];
    while received < expected {
        let take = (expected - received).min(buf.len() as u64) as usize;
        let n = stream.read(&mut buf[..take]).await?;
        if n == 0 {
            bail!("connection closed during upload body");
        }
        received += n as u64;
    }

    let body = format!("{{\"bytes\":{received}}}\n");
    send_response(stream, "200 OK", "application/json", body.as_bytes()).await
}

async fn send_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.shutdown().await?;
    Ok(())
}
