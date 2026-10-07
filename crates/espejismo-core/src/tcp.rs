//! TCP listener and outbound connection helpers with configured socket options.

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, Result};
use socket2::{Domain, Protocol, SockAddr, SockRef, Socket, TcpKeepalive, Type};
use tokio::net::{TcpListener, TcpSocket, TcpStream};

use crate::config::TcpConfig;

pub async fn connect_tcp_stream(authority: &str, options: &TcpConfig) -> Result<TcpStream> {
    let mut last_error = None;
    for addr in crate::dns::resolve_socket_addrs(authority).await? {
        match connect_tcp_addr(addr, options).await {
            Ok(stream) => return Ok(stream),
            Err(err) => last_error = Some(err),
        }
    }
    match last_error {
        Some(err) => Err(err).with_context(|| format!("connect {authority}")),
        None => anyhow::bail!("resolve {authority}: no addresses"),
    }
}

async fn connect_tcp_addr(addr: SocketAddr, options: &TcpConfig) -> Result<TcpStream> {
    let socket = if addr.is_ipv4() {
        TcpSocket::new_v4()
    } else {
        TcpSocket::new_v6()
    }
    .with_context(|| format!("create TCP socket {addr}"))?;
    apply_tcp_socket_options(&socket, options)
        .with_context(|| format!("apply pre-connect TCP options to {addr}"))?;
    let stream = socket
        .connect(addr)
        .await
        .with_context(|| format!("connect {addr}"))?;
    apply_tcp_options(&stream, options).with_context(|| format!("apply TCP options to {addr}"))?;
    Ok(stream)
}

pub fn bind_tcp_listener(addr: SocketAddr, options: &TcpConfig) -> Result<TcpListener> {
    bind_tcp_listener_with_backlog(addr, options, 1024)
}

fn bind_tcp_listener_with_backlog(
    addr: SocketAddr,
    options: &TcpConfig,
    backlog: i32,
) -> Result<TcpListener> {
    let socket = Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP))
        .with_context(|| format!("create listener socket {addr}"))?;
    socket
        .set_reuse_address(true)
        .with_context(|| format!("set SO_REUSEADDR on {addr}"))?;
    apply_socket_buffer_options(&socket, options)?;
    socket
        .bind(&SockAddr::from(addr))
        .with_context(|| format!("bind {addr}"))?;
    socket
        // The OS may cap or reinterpret this hint; it is not an exact queue size.
        .listen(backlog)
        .with_context(|| format!("listen {addr}"))?;
    socket
        .set_nonblocking(true)
        .with_context(|| format!("set nonblocking {addr}"))?;
    let std_listener: std::net::TcpListener = socket.into();
    TcpListener::from_std(std_listener).with_context(|| format!("install tokio listener {addr}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    // Requires loopback TCP bind, unavailable in the Codex sandbox; the gate
    // skips it there. Run outside the sandbox with
    // `cargo test -p espejismo-core -- --ignored` to execute it.
    #[tokio::test]
    #[ignore = "requires loopback bind"]
    async fn listener_recovers_after_accept_queue_is_drained() {
        let listener = bind_tcp_listener_with_backlog(
            "127.0.0.1:0".parse().unwrap(),
            &TcpConfig::default(),
            1,
        )
        .unwrap();
        let addr = listener.local_addr().unwrap();

        // Hold clients open while the listener is deliberately not accepting. A
        // small backlog lets the kernel queue fill; exact overflow behavior varies
        // by OS, so bound each attempt and assert recovery after draining instead.
        let mut clients = Vec::new();
        for _ in 0..8 {
            if let Ok(Ok(client)) =
                timeout(Duration::from_millis(100), TcpStream::connect(addr)).await
            {
                clients.push(client);
            }
        }
        assert!(
            !clients.is_empty(),
            "at least one connection should enter the queue"
        );

        let mut accepted = 0;
        while let Ok(Ok((_stream, _peer))) =
            timeout(Duration::from_millis(100), listener.accept()).await
        {
            accepted += 1;
            if accepted == clients.len() {
                break;
            }
        }
        assert!(accepted > 0, "queued connections should remain acceptable");

        let follow_up = timeout(Duration::from_secs(1), TcpStream::connect(addr))
            .await
            .expect("listener should accept new connection after queue drain")
            .expect("follow-up connect should succeed");
        let _accepted = timeout(Duration::from_secs(1), listener.accept())
            .await
            .expect("follow-up connection should be accepted")
            .unwrap();
        drop((clients, follow_up));
    }
}

pub fn apply_tcp_options(stream: &TcpStream, options: &TcpConfig) -> Result<()> {
    stream.set_nodelay(options.nodelay)?;
    let sock = SockRef::from(stream);
    apply_sockref_buffer_options(&sock, options)?;
    if options.keepalive_secs > 0 {
        sock.set_keepalive(true)?;
        let keepalive = TcpKeepalive::new().with_time(Duration::from_secs(options.keepalive_secs));
        sock.set_tcp_keepalive(&keepalive)?;
    }
    apply_platform_tcp_options(&sock, options)?;
    Ok(())
}

fn apply_socket_buffer_options(socket: &Socket, options: &TcpConfig) -> io::Result<()> {
    if options.send_buffer_bytes > 0 {
        socket.set_send_buffer_size(options.send_buffer_bytes)?;
    }
    if options.recv_buffer_bytes > 0 {
        socket.set_recv_buffer_size(options.recv_buffer_bytes)?;
    }
    Ok(())
}

fn apply_tcp_socket_options(socket: &TcpSocket, options: &TcpConfig) -> io::Result<()> {
    socket.set_nodelay(options.nodelay)?;
    if options.keepalive_secs > 0 {
        socket.set_keepalive(true)?;
    }
    if options.send_buffer_bytes > 0 {
        socket.set_send_buffer_size(options.send_buffer_bytes as u32)?;
    }
    if options.recv_buffer_bytes > 0 {
        socket.set_recv_buffer_size(options.recv_buffer_bytes as u32)?;
    }
    Ok(())
}

fn apply_sockref_buffer_options(socket: &SockRef<'_>, options: &TcpConfig) -> io::Result<()> {
    if options.send_buffer_bytes > 0 {
        socket.set_send_buffer_size(options.send_buffer_bytes)?;
    }
    if options.recv_buffer_bytes > 0 {
        socket.set_recv_buffer_size(options.recv_buffer_bytes)?;
    }
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "fuchsia",
    target_os = "cygwin"
))]
fn apply_platform_tcp_options(socket: &SockRef<'_>, options: &TcpConfig) -> io::Result<()> {
    if options.user_timeout_ms > 0 {
        socket.set_tcp_user_timeout(Some(Duration::from_millis(options.user_timeout_ms)))?;
    }
    apply_congestion(socket, options)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "fuchsia",
    target_os = "cygwin"
)))]
fn apply_platform_tcp_options(_socket: &SockRef<'_>, _options: &TcpConfig) -> io::Result<()> {
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn apply_congestion(socket: &SockRef<'_>, options: &TcpConfig) -> io::Result<()> {
    if let Some(algorithm) = options.congestion_control.as_deref() {
        socket.set_tcp_congestion(algorithm.as_bytes())?;
    }
    Ok(())
}

#[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "cygwin"))]
fn apply_congestion(_socket: &SockRef<'_>, _options: &TcpConfig) -> io::Result<()> {
    Ok(())
}
