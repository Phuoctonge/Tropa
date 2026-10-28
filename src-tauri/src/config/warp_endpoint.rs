use anyhow::{Result, ensure};
use std::net::SocketAddr;
#[cfg(any(feature = "tropa-probe", test))]
use std::path::Path;

#[derive(Clone, Copy)]
pub(crate) struct WarpEndpoint(SocketAddr);

impl WarpEndpoint {
    pub(crate) fn parse(output: &str) -> Result<Self> {
        ensure!(output.len() <= 4096, "WarpScout output exceeded its limit");
        let address: SocketAddr = output
            .trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("WarpScout did not return a single IP endpoint"))?;
        let pool = match address {
            SocketAddr::V6(address) => {
                let segments = address.ip().segments();
                address.scope_id() == 0
                    && address.flowinfo() == 0
                    && segments[0] == 0x2606
                    && segments[1] == 0x4700
                    && matches!(segments[2], 0xd0 | 0xd1)
            }
            SocketAddr::V4(address) => {
                let octets = address.ip().octets();
                matches!(
                    &octets[..3],
                    [8, 6, 112]
                        | [8, 34, 70]
                        | [8, 34, 146]
                        | [8, 35, 211]
                        | [8, 39, 125]
                        | [8, 39, 204]
                        | [8, 39, 214]
                        | [8, 47, 69]
                        | [162, 159, 192]
                        | [162, 159, 195]
                        | [188, 114, 96]
                        | [188, 114, 97]
                        | [188, 114, 98]
                        | [188, 114, 99]
                )
            }
        };
        let port = matches!(
            address.port(),
            2408 | 500
                | 1701
                | 4500
                | 854
                | 859
                | 864
                | 878
                | 880
                | 890
                | 891
                | 894
                | 903
                | 908
                | 928
                | 934
                | 939
                | 942
                | 943
                | 945
                | 946
                | 955
                | 968
                | 987
                | 988
                | 1002
                | 1010
                | 1014
                | 1018
                | 1070
                | 1074
                | 1180
                | 1387
                | 1843
                | 2371
                | 2506
                | 3138
                | 3476
                | 3581
                | 3854
                | 4177
                | 4198
                | 4233
                | 5279
                | 5956
                | 7103
                | 7152
                | 7156
                | 7281
                | 7559
                | 8319
                | 8742
                | 8854
                | 8886
        );
        ensure!(
            pool && port,
            "WarpScout endpoint is outside the pinned WARP pools/ports"
        );
        Ok(Self(address))
    }

    pub(crate) fn address(self) -> SocketAddr {
        self.0
    }

    #[cfg(any(feature = "tropa-probe", test))]
    pub(crate) async fn load(path: &Path) -> Result<Option<Self>> {
        use tokio::io::AsyncReadExt;
        let file = match tokio::fs::File::open(path).await {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(anyhow::anyhow!("Unable to read saved WARP endpoint")),
        };
        let mut bytes = Vec::new();
        file.take(4097)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| anyhow::anyhow!("Unable to read saved WARP endpoint"))?;
        ensure!(bytes.len() <= 4096, "Saved WARP endpoint exceeded its limit");
        #[derive(serde::Deserialize)]
        struct Saved {
            version: u32,
            endpoint: String,
        }
        let saved: Saved =
            serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("Invalid saved WARP endpoint"))?;
        ensure!(saved.version == 1, "Unsupported WARP endpoint version");
        Self::parse(&saved.endpoint).map(Some)
    }

    #[cfg(test)]
    pub(crate) async fn save(self, path: &Path) -> Result<()> {
        let value = serde_json::json!({"version":1,"endpoint":self.0.to_string()});
        crate::utils::help::save_yaml_str(path, &value.to_string())
            .await
            .map_err(|_| anyhow::anyhow!("Unable to save validated WARP endpoint"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv6_endpoints_accept_only_pinned_pools_and_unscoped_addresses() -> anyhow::Result<()> {
        for endpoint in ["[2606:4700:d0::1]:2408", "[2606:4700:d1:ffff::abcd]:500"] {
            assert_eq!(WarpEndpoint::parse(endpoint)?.address().to_string(), endpoint);
        }
        for endpoint in [
            "[2606:4700:d2::1]:2408",
            "[2606:4700:d0::1]:443",
            "[2606:4700:d0::1%3]:2408",
            "[::ffff:188.114.96.1]:2408",
            "[::1]:2408",
        ] {
            assert!(WarpEndpoint::parse(endpoint).is_err(), "{endpoint}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn scout_output_must_be_a_single_warp_endpoint_and_rejection_preserves_selection() -> anyhow::Result<()> {
        let directory = std::env::temp_dir().join(format!("tropa-endpoint-{}", nanoid::nanoid!()));
        tokio::fs::create_dir(&directory).await?;
        let path = directory.join("warp-endpoint.json");
        let endpoint = WarpEndpoint::parse("188.114.96.10:500\n")?;
        endpoint.save(&path).await?;
        let before = tokio::fs::read(&path).await?;
        assert_eq!(WarpEndpoint::load(&path).await?.unwrap().address(), endpoint.address());
        for invalid in [
            "",
            "192.168.1.1:2408",
            "162.159.192.1:0",
            "162.159.192.1:22",
            "188.114.96.10:500\n188.114.97.1:500",
            "working: 188.114.96.10:500",
        ] {
            assert!(WarpEndpoint::parse(invalid).is_err(), "{invalid}");
        }
        assert!(WarpEndpoint::parse(&"1".repeat(4097)).is_err());
        assert_eq!(tokio::fs::read(&path).await?, before);
        assert_eq!(endpoint.address().to_string(), "188.114.96.10:500");
        tokio::fs::write(&path, br#"{"version":1,"endpoint":"192.168.1.1:2408"}"#).await?;
        assert!(WarpEndpoint::load(&path).await.is_err());
        tokio::fs::remove_file(path).await?;
        tokio::fs::remove_dir(directory).await?;
        Ok(())
    }
}
