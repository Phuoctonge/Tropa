use crate::config::warp_profiles::TransportKind;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

const WG_V4: [[u8; 3]; 14] = [
    [8, 6, 112],
    [8, 34, 70],
    [8, 34, 146],
    [8, 35, 211],
    [8, 39, 125],
    [8, 39, 204],
    [8, 39, 214],
    [8, 47, 69],
    [162, 159, 192],
    [162, 159, 195],
    [188, 114, 96],
    [188, 114, 97],
    [188, 114, 98],
    [188, 114, 99],
];
const WG_PORTS: &[u16] = &[
    2408, 500, 1701, 4500, 854, 859, 864, 878, 880, 890, 891, 894, 903, 908, 928, 934, 939, 942, 943, 945, 946, 955,
    968, 987, 988, 1002, 1010, 1014, 1018, 1070, 1074, 1180, 1387, 1843, 2371, 2506, 3138, 3476, 3581, 3854, 4177,
    4198, 4233, 5279, 5956, 7103, 7152, 7156, 7281, 7559, 8319, 8742, 8854, 8886,
];
const MASQUE_PORTS: &[u16] = &[443, 500, 1701, 4500, 4443, 8443, 8095];

fn standard_ips(kind: TransportKind, ipv6: bool) -> Vec<IpAddr> {
    match (kind, ipv6) {
        (TransportKind::MasqueH3 | TransportKind::MasqueH2, false) => [2, 1]
            .into_iter()
            .map(|host| Ipv4Addr::new(162, 159, 198, host).into())
            .collect(),
        (TransportKind::MasqueH3 | TransportKind::MasqueH2, true) => [0x103, 0x104]
            .into_iter()
            .flat_map(|prefix| [2, 1].map(|host| Ipv6Addr::new(0x2606, 0x4700, prefix, 0, 0, 0, 0, host).into()))
            .collect(),
        (TransportKind::WireGuard | TransportKind::Awg, false) => vec![Ipv4Addr::new(188, 114, 98, 58).into()],
        _ => Vec::new(),
    }
}

pub(crate) fn targets(kind: TransportKind, ipv6: bool, extended: bool, seed: u64) -> Vec<SocketAddr> {
    let masque = matches!(kind, TransportKind::MasqueH3 | TransportKind::MasqueH2);
    let ports = if masque { MASQUE_PORTS } else { WG_PORTS };
    let mut ips = standard_ips(kind, ipv6);
    let mut addresses: Vec<_> = ips.iter().map(|ip| SocketAddr::new(*ip, ports[0])).collect();
    let mut seen: HashSet<_> = addresses.iter().copied().collect();
    if kind != TransportKind::MasqueH3 {
        let v4: Vec<_> = if masque {
            vec![[162, 159, 198], [162, 159, 199]]
        } else {
            WG_V4.to_vec()
        };
        let v6: &[u16] = if masque { &[0x103, 0x104] } else { &[0xd0, 0xd1] };
        let pools = if ipv6 { v6.len() } else { v4.len() };
        for sample in 0..if extended { 5_u8 } else { 1 } {
            for pool in 0..pools {
                let mut hash = Sha256::new();
                hash.update(seed.to_be_bytes());
                hash.update([pool as u8, u8::from(ipv6)]);
                let base = hash.finalize();
                let ip = if ipv6 {
                    let mut octets = [0_u8; 16];
                    octets[..6].copy_from_slice(&[0x26, 0x06, 0x47, 0x00, (v6[pool] >> 8) as u8, v6[pool] as u8]);
                    octets[8..16].copy_from_slice(&base[..8]);
                    octets[15] = octets[15].wrapping_add(sample);
                    IpAddr::V6(Ipv6Addr::from(octets))
                } else {
                    let prefix = v4[pool];
                    let host = base[0].wrapping_add(sample.wrapping_mul(base[1] | 1));
                    IpAddr::V4(Ipv4Addr::new(prefix[0], prefix[1], prefix[2], host))
                };
                ips.push(ip);
            }
        }
    }
    for &port in ports {
        for &ip in &ips {
            let endpoint = SocketAddr::new(ip, port);
            if seen.insert(endpoint) {
                addresses.push(endpoint);
            }
        }
    }
    addresses
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::warp_profiles::{WarpProfile, WarpTransport};

    #[test]
    fn h2_standard_endpoints_precede_random_subnet_expansion() {
        for seed in [0, 38, 155, u64::MAX] {
            let addresses = targets(TransportKind::MasqueH2, false, true, seed);
            assert_eq!(
                &addresses[..2],
                &[
                    "162.159.198.2:443".parse().unwrap(),
                    "162.159.198.1:443".parse().unwrap()
                ]
            );
            assert!(
                addresses
                    .iter()
                    .any(|address| address.ip().to_string().starts_with("162.159.199."))
            );
        }
    }

    #[test]
    fn family_specific_targets_are_bounded_unique_and_supported_by_the_runtime() -> anyhow::Result<()> {
        for kind in [
            TransportKind::WireGuard,
            TransportKind::Awg,
            TransportKind::MasqueH3,
            TransportKind::MasqueH2,
        ] {
            for ipv6 in [false, true] {
                let addresses = targets(kind, ipv6, true, 42);
                assert!(!addresses.is_empty() && addresses.len() <= 5000);
                assert_eq!(
                    addresses
                        .iter()
                        .copied()
                        .collect::<std::collections::HashSet<_>>()
                        .len(),
                    addresses.len()
                );
                for address in addresses {
                    assert_eq!(address.is_ipv6(), ipv6);
                    let transport = match kind {
                        TransportKind::WireGuard => WarpTransport::WireGuard,
                        TransportKind::Awg => WarpTransport::Awg {
                            jc: 4,
                            jmin: 40,
                            jmax: 70,
                            i1: "<r 32>".into(),
                        },
                        TransportKind::MasqueH3 => WarpTransport::MasqueH3 {
                            sni: "consumer-masque.cloudflareclient.com".into(),
                        },
                        TransportKind::MasqueH2 => WarpTransport::MasqueH2 {
                            sni: "consumer-masque.cloudflareclient.com".into(),
                        },
                    };
                    WarpProfile::new(address, transport)?;
                }
            }
        }
        Ok(())
    }

    #[test]
    fn explicit_h3_ipv6_hosts_are_checked_before_ports_without_enumerating_prefixes() {
        let addresses = targets(TransportKind::MasqueH3, true, true, 7);
        let expected: Vec<SocketAddr> = [
            "[2606:4700:103::2]:443",
            "[2606:4700:103::1]:443",
            "[2606:4700:104::2]:443",
            "[2606:4700:104::1]:443",
        ]
        .into_iter()
        .map(|address| address.parse().unwrap())
        .collect();
        assert_eq!(&addresses[..4], expected.as_slice());
        assert_eq!(addresses.len(), 28);
    }
}
