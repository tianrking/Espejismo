use anyhow::Result;
use espejismo_core::config::LocalTunConfig;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub type RouteGuard = linux::LinuxRouteGuard;

#[cfg(target_os = "linux")]
pub async fn install_tun_routes(config: &LocalTunConfig, server: &str) -> Result<RouteGuard> {
    linux::install(config, server).await
}

#[cfg(not(target_os = "linux"))]
pub struct RouteGuard;

#[cfg(not(target_os = "linux"))]
pub async fn install_tun_routes(_config: &LocalTunConfig, _server: &str) -> Result<RouteGuard> {
    anyhow::bail!("automatic TUN route/DNS takeover is currently implemented only on Linux")
}
