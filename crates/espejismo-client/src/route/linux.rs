use std::net::{IpAddr, Ipv4Addr};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use espejismo_core::config::LocalTunConfig;
use espejismo_core::split_authority;
use serde::{Deserialize, Serialize};
use tokio::net::lookup_host;
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct LinuxRouteGuard {
    tun_name: String,
    original_default: Option<DefaultRoute>,
    protected_routes: Vec<ProtectedRoute>,
    dns_revert: bool,
    active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DefaultRoute {
    gateway: Option<Ipv4Addr>,
    dev: String,
    metric: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProtectedRoute {
    ip: Ipv4Addr,
    original: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LinuxRouteState {
    tun_name: String,
    original_default: Option<DefaultRoute>,
    protected_routes: Vec<ProtectedRoute>,
    dns_revert: bool,
}

pub async fn install(config: &LocalTunConfig, server: &str) -> Result<LinuxRouteGuard> {
    let original_default = read_default_route().context("read current Linux default route")?;
    let server_ips = resolve_server_ipv4(server).await?;
    let mut guard = LinuxRouteGuard {
        tun_name: config.name.clone(),
        original_default,
        protected_routes: Vec::new(),
        dns_revert: false,
        active: true,
    };

    if config.route.protect_server_route {
        let default_route = guard
            .original_default
            .as_ref()
            .context("cannot protect remote server without an existing default route")?;
        for ip in server_ips {
            let original = read_host_route(ip)?;
            replace_host_route(ip, default_route)
                .with_context(|| format!("protect route to remote server {ip}"))?;
            guard.protected_routes.push(ProtectedRoute { ip, original });
        }
    }

    replace_default_route_to_tun(&config.name).context("replace default route to TUN")?;
    if config.route.dns_enabled {
        apply_resolved_dns(&config.name, &config.route.dns_servers)
            .context("apply systemd-resolved DNS to TUN")?;
        guard.dns_revert = true;
    }
    write_state(&guard)?;

    info!(
        tun = %config.name,
        protected_routes = guard.protected_routes.len(),
        dns = guard.dns_revert,
        "Linux TUN route manager installed"
    );
    Ok(guard)
}

impl LinuxRouteGuard {
    pub fn restore(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let mut errors = Vec::new();

        if self.dns_revert {
            if let Err(err) = run("resolvectl", &["revert", &self.tun_name]) {
                errors.push(format!("restore DNS: {err}"));
            }
        }

        if let Some(default_route) = &self.original_default {
            if let Err(err) = restore_default_route(default_route) {
                errors.push(format!("restore default route: {err}"));
            }
        }

        for route in &self.protected_routes {
            let result = if let Some(original) = &route.original {
                restore_route_line(original)
            } else {
                let cidr = format!("{}/32", route.ip);
                run("ip", &["route", "del", &cidr])
            };
            if let Err(err) = result {
                debug!(ip = %route.ip, error = %err, "protected route restore skipped");
            }
        }

        if errors.is_empty() {
            let _ = std::fs::remove_file(super::state_path(&self.tun_name));
            info!("Linux TUN route manager restored");
            Ok(())
        } else {
            anyhow::bail!(errors.join("; "))
        }
    }
}

pub async fn cleanup(config: &LocalTunConfig) -> Result<()> {
    let path = super::state_path(&config.name);
    let state = std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str::<LinuxRouteState>(&content).ok());
    let mut errors = Vec::new();
    if let Some(state) = state {
        let mut guard = LinuxRouteGuard {
            tun_name: state.tun_name,
            original_default: state.original_default,
            protected_routes: state.protected_routes,
            dns_revert: state.dns_revert,
            active: true,
        };
        guard.restore()?;
        return Ok(());
    }

    if let Err(err) = run_quiet("resolvectl", &["revert", &config.name]) {
        debug!(error = %err, "Linux DNS cleanup skipped");
    }
    if let Err(err) = run_quiet("ip", &["route", "del", "0.0.0.0/1"]) {
        debug!(error = %err, "Linux split route cleanup skipped");
    }
    if let Err(err) = run_quiet("ip", &["route", "del", "128.0.0.0/1"]) {
        debug!(error = %err, "Linux split route cleanup skipped");
    }
    if read_default_route()
        .ok()
        .flatten()
        .is_some_and(|route| route.dev == config.name)
    {
        errors.push(format!(
            "default route still points to {}; no saved state was found at {}",
            config.name,
            path.display()
        ));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(errors.join("; "))
    }
}

fn write_state(guard: &LinuxRouteGuard) -> Result<()> {
    let state = LinuxRouteState {
        tun_name: guard.tun_name.clone(),
        original_default: guard.original_default.clone(),
        protected_routes: guard.protected_routes.clone(),
        dns_revert: guard.dns_revert,
    };
    std::fs::write(
        super::state_path(&guard.tun_name),
        serde_json::to_vec_pretty(&state)?,
    )
    .context("write Linux route recovery state")
}

impl Drop for LinuxRouteGuard {
    fn drop(&mut self) {
        if let Err(err) = self.restore() {
            warn!(error = %err, "Linux TUN route restore was incomplete");
        }
    }
}

async fn resolve_server_ipv4(server: &str) -> Result<Vec<Ipv4Addr>> {
    let (_, port) = split_authority(server)?;
    let addrs = lookup_host(server)
        .await
        .with_context(|| format!("resolve local.server {server}"))?;
    let mut ips = Vec::new();
    for addr in addrs {
        if addr.port() != port {
            continue;
        }
        if let IpAddr::V4(ip) = addr.ip() {
            if !ips.contains(&ip) {
                ips.push(ip);
            }
        }
    }
    anyhow::ensure!(
        !ips.is_empty(),
        "local.server must resolve to at least one IPv4 address for Linux auto-route"
    );
    Ok(ips)
}

fn read_default_route() -> Result<Option<DefaultRoute>> {
    let output = output("ip", &["-4", "route", "show", "default"])?;
    let Some(line) = output.lines().find(|line| !line.trim().is_empty()) else {
        return Ok(None);
    };
    Ok(Some(parse_default_route(line)?))
}

fn read_host_route(ip: Ipv4Addr) -> Result<Option<String>> {
    let cidr = format!("{ip}/32");
    let output = output("ip", &["-4", "route", "show", "exact", &cidr])?;
    Ok(output
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::to_string))
}

fn parse_default_route(line: &str) -> Result<DefaultRoute> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    anyhow::ensure!(
        parts.first() == Some(&"default"),
        "not a default route: {line}"
    );
    let gateway = find_after(&parts, "via")
        .map(str::parse)
        .transpose()
        .with_context(|| format!("parse default gateway from {line}"))?;
    let dev = find_after(&parts, "dev")
        .context("default route has no dev")?
        .to_string();
    let metric = find_after(&parts, "metric")
        .map(str::parse)
        .transpose()
        .with_context(|| format!("parse default metric from {line}"))?;
    Ok(DefaultRoute {
        gateway,
        dev,
        metric,
    })
}

fn find_after<'a>(parts: &'a [&str], key: &str) -> Option<&'a str> {
    parts
        .windows(2)
        .find_map(|pair| (pair[0] == key).then_some(pair[1]))
}

fn replace_host_route(ip: Ipv4Addr, default_route: &DefaultRoute) -> Result<()> {
    let mut owned = vec![
        "route".to_string(),
        "replace".to_string(),
        format!("{ip}/32"),
    ];
    if let Some(gateway) = default_route.gateway {
        owned.push("via".to_string());
        owned.push(gateway.to_string());
    }
    owned.push("dev".to_string());
    owned.push(default_route.dev.clone());
    let args: Vec<&str> = owned.iter().map(String::as_str).collect();
    run("ip", &args)
}

fn replace_default_route_to_tun(tun_name: &str) -> Result<()> {
    run("ip", &["route", "replace", "default", "dev", tun_name])
}

fn restore_default_route(default_route: &DefaultRoute) -> Result<()> {
    let mut owned = vec![
        "route".to_string(),
        "replace".to_string(),
        "default".to_string(),
    ];
    if let Some(gateway) = default_route.gateway {
        owned.push("via".to_string());
        owned.push(gateway.to_string());
    }
    owned.push("dev".to_string());
    owned.push(default_route.dev.clone());
    if let Some(metric) = default_route.metric {
        owned.push("metric".to_string());
        owned.push(metric.to_string());
    }
    let args: Vec<&str> = owned.iter().map(String::as_str).collect();
    run("ip", &args)
}

fn restore_route_line(line: &str) -> Result<()> {
    let mut owned = vec!["route".to_string(), "replace".to_string()];
    owned.extend(line.split_whitespace().map(str::to_string));
    let args: Vec<&str> = owned.iter().map(String::as_str).collect();
    run("ip", &args)
}

fn apply_resolved_dns(tun_name: &str, servers: &[IpAddr]) -> Result<()> {
    let dns_args: Vec<String> = std::iter::once("dns".to_string())
        .chain(std::iter::once(tun_name.to_string()))
        .chain(servers.iter().map(ToString::to_string))
        .collect();
    let dns_refs: Vec<&str> = dns_args.iter().map(String::as_str).collect();
    run("resolvectl", &dns_refs)?;
    run("resolvectl", &["domain", tun_name, "~."])?;
    run("resolvectl", &["default-route", tun_name, "yes"])?;
    Ok(())
}

fn output(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("run {program} {}", args.join(" ")))?;
    if !output.status.success() {
        anyhow::bail!(
            "{} {} failed: {}",
            program,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("command output is not UTF-8")
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("run {program} {}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("{program} {} exited with {status}", args.join(" "))
    }
}

fn run_quiet(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("run {program} {}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("{program} {} exited with {status}", args.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_default_route;

    #[test]
    fn parses_default_route_with_gateway_metric() {
        let route =
            parse_default_route("default via 192.168.1.1 dev wlan0 proto dhcp metric 600").unwrap();
        assert_eq!(route.gateway.unwrap().to_string(), "192.168.1.1");
        assert_eq!(route.dev, "wlan0");
        assert_eq!(route.metric, Some(600));
    }

    #[test]
    fn parses_default_route_without_gateway() {
        let route = parse_default_route("default dev ppp0 scope link").unwrap();
        assert!(route.gateway.is_none());
        assert_eq!(route.dev, "ppp0");
    }
}
