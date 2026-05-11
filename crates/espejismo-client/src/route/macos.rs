#![cfg_attr(test, allow(dead_code))]

use std::net::{IpAddr, Ipv4Addr};
use std::process::Command;

use anyhow::{Context, Result};
use espejismo_core::config::LocalTunConfig;
use espejismo_core::split_authority;
use tokio::net::lookup_host;
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct MacosRouteGuard {
    tun_name: String,
    split_default_installed: bool,
    protected_routes: Vec<ProtectedRoute>,
    dns_restore: Vec<ServiceDnsRestore>,
    active: bool,
}

#[derive(Clone, Debug)]
struct DefaultRoute {
    gateway: Ipv4Addr,
    interface: String,
}

#[derive(Clone, Debug)]
struct HostRoute {
    gateway: Option<Ipv4Addr>,
    interface: Option<String>,
}

#[derive(Clone, Debug)]
struct ProtectedRoute {
    ip: Ipv4Addr,
    original: Option<HostRoute>,
}

#[derive(Clone, Debug)]
struct ServiceDnsRestore {
    service: String,
    dns: DnsRestore,
}

#[derive(Clone, Debug)]
enum DnsRestore {
    Empty,
    Static(Vec<IpAddr>),
}

pub async fn install(config: &LocalTunConfig, server: &str) -> Result<MacosRouteGuard> {
    let default_route = read_default_route().context("read current macOS default route")?;
    let server_ips = resolve_server_ipv4(server).await?;
    let mut guard = MacosRouteGuard {
        tun_name: config.name.clone(),
        split_default_installed: false,
        protected_routes: Vec::new(),
        dns_restore: Vec::new(),
        active: true,
    };

    if config.route.protect_server_route {
        let default_route = default_route
            .as_ref()
            .context("cannot protect remote server without an existing default route")?;
        for ip in server_ips {
            let original = read_host_route(ip)?;
            add_or_change_host_route_via_gateway(ip, default_route.gateway)
                .with_context(|| format!("protect route to remote server {ip}"))?;
            guard.protected_routes.push(ProtectedRoute { ip, original });
        }
    }

    add_split_default_routes(&config.name).context("install macOS split default")?;
    guard.split_default_installed = true;

    if config.route.dns_enabled {
        guard.dns_restore = read_dns_restore()?;
        apply_dns(&config.route.dns_servers).context("apply macOS DNS")?;
    }

    info!(
        tun = %config.name,
        default_interface = default_route.as_ref().map(|route| route.interface.as_str()).unwrap_or("unknown"),
        protected_routes = guard.protected_routes.len(),
        dns_services = guard.dns_restore.len(),
        "macOS TUN route manager installed"
    );
    Ok(guard)
}

impl MacosRouteGuard {
    pub fn restore(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let mut errors = Vec::new();

        for service in &self.dns_restore {
            if let Err(err) = restore_service_dns(service) {
                errors.push(format!("restore DNS for {}: {err}", service.service));
            }
        }

        if self.split_default_installed {
            if let Err(err) = delete_net_route("0.0.0.0/1") {
                errors.push(format!("delete 0.0.0.0/1 route: {err}"));
            }
            if let Err(err) = delete_net_route("128.0.0.0/1") {
                errors.push(format!("delete 128.0.0.0/1 route: {err}"));
            }
        }

        for route in &self.protected_routes {
            let result = if let Some(original) = &route.original {
                restore_host_route(route.ip, original)
            } else {
                delete_host_route(route.ip)
            };
            if let Err(err) = result {
                debug!(ip = %route.ip, error = %err, "protected macOS route restore skipped");
            }
        }

        if errors.is_empty() {
            info!(tun = %self.tun_name, "macOS TUN route manager restored");
            Ok(())
        } else {
            anyhow::bail!(errors.join("; "))
        }
    }
}

impl Drop for MacosRouteGuard {
    fn drop(&mut self) {
        if let Err(err) = self.restore() {
            warn!(error = %err, "macOS TUN route restore was incomplete");
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
        "local.server must resolve to at least one IPv4 address for macOS auto-route"
    );
    Ok(ips)
}

fn read_default_route() -> Result<Option<DefaultRoute>> {
    let output = output("route", &["-n", "get", "default"])?;
    if output.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(parse_default_route(&output)?))
}

fn read_host_route(ip: Ipv4Addr) -> Result<Option<HostRoute>> {
    let ip_s = ip.to_string();
    match output("route", &["-n", "get", &ip_s]) {
        Ok(output) => Ok(Some(parse_host_route(&output)?)),
        Err(_) => Ok(None),
    }
}

fn parse_default_route(output: &str) -> Result<DefaultRoute> {
    let gateway = route_field(output, "gateway")
        .context("macOS default route has no gateway")?
        .parse()
        .context("parse macOS default route gateway")?;
    let interface = route_field(output, "interface")
        .context("macOS default route has no interface")?
        .to_string();
    Ok(DefaultRoute { gateway, interface })
}

fn parse_host_route(output: &str) -> Result<HostRoute> {
    let gateway = route_field(output, "gateway")
        .map(str::parse)
        .transpose()
        .context("parse macOS host route gateway")?;
    let interface = route_field(output, "interface").map(str::to_string);
    Ok(HostRoute { gateway, interface })
}

fn route_field<'a>(output: &'a str, key: &str) -> Option<&'a str> {
    output.lines().find_map(|line| {
        let (name, value) = line.trim().split_once(':')?;
        (name.trim() == key).then_some(value.trim())
    })
}

fn add_split_default_routes(tun_name: &str) -> Result<()> {
    add_or_change_net_route_to_interface("0.0.0.0/1", tun_name)?;
    add_or_change_net_route_to_interface("128.0.0.0/1", tun_name)
}

fn add_or_change_host_route_via_gateway(ip: Ipv4Addr, gateway: Ipv4Addr) -> Result<()> {
    let ip_s = ip.to_string();
    let gateway_s = gateway.to_string();
    add_or_change_route(
        &["-n", "add", "-host", &ip_s, &gateway_s],
        &["-n", "change", "-host", &ip_s, &gateway_s],
    )
}

fn add_or_change_net_route_to_interface(cidr: &str, interface: &str) -> Result<()> {
    add_or_change_route(
        &["-n", "add", "-net", cidr, "-interface", interface],
        &["-n", "change", "-net", cidr, "-interface", interface],
    )
}

fn restore_host_route(ip: Ipv4Addr, original: &HostRoute) -> Result<()> {
    if let Some(gateway) = original.gateway {
        return add_or_change_host_route_via_gateway(ip, gateway);
    }
    if let Some(interface) = &original.interface {
        let ip_s = ip.to_string();
        return add_or_change_route(
            &["-n", "add", "-host", &ip_s, "-interface", interface],
            &["-n", "change", "-host", &ip_s, "-interface", interface],
        );
    }
    delete_host_route(ip)
}

fn delete_host_route(ip: Ipv4Addr) -> Result<()> {
    run("route", &["-n", "delete", "-host", &ip.to_string()])
}

fn delete_net_route(cidr: &str) -> Result<()> {
    run("route", &["-n", "delete", "-net", cidr])
}

fn add_or_change_route(add_args: &[&str], change_args: &[&str]) -> Result<()> {
    if run("route", add_args).is_ok() {
        return Ok(());
    }
    run("route", change_args)
}

fn read_dns_restore() -> Result<Vec<ServiceDnsRestore>> {
    let services = list_network_services()?;
    let mut restore = Vec::new();
    for service in services {
        match read_service_dns(&service) {
            Ok(dns) => restore.push(ServiceDnsRestore { service, dns }),
            Err(err) => {
                debug!(service = %service, error = %err, "skip unreadable macOS DNS service")
            }
        }
    }
    Ok(restore)
}

fn list_network_services() -> Result<Vec<String>> {
    let output = output("networksetup", &["-listallnetworkservices"])?;
    Ok(parse_network_services(&output))
}

fn parse_network_services(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.contains("denotes that a network service is disabled"))
        .map(|line| line.trim_start_matches("*").trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn read_service_dns(service: &str) -> Result<DnsRestore> {
    let output = output("networksetup", &["-getdnsservers", service])?;
    Ok(parse_service_dns(&output))
}

fn parse_service_dns(output: &str) -> DnsRestore {
    if output
        .to_ascii_lowercase()
        .contains("there aren't any dns servers")
    {
        return DnsRestore::Empty;
    }
    let servers = output
        .lines()
        .filter_map(|line| line.trim().parse::<IpAddr>().ok())
        .collect();
    DnsRestore::Static(servers)
}

fn apply_dns(servers: &[IpAddr]) -> Result<()> {
    anyhow::ensure!(
        !servers.is_empty(),
        "local.tun.route.dns_servers must not be empty when DNS is enabled"
    );
    let services = list_network_services()?;
    for service in services {
        let mut owned = vec!["-setdnsservers".to_string(), service.clone()];
        owned.extend(servers.iter().map(ToString::to_string));
        let args: Vec<&str> = owned.iter().map(String::as_str).collect();
        if let Err(err) = run("networksetup", &args) {
            debug!(service = %service, error = %err, "skip macOS DNS apply for service");
        }
    }
    Ok(())
}

fn restore_service_dns(service: &ServiceDnsRestore) -> Result<()> {
    match &service.dns {
        DnsRestore::Empty => run(
            "networksetup",
            &["-setdnsservers", &service.service, "Empty"],
        ),
        DnsRestore::Static(servers) if servers.is_empty() => run(
            "networksetup",
            &["-setdnsservers", &service.service, "Empty"],
        ),
        DnsRestore::Static(servers) => {
            let mut owned = vec!["-setdnsservers".to_string(), service.service.clone()];
            owned.extend(servers.iter().map(ToString::to_string));
            let args: Vec<&str> = owned.iter().map(String::as_str).collect();
            run("networksetup", &args)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{parse_default_route, parse_network_services, parse_service_dns, DnsRestore};

    #[test]
    fn parses_default_route() {
        let route = parse_default_route(
            r#"
   route to: default
destination: default
       mask: default
    gateway: 192.168.1.1
  interface: en0
"#,
        )
        .unwrap();
        assert_eq!(route.gateway.to_string(), "192.168.1.1");
        assert_eq!(route.interface, "en0");
    }

    #[test]
    fn parses_network_services() {
        let services = parse_network_services(
            "An asterisk (*) denotes that a network service is disabled.\nWi-Fi\n*Thunderbolt Bridge\nUSB 10/100/1000 LAN\n",
        );
        assert_eq!(
            services,
            vec!["Wi-Fi", "Thunderbolt Bridge", "USB 10/100/1000 LAN"]
        );
    }

    #[test]
    fn parses_empty_dns_marker() {
        assert!(matches!(
            parse_service_dns("There aren't any DNS Servers set on Wi-Fi."),
            DnsRestore::Empty
        ));
    }
}
