use std::path::PathBuf;

use anyhow::Result;
use espejismo_core::config::LocalTunConfig;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(any(target_os = "macos", test))]
mod macos_parse;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub type RouteGuard = linux::LinuxRouteGuard;

#[cfg(target_os = "macos")]
pub type RouteGuard = macos::MacosRouteGuard;

#[cfg(target_os = "windows")]
pub type RouteGuard = windows::WindowsRouteGuard;

#[cfg(target_os = "linux")]
pub async fn install_tun_routes(config: &LocalTunConfig, server: &str) -> Result<RouteGuard> {
    linux::install(config, server).await
}

#[cfg(target_os = "linux")]
pub async fn cleanup_tun_routes(config: &LocalTunConfig) -> Result<()> {
    linux::cleanup(config).await
}

#[cfg(target_os = "macos")]
pub async fn install_tun_routes(config: &LocalTunConfig, server: &str) -> Result<RouteGuard> {
    macos::install(config, server).await
}

#[cfg(target_os = "macos")]
pub async fn cleanup_tun_routes(config: &LocalTunConfig) -> Result<()> {
    macos::cleanup(config).await
}

#[cfg(target_os = "windows")]
pub async fn install_tun_routes(config: &LocalTunConfig, server: &str) -> Result<RouteGuard> {
    windows::install(config, server).await
}

#[cfg(target_os = "windows")]
pub async fn cleanup_tun_routes(config: &LocalTunConfig) -> Result<()> {
    windows::cleanup(config).await
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub struct RouteGuard;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub async fn install_tun_routes(_config: &LocalTunConfig, _server: &str) -> Result<RouteGuard> {
    anyhow::bail!(
        "automatic TUN route/DNS takeover is currently implemented only on Linux, macOS, and Windows"
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub async fn cleanup_tun_routes(_config: &LocalTunConfig) -> Result<()> {
    anyhow::bail!(
        "automatic TUN route/DNS cleanup is currently implemented only on Linux, macOS, and Windows"
    )
}

pub(crate) fn state_path(tun_name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("espejismo-route-{tun_name}.json"))
}
