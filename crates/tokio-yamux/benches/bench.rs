//! Loopback baselines for raw TCP and yamux. Each measured iteration transfers
//! one fixed payload; listener ports are ephemeral to avoid collisions.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use futures::StreamExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_yamux::{config::Config, session::Session};

const PAYLOAD_SIZE: usize = 512 * 1024;

fn rt() -> &'static tokio::runtime::Runtime {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().expect("create benchmark runtime"))
}

async fn start_tcp_echo() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut data = vec![0; PAYLOAD_SIZE];
                if socket.read_exact(&mut data).await.is_ok() {
                    let _ = socket.write_all(&data).await;
                }
            });
        }
    });
    addr
}

async fn start_yamux_echo() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut session = Session::new_server(socket, Config::default());
                while let Some(Ok(mut stream)) = session.next().await {
                    tokio::spawn(async move {
                        let mut data = vec![0; PAYLOAD_SIZE];
                        if stream.read_exact(&mut data).await.is_ok() {
                            let _ = stream.write_all(&data).await;
                        }
                    });
                }
            });
        }
    });
    addr
}

async fn tcp_roundtrip(addr: std::net::SocketAddr, data: &[u8]) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(data).await.unwrap();
    let mut response = vec![0; data.len()];
    stream.read_exact(&mut response).await.unwrap();
    assert_eq!(response, data);
}

async fn yamux_roundtrip(addr: std::net::SocketAddr, data: &[u8]) {
    let socket = TcpStream::connect(addr).await.unwrap();
    let mut session = Session::new_client(socket, Config::default());
    let mut stream = session.open_stream().unwrap();
    tokio::spawn(async move { while session.next().await.is_some() {} });
    stream.write_all(data).await.unwrap();
    let mut response = vec![0; data.len()];
    stream.read_exact(&mut response).await.unwrap();
    assert_eq!(response, data);
}

fn criterion_benchmark(c: &mut Criterion) {
    let payload = vec![0x5a; PAYLOAD_SIZE];
    let (tcp_addr, yamux_addr) =
        rt().block_on(async { (start_tcp_echo().await, start_yamux_echo().await) });

    let mut group = c.benchmark_group("loopback_echo_512k");
    group.bench_function(BenchmarkId::new("raw_tcp", "512_kib"), |b| {
        b.to_async(rt()).iter(|| tcp_roundtrip(tcp_addr, &payload));
    });
    group.bench_function(BenchmarkId::new("yamux", "512_kib"), |b| {
        b.to_async(rt())
            .iter(|| yamux_roundtrip(yamux_addr, &payload));
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
