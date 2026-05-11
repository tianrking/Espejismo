use std::net::{IpAddr, Ipv4Addr};
use std::process::Command;

use anyhow::{Context, Result};
use espejismo_core::config::LocalTunConfig;
use espejismo_core::split_authority;
use tokio::net::lookup_host;
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct WindowsRouteGuard {
    tun_name: String,
    tun_ifindex: u32,
    split_default_installed: bool,
    protected_routes: Vec<ProtectedRoute>,
    dns_restore: Option<DnsRestore>,
    active: bool,
}

#[derive(Clone, Debug)]
struct DefaultRoute {
    gateway: Ipv4Addr,
    interface_index: u32,
}

#[derive(Clone, Debug)]
struct ProtectedRoute {
    ip: Ipv4Addr,
    installed_interface_index: u32,
    original: Option<HostRoute>,
}

#[derive(Clone, Debug)]
struct HostRoute {
    gateway: Ipv4Addr,
    interface_index: u32,
    metric: u32,
}

#[derive(Clone, Debug)]
enum DnsRestore {
    Dhcp,
    Static(Vec<IpAddr>),
}

pub async fn install(config: &LocalTunConfig, server: &str) -> Result<WindowsRouteGuard> {
    let tun_ifindex = find_interface_index(&config.name)
        .with_context(|| format!("find Windows TUN interface '{}'", config.name))?;
    let default_route = read_default_route().context("read current Windows default route")?;
    let server_ips = resolve_server_ipv4(server).await?;
    let mut guard = WindowsRouteGuard {
        tun_name: config.name.clone(),
        tun_ifindex,
        split_default_installed: false,
        protected_routes: Vec::new(),
        dns_restore: None,
        active: true,
    };

    if config.route.protect_server_route {
        let default_route = default_route
            .as_ref()
            .context("cannot protect remote server without an existing default route")?;
        for ip in server_ips {
            let original = read_host_route(ip)?;
            add_or_change_ipv4_route(
                ip,
                Ipv4Addr::new(255, 255, 255, 255),
                default_route.gateway,
                default_route.interface_index,
                1,
            )
            .with_context(|| format!("protect route to remote server {ip}"))?;
            guard.protected_routes.push(ProtectedRoute {
                ip,
                installed_interface_index: default_route.interface_index,
                original,
            });
        }
    }

    add_split_default_routes(config.destination, tun_ifindex).context("install split default")?;
    guard.split_default_installed = true;

    if config.route.dns_enabled {
        guard.dns_restore = Some(read_dns_restore(&config.name)?);
        apply_dns(&config.name, &config.route.dns_servers).context("apply Windows TUN DNS")?;
    }

    info!(
        tun = %config.name,
        ifindex = tun_ifindex,
        protected_routes = guard.protected_routes.len(),
        dns = guard.dns_restore.is_some(),
        "Windows TUN route manager installed"
    );
    Ok(guard)
}

impl WindowsRouteGuard {
    pub fn restore(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let mut errors = Vec::new();

        if let Some(restore) = &self.dns_restore {
            if let Err(err) = restore_dns(&self.tun_name, restore) {
                errors.push(format!("restore DNS: {err}"));
            }
        }

        if self.split_default_installed {
            if let Err(err) = delete_ipv4_route(
                Ipv4Addr::new(0, 0, 0, 0),
                Ipv4Addr::new(128, 0, 0, 0),
                self.tun_ifindex,
            ) {
                errors.push(format!("delete 0.0.0.0/1 route: {err}"));
            }
            if let Err(err) = delete_ipv4_route(
                Ipv4Addr::new(128, 0, 0, 0),
                Ipv4Addr::new(128, 0, 0, 0),
                self.tun_ifindex,
            ) {
                errors.push(format!("delete 128.0.0.0/1 route: {err}"));
            }
        }

        for route in &self.protected_routes {
            let result = if let Some(original) = &route.original {
                add_or_change_ipv4_route(
                    route.ip,
                    Ipv4Addr::new(255, 255, 255, 255),
                    original.gateway,
                    original.interface_index,
                    original.metric,
                )
            } else {
                delete_ipv4_route(
                    route.ip,
                    Ipv4Addr::new(255, 255, 255, 255),
                    route.installed_interface_index,
                )
            };
            if let Err(err) = result {
                debug!(ip = %route.ip, error = %err, "protected Windows route restore skipped");
            }
        }

        if errors.is_empty() {
            info!("Windows TUN route manager restored");
            Ok(())
        } else {
            anyhow::bail!(errors.join("; "))
        }
    }
}

impl Drop for WindowsRouteGuard {
    fn drop(&mut self) {
        if let Err(err) = self.restore() {
            warn!(error = %err, "Windows TUN route restore was incomplete");
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
        "local.server must resolve to at least one IPv4 address for Windows auto-route"
    );
    Ok(ips)
}

fn find_interface_index(name: &str) -> Result<u32> {
    let script = format!(
        "(Get-NetAdapter -Name '{}' -ErrorAction Stop).ifIndex",
        ps_quote(name)
    );
    let output = powershell(&script)?;
    output
        .trim()
        .parse()
        .with_context(|| format!("parse Windows interface index from {output:?}"))
}

fn read_default_route() -> Result<Option<DefaultRoute>> {
    let script = r#"
$r = Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' |
  Where-Object { $_.NextHop -ne '0.0.0.0' } |
  Sort-Object RouteMetric, InterfaceMetric |
  Select-Object -First 1
if ($null -ne $r) { "$($r.NextHop)|$($r.InterfaceIndex)" }
"#;
    let output = powershell(script)?;
    let line = output.lines().find(|line| !line.trim().is_empty());
    line.map(parse_default_route_record).transpose()
}

fn read_host_route(ip: Ipv4Addr) -> Result<Option<HostRoute>> {
    let script = format!(
        r#"
$r = Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '{ip}/32' |
  Sort-Object RouteMetric, InterfaceMetric |
  Select-Object -First 1
if ($null -ne $r) {{ "$($r.NextHop)|$($r.InterfaceIndex)|$($r.RouteMetric)" }}
"#
    );
    let output = powershell(&script)?;
    let line = output.lines().find(|line| !line.trim().is_empty());
    line.map(parse_host_route_record).transpose()
}

fn parse_default_route_record(line: &str) -> Result<DefaultRoute> {
    let mut parts = line.trim().split('|');
    let gateway = parts
        .next()
        .context("missing Windows default gateway")?
        .parse()
        .with_context(|| format!("parse Windows default gateway from {line:?}"))?;
    let interface_index = parts
        .next()
        .context("missing Windows default interface index")?
        .parse()
        .with_context(|| format!("parse Windows default interface index from {line:?}"))?;
    Ok(DefaultRoute {
        gateway,
        interface_index,
    })
}

fn parse_host_route_record(line: &str) -> Result<HostRoute> {
    let mut parts = line.trim().split('|');
    let gateway = parts
        .next()
        .context("missing Windows host route gateway")?
        .parse()
        .with_context(|| format!("parse Windows host route gateway from {line:?}"))?;
    let interface_index = parts
        .next()
        .context("missing Windows host route interface index")?
        .parse()
        .with_context(|| format!("parse Windows host route interface index from {line:?}"))?;
    let metric = parts
        .next()
        .context("missing Windows host route metric")?
        .parse()
        .with_context(|| format!("parse Windows host route metric from {line:?}"))?;
    Ok(HostRoute {
        gateway,
        interface_index,
        metric,
    })
}

fn add_split_default_routes(gateway: Ipv4Addr, ifindex: u32) -> Result<()> {
    add_or_change_ipv4_route(
        Ipv4Addr::new(0, 0, 0, 0),
        Ipv4Addr::new(128, 0, 0, 0),
        gateway,
        ifindex,
        1,
    )?;
    add_or_change_ipv4_route(
        Ipv4Addr::new(128, 0, 0, 0),
        Ipv4Addr::new(128, 0, 0, 0),
        gateway,
        ifindex,
        1,
    )
}

fn add_or_change_ipv4_route(
    destination: Ipv4Addr,
    mask: Ipv4Addr,
    gateway: Ipv4Addr,
    ifindex: u32,
    metric: u32,
) -> Result<()> {
    let args = [
        "ADD".to_string(),
        destination.to_string(),
        "MASK".to_string(),
        mask.to_string(),
        gateway.to_string(),
        "METRIC".to_string(),
        metric.to_string(),
        "IF".to_string(),
        ifindex.to_string(),
    ];
    if run_route(&args).is_ok() {
        return Ok(());
    }
    let args = [
        "CHANGE".to_string(),
        destination.to_string(),
        "MASK".to_string(),
        mask.to_string(),
        gateway.to_string(),
        "METRIC".to_string(),
        metric.to_string(),
        "IF".to_string(),
        ifindex.to_string(),
    ];
    run_route(&args)
}

fn delete_ipv4_route(destination: Ipv4Addr, mask: Ipv4Addr, ifindex: u32) -> Result<()> {
    let args = [
        "DELETE".to_string(),
        destination.to_string(),
        "MASK".to_string(),
        mask.to_string(),
        "IF".to_string(),
        ifindex.to_string(),
    ];
    run_route(&args)
}

fn read_dns_restore(name: &str) -> Result<DnsRestore> {
    let output = netsh(&[
        "interface",
        "ipv4",
        "show",
        "dnsservers",
        &format!("name={name}"),
    ])?;
    Ok(parse_dns_restore(&output))
}

fn parse_dns_restore(output: &str) -> DnsRestore {
    if output.to_ascii_lowercase().contains("dhcp") {
        return DnsRestore::Dhcp;
    }
    let servers = output
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .filter_map(|value| value.parse::<IpAddr>().ok())
        .collect();
    DnsRestore::Static(servers)
}

fn apply_dns(name: &str, servers: &[IpAddr]) -> Result<()> {
    let mut iter = servers.iter();
    let first = iter
        .next()
        .context("local.tun.route.dns_servers must not be empty when DNS is enabled")?;
    netsh(&[
        "interface",
        "ipv4",
        "set",
        "dnsservers",
        &format!("name={name}"),
        "static",
        &first.to_string(),
        "primary",
    ])?;
    for (index, server) in iter.enumerate() {
        netsh(&[
            "interface",
            "ipv4",
            "add",
            "dnsservers",
            &format!("name={name}"),
            &server.to_string(),
            &format!("index={}", index + 2),
        ])?;
    }
    Ok(())
}

fn restore_dns(name: &str, restore: &DnsRestore) -> Result<()> {
    match restore {
        DnsRestore::Dhcp => netsh(&[
            "interface",
            "ipv4",
            "set",
            "dnsservers",
            &format!("name={name}"),
            "dhcp",
        ])
        .map(|_| ()),
        DnsRestore::Static(servers) if servers.is_empty() => netsh(&[
            "interface",
            "ipv4",
            "delete",
            "dnsservers",
            &format!("name={name}"),
            "all",
        ])
        .map(|_| ()),
        DnsRestore::Static(servers) => apply_dns(name, servers),
    }
}

fn ps_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn powershell(script: &str) -> Result<String> {
    output("powershell", &["-NoProfile", "-Command", script])
}

fn netsh(args: &[&str]) -> Result<String> {
    output("netsh", args)
}

fn run_route(args: &[String]) -> Result<()> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run("route", &refs)
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
    use super::{
        parse_default_route_record, parse_dns_restore, parse_host_route_record, DnsRestore,
    };

    #[test]
    fn parses_default_route_record() {
        let route = parse_default_route_record("192.168.1.1|12").unwrap();
        assert_eq!(route.gateway.to_string(), "192.168.1.1");
        assert_eq!(route.interface_index, 12);
    }

    #[test]
    fn parses_host_route_record() {
        let route = parse_host_route_record("192.168.1.1|12|3").unwrap();
        assert_eq!(route.gateway.to_string(), "192.168.1.1");
        assert_eq!(route.interface_index, 12);
        assert_eq!(route.metric, 3);
    }

    #[test]
    fn parses_static_dns_servers() {
        let parsed = parse_dns_restore(
            "Statically Configured DNS Servers:  1.1.1.1\n                                 8.8.8.8",
        );
        match parsed {
            DnsRestore::Static(servers) => assert_eq!(servers.len(), 2),
            DnsRestore::Dhcp => panic!("expected static DNS"),
        }
    }
}
