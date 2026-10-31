use crate::config::{
    warp_account::WarpAccount,
    warp_masque_account::MasqueAccount,
    warp_profiles::{WarpProfile, WarpTransport},
};

#[derive(Clone, Copy)]
pub(crate) enum RuntimeIdentity<'a> {
    WireGuard(&'a WarpAccount),
    Masque(&'a MasqueAccount),
}

pub(crate) enum RuntimeAccount {
    WireGuard(WarpAccount),
    Masque(MasqueAccount),
}

impl RuntimeAccount {
    pub(crate) fn identity(&self) -> RuntimeIdentity<'_> {
        match self {
            Self::WireGuard(account) => RuntimeIdentity::WireGuard(account),
            Self::Masque(account) => RuntimeIdentity::Masque(account),
        }
    }

    pub(crate) fn bind_catalog(&self, catalog: &mut crate::config::warp_profiles::WarpCatalog) {
        match self {
            Self::WireGuard(account) => catalog.bind_registration(Some(account)),
            Self::Masque(account) => catalog.bind_masque_registration(Some(account)),
        }
    }

    async fn load_optional(root: &std::path::Path, profile: &WarpProfile) -> anyhow::Result<Option<Self>> {
        use crate::config::{
            warp_account::WarpAccountStore, warp_masque_account::MasqueAccountStore, warp_profiles::RegistrationFamily,
        };
        profile.validate()?;
        Ok(match profile.transport.family() {
            RegistrationFamily::WireGuard => WarpAccountStore::new(root.into()).load().await?.map(Self::WireGuard),
            RegistrationFamily::Masque => MasqueAccountStore::new(root.into()).load().await?.map(Self::Masque),
        })
    }

    pub(crate) async fn load(root: &std::path::Path, profile: &WarpProfile) -> anyhow::Result<Self> {
        Self::load_optional(root, profile)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Required WARP transport identity is missing"))
    }

    pub(crate) async fn ensure(root: &std::path::Path, profile: &WarpProfile) -> anyhow::Result<(Self, bool)> {
        Self::ensure_with_route(root, profile, Default::default()).await
    }

    pub(crate) async fn ensure_with_route(
        root: &std::path::Path,
        profile: &WarpProfile,
        route: std::sync::Arc<super::warp_registration_http::RegistrationRoute>,
    ) -> anyhow::Result<(Self, bool)> {
        use crate::config::{
            warp_account::WarpAccountStore, warp_masque_account::MasqueAccountStore, warp_profiles::RegistrationFamily,
        };
        if let Some(account) = Self::load_optional(root, profile).await? {
            return Ok((account, true));
        }
        let account = match profile.transport.family() {
            RegistrationFamily::WireGuard => {
                let store = WarpAccountStore::new(root.into());
                super::warp_registration::ensure_registration(
                    &store,
                    &super::warp_registration::HttpWarpRegistrar::new()?.using_route(route),
                )
                .await?;
                Self::WireGuard(
                    store
                        .load()
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("WARP registration is missing"))?,
                )
            }
            RegistrationFamily::Masque => Self::Masque(
                super::warp_masque_registration::ensure_registration(
                    &MasqueAccountStore::new(root.into()),
                    &super::warp_masque_registration::HttpMasqueRegistrar::new()?.using_route(route),
                )
                .await?,
            ),
        };
        Ok((account, false))
    }
}

pub(crate) fn profile_worker_request(
    identity: RuntimeIdentity<'_>,
    profile: &WarpProfile,
    targets: Vec<std::net::SocketAddr>,
    timeout_ms: u32,
) -> anyhow::Result<super::warp_engine::wire::ProbeRequest> {
    use super::warp_engine::wire::{
        AwgParameters, EcIdentity, Identity, Parameters, ProbeRequest, Transport, WgIdentity,
    };
    profile.validate()?;
    for endpoint in &targets {
        WarpProfile::new(*endpoint, profile.transport.clone())?;
    }
    let identity = match (identity, profile.transport.family()) {
        (RuntimeIdentity::WireGuard(account), crate::config::warp_profiles::RegistrationFamily::WireGuard) => {
            let (private, peer, ipv4, ipv6) = account.tunnel_identity();
            Identity::WireGuard(WgIdentity {
                private_key: private.into(),
                peer_public_key: peer.into(),
                ipv4,
                ipv6,
                reserved: [0; 3],
            })
        }
        (RuntimeIdentity::Masque(account), crate::config::warp_profiles::RegistrationFamily::Masque) => {
            let (private, peer, ipv4, ipv6) = account.tunnel_identity();
            Identity::Masque(EcIdentity {
                private_key_der: private.into(),
                peer_public_key_der: peer.into(),
                ipv4,
                ipv6,
            })
        }
        _ => anyhow::bail!("WARP worker identity does not match transport family"),
    };
    let (transport, parameters) = match &profile.transport {
        WarpTransport::WireGuard => (Transport::WireGuard, Parameters { awg: None, sni: None }),
        WarpTransport::Awg { jc, jmin, jmax, i1 } => (
            Transport::Awg,
            Parameters {
                awg: Some(AwgParameters {
                    jc: *jc,
                    jmin: *jmin,
                    jmax: *jmax,
                    i1: i1.clone(),
                }),
                sni: None,
            },
        ),
        WarpTransport::MasqueH3 { sni } => (
            Transport::MasqueH3,
            Parameters {
                awg: None,
                sni: Some(sni.clone()),
            },
        ),
        WarpTransport::MasqueH2 { sni } => (
            Transport::MasqueH2,
            Parameters {
                awg: None,
                sni: Some(sni.clone()),
            },
        ),
    };
    let request = ProbeRequest {
        transport,
        ipv6: profile.endpoint.is_ipv6(),
        targets,
        identity,
        parameters,
        timeout_ms,
        tunnel_limit: 4,
    };
    request.validate()?;
    Ok(request)
}

pub(crate) fn profile_probe_config(
    identity: RuntimeIdentity<'_>,
    port: u16,
    profile: &WarpProfile,
) -> anyhow::Result<serde_json::Value> {
    profile.validate()?;
    let proxy = match (identity, &profile.transport) {
        (RuntimeIdentity::WireGuard(account), WarpTransport::WireGuard) => {
            account.wireguard_proxy(profile.endpoint.ip(), profile.endpoint.port())
        }
        (RuntimeIdentity::WireGuard(account), WarpTransport::Awg { jc, jmin, jmax, i1 }) => {
            let mut proxy = account.wireguard_proxy(profile.endpoint.ip(), profile.endpoint.port());
            proxy["amnezia-wg-option"] = serde_json::json!({
                "jc":jc,"jmin":jmin,"jmax":jmax,
                "s1":0,"s2":0,"h1":1,"h2":2,"h3":3,"h4":4
            });
            if !i1.is_empty() {
                proxy["amnezia-wg-option"]["i1"] = i1.clone().into();
            }
            proxy
        }
        (RuntimeIdentity::Masque(account), WarpTransport::MasqueH3 { sni } | WarpTransport::MasqueH2 { sni }) => {
            let mut proxy = account.masque_proxy(profile.endpoint.ip(), profile.endpoint.port());
            proxy["sni"] = sni.clone().into();
            if matches!(profile.transport, WarpTransport::MasqueH2 { .. }) {
                proxy["network"] = "h2".into();
            }
            proxy
        }
        _ => anyhow::bail!("WARP runtime identity does not match transport family"),
    };
    Ok(serde_json::json!({
        "mixed-port":port, "bind-address":"127.0.0.1", "allow-lan":false,
        "mode":"rule", "log-level":"debug", "ipv6":false,
        "tun":{"enable":false}, "dns":{"enable":false},
        "proxies":[proxy], "rules":["MATCH,WARP"]
    }))
}

pub(crate) fn profile_runtime_config(
    identity: RuntimeIdentity<'_>,
    profile: &WarpProfile,
    routing: &crate::config::tropa::RoutingPreferences,
) -> anyhow::Result<serde_json::Value> {
    let mut config = profile_probe_config(identity, 0, profile)?;
    config["rules"] = serde_json::to_value(routing.rules()?)?;
    config["find-process-mode"] = "always".into();
    config["log-level"] = "error".into();
    config["ipv6"] = true.into();
    config["tun"] = serde_json::json!({
        "enable":false, "device":"Tropa", "stack":"mixed",
        "auto-route":true, "strict-route":true, "auto-detect-interface":true,
        "dns-hijack":["any:53", "tcp://any:53"],
        "route-exclude-address":[format!("{}/{}", profile.endpoint.ip(), if profile.endpoint.is_ipv4() { 32 } else { 128 })]
    });
    config["dns"] = serde_json::json!({
        "enable":true, "listen":"127.0.0.1:0", "ipv6":true,
        "enhanced-mode":"fake-ip", "fake-ip-range":"198.18.0.1/16",
        "default-nameserver":["https://1.1.1.1/dns-query#DIRECT"],
        "nameserver":[if routing.mode == crate::config::tropa::RoutingMode::All {
            "https://1.1.1.1/dns-query#WARP"
        } else {
            "https://1.1.1.1/dns-query#DIRECT"
        }]
    });
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn awg_without_init_packet_retains_junk_parameters_and_omits_i1() -> anyhow::Result<()> {
        let account = wireguard()?;
        let profile = WarpProfile::new(
            "188.114.98.58:2408".parse()?,
            WarpTransport::Awg {
                jc: 5,
                jmin: 20,
                jmax: 80,
                i1: String::new(),
            },
        )?;
        let config = profile_probe_config(RuntimeIdentity::WireGuard(&account), 12345, &profile)?;
        let options = &config["proxies"][0]["amnezia-wg-option"];
        assert_eq!(options["jc"], 5);
        assert_eq!(options["jmin"], 20);
        assert_eq!(options["jmax"], 80);
        assert!(options.get("i1").is_none());
        assert_eq!(options["h1"], 1);
        Ok(())
    }
    use crate::config::{
        tropa::{RoutingMode, RoutingPreferences},
        warp_account::WarpAccount,
        warp_masque_account::{MasqueAccount, MasqueKeys, PendingMasque},
        warp_profiles::{WarpProfile, WarpTransport},
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    fn wireguard() -> anyhow::Result<WarpAccount> {
        WarpAccount::from_warpscout_json(&serde_json::to_vec(&serde_json::json!({
            "id":"synthetic-wg-device", "token":"synthetic-wg-token",
            "private_key":STANDARD.encode([7u8;32]), "peer_public_key":STANDARD.encode([9u8;32]),
            "ipv4":"172.16.0.2","ipv6":null
        }))?)
    }

    fn masque() -> anyhow::Result<(MasqueAccount, serde_json::Value, String)> {
        let keys = MasqueKeys::generate()?;
        let expected_keys = serde_json::to_value(&keys)?;
        let peer = serde_json::to_value(MasqueKeys::generate()?)?["public_key"]
            .as_str()
            .unwrap()
            .to_owned();
        let pending = PendingMasque::from_created(
            keys,
            br#"{"id":"synthetic-masque-device","token":"synthetic-masque-token"}"#,
        )?;
        let account = MasqueAccount::from_enrollment(
            pending,
            &serde_json::to_vec(&serde_json::json!({"config":{
                "peers":[{"public_key":format!("-----BEGIN PUBLIC KEY-----\n{peer}\n-----END PUBLIC KEY-----")}],
                "interface":{"addresses":{"v4":"172.16.0.2","v6":"2606:4700:110:8abc::2"}}
            }}))?,
        )?;
        Ok((account, expected_keys, peer))
    }

    #[test]
    fn i1_only_awg_reaches_both_engines_without_enabling_junk_packets() -> anyhow::Result<()> {
        let account = wireguard()?;
        let profile = WarpProfile::new(
            "188.114.98.58:2408".parse()?,
            WarpTransport::Awg {
                jc: 0,
                jmin: 0,
                jmax: 0,
                i1: "<r 32>".into(),
            },
        )?;
        let runtime = profile_probe_config(RuntimeIdentity::WireGuard(&account), 12345, &profile)?;
        assert_eq!(runtime["proxies"][0]["amnezia-wg-option"]["jc"], 0);
        assert_eq!(runtime["proxies"][0]["amnezia-wg-option"]["i1"], "<r 32>");
        let request = profile_worker_request(
            RuntimeIdentity::WireGuard(&account),
            &profile,
            vec![profile.endpoint],
            5000,
        )?;
        let payload = serde_json::to_value(&request)?;
        assert_eq!(payload["parameters"]["awg"]["jc"], 0);
        assert_eq!(payload["parameters"]["awg"]["i1"], "<r 32>");
        assert!(
            WarpProfile::new(
                profile.endpoint,
                WarpTransport::Awg {
                    jc: 0,
                    jmin: 0,
                    jmax: 0,
                    i1: String::new()
                }
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn worker_requests_keep_enrolled_der_and_transport_parameters_without_registration_tokens() -> anyhow::Result<()> {
        let wg = wireguard()?;
        let (ec, keys, peer) = masque()?;
        for (identity, endpoint, transport) in [
            (
                RuntimeIdentity::WireGuard(&wg),
                "188.114.98.58:2408",
                WarpTransport::WireGuard,
            ),
            (
                RuntimeIdentity::WireGuard(&wg),
                "188.114.98.58:2408",
                WarpTransport::Awg {
                    jc: 5,
                    jmin: 20,
                    jmax: 80,
                    i1: "<b 0x010203>".into(),
                },
            ),
            (
                RuntimeIdentity::Masque(&ec),
                "162.159.198.2:443",
                WarpTransport::MasqueH2 {
                    sni: "www.google.com".into(),
                },
            ),
            (
                RuntimeIdentity::Masque(&ec),
                "[2606:4700:103::2]:443",
                WarpTransport::MasqueH3 {
                    sni: "www.google.com".into(),
                },
            ),
        ] {
            let profile = WarpProfile::new(endpoint.parse()?, transport)?;
            let request = profile_worker_request(identity, &profile, vec![profile.endpoint], 7000)?;
            request.validate()?;
            let payload = serde_json::to_value(&request)?;
            assert_eq!(payload["targets"], serde_json::json!([endpoint]));
            assert_eq!(payload["ipv6"], profile.endpoint.is_ipv6());
            assert_eq!(payload["timeoutMs"], 7000);
            assert_eq!(payload["tunnelLimit"], 4);
            match &profile.transport {
                WarpTransport::WireGuard | WarpTransport::Awg { .. } => {
                    assert_eq!(
                        payload["identity"]["wireGuard"]["privateKey"],
                        STANDARD.encode([7u8; 32])
                    );
                    assert_eq!(
                        payload["identity"]["wireGuard"]["peerPublicKey"],
                        STANDARD.encode([9u8; 32])
                    );
                    assert_eq!(
                        payload["identity"]["wireGuard"]["reserved"],
                        serde_json::json!([0, 0, 0])
                    );
                    assert!(payload["identity"]["wireGuard"].get("ipv6").is_none());
                    if matches!(profile.transport, WarpTransport::Awg { .. }) {
                        assert_eq!(
                            payload["parameters"]["awg"],
                            serde_json::json!({"jc":5,"jmin":20,"jmax":80,"i1":"<b 0x010203>"})
                        );
                    }
                }
                _ => {
                    assert_eq!(payload["identity"]["masque"]["privateKeyDer"], keys["private_key"]);
                    assert_eq!(payload["identity"]["masque"]["peerPublicKeyDer"], peer);
                    assert_eq!(payload["identity"]["masque"]["ipv6"], "2606:4700:110:8abc::2");
                    assert_eq!(payload["parameters"]["sni"], "www.google.com");
                }
            }
            let encoded = serde_json::to_string(&request)?;
            assert!(!encoded.contains("synthetic-wg-token") && !encoded.contains("synthetic-masque-token"));
        }
        let profile = WarpProfile::new(
            "162.159.198.2:443".parse()?,
            WarpTransport::MasqueH2 {
                sni: "www.google.com".into(),
            },
        )?;
        assert!(
            profile_worker_request(RuntimeIdentity::WireGuard(&wg), &profile, vec![profile.endpoint], 7000).is_err()
        );
        assert!(
            profile_worker_request(
                RuntimeIdentity::Masque(&ec),
                &profile,
                vec!["[2606:4700:103::2]:443".parse()?],
                7000
            )
            .is_err()
        );
        Ok(())
    }

    #[tokio::test]
    async fn runtime_loads_only_the_profiles_family_and_never_substitutes_another_identity() -> anyhow::Result<()> {
        use crate::config::{warp_account::WarpAccountStore, warp_masque_account::MasqueAccountStore};
        let root = std::env::temp_dir().join(format!("tropa-runtime-identity-{}", nanoid::nanoid!()));
        let wg = wireguard()?;
        let (masque, keys, peer) = masque()?;
        WarpAccountStore::new(root.clone()).save(&wg).await?;
        MasqueAccountStore::new(root.clone()).save(&masque).await?;
        let wg_profile = WarpProfile::new("188.114.96.10:500".parse()?, WarpTransport::WireGuard)?;
        let masque_profile = WarpProfile::new(
            "162.159.198.1:443".parse()?,
            WarpTransport::MasqueH2 {
                sni: "consumer-masque.cloudflareclient.com".into(),
            },
        )?;
        let loaded = RuntimeAccount::load(&root, &masque_profile).await?;
        let config = profile_probe_config(loaded.identity(), 12345, &masque_profile)?;
        assert_eq!(config["proxies"][0]["private-key"], keys["private_key"]);
        assert_eq!(config["proxies"][0]["public-key"], peer);
        tokio::fs::write(root.join("secrets/warp-account.json"), b"invalid-wg-marker").await?;
        RuntimeAccount::load(&root, &masque_profile).await?;
        assert!(RuntimeAccount::load(&root, &wg_profile).await.is_err());
        WarpAccountStore::new(root.clone()).save(&wg).await?;
        tokio::fs::write(root.join("secrets/masque-account.json"), b"invalid-masque-marker").await?;
        let loaded = RuntimeAccount::load(&root, &wg_profile).await?;
        let config = profile_probe_config(loaded.identity(), 12345, &wg_profile)?;
        assert_eq!(config["proxies"][0]["private-key"], STANDARD.encode([7u8; 32]));
        assert!(RuntimeAccount::load(&root, &masque_profile).await.is_err());
        for name in ["warp-account.json", "masque-account.json"] {
            tokio::fs::remove_file(root.join("secrets").join(name)).await?;
        }
        assert!(RuntimeAccount::load(&root, &wg_profile).await.is_err());
        assert!(RuntimeAccount::load(&root, &masque_profile).await.is_err());
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    #[test]
    fn ipv6_outer_endpoint_is_a_literal_server_with_a_single_host_exclusion() -> anyhow::Result<()> {
        let wg = wireguard()?;
        let (masque, _, _) = masque()?;
        for (endpoint, transport, identity, server) in [
            (
                "[2606:4700:d0::1234]:2408",
                WarpTransport::WireGuard,
                RuntimeIdentity::WireGuard(&wg),
                "2606:4700:d0::1234",
            ),
            (
                "[2606:4700:103::1]:443",
                WarpTransport::MasqueH3 {
                    sni: "www.google.com".into(),
                },
                RuntimeIdentity::Masque(&masque),
                "2606:4700:103::1",
            ),
        ] {
            let profile = WarpProfile::new(endpoint.parse()?, transport)?;
            let config = profile_runtime_config(identity, &profile, &Default::default())?;
            assert_eq!(config["proxies"][0]["server"], server);
            assert_eq!(
                config["tun"]["route-exclude-address"],
                serde_json::json!([format!("{server}/128")])
            );
        }
        Ok(())
    }

    #[test]
    fn runtime_excludes_only_its_outer_endpoint_and_uses_ip_literal_bootstrap_dns() -> anyhow::Result<()> {
        let wg = wireguard()?;
        let (masque, _, _) = masque()?;
        let wg_profile = WarpProfile::new("188.114.96.10:500".parse()?, WarpTransport::WireGuard)?;
        let masque_profile = WarpProfile::new(
            "162.159.198.1:443".parse()?,
            WarpTransport::MasqueH3 {
                sni: "consumer-masque.cloudflareclient.com".into(),
            },
        )?;
        for (identity, profile, excluded) in [
            (RuntimeIdentity::WireGuard(&wg), &wg_profile, "188.114.96.10/32"),
            (RuntimeIdentity::Masque(&masque), &masque_profile, "162.159.198.1/32"),
        ] {
            let config = profile_runtime_config(identity, profile, &Default::default())?;
            assert_eq!(config["tun"]["route-exclude-address"], serde_json::json!([excluded]));
            assert_eq!(
                config["dns"]["default-nameserver"],
                serde_json::json!(["https://1.1.1.1/dns-query#DIRECT"])
            );
            assert_eq!(
                config["dns"]["nameserver"],
                serde_json::json!(["https://1.1.1.1/dns-query#WARP"])
            );
            assert_eq!(config["rules"], serde_json::json!(["MATCH,WARP"]));
        }
        Ok(())
    }

    #[test]
    fn masque_modes_keep_enrolled_keys_sni_and_common_routing_without_disabling_peer_verification() -> anyhow::Result<()>
    {
        let (account, keys, peer) = masque()?;
        for transport in [
            WarpTransport::MasqueH3 {
                sni: "www.google.com".into(),
            },
            WarpTransport::MasqueH2 {
                sni: "www.google.com".into(),
            },
        ] {
            let h2 = matches!(transport, WarpTransport::MasqueH2 { .. });
            let profile = WarpProfile::new("162.159.198.1:443".parse()?, transport)?;
            let probe = profile_probe_config(RuntimeIdentity::Masque(&account), 12345, &profile)?;
            let proxy = &probe["proxies"][0];
            assert_eq!(proxy["type"], "masque");
            assert_eq!(proxy["private-key"], keys["private_key"]);
            assert_eq!(proxy["public-key"], peer);
            assert_ne!(proxy["public-key"], keys["public_key"]);
            assert_eq!(proxy["server"], "162.159.198.1");
            assert_eq!(proxy["port"], 443);
            assert_eq!(proxy["sni"], "www.google.com");
            assert_eq!(proxy["uri"], "https://cloudflareaccess.com");
            assert_eq!(proxy["udp"], true);
            assert_eq!(proxy["ip"], "172.16.0.2");
            assert_eq!(proxy["ipv6"], "2606:4700:110:8abc::2");
            assert_eq!(proxy["skip-cert-verify"], false);
            assert_eq!(
                proxy.get("network").and_then(serde_json::Value::as_str),
                if h2 { Some("h2") } else { None }
            );
            assert_eq!(probe["mixed-port"], 12345);
            assert_eq!(probe["bind-address"], "127.0.0.1");
            assert_eq!(probe["allow-lan"], false);
            assert_eq!(probe["tun"]["enable"], false);
            assert_eq!(probe["rules"], serde_json::json!(["MATCH,WARP"]));
            assert!(!probe.to_string().contains("synthetic-"));
            let routing = RoutingPreferences {
                mode: RoutingMode::Selected,
                applications: vec![r"C:\Apps\Firefox\firefox.exe".into()],
            };
            let runtime = profile_runtime_config(RuntimeIdentity::Masque(&account), &profile, &routing)?;
            assert_eq!(runtime["proxies"], probe["proxies"]);
            assert_eq!(runtime["tun"]["enable"], false);
            assert_eq!(runtime["find-process-mode"], "always");
            assert_eq!(runtime["rules"][1], "MATCH,DIRECT");
            assert!(runtime["rules"][0].as_str().unwrap().starts_with("PROCESS-PATH-REGEX,"));
            assert_eq!(
                runtime["dns"]["nameserver"],
                serde_json::json!(["https://1.1.1.1/dns-query#DIRECT"])
            );
            assert!(!runtime.to_string().contains("synthetic-"));
        }
        Ok(())
    }

    #[test]
    fn incompatible_identity_is_rejected_before_config_or_child_process_creation() -> anyhow::Result<()> {
        let wg = wireguard()?;
        let (masque, _, _) = masque()?;
        let wg_profile = WarpProfile::new("188.114.96.10:500".parse()?, WarpTransport::WireGuard)?;
        let masque_profile = WarpProfile::new(
            "162.159.198.1:443".parse()?,
            WarpTransport::MasqueH3 {
                sni: "consumer-masque.cloudflareclient.com".into(),
            },
        )?;
        for (identity, profile) in [
            (RuntimeIdentity::WireGuard(&wg), &masque_profile),
            (RuntimeIdentity::Masque(&masque), &wg_profile),
        ] {
            let error = profile_probe_config(identity, 12345, profile).unwrap_err();
            assert!(error.to_string().contains("identity"));
            assert!(!error.to_string().contains("synthetic"));
        }
        Ok(())
    }
}
