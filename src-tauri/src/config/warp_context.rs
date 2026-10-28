use network_interface::{NetworkInterface, NetworkInterfaceConfig as _};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ContextMatch {
    Same,
    Changed,
    Unknown,
}

pub(crate) fn compare(recorded: Option<&str>, current: Option<&str>) -> ContextMatch {
    match (recorded, current) {
        (Some(old), Some(new)) if old == new => ContextMatch::Same,
        (Some(_), Some(_)) => ContextMatch::Changed,
        _ => ContextMatch::Unknown,
    }
}

pub(crate) fn capture() -> Option<String> {
    static SALT: std::sync::LazyLock<Option<[u8; 32]>> = std::sync::LazyLock::new(|| {
        let mut salt = [0; 32];
        getrandom::fill(&mut salt).ok()?;
        Some(salt)
    });
    fingerprint(&NetworkInterface::show().ok()?, SALT.as_ref()?)
}

pub(crate) fn ipv6_scan_available() -> bool {
    NetworkInterface::show().is_ok_and(|interfaces| has_public_ipv6(&interfaces))
}

fn has_public_ipv6(interfaces: &[NetworkInterface]) -> bool {
    interfaces.iter().any(|interface| {
        !interface.internal
            && !interface.name.eq_ignore_ascii_case("Tropa")
            && interface.addr.iter().any(|addr| match addr.ip() {
                // A local global-unicast address is a hint, not proof of an IPv6 route.
                std::net::IpAddr::V6(ip) => ip.segments()[0] & 0xe000 == 0x2000,
                _ => false,
            })
    })
}

pub(crate) fn valid(value: Option<&str>) -> bool {
    value.is_none_or(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn fingerprint(interfaces: &[NetworkInterface], salt: &[u8; 32]) -> Option<String> {
    let mut addresses = Vec::new();
    for interface in interfaces {
        // The owned TUN must not invalidate measurements merely by being enabled.
        if interface.internal || interface.name.eq_ignore_ascii_case("Tropa") {
            continue;
        }
        for addr in &interface.addr {
            let ip = addr.ip();
            if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
                continue;
            }
            addresses.push((interface.index, interface.name.as_str(), ip));
        }
    }
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() {
        return None;
    }
    let mut hash = Sha256::new();
    hash.update(salt);
    for (index, name, ip) in addresses {
        hash.update(index.to_be_bytes());
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        match ip {
            std::net::IpAddr::V4(ip) => {
                hash.update([4]);
                hash.update(ip.octets());
            }
            std::net::IpAddr::V6(ip) => {
                hash.update([6]);
                hash.update(ip.octets());
            }
        }
    }
    Some(hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use network_interface::NetworkInterface;

    #[test]
    fn context_changes_with_addresses_but_not_enumeration_order_or_owned_tun() {
        let ethernet = NetworkInterface::new_afinet("Ethernet", "192.0.2.2".parse().unwrap(), None, None, 1, false);
        let wifi = NetworkInterface::new_afinet("Wi-Fi", "198.51.100.2".parse().unwrap(), None, None, 2, false);
        let tun = NetworkInterface::new_afinet("Tropa", "198.18.0.1".parse().unwrap(), None, None, 3, false);
        let original = fingerprint(&[ethernet.clone(), wifi.clone()], &[7; 32]);
        assert!(original.is_some());
        assert_eq!(original, fingerprint(&[wifi.clone(), tun, ethernet.clone()], &[7; 32]));
        assert_ne!(original, fingerprint(&[ethernet.clone()], &[7; 32]));
        assert_ne!(original, fingerprint(&[ethernet, wifi], &[8; 32]));
        assert_eq!(fingerprint(&[], &[7; 32]), None);
        let encoded = serde_json::to_string(&original).unwrap();
        assert!(!encoded.contains("192.0.2"));
        assert!(!encoded.contains("Ethernet"));
    }

    #[test]
    fn ipv6_scan_hint_excludes_owned_tun_and_non_public_local_addresses() {
        let interface = |name: &str, ip: &str| match ip.parse::<std::net::IpAddr>().unwrap() {
            std::net::IpAddr::V4(ip) => NetworkInterface::new_afinet(name, ip, None, None, 1, false),
            std::net::IpAddr::V6(ip) => NetworkInterface::new_afinet6(name, ip, None, None, 1, false),
        };
        for ip in ["::1", "::", "fe80::1", "fd00::1", "192.0.2.1"] {
            assert!(!has_public_ipv6(&[interface("Ethernet", ip)]));
        }
        assert!(!has_public_ipv6(&[interface("Tropa", "2606:4700:110::2")]));
        assert!(has_public_ipv6(&[interface("Ethernet", "2001:4860:1234::2")]));
    }

    #[test]
    fn missing_context_never_counts_as_a_match() {
        assert_eq!(compare(None, None), ContextMatch::Unknown);
        assert_eq!(compare(Some("one"), None), ContextMatch::Unknown);
        assert_eq!(compare(None, Some("one")), ContextMatch::Unknown);
        assert_eq!(compare(Some("one"), Some("two")), ContextMatch::Changed);
        assert_eq!(compare(Some("one"), Some("one")), ContextMatch::Same);
    }
}
