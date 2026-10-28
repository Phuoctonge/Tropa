use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

fn observe_file(path: &Path, expected: Option<&str>) -> serde_json::Value {
    let checksum = (|| -> std::io::Result<String> {
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.len() > 128 * 1024 * 1024 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        let mut file = std::fs::File::open(path)?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > 128 * 1024 * 1024 {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            hash.update(&buffer[..count]);
        }
        Ok(hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
    })();
    let (actual, integrity) = match checksum {
        Ok(hash) => {
            let integrity = match expected {
                Some(expected) if hash == expected => "matched",
                Some(_) => "mismatch",
                None => "observed",
            };
            (Some(hash), integrity)
        }
        Err(error) => (
            None,
            if error.kind() == std::io::ErrorKind::NotFound {
                "missing"
            } else {
                "unreadable"
            },
        ),
    };
    serde_json::json!({ "actualSha256": actual, "expectedSha256": expected, "integrity": integrity })
}

pub(crate) fn component_report(executable: &Path, resources: &Path) -> anyhow::Result<serde_json::Value> {
    let inventory: serde_json::Value = serde_json::from_str(include_str!("../../../scripts/tropa-components.json"))?;
    let mut components = Vec::new();
    for (name, filename) in [("mihomo", "verge-mihomo.exe"), ("WarpScout", "warpscout.exe")] {
        let component = inventory["components"]
            .as_array()
            .and_then(|entries| entries.iter().find(|entry| entry["name"] == name))
            .ok_or_else(|| anyhow::anyhow!("Missing pinned component metadata"))?;
        let checksum = component["sha256"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing pinned checksum"))?;
        components.push(serde_json::json!({
            "name": name,
            "version": component["version"],
            "file": observe_file(&resources.join(filename), Some(checksum)),
        }));
    }
    Ok(serde_json::json!({
        "application": { "version": env!("CARGO_PKG_VERSION"), "file": observe_file(executable, None) },
        "components": components,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_observations_distinguish_integrity_without_exporting_paths() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!("tropa-diagnostics-{}", nanoid::nanoid!()));
        std::fs::create_dir(&root)?;
        let _cleanup = scopeguard::guard(root.clone(), |path| {
            let _ = std::fs::remove_dir_all(path);
        });
        let path = root.join("private-path-marker.exe");
        std::fs::write(&path, b"abc")?;
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let matched = observe_file(&path, Some(expected));
        assert_eq!(matched["integrity"], "matched");
        assert_eq!(matched["actualSha256"], expected);
        assert!(!matched.to_string().contains("private-"));
        assert_eq!(observe_file(&path, Some("wrong"))["integrity"], "mismatch");
        assert_eq!(observe_file(&path, None)["integrity"], "observed");
        assert_eq!(observe_file(&root, None)["integrity"], "unreadable");
        std::fs::write(root.join("verge-mihomo.exe"), b"abc")?;
        let components = component_report(&path, &root)?;
        assert_eq!(components["application"]["file"]["actualSha256"], expected);
        assert_eq!(components["components"][0]["version"], "1.19.33");
        assert_eq!(components["components"][0]["file"]["integrity"], "mismatch");
        assert_eq!(components["components"][1]["file"]["integrity"], "missing");
        assert!(!components.to_string().contains("private-"));
        std::fs::remove_file(&path)?;
        assert_eq!(observe_file(&path, Some(expected))["integrity"], "missing");
        Ok(())
    }
}
