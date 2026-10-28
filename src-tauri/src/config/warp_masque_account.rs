use anyhow::{Result, anyhow, ensure};
use aws_lc_rs::{
    encoding::{AsDer, EcPrivateKeyRfc5915Der, PublicKeyX509Der},
    signature::{self, EcdsaKeyPair, KeyPair},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::PathBuf,
};
use tokio::io::AsyncReadExt;

const MAX_PENDING_BYTES: u64 = 64 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MasqueKeys {
    private_key: String,
    public_key: String,
}

impl MasqueKeys {
    pub(crate) fn generate() -> Result<Self> {
        let key = EcdsaKeyPair::generate(&signature::ECDSA_P256_SHA256_ASN1_SIGNING)
            .map_err(|_| anyhow!("Unable to generate MASQUE EC identity"))?;
        let private: EcPrivateKeyRfc5915Der = key
            .private_key()
            .as_der()
            .map_err(|_| anyhow!("Unable to encode MASQUE private key"))?;
        let public: PublicKeyX509Der = key
            .public_key()
            .as_der()
            .map_err(|_| anyhow!("Unable to encode MASQUE public key"))?;
        Ok(Self {
            private_key: STANDARD.encode(private.as_ref()),
            public_key: STANDARD.encode(public.as_ref()),
        })
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.private_key.len() <= 4096 && self.public_key.len() <= 4096,
            "MASQUE EC key exceeds its limit"
        );
        let private = STANDARD
            .decode(&self.private_key)
            .map_err(|_| anyhow!("Invalid MASQUE private key encoding"))?;
        let public = STANDARD
            .decode(&self.public_key)
            .map_err(|_| anyhow!("Invalid MASQUE public key encoding"))?;
        let key = EcdsaKeyPair::from_private_key_der(&signature::ECDSA_P256_SHA256_ASN1_SIGNING, &private)
            .map_err(|_| anyhow!("Invalid MASQUE P-256 private key"))?;
        let canonical_private: EcPrivateKeyRfc5915Der = key
            .private_key()
            .as_der()
            .map_err(|_| anyhow!("Unable to validate MASQUE private key format"))?;
        let canonical_public: PublicKeyX509Der = key
            .public_key()
            .as_der()
            .map_err(|_| anyhow!("Unable to validate MASQUE public key format"))?;
        ensure!(
            private == canonical_private.as_ref(),
            "MASQUE private key must use SEC1 DER"
        );
        ensure!(
            public == canonical_public.as_ref(),
            "MASQUE public key does not match the private key"
        );
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PendingMasque {
    id: String,
    token: String,
    keys: MasqueKeys,
}

impl PendingMasque {
    pub(crate) fn from_created(keys: MasqueKeys, bytes: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct Created {
            id: String,
            token: String,
        }
        ensure!(
            bytes.len() as u64 <= MAX_PENDING_BYTES,
            "MASQUE registration response exceeds its limit"
        );
        let created: Created =
            serde_json::from_slice(bytes).map_err(|_| anyhow!("Invalid MASQUE registration response"))?;
        let pending = Self {
            id: created.id,
            token: created.token,
            keys,
        };
        pending.validate()?;
        Ok(pending)
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn token(&self) -> &str {
        &self.token
    }
    pub(crate) fn public_key(&self) -> &str {
        &self.keys.public_key
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty()
                && self.id.len() <= 128
                && self
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "Invalid MASQUE device ID"
        );
        ensure!(
            !self.token.is_empty() && self.token.len() <= 4096 && self.token.bytes().all(|b| b.is_ascii_graphic()),
            "Invalid MASQUE device token"
        );
        self.keys.validate()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MasqueAccount {
    registration: PendingMasque,
    peer_public_key: String,
    ipv4: Ipv4Addr,
    ipv6: Option<Ipv6Addr>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum MasqueAccountStatus {
    Missing,
    Pending,
    Registered {
        #[serde(rename = "hasIpv6")]
        has_ipv6: bool,
    },
}

impl MasqueAccount {
    #[cfg(any(feature = "tropa-probe", feature = "portable", test))]
    pub(crate) fn tunnel_identity(&self) -> (&str, &str, Ipv4Addr, Option<Ipv6Addr>) {
        (
            &self.registration.keys.private_key,
            &self.peer_public_key,
            self.ipv4,
            self.ipv6,
        )
    }

    pub(crate) fn id(&self) -> &str {
        self.registration.id()
    }

    #[cfg(any(feature = "tropa-probe", feature = "portable", test))]
    pub(crate) fn scout_account(&self) -> serde_json::Value {
        let peer = format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
            self.peer_public_key
        );
        let masque = serde_json::json!({
            "private_key":self.registration.keys.private_key,
            "peer_public_key":peer,
            "ipv4":self.ipv4.to_string(),
            "ipv6":self.ipv6.map(|ip| ip.to_string()).unwrap_or_default()
        });
        // v0.16.1 requires nonempty root keys even for MASQUE; only the nested EC identity is used.
        // Keep genuine EC values here so this envelope cannot accidentally serve as a WG identity.
        let mut envelope = masque.clone();
        envelope["masque"] = masque;
        envelope
    }

    pub(crate) fn masque_proxy(&self, server: IpAddr, port: u16) -> serde_json::Value {
        let mut proxy = serde_json::json!({
            "name":"WARP", "type":"masque", "server":server.to_string(), "port":port,
            "private-key":self.registration.keys.private_key, "public-key":self.peer_public_key,
            "ip":self.ipv4.to_string(), "udp":true, "mtu":1280,
            "uri":"https://cloudflareaccess.com", "skip-cert-verify":false,
            "remote-dns-resolve":true, "dns":["1.1.1.1"]
        });
        if let Some(ipv6) = self.ipv6 {
            proxy["ipv6"] = ipv6.to_string().into();
        }
        proxy
    }

    pub(crate) fn fingerprint(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(self.registration.id().as_bytes());
        hash.update([0]);
        hash.update(self.registration.public_key().as_bytes());
        hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
    }

    pub(crate) fn status(&self) -> MasqueAccountStatus {
        MasqueAccountStatus::Registered {
            has_ipv6: self.ipv6.is_some(),
        }
    }

    pub(crate) fn from_enrollment(registration: PendingMasque, bytes: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct Response {
            config: Config,
        }
        #[derive(Deserialize)]
        struct Config {
            peers: Vec<Peer>,
            interface: Interface,
        }
        #[derive(Deserialize)]
        struct Peer {
            public_key: String,
        }
        #[derive(Deserialize)]
        struct Interface {
            addresses: Addresses,
        }
        #[derive(Deserialize)]
        struct Addresses {
            v4: String,
            #[serde(default)]
            v6: String,
        }
        ensure!(
            bytes.len() as u64 <= MAX_PENDING_BYTES,
            "MASQUE enrollment response exceeds its limit"
        );
        let response: Response =
            serde_json::from_slice(bytes).map_err(|_| anyhow!("Invalid MASQUE enrollment response"))?;
        let peer = response
            .config
            .peers
            .first()
            .ok_or_else(|| anyhow!("MASQUE enrollment has no peer"))?;
        let encoded = peer
            .public_key
            .trim()
            .strip_prefix("-----BEGIN PUBLIC KEY-----")
            .and_then(|value| value.strip_suffix("-----END PUBLIC KEY-----"))
            .ok_or_else(|| anyhow!("Invalid MASQUE peer PEM"))?;
        let encoded: String = encoded.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        let peer_bytes = STANDARD
            .decode(encoded)
            .map_err(|_| anyhow!("Invalid MASQUE peer encoding"))?;
        let account = Self {
            registration,
            peer_public_key: STANDARD.encode(peer_bytes),
            ipv4: response
                .config
                .interface
                .addresses
                .v4
                .parse()
                .map_err(|_| anyhow!("Invalid MASQUE IPv4 address"))?,
            ipv6: if response.config.interface.addresses.v6.is_empty() {
                None
            } else {
                Some(
                    response
                        .config
                        .interface
                        .addresses
                        .v6
                        .parse()
                        .map_err(|_| anyhow!("Invalid MASQUE IPv6 address"))?,
                )
            },
        };
        account.validate()?;
        Ok(account)
    }

    fn validate(&self) -> Result<()> {
        self.registration.validate()?;
        ensure!(self.peer_public_key.len() <= 4096, "MASQUE peer key exceeds its limit");
        let peer = STANDARD
            .decode(&self.peer_public_key)
            .map_err(|_| anyhow!("Invalid MASQUE peer encoding"))?;
        let parsed = signature::ParsedPublicKey::new(&signature::ECDSA_P256_SHA256_ASN1, &peer)
            .map_err(|_| anyhow!("Invalid MASQUE P-256 peer key"))?;
        let canonical: PublicKeyX509Der = parsed.as_der().map_err(|_| anyhow!("Invalid MASQUE peer format"))?;
        ensure!(peer == canonical.as_ref(), "MASQUE peer key must use SPKI DER");
        ensure!(
            !self.ipv4.is_unspecified()
                && !self.ipv4.is_loopback()
                && !self.ipv4.is_multicast()
                && !self.ipv4.is_broadcast()
                && !self.ipv4.is_link_local(),
            "Invalid MASQUE IPv4 address"
        );
        if let Some(ip) = self.ipv6 {
            ensure!(
                !ip.is_unspecified() && !ip.is_loopback() && !ip.is_multicast() && !ip.is_unicast_link_local(),
                "Invalid MASQUE IPv6 address"
            );
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAccount {
    version: u32,
    registration: MasqueAccount,
}

#[derive(Serialize)]
struct StoredAccountRef<'a> {
    version: u32,
    registration: &'a MasqueAccount,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredPending {
    version: u32,
    registration: PendingMasque,
}

#[derive(Serialize)]
struct StoredPendingRef<'a> {
    version: u32,
    registration: &'a PendingMasque,
}

pub(crate) struct MasqueAccountStore {
    root: PathBuf,
}

impl MasqueAccountStore {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) async fn status(&self) -> Result<MasqueAccountStatus> {
        if let Some(account) = self.load().await? {
            return Ok(account.status());
        }
        Ok(if self.load_pending().await?.is_some() {
            MasqueAccountStatus::Pending
        } else {
            MasqueAccountStatus::Missing
        })
    }

    pub(crate) async fn load(&self) -> Result<Option<MasqueAccount>> {
        let file = match tokio::fs::File::open(self.root.join("secrets/masque-account.json")).await {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(anyhow!("Unable to read MASQUE account")),
        };
        let mut bytes = Vec::new();
        file.take(MAX_PENDING_BYTES + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| anyhow!("Unable to read MASQUE account"))?;
        ensure!(
            bytes.len() as u64 <= MAX_PENDING_BYTES,
            "MASQUE account exceeds its limit"
        );
        let stored: StoredAccount =
            serde_json::from_slice(&bytes).map_err(|_| anyhow!("Invalid stored MASQUE account"))?;
        ensure!(stored.version == 1, "Unsupported MASQUE account version");
        stored.registration.validate()?;
        Ok(Some(stored.registration))
    }

    pub(crate) async fn save(&self, account: &MasqueAccount) -> Result<()> {
        account.validate()?;
        let json = serde_json::to_string_pretty(&StoredAccountRef {
            version: 1,
            registration: account,
        })
        .map_err(|_| anyhow!("Unable to serialize MASQUE account"))?;
        ensure!(
            json.len() as u64 <= MAX_PENDING_BYTES,
            "MASQUE account exceeds its limit"
        );
        tokio::fs::create_dir_all(self.root.join("secrets"))
            .await
            .map_err(|_| anyhow!("Unable to create MASQUE account directory"))?;
        crate::utils::help::save_yaml_str(&self.root.join("secrets/masque-account.json"), &json)
            .await
            .map_err(|_| anyhow!("Unable to save MASQUE account"))
    }

    pub(crate) async fn clear_pending(&self) -> Result<()> {
        match tokio::fs::remove_file(self.root.join("secrets/masque-pending.json")).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(anyhow!("Unable to remove pending MASQUE registration")),
        }
    }

    pub(crate) async fn save_pending(&self, pending: &PendingMasque) -> Result<()> {
        pending.validate()?;
        let json = serde_json::to_string_pretty(&StoredPendingRef {
            version: 1,
            registration: pending,
        })
        .map_err(|_| anyhow!("Unable to serialize pending MASQUE registration"))?;
        ensure!(
            json.len() as u64 <= MAX_PENDING_BYTES,
            "Pending MASQUE registration exceeds its limit"
        );
        tokio::fs::create_dir_all(self.root.join("secrets"))
            .await
            .map_err(|_| anyhow!("Unable to create MASQUE account directory"))?;
        crate::utils::help::save_yaml_str(&self.root.join("secrets/masque-pending.json"), &json)
            .await
            .map_err(|_| anyhow!("Unable to save pending MASQUE registration"))
    }

    pub(crate) async fn load_pending(&self) -> Result<Option<PendingMasque>> {
        let file = match tokio::fs::File::open(self.root.join("secrets/masque-pending.json")).await {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(anyhow!("Unable to read pending MASQUE registration")),
        };
        let mut bytes = Vec::new();
        file.take(MAX_PENDING_BYTES + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| anyhow!("Unable to read pending MASQUE registration"))?;
        ensure!(
            bytes.len() as u64 <= MAX_PENDING_BYTES,
            "Pending MASQUE registration exceeds its limit"
        );
        let pending: StoredPending =
            serde_json::from_slice(&bytes).map_err(|_| anyhow!("Invalid pending MASQUE registration"))?;
        ensure!(pending.version == 1, "Unsupported pending MASQUE registration version");
        pending.registration.validate()?;
        Ok(Some(pending.registration))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lc_rs::{
        encoding::{AsDer, EcPrivateKeyRfc5915Der, PublicKeyX509Der},
        signature::{self, EcdsaKeyPair, KeyPair},
    };
    use base64::engine::general_purpose::STANDARD;

    #[test]
    fn scanner_export_is_masque_only_without_device_management_credentials() -> Result<()> {
        let keys = MasqueKeys::generate()?;
        let private = keys.private_key.clone();
        let peer = MasqueKeys::generate()?;
        let account = MasqueAccount {
            registration: PendingMasque {
                id: "synthetic-device".into(),
                token: "synthetic-token".into(),
                keys,
            },
            peer_public_key: peer.public_key.clone(),
            ipv4: "172.16.0.2".parse()?,
            ipv6: None,
        };
        let exported = account.scout_account();
        assert_eq!(exported["masque"]["private_key"], private);
        assert_eq!(exported["masque"]["ipv4"], "172.16.0.2");
        assert_eq!(exported["masque"]["ipv6"], "");
        assert_eq!(exported["private_key"], exported["masque"]["private_key"]);
        assert_eq!(exported["peer_public_key"], exported["masque"]["peer_public_key"]);
        let pem = exported["masque"]["peer_public_key"].as_str().unwrap();
        assert!(pem.starts_with("-----BEGIN PUBLIC KEY-----\n"));
        let body: String = pem.lines().filter(|line| !line.starts_with("-----")).collect();
        assert_eq!(body, peer.public_key);
        let text = exported.to_string();
        assert!(!text.contains("synthetic-device"));
        assert!(!text.contains("synthetic-token"));
        assert!(exported.get("outer").is_none());
        Ok(())
    }

    #[tokio::test]
    async fn completed_enrollment_validates_peer_and_addresses_before_replacing_pending() -> Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-masque-{}", nanoid::nanoid!()));
        let store = MasqueAccountStore { root: root.clone() };
        let pending = PendingMasque {
            id: "synthetic-device".into(),
            token: "synthetic-token".into(),
            keys: MasqueKeys::generate()?,
        };
        store.save_pending(&pending).await?;
        let peer = MasqueKeys::generate()?;
        let response = serde_json::json!({"config": {
            "peers": [{"public_key": format!("-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----", peer.public_key)}],
            "interface": {"addresses": {"v4": "172.16.0.2", "v6": "2606:4700:110:8abc::2"}}
        }});
        for invalid in [
            serde_json::json!({}),
            {
                let mut v = response.clone();
                v["config"]["peers"][0]["public_key"] = "private-marker".into();
                v
            },
            {
                let mut v = response.clone();
                v["config"]["interface"]["addresses"]["v4"] = "127.0.0.1".into();
                v
            },
            {
                let mut v = response.clone();
                v["config"]["interface"]["addresses"]["v6"] = "::1".into();
                v
            },
        ] {
            let pending = store.load_pending().await?.unwrap();
            let error = MasqueAccount::from_enrollment(pending, &serde_json::to_vec(&invalid)?)
                .err()
                .unwrap();
            assert!(!error.to_string().contains("private-marker"));
            assert!(store.load_pending().await?.is_some());
            assert!(store.load().await?.is_none());
        }
        let account = MasqueAccount::from_enrollment(pending, &serde_json::to_vec(&response)?)?;
        store.save(&account).await?;
        let restarted = MasqueAccountStore { root: root.clone() };
        let loaded = restarted.load().await?.unwrap();
        assert_eq!(loaded.registration.id, "synthetic-device");
        assert_eq!(loaded.peer_public_key, peer.public_key);
        assert_eq!(
            loaded.registration.keys.private_key,
            account.registration.keys.private_key
        );
        assert_eq!(loaded.ipv4.to_string(), "172.16.0.2");
        assert_eq!(loaded.ipv6.unwrap().to_string(), "2606:4700:110:8abc::2");
        assert!(store.load_pending().await?.is_some());
        store.clear_pending().await?;
        assert!(store.load_pending().await?.is_none());
        assert!(store.load().await?.is_some());
        for name in ["masque-account.json"] {
            tokio::fs::remove_file(root.join("secrets").join(name)).await?;
        }
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    #[tokio::test]
    async fn pending_identity_survives_restart_without_touching_wireguard_files() -> Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-masque-{}", nanoid::nanoid!()));
        tokio::fs::create_dir_all(root.join("secrets")).await?;
        let wg = root.join("secrets/warp-account.json");
        tokio::fs::write(&wg, b"synthetic-existing-wg-marker").await?;
        let mut pending = PendingMasque {
            id: "synthetic-device".into(),
            token: "synthetic-token".into(),
            keys: MasqueKeys::generate()?,
        };
        let store = MasqueAccountStore { root: root.clone() };
        store.save_pending(&pending).await?;
        let restarted = MasqueAccountStore { root: root.clone() };
        let loaded = restarted
            .load_pending()
            .await?
            .expect("pending identity must survive restart");
        assert_eq!(loaded.keys.public_key, pending.keys.public_key);
        assert_eq!(loaded.id, "synthetic-device");
        assert_eq!(loaded.keys.private_key, pending.keys.private_key);
        let previous = tokio::fs::read(root.join("secrets/masque-pending.json")).await?;
        pending.keys.public_key = MasqueKeys::generate()?.public_key;
        assert!(store.save_pending(&pending).await.is_err());
        assert_eq!(
            tokio::fs::read(root.join("secrets/masque-pending.json")).await?,
            previous
        );
        assert_eq!(tokio::fs::read(&wg).await?, b"synthetic-existing-wg-marker");
        tokio::fs::remove_file(wg).await?;
        tokio::fs::remove_file(root.join("secrets/masque-pending.json")).await?;
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    #[test]
    fn generated_identity_is_sec1_private_and_matching_spki_public_not_wireguard() -> Result<()> {
        let keys = MasqueKeys::generate()?;
        let private = STANDARD.decode(&keys.private_key)?;
        let public = STANDARD.decode(&keys.public_key)?;
        let parsed = EcdsaKeyPair::from_private_key_der(&signature::ECDSA_P256_SHA256_ASN1_SIGNING, &private).unwrap();
        let sec1: EcPrivateKeyRfc5915Der = parsed.private_key().as_der().unwrap();
        let spki: PublicKeyX509Der = parsed.public_key().as_der().unwrap();
        assert_eq!(private, sec1.as_ref());
        assert_eq!(public, spki.as_ref());
        assert_ne!(private.len(), 32);
        let other = MasqueKeys::generate()?;
        assert_ne!(keys.public_key, other.public_key);
        let wrong_family = MasqueKeys {
            private_key: STANDARD.encode([7u8; 32]),
            public_key: keys.public_key,
        };
        assert!(wrong_family.validate().is_err());
        Ok(())
    }
}
