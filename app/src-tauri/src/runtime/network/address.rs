//! What the observer knows about one address, and where it reads them: the
//! OS interface list (`if-addrs`), which names the interface an address is
//! on. `if-watch` only says that an address came or went.

use serde::Serialize;
use std::net::IpAddr;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    Ipv4,
    Ipv6,
}

/// One address on one interface. Every address is kept, flags and all;
/// `usable` is the filter for "another device could try this".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalAddress {
    pub interface: String,
    pub ip: IpAddr,
    pub family: Family,
    pub loopback: bool,
    /// 169.254.0.0/16 or fe80::/10.
    pub link_local: bool,
    /// The interface is operationally up.
    pub up: bool,
    pub point_to_point: bool,
}

impl LocalAddress {
    pub fn new(interface: &str, ip: IpAddr, up: bool, point_to_point: bool) -> Self {
        let link_local = match ip {
            IpAddr::V4(ip) => ip.is_link_local(),
            IpAddr::V6(ip) => ip.segments()[0] & 0xffc0 == 0xfe80,
        };
        Self {
            interface: interface.to_string(),
            ip,
            family: if ip.is_ipv4() {
                Family::Ipv4
            } else {
                Family::Ipv6
            },
            loopback: ip.is_loopback(),
            link_local,
            up,
            point_to_point,
        }
    }

    /// Up, and neither loopback, link-local, unspecified nor multicast.
    pub fn usable(&self) -> bool {
        self.up
            && !self.loopback
            && !self.link_local
            && !self.ip.is_unspecified()
            && !self.ip.is_multicast()
    }
}

/// The address list at one moment. `generation` moves only when the list
/// does; it starts at 1 with the first read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub generation: u64,
    pub addrs: Arc<[LocalAddress]>,
}

impl Snapshot {
    pub fn usable(&self) -> impl Iterator<Item = &LocalAddress> {
        self.addrs.iter().filter(|a| a.usable())
    }
}

/// Reads the current address list.
pub trait Provider: Send + Sync + 'static {
    fn read(&self) -> std::io::Result<Vec<LocalAddress>>;
}

/// The OS interface list.
pub struct Interfaces;

impl Provider for Interfaces {
    fn read(&self) -> std::io::Result<Vec<LocalAddress>> {
        Ok(if_addrs::get_if_addrs()?
            .into_iter()
            .map(|i| LocalAddress::new(&i.name, i.ip(), i.is_oper_up(), i.is_p2p()))
            .collect())
    }
}

/// Sorted by interface then address, duplicates dropped, so two reads of
/// the same network compare equal.
pub fn normalize(mut addrs: Vec<LocalAddress>) -> Arc<[LocalAddress]> {
    addrs.sort();
    addrs.dedup();
    addrs.into()
}
