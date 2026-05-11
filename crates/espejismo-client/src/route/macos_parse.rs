use std::net::{IpAddr, Ipv4Addr};

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub(super) struct DefaultRoute {
    pub(super) gateway: Ipv4Addr,
    pub(super) interface: String,
}

#[derive(Clone, Debug)]
pub(super) struct HostRoute {
    pub(super) gateway: Option<Ipv4Addr>,
    pub(super) interface: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) enum DnsRestore {
    Empty,
    Static(Vec<IpAddr>),
}

pub(super) fn parse_default_route(output: &str) -> Result<DefaultRoute> {
    let gateway = route_field(output, "gateway")
        .context("macOS default route has no gateway")?
        .parse()
        .context("parse macOS default route gateway")?;
    let interface = route_field(output, "interface")
        .context("macOS default route has no interface")?
        .to_string();
    Ok(DefaultRoute { gateway, interface })
}

pub(super) fn parse_host_route(output: &str) -> Result<HostRoute> {
    let gateway = route_field(output, "gateway")
        .map(str::parse)
        .transpose()
        .context("parse macOS host route gateway")?;
    let interface = route_field(output, "interface").map(str::to_string);
    Ok(HostRoute { gateway, interface })
}

pub(super) fn parse_network_services(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.contains("denotes that a network service is disabled"))
        .map(|line| line.trim_start_matches("*").trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

pub(super) fn parse_service_dns(output: &str) -> DnsRestore {
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

fn route_field<'a>(output: &'a str, key: &str) -> Option<&'a str> {
    output.lines().find_map(|line| {
        let (name, value) = line.trim().split_once(':')?;
        (name.trim() == key).then_some(value.trim())
    })
}

#[cfg(test)]
mod tests {
    use super::{
        parse_default_route, parse_host_route, parse_network_services, parse_service_dns,
        DnsRestore,
    };

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

    #[test]
    fn parses_host_route() {
        let route = parse_host_route(
            r#"
   route to: 203.0.113.10
destination: 203.0.113.10
    gateway: 192.168.1.1
  interface: en0
"#,
        )
        .unwrap();
        assert_eq!(route.gateway.unwrap().to_string(), "192.168.1.1");
        assert_eq!(route.interface.unwrap(), "en0");
    }

    #[test]
    fn parses_static_dns_servers() {
        match parse_service_dns("1.1.1.1\n8.8.8.8\n") {
            DnsRestore::Static(servers) => {
                assert_eq!(servers.len(), 2);
                assert_eq!(servers[0].to_string(), "1.1.1.1");
            }
            DnsRestore::Empty => panic!("expected static DNS servers"),
        }
    }
}
