//! This Mac's addresses, as share-link endpoints and as the Host allow-list,
//! from the process's address observer (`runtime::network`). An endpoint is
//! an up address that is neither loopback nor link-local, says which address
//! and interface it is, and is `reachable` only once a device's socket
//! actually came in through it.

mod label;

use crate::runtime::network::{Observer, Subscription};
use serde::Serialize;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub use crate::runtime::network::{Event, Family, LocalAddress};

/// One way to reach the server: `url` is the share link when it names a
/// share, the bare `/c/` page otherwise, and `None` while nothing listens.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    /// `<interface>/<address>`: stable while that address is on that
    /// interface, so a choice survives a list that changes around it.
    pub id: String,
    pub url: Option<String>,
    pub address: String,
    pub family: Family,
    pub interface: String,
    pub label: String,
    /// Where the address came from: `interface`, the OS interface list.
    pub source: &'static str,
    pub generation: u64,
    /// Devices whose live socket came in through this address.
    pub connected_clients: usize,
    /// `connected_clients > 0`: never guessed from the address itself.
    pub reachable: bool,
}

/// A Host the list does not hold forces a re-read at most this often.
const RECHECK: Duration = Duration::from_millis(250);

/// The observer, seen as this server's address list.
pub struct Addresses {
    network: Observer,
    last_miss: Mutex<Option<Instant>>,
}

impl Addresses {
    pub fn new(network: Observer) -> Self {
        Self {
            network,
            last_miss: Mutex::new(None),
        }
    }

    /// The usable addresses now and the observer's generation.
    pub fn current(&self) -> (Vec<LocalAddress>, u64) {
        let snapshot = self.network.snapshot();
        (snapshot.usable().cloned().collect(), snapshot.generation)
    }

    /// Re-read the OS list now.
    pub fn refresh(&self) {
        self.network.refresh();
    }

    /// Calls `callback` with the observer's events until dropped.
    pub fn subscribe(&self, callback: impl Fn(&Event) + Send + Sync + 'static) -> Subscription {
        self.network.subscribe(callback)
    }

    fn holds(&self, ip: IpAddr) -> bool {
        self.network.snapshot().usable().any(|a| a.ip == ip)
    }

    /// Whether `ip` is one of this Mac's usable addresses now. A miss
    /// re-reads the list (rate-limited), so an address that has just come
    /// up is accepted before the observer's debounce has run.
    pub fn contains(&self, ip: IpAddr) -> bool {
        if self.holds(ip) {
            return true;
        }
        {
            let mut last = self.last_miss.lock().unwrap_or_else(|e| e.into_inner());
            if last.is_some_and(|at| at.elapsed() < RECHECK) {
                return false;
            }
            *last = Some(Instant::now());
        }
        self.network.refresh();
        self.holds(ip)
    }
}

/// The endpoints for `port`; IPv6 ones only when an IPv6 listener is bound.
/// `fragment` is appended to each URL as `#fragment`; `connected` counts the
/// devices that came in through an address.
pub fn build(
    addresses: &[LocalAddress],
    generation: u64,
    port: Option<u16>,
    v6: bool,
    fragment: Option<&str>,
    connected: &dyn Fn(IpAddr) -> usize,
) -> Vec<Endpoint> {
    addresses
        .iter()
        .filter(|a| a.usable())
        .filter(|a| a.ip.is_ipv4() || (v6 && port.is_some()))
        .map(|a| {
            let host = match a.ip {
                IpAddr::V4(ip) => ip.to_string(),
                IpAddr::V6(ip) => format!("[{ip}]"),
            };
            let url = port.map(|port| {
                let base = format!("http://{host}:{port}/c/");
                match fragment {
                    Some(fragment) => format!("{base}#{fragment}"),
                    None => base,
                }
            });
            let connected_clients = connected(a.ip);
            Endpoint {
                id: format!("{}/{}", a.interface, a.ip),
                url,
                address: a.ip.to_string(),
                family: a.family,
                interface: a.interface.clone(),
                label: label::label(&a.interface, a.ip, a.point_to_point),
                source: "interface",
                generation,
                connected_clients,
                reachable: connected_clients > 0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::network::fake::{addr, fake};

    fn none(_: IpAddr) -> usize {
        0
    }

    #[test]
    fn endpoints_name_their_address_and_never_guess_reachability() {
        let list = [addr("192.168.1.20", "en0"), addr("fd00::5", "en0")];
        let shared = build(&list, 3, Some(43821), false, Some("id.secret"), &none);
        assert_eq!(shared.len(), 1, "IPv6 needs a bound IPv6 listener");
        let first = &shared[0];
        assert_eq!(
            first.url.as_deref(),
            Some("http://192.168.1.20:43821/c/#id.secret")
        );
        assert_eq!(first.id, "en0/192.168.1.20");
        assert_eq!(first.family, Family::Ipv4);
        assert_eq!(first.generation, 3);
        assert!(!first.reachable);
        let both = build(&list, 3, Some(43821), true, None, &none);
        assert_eq!(both[1].url.as_deref(), Some("http://[fd00::5]:43821/c/"));
        assert!(build(&list, 3, None, true, None, &none)[0].url.is_none());
        let one = |ip: IpAddr| usize::from(ip == list[0].ip);
        let seen = build(&list, 3, Some(43821), true, None, &one);
        assert_eq!((seen[0].connected_clients, seen[0].reachable), (1, true));
        assert_eq!((seen[1].connected_clients, seen[1].reachable), (0, false));
    }

    #[test]
    fn loopback_down_and_link_local_addresses_are_not_endpoints() {
        let down = LocalAddress::new("en1", "192.168.7.2".parse().unwrap(), false, false);
        let list = [
            addr("127.0.0.1", "lo0"),
            addr("169.254.10.1", "en0"),
            addr("fe80::1", "en0"),
            down,
            addr("100.86.3.4", "en0"),
        ];
        let endpoints = build(&list, 1, Some(1), true, None, &none);
        let ids: Vec<&str> = endpoints.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["en0/100.86.3.4"]);
        assert_eq!(endpoints[0].label, "Wi\u{2011}Fi/Ethernet en0");
    }

    #[tokio::test]
    async fn a_missed_host_re_reads_the_list_at_most_every_recheck() {
        let (observer, net) = fake(vec![addr("10.0.0.2", "en0")], Duration::from_secs(60));
        let addresses = Addresses::new(observer);
        let (_, generation) = addresses.current();
        assert!(addresses.contains("10.0.0.2".parse().unwrap()));
        net.set(vec![addr("10.0.0.2", "en0"), addr("10.0.0.3", "en0")]);
        assert!(
            addresses.contains("10.0.0.3".parse().unwrap()),
            "a miss re-reads"
        );
        assert_eq!(addresses.current().1, generation + 1);
        let reads = net.reads();
        let started = std::time::Instant::now();
        assert!(!addresses.contains("10.0.0.4".parse().unwrap()));
        // A loaded machine can stretch the lookup past the window; only a prompt one proves the limit.
        if started.elapsed() < RECHECK / 2 {
            assert_eq!(
                net.reads(),
                reads,
                "a second miss within RECHECK reads nothing"
            );
        }
    }
}
