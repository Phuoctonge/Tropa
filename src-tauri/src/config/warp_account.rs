use std::{
    io::ErrorKind,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use crate::utils::help::save_yaml_str;

const MAX_ACCOUNT_BYTES: u64 = 256 * 1024;

pub(crate) struct WarpAccount {
    registration: Registration,
}

#[derive(Deserialize, Serialize)]
struct StoredAccount {
    version: u32,
    registration: Registration,
}

#[derive(Serialize)]
struct StoredAccountRef<'a> {
    version: u32,
    registration: &'a Registration,
}

pub(crate) struct WarpAccountStore {
    root: PathBuf,
}

async fn read_account_file(path: &Path) -> Result<Option<Vec<u8>>> {
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(anyhow!("Unable to read WARP account file")),
    };
    let mut bytes = Vec::new();
    file.take(MAX_ACCOUNT_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| anyhow!("Unable to read WARP account file"))?;
    ensure!(
        bytes.len() as u64 <= MAX_ACCOUNT_BYTES,
        "WARP account file is too large"
    );
    Ok(Some(bytes))
}

impl WarpAccountStore {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn path(&self) -> PathBuf {
        self.root.join("secrets/warp-account.json")
    }

    pub(crate) async fn status(&self) -> Result<WarpAccountStatus> {
        if let Some(account) = self.load().await? {
            return Ok(account.status());
        }
        self.load_pending().await?;
        Ok(WarpAccountStatus::Missing)
    }

    pub(crate) async fn load(&self) -> Result<Option<WarpAccount>> {
        Self::load_path(&self.path()).await
    }

    pub(crate) async fn load_pending(&self) -> Result<Option<WarpAccount>> {
        Self::load_path(&self.root.join("secrets/warp-pending.json")).await
    }

    async fn load_path(path: &Path) -> Result<Option<WarpAccount>> {
        let Some(bytes) = read_account_file(path).await? else {
            return Ok(None);
        };
        let stored: StoredAccount =
            serde_json::from_slice(&bytes).map_err(|_| anyhow!("Invalid stored WARP account format"))?;
        ensure!(stored.version == 1, "Unsupported WARP account version");
        Ok(Some(WarpAccount::validate(stored.registration)?))
    }

    #[cfg(test)]
    pub(crate) async fn import_file(&self, source: &Path) -> Result<WarpAccountStatus> {
        let bytes = read_account_file(source)
            .await?
            .ok_or_else(|| anyhow!("WARP account source file is missing"))?;
        let account = WarpAccount::from_warpscout_json(&bytes)?;
        let status = account.status();
        self.save(&account).await?;
        Ok(status)
    }

    pub(crate) async fn save(&self, account: &WarpAccount) -> Result<()> {
        self.save_path(&self.path(), account).await
    }

    pub(crate) async fn save_pending(&self, account: &WarpAccount) -> Result<()> {
        self.save_path(&self.root.join("secrets/warp-pending.json"), account)
            .await
    }

    async fn save_path(&self, path: &Path, account: &WarpAccount) -> Result<()> {
        let stored = StoredAccountRef {
            version: 1,
            registration: &account.registration,
        };
        let json = serde_json::to_string_pretty(&stored).map_err(|_| anyhow!("Unable to serialize WARP account"))?;
        ensure!(
            json.len() as u64 <= MAX_ACCOUNT_BYTES,
            "Normalized WARP account file is too large"
        );
        tokio::fs::create_dir_all(self.root.join("secrets"))
            .await
            .map_err(|_| anyhow!("Unable to create WARP account directory"))?;
        save_yaml_str(path, &json)
            .await
            .map_err(|_| anyhow!("Unable to save WARP account"))?;
        Ok(())
    }

    pub(crate) async fn clear_pending(&self) -> Result<()> {
        Self::remove_path(&self.root.join("secrets/warp-pending.json")).await
    }

    #[cfg(test)]
    pub(crate) async fn remove(&self) -> Result<WarpAccountStatus> {
        Self::remove_path(&self.path()).await?;
        Ok(WarpAccountStatus::Missing)
    }

    async fn remove_path(path: &Path) -> Result<()> {
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(_) => Err(anyhow!("Unable to remove WARP account")),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct Registration {
    id: String,
    token: String,
    private_key: String,
    peer_public_key: String,
    ipv4: Ipv4Addr,
    ipv6: Option<Ipv6Addr>,
}

#[derive(Serialize, PartialEq, Eq, Debug)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum WarpAccountStatus {
    Missing,
    Registered {
        #[serde(rename = "hasIpv6")]
        has_ipv6: bool,
    },
}

impl WarpAccount {
    #[cfg(any(feature = "tropa-probe", feature = "portable", test))]
    pub(crate) fn tunnel_identity(&self) -> (&str, &str, Ipv4Addr, Option<Ipv6Addr>) {
        (
            &self.registration.private_key,
            &self.registration.peer_public_key,
            self.registration.ipv4,
            self.registration.ipv6,
        )
    }

    #[cfg(any(feature = "tropa-probe", feature = "portable", test))]
    pub(crate) fn wireguard_proxy(&self, server: IpAddr, port: u16) -> serde_json::Value {
        let mut proxy = serde_json::json!({
            "name":"WARP", "type":"wireguard", "server":server.to_string(), "port":port,
            "ip":self.registration.ipv4.to_string(), "private-key":self.registration.private_key,
            "public-key":self.registration.peer_public_key, "udp":true, "mtu":1280,
            "remote-dns-resolve":true, "dns":["1.1.1.1"]
        });
        if let Some(ipv6) = self.registration.ipv6 {
            proxy["ipv6"] = ipv6.to_string().into();
        }
        proxy
    }

    pub(crate) fn id(&self) -> &str {
        &self.registration.id
    }

    pub(crate) fn token(&self) -> &str {
        &self.registration.token
    }

    #[cfg(test)]
    pub(crate) fn test_private_key(&self) -> &str {
        &self.registration.private_key
    }

    pub(crate) fn from_api_response(bytes: &[u8], private_key: String) -> Result<Self> {
        #[derive(Deserialize)]
        struct Response {
            id: String,
            token: String,
            config: ApiConfig,
        }
        #[derive(Deserialize)]
        struct ApiConfig {
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
            v4: Ipv4Addr,
            v6: Option<String>,
        }
        let response: Response =
            serde_json::from_slice(bytes).map_err(|_| anyhow!("Invalid WARP registration response"))?;
        let peer = response
            .config
            .peers
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Missing WARP registration peer"))?;
        let ipv6 = response
            .config
            .interface
            .addresses
            .v6
            .filter(|value| !value.is_empty())
            .map(|value| {
                value
                    .parse::<Ipv6Addr>()
                    .map_err(|_| anyhow!("Invalid WARP registration address"))
            })
            .transpose()?;
        Self::validate(Registration {
            id: response.id,
            token: response.token,
            private_key,
            peer_public_key: peer.public_key,
            ipv4: response.config.interface.addresses.v4,
            ipv6,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_warpscout_json(bytes: &[u8]) -> Result<Self> {
        let registration: Registration =
            serde_json::from_slice(bytes).map_err(|_| anyhow!("Invalid WARP account format"))?;
        Self::validate(registration)
    }

    fn validate(registration: Registration) -> Result<Self> {
        ensure!(
            !registration.id.trim().is_empty() && !registration.token.trim().is_empty(),
            "Invalid WARP account identity"
        );
        for key in [&registration.private_key, &registration.peer_public_key] {
            let bytes = STANDARD.decode(key).map_err(|_| anyhow!("Invalid WARP account key"))?;
            ensure!(
                bytes.len() == 32 && bytes.iter().any(|byte| *byte != 0),
                "Invalid WARP account key"
            );
        }
        Ok(Self { registration })
    }

    pub(crate) fn status(&self) -> WarpAccountStatus {
        WarpAccountStatus::Registered {
            has_ipv6: self.registration.ipv6.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_registration() -> serde_json::Value {
        serde_json::json!({
            "id": "synthetic-device", "token": "synthetic-token",
            "private_key": STANDARD.encode([7_u8; 32]),
            "peer_public_key": STANDARD.encode([9_u8; 32]),
            "ipv4": "172.16.0.2", "ipv6": "2001:db8::2",
            "masque": { "token": "ignored-synthetic-token" }
        })
    }

    #[test]
    fn validates_registration_without_exposing_credentials() -> anyhow::Result<()> {
        let account = WarpAccount::from_warpscout_json(&serde_json::to_vec(&example_registration())?)?;
        assert_eq!(
            serde_json::to_value(account.status())?,
            serde_json::json!({"status":"registered", "hasIpv6":true})
        );
        for (field, value) in [
            ("private_key", serde_json::json!("invalid-secret-marker")),
            ("peer_public_key", serde_json::json!(STANDARD.encode([0_u8; 32]))),
            ("private_key", serde_json::json!(STANDARD.encode([7_u8; 31]))),
            ("id", serde_json::Value::Null),
            ("token", serde_json::json!("  ")),
            ("token", serde_json::json!(123)),
            ("ipv4", serde_json::json!("2001:db8::2")),
            ("ipv6", serde_json::json!("172.16.0.2")),
        ] {
            let mut input = example_registration();
            input[field] = value;
            let error = WarpAccount::from_warpscout_json(&serde_json::to_vec(&input)?)
                .err()
                .expect("invalid registration accepted");
            assert!(!format!("{error:#}").contains("invalid-secret-marker"));
        }
        for field in ["id", "token"] {
            let mut input = example_registration();
            input.as_object_mut().unwrap().remove(field);
            assert!(WarpAccount::from_warpscout_json(&serde_json::to_vec(&input)?).is_err());
        }
        let mut input = example_registration();
        input["ipv6"] = serde_json::Value::Null;
        assert_eq!(
            WarpAccount::from_warpscout_json(&serde_json::to_vec(&input)?)?.status(),
            WarpAccountStatus::Registered { has_ipv6: false }
        );
        Ok(())
    }

    #[tokio::test]
    async fn rejected_import_preserves_the_previous_account() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-warp-test-{}", nanoid::nanoid!()));
        tokio::fs::create_dir(&root).await?;
        let source = root.join("source.json");
        let invalid = root.join("invalid.json");
        let store = WarpAccountStore::new(root.clone());
        assert_eq!(store.status().await?, WarpAccountStatus::Missing);
        tokio::fs::write(&source, serde_json::to_vec(&example_registration())?).await?;
        store.import_file(&source).await?;
        let saved_path = root.join("secrets/warp-account.json");
        let before = tokio::fs::read(&saved_path).await?;
        let saved: serde_json::Value = serde_json::from_slice(&before)?;
        assert_eq!(saved["version"], 1);
        assert!(!String::from_utf8_lossy(&before).contains("ignored-synthetic-token"));
        let mut large = example_registration();
        let overhead = serde_json::to_vec(&large)?.len() - "synthetic-token".len();
        large["token"] = "x".repeat(MAX_ACCOUNT_BYTES as usize - overhead).into();
        let near_limit = serde_json::to_vec(&large)?;
        assert_eq!(near_limit.len(), MAX_ACCOUNT_BYTES as usize);
        tokio::fs::write(&invalid, near_limit).await?;
        assert!(store.import_file(&invalid).await.is_err());
        assert_eq!(tokio::fs::read(&saved_path).await?, before);

        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let locked = std::fs::OpenOptions::new().read(true).share_mode(1).open(&saved_path)?;
            assert!(store.import_file(&source).await.is_err());
            assert_eq!(tokio::fs::read(&saved_path).await?, before);
            drop(locked);
        }
        for bytes in [b"not-json-invalid-secret-marker".to_vec(), vec![b' '; 256 * 1024 + 1]] {
            tokio::fs::write(&invalid, bytes).await?;
            let error = store
                .import_file(&invalid)
                .await
                .err()
                .expect("invalid import accepted");
            assert!(!format!("{error:#}").contains("invalid-secret-marker"));
            assert_eq!(tokio::fs::read(&saved_path).await?, before);
        }
        assert_eq!(
            WarpAccountStore::new(root.clone()).status().await?,
            WarpAccountStatus::Registered { has_ipv6: true }
        );
        let mut future = saved;
        future["version"] = 2.into();
        tokio::fs::write(&saved_path, serde_json::to_vec(&future)?).await?;
        assert!(store.status().await.is_err());
        tokio::fs::write(&saved_path, b"broken-json").await?;
        assert!(store.status().await.is_err());
        store.import_file(&source).await?;
        assert_eq!(store.status().await?, WarpAccountStatus::Registered { has_ipv6: true });
        assert_eq!(store.remove().await?, WarpAccountStatus::Missing);
        assert!(tokio::fs::try_exists(&source).await?);
        assert_eq!(store.remove().await?, WarpAccountStatus::Missing);
        tokio::fs::remove_dir(root.join("secrets")).await?;
        tokio::fs::write(root.join("secrets"), b"blocked-directory").await?;
        assert!(store.import_file(&source).await.is_err());
        for file in [&source, &invalid, &root.join("secrets")] {
            tokio::fs::remove_file(file).await?;
        }
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }
}
