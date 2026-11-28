//! systemd-networkd's links with their addresses, gateways and DNS servers,
//! read-only, from one `Describe` call. The DNS servers are the ones networkd
//! learned or was configured with and hands to systemd-resolved.
//!
//! ```no_run
//! use canopa_services::network::networkd;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! for link in networkd::read(&system).await?.iter().filter(|link| link.kind == "ether") {
//!     println!("{} {} {:?}", link.name, link.operational, link.addresses);
//! }
//! # Ok(()) }
//! ```
use crate::{Error, bus::StateStream};
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub const SERVICE: &str = super::bundle::NETWORKD;
const PATH: &str = "/org/freedesktop/network1";
const MANAGER: &str = "org.freedesktop.network1.Manager";

/// One network interface as networkd reports it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Link {
    pub index: i32,
    pub name: String,
    /// `ether`, `wlan`, `loopback`, …
    pub kind: String,
    /// `routable`, `degraded`, `carrier`, `no-carrier`, `off`, …
    pub operational: String,
    /// With their prefix, such as `192.168.1.24/24`.
    pub addresses: Vec<String>,
    /// Gateways of default routes.
    pub gateways: Vec<String>,
    pub dns: Vec<String>,
    /// The `.network` file that configures the link; `None` when networkd
    /// does not manage it.
    pub network_file: Option<String>,
}

impl Link {
    /// A cable is plugged in, or the link is otherwise up.
    pub fn has_carrier(&self) -> bool {
        !matches!(
            self.operational.as_str(),
            "off" | "no-carrier" | "missing" | ""
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Description {
    #[serde(default)]
    interfaces: Vec<Interface>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Interface {
    index: i32,
    name: String,
    #[serde(rename = "Type", default)]
    kind: String,
    #[serde(default)]
    operational_state: String,
    network_file: Option<String>,
    #[serde(default)]
    addresses: Vec<Address>,
    #[serde(default)]
    routes: Vec<Route>,
    #[serde(rename = "DNS", default)]
    dns: Vec<Address>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Address {
    family: i32,
    address: Vec<u8>,
    prefix_length: Option<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Route {
    family: i32,
    #[serde(default)]
    destination_prefix_length: u8,
    gateway: Option<Vec<u8>>,
}

/// An address from networkd's family (`AF_INET` 2, `AF_INET6` 10) and bytes.
fn ip(family: i32, bytes: &[u8]) -> Option<IpAddr> {
    match family {
        2 => <[u8; 4]>::try_from(bytes)
            .ok()
            .map(|b| Ipv4Addr::from(b).into()),
        10 => <[u8; 16]>::try_from(bytes)
            .ok()
            .map(|b| Ipv6Addr::from(b).into()),
        _ => None,
    }
}

/// The links in networkd's `Describe` reply, loopback left out, by index.
pub fn parse(json: &str) -> Result<Vec<Link>, Error> {
    let description: Description = serde_json::from_str(json)
        .map_err(|_| Error::Refused("unreadable link description".into()))?;
    let mut links: Vec<Link> = description
        .interfaces
        .into_iter()
        .filter(|interface| interface.kind != "loopback")
        .map(|interface| {
            let mut gateways = Vec::new();
            for route in &interface.routes {
                if route.destination_prefix_length == 0
                    && let Some(gateway) =
                        route.gateway.as_deref().and_then(|g| ip(route.family, g))
                    && !gateway.is_unspecified()
                    && !gateways.contains(&gateway.to_string())
                {
                    gateways.push(gateway.to_string());
                }
            }
            Link {
                index: interface.index,
                name: interface.name,
                kind: interface.kind,
                operational: interface.operational_state,
                addresses: interface
                    .addresses
                    .iter()
                    .filter_map(|address| {
                        let ip = ip(address.family, &address.address)?;
                        Some(match address.prefix_length {
                            Some(prefix) => format!("{ip}/{prefix}"),
                            None => ip.to_string(),
                        })
                    })
                    .collect(),
                gateways,
                dns: interface
                    .dns
                    .iter()
                    .filter_map(|server| ip(server.family, &server.address))
                    .map(|ip| ip.to_string())
                    .collect(),
                network_file: interface.network_file.filter(|file| !file.is_empty()),
            }
        })
        .collect();
    links.sort_by_key(|link| link.index);
    Ok(links)
}

/// Read every link but loopback.
pub async fn read(conn: &zbus::Connection) -> Result<Vec<Link>, Error> {
    let proxy = zbus::Proxy::new(conn, SERVICE, PATH, MANAGER).await?;
    let json: String =
        tokio::time::timeout(crate::bus::DEADLINE, proxy.call("Describe", &())).await??;
    parse(&json)
}

/// Observe networkd: the links again after each burst of its signals.
pub fn watch(conn: zbus::Connection) -> StateStream<Vec<Link>> {
    crate::bus::watch_service(conn, SERVICE, |conn| async move { read(&conn).await })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_description_becomes_links_with_addresses_gateways_and_dns() {
        let links = parse(
            r#"{"Interfaces":[
              {"Index":1,"Name":"lo","Type":"loopback","OperationalState":"carrier"},
              {"Index":2,"Name":"enp5s0","Type":"ether","OperationalState":"routable",
               "NetworkFile":"/etc/systemd/network/20-wired.network",
               "Addresses":[{"Family":2,"Address":[192,168,1,24],"PrefixLength":24},
                            {"Family":10,"Address":[254,128,0,0,0,0,0,0,0,0,0,0,0,0,0,1],"PrefixLength":64}],
               "Routes":[{"Family":2,"Destination":[0,0,0,0],"DestinationPrefixLength":0,"Gateway":[192,168,1,1]},
                         {"Family":2,"Destination":[192,168,1,0],"DestinationPrefixLength":24}],
               "DNS":[{"Family":2,"Address":[192,168,1,1],"ConfigSource":"DHCPv4"}]},
              {"Index":3,"Name":"enp6s0","Type":"ether","OperationalState":"no-carrier","NetworkFile":""}
            ]}"#,
        )
        .unwrap();
        assert_eq!(links.len(), 2);
        let wired = &links[0];
        assert_eq!(wired.addresses, ["192.168.1.24/24", "fe80::1/64"]);
        assert_eq!(wired.gateways, ["192.168.1.1"]);
        assert_eq!(wired.dns, ["192.168.1.1"]);
        assert!(wired.has_carrier());
        assert!(!links[1].has_carrier());
        assert_eq!(links[1].network_file, None);
        assert!(parse("not json").is_err());
    }
}
