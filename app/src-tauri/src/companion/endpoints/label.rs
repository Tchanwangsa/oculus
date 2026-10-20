//! A neutral label for an endpoint, from its interface. macOS names Wi-Fi and
//! Ethernet both `en*`, so they share a label. A provider is named only when
//! the interface name says so too: 100.64.0.0/10 alone is carrier-grade NAT
//! space that many VPNs and ISPs use, so it is never read as Tailscale.

use std::net::IpAddr;

pub fn label(interface: &str, ip: IpAddr, point_to_point: bool) -> String {
    let name = interface.to_ascii_lowercase();
    let kind = if name.contains("tailscale") && tailscale_range(ip) {
        "Tailscale"
    } else if point_to_point
        || ["utun", "tun", "tap", "ipsec", "ppp", "wg", "gif", "zt"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
    {
        "VPN/tunnel"
    } else if name.starts_with("en") && name[2..].chars().all(|c| c.is_ascii_digit()) {
        "Wi\u{2011}Fi/Ethernet"
    } else if ["wi-fi", "wifi", "wlan", "wireless"]
        .iter()
        .any(|part| name.contains(part))
    {
        "Wi\u{2011}Fi"
    } else if name.starts_with("eth") || name.contains("ethernet") {
        "Ethernet"
    } else if name.starts_with("bridge") {
        "Bridge"
    } else {
        "Network"
    };
    format!("{kind} {interface}")
}

fn tailscale_range(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, ..] = ip.octets();
            a == 100 && (64..128).contains(&b)
        }
        IpAddr::V6(ip) => ip.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(interface: &str, ip: &str, p2p: bool) -> String {
        label(interface, ip.parse().unwrap(), p2p)
    }

    #[test]
    fn labels_follow_the_interface_not_the_address_range() {
        assert_eq!(of("en0", "192.168.1.4", false), "Wi\u{2011}Fi/Ethernet en0");
        assert_eq!(of("utun9", "100.101.1.2", true), "VPN/tunnel utun9");
        assert_eq!(of("utun4", "100.101.1.2", false), "VPN/tunnel utun4");
        assert_eq!(of("en5", "100.72.0.1", false), "Wi\u{2011}Fi/Ethernet en5");
        assert_eq!(of("en0", "100.86.3.4", false), "Wi\u{2011}Fi/Ethernet en0");
        assert_eq!(of("utun6", "100.86.3.4", false), "VPN/tunnel utun6");
        assert_eq!(of("wg0", "10.6.0.2", false), "VPN/tunnel wg0");
        assert_eq!(of("ppp0", "10.6.0.2", false), "VPN/tunnel ppp0");
        assert_eq!(of("Tailscale", "100.86.3.4", false), "Tailscale Tailscale");
        assert_eq!(
            of("tailscale0", "100.72.0.1", false),
            "Tailscale tailscale0"
        );
        assert_eq!(of("tailscale0", "10.0.0.1", false), "Network tailscale0");
        assert_eq!(of("bridge100", "192.168.64.1", false), "Bridge bridge100");
        assert_eq!(of("Wi-Fi", "192.168.1.4", false), "Wi\u{2011}Fi Wi-Fi");
        assert_eq!(of("anpi0", "10.0.0.1", false), "Network anpi0");
    }
}
