use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, Result};
use socket2::{Domain, Protocol, SockAddr, SockRef, Socket, TcpKeepalive, Type};
use tokio::net::{TcpListener, TcpStream};

use crate::config::TcpConfig;

pub async fn connect_tcp_stream(authority: &str, options: &TcpConfig) -> Result<TcpStream> {
    let stream = TcpStream::connect(authority)
        .await
        .with_context(|| format!("connect {authority}"))?;
    apply_tcp_options(&stream, options)
        .with_context(|| format!("apply TCP options to {authority}"))?;
    Ok(stream)
}

pub fn bind_tcp_listener(addr: SocketAddr, options: &TcpConfig) -> Result<TcpListener> {
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
        .listen(1024)
        .with_context(|| format!("listen {addr}"))?;
    socket
        .set_nonblocking(true)
        .with_context(|| format!("set nonblocking {addr}"))?;
    let std_listener: std::net::TcpListener = socket.into();
    TcpListener::from_std(std_listener).with_context(|| format!("install tokio listener {addr}"))
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
