use super::warp_failure::{FailureReason, FailureStage, WarpFailure};
use anyhow::{Result, ensure};

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct NodeMetadata {
    pub(crate) node: Option<String>,
    pub(crate) country: Option<String>,
}

pub(crate) fn parse_metadata(bytes: &[u8]) -> Result<NodeMetadata> {
    ensure!(
        bytes.len() <= 16 * 1024,
        WarpFailure::new(FailureStage::Validation, FailureReason::InvalidOutput)
    );
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| WarpFailure::new(FailureStage::Validation, FailureReason::InvalidOutput))?;
    let code = |key, width| {
        value["colo"][key]
            .as_str()
            .filter(|value| value.len() == width && value.bytes().all(|b| b.is_ascii_uppercase()))
            .map(str::to_owned)
    };
    Ok(NodeMetadata {
        node: code("iata", 3),
        country: code("cca2", 2),
    })
}

pub(crate) async fn fetch_metadata(client: &reqwest::Client) -> Result<NodeMetadata> {
    let mut response = client
        .get("https://speed.cloudflare.com/meta")
        .header("Referer", "https://speed.cloudflare.com")
        .timeout(std::time::Duration::from_secs(4))
        .send()
        .await?
        .error_for_status()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 16 * 1024,
            WarpFailure::new(FailureStage::Validation, FailureReason::InvalidOutput)
        );
        bytes.extend_from_slice(&chunk);
    }
    parse_metadata(&bytes)
}

pub(crate) fn country_from_location(location: &str) -> Option<String> {
    let code = location.rsplit_once(", ").map_or(location, |(_, code)| code);
    (code.len() == 2 && code.bytes().all(|b| b.is_ascii_uppercase())).then(|| code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_country_is_the_colo_country_not_the_exit_country() -> Result<()> {
        let parsed = parse_metadata(
            br#"{"ip":"sensitive-marker","country":"RU","colo":{"iata":"FRA","city":"Frankfurt","cca2":"DE"}}"#,
        )?;
        assert_eq!(
            parsed,
            NodeMetadata {
                node: Some("FRA".into()),
                country: Some("DE".into())
            }
        );
        assert_eq!(parse_metadata(br#"{"country":"RU"}"#)?, NodeMetadata::default());
        assert_eq!(country_from_location("Frankfurt-am-Main, DE"), Some("DE".into()));
        assert_eq!(country_from_location("FI"), Some("FI".into()));
        for unknown in ["?", "Frankfurt-DE", "Frankfurt, de", "Frankfurt, UNKNOWN"] {
            assert_eq!(country_from_location(unknown), None);
        }
        Ok(())
    }
}
