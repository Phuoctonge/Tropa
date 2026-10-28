use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::utils::help;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TropaPreferences {
    pub game: Option<PathBuf>,
    #[serde(default)]
    pub routing: RoutingPreferences,
    #[serde(
        default,
        rename = "selectionPolicy",
        skip_serializing_if = "super::warp_selection::SelectionPolicy::is_default"
    )]
    pub(crate) selection_policy: super::warp_selection::SelectionPolicy,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoutingMode {
    #[default]
    All,
    Selected,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingPreferences {
    pub mode: RoutingMode,
    pub applications: Vec<PathBuf>,
}

impl RoutingPreferences {
    pub async fn normalized(mut self) -> Result<Self> {
        anyhow::ensure!(self.applications.len() <= 64, "Too many application executables");
        let mut paths = Vec::new();
        for original in self.applications {
            anyhow::ensure!(original.is_absolute(), "Application path must be absolute");
            let original_text = original
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid application path encoding"))?;
            anyhow::ensure!(
                original
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
                    && !original_text.chars().any(char::is_control),
                "Select an application executable (.exe)"
            );
            if self.mode == RoutingMode::All {
                paths.push(original);
                continue;
            }
            let mut path = original;
            if path
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("FirefoxPortable.exe"))
            {
                let parent = path
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("Missing launcher directory"))?;
                let mut resolved = None;
                for relative in ["App/Firefox64/firefox.exe", "App/Firefox/firefox.exe"] {
                    let candidate = parent.join(relative);
                    if tokio::fs::metadata(&candidate)
                        .await
                        .is_ok_and(|metadata| metadata.is_file())
                    {
                        resolved = Some(candidate);
                        break;
                    }
                }
                path = resolved.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Select Firefox Portable's actual App/Firefox64/firefox.exe or App/Firefox/firefox.exe"
                    )
                })?;
            }
            anyhow::ensure!(
                path.extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
                    && tokio::fs::metadata(&path).await?.is_file(),
                "Select an existing application executable (.exe)"
            );
            let canonical = tokio::fs::canonicalize(path).await?;
            let text = canonical
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid application path encoding"))?;
            let text = text
                .strip_prefix(r"\\?\UNC\")
                .map(|unc| format!(r"\\{unc}"))
                .unwrap_or_else(|| text.strip_prefix(r"\\?\").unwrap_or(text).to_owned());
            anyhow::ensure!(!text.chars().any(char::is_control), "Invalid application path");
            let canonical = PathBuf::from(text);
            if !paths.iter().any(|saved: &PathBuf| {
                saved
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&canonical.to_string_lossy())
            }) {
                paths.push(canonical);
            }
        }
        self.applications = paths;
        Ok(self)
    }

    pub async fn validate_connection(&self) -> Result<()> {
        if self.mode == RoutingMode::Selected {
            anyhow::ensure!(
                !self.applications.is_empty(),
                "Choose at least one application for split tunneling"
            );
            let normalized = self.clone().normalized().await?;
            anyhow::ensure!(
                normalized == *self,
                "Application paths changed; select the executable again"
            );
        }
        Ok(())
    }

    pub fn rules(&self) -> Result<Vec<String>> {
        if self.mode == RoutingMode::All {
            return Ok(vec!["MATCH,WARP".into()]);
        }
        anyhow::ensure!(
            !self.applications.is_empty() && self.applications.len() <= 64,
            "Choose application executables before connecting"
        );
        let mut rules = Vec::new();
        for path in &self.applications {
            let text = path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid application path encoding"))?;
            anyhow::ensure!(
                path.is_absolute() && !text.chars().any(char::is_control),
                "Invalid application path"
            );
            let expression = regex::escape(text).replace(',', r"\x2C");
            rules.push(format!("PROCESS-PATH-REGEX,^(?i:{expression})$,WARP"));
        }
        rules.push("MATCH,DIRECT".into());
        Ok(rules)
    }
}

impl TropaPreferences {
    pub async fn load(path: &Path) -> Result<Self> {
        match tokio::fs::read_to_string(path).await {
            Ok(contents) => {
                let mut preferences: Self = serde_yaml_ng::from_str(&contents)?;
                preferences.selection_policy = preferences.selection_policy.normalized()?;
                Ok(preferences)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn save(&self, path: &Path) -> Result<()> {
        if let Some(game) = &self.game {
            anyhow::ensure!(
                game.is_absolute()
                    && game
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe")),
                "Select a game executable (.exe) using its absolute path"
            );
        }
        let mut normalized = self.clone();
        normalized.selection_policy = normalized.selection_policy.normalized()?;
        help::save_yaml(path, &normalized, Some("# Tropa preferences")).await
    }
}

#[cfg(test)]
mod tests {
    use super::TropaPreferences;

    #[tokio::test]
    async fn selection_policy_survives_preferences_roundtrip() -> anyhow::Result<()> {
        let directory = std::env::temp_dir().join(format!("tropa-policy-{}", nanoid::nanoid!()));
        tokio::fs::create_dir(&directory).await?;
        let _cleanup = scopeguard::guard(directory.clone(), |path| {
            let _ = std::fs::remove_dir_all(path);
        });
        let path = directory.join("tropa.yaml");
        tokio::fs::write(
            &path,
            "game: null\nselectionPolicy:\n  includeNodes: [hel, HEL]\n  excludeNodes: [dme]\n",
        )
        .await?;
        let preferences = TropaPreferences::load(&path).await?;
        preferences.save(&path).await?;
        let saved = TropaPreferences::load(&path).await?;
        let value = serde_json::to_value(saved)?;
        assert_eq!(value["selectionPolicy"]["includeNodes"], serde_json::json!(["HEL"]));
        assert_eq!(value["selectionPolicy"]["excludeNodes"], serde_json::json!(["DME"]));
        Ok(())
    }

    #[tokio::test]
    async fn all_mode_does_not_require_a_removed_application_to_exist() -> anyhow::Result<()> {
        let routing = super::RoutingPreferences {
            mode: super::RoutingMode::All,
            applications: vec![r"C:\tropa-removed-application\firefox.exe".into()],
        };
        assert_eq!(routing.clone().normalized().await?, routing);
        routing.validate_connection().await?;
        Ok(())
    }

    #[test]
    fn legacy_game_selection_does_not_enable_split_tunneling() -> anyhow::Result<()> {
        let preferences: TropaPreferences = serde_yaml_ng::from_str("game: C:\\Apps\\FirefoxPortable.exe")?;
        assert_eq!(preferences.routing, Default::default());
        Ok(())
    }

    #[test]
    fn process_rules_match_only_the_exact_path_and_escape_rule_separators() -> anyhow::Result<()> {
        use super::{RoutingMode, RoutingPreferences};
        let routing = RoutingPreferences {
            mode: RoutingMode::Selected,
            applications: vec![r"C:\Apps, browser\Firefox (test)\firefox.exe".into()],
        };
        let rules = routing.rules()?;
        let parts: Vec<_> = rules[0].split(',').collect();
        assert_eq!(parts.len(), 3);
        let expression = regex::Regex::new(parts[1])?;
        assert!(expression.is_match(r"c:\apps, BROWSER\firefox (test)\FIREFOX.EXE"));
        assert!(!expression.is_match(r"C:\Other\firefox.exe"));
        assert!(!expression.is_match(r"C:\Apps, browser\Firefox (test)\firefox.exe.extra"));
        assert_eq!(rules[1], "MATCH,DIRECT");
        assert!(
            RoutingPreferences {
                mode: RoutingMode::Selected,
                applications: vec![]
            }
            .rules()
            .is_err()
        );
        Ok(())
    }

    #[tokio::test]
    async fn firefox_portable_selection_resolves_the_network_process() -> anyhow::Result<()> {
        use super::{RoutingMode, RoutingPreferences};
        let root = std::env::temp_dir().join(format!("tropa-firefox-{}", nanoid::nanoid!()));
        let child_dir = root.join("App/Firefox64");
        tokio::fs::create_dir_all(&child_dir).await?;
        let launcher = root.join("FirefoxPortable.exe");
        let child = child_dir.join("firefox.exe");
        tokio::fs::write(&launcher, b"not executed").await?;
        tokio::fs::write(&child, b"not executed").await?;
        let normalized = RoutingPreferences {
            mode: RoutingMode::Selected,
            applications: vec![launcher.clone()],
        }
        .normalized()
        .await?;
        assert_eq!(normalized.applications, vec![child.clone()]);
        normalized.validate_connection().await?;
        tokio::fs::remove_file(&child).await?;
        assert!(normalized.validate_connection().await.is_err());
        assert!(
            RoutingPreferences {
                mode: RoutingMode::Selected,
                applications: vec![launcher.clone()]
            }
            .normalized()
            .await
            .is_err()
        );
        tokio::fs::remove_file(launcher).await?;
        tokio::fs::remove_dir(child_dir).await?;
        tokio::fs::remove_dir(root.join("App")).await?;
        tokio::fs::remove_dir(root).await?;
        Ok(())
    }

    #[test]
    fn selected_application_policy_survives_preferences_round_trip() -> anyhow::Result<()> {
        let input = serde_json::json!({
            "game":null,
            "routing":{"mode":"selected","applications":["C:\\Apps\\Firefox\\firefox.exe"]}
        });
        let preferences: TropaPreferences = serde_json::from_value(input.clone())?;
        assert_eq!(serde_json::to_value(preferences)?, input);
        Ok(())
    }

    #[tokio::test]
    async fn invalid_game_selection_preserves_the_saved_game() -> anyhow::Result<()> {
        let directory = std::env::temp_dir().join(format!("tropa-preferences-{}", nanoid::nanoid!()));
        tokio::fs::create_dir(&directory).await?;
        let config = directory.join("tropa.yaml");
        let game = directory.join("game.exe");
        let document = directory.join("notes.txt");
        tokio::fs::write(&game, b"not executed").await?;
        tokio::fs::write(&document, b"not a game").await?;
        let absent = TropaPreferences::load(&config).await?;
        let selected = TropaPreferences {
            game: Some(game.clone()),
            ..Default::default()
        };
        selected.save(&config).await?;
        let loaded = TropaPreferences::load(&config).await?;
        let rejected = TropaPreferences {
            game: Some(document.clone()),
            ..Default::default()
        }
        .save(&config)
        .await;
        let after_rejection = TropaPreferences::load(&config).await?;
        tokio::fs::write(&config, "game: [broken").await?;
        let malformed = TropaPreferences::load(&config).await;
        for file in [config, game, document] {
            tokio::fs::remove_file(file).await?;
        }
        tokio::fs::remove_dir(directory).await?;
        assert_eq!(absent, TropaPreferences::default());
        assert_eq!(loaded, selected);
        assert!(rejected.is_err(), "non-executable selection must be rejected");
        assert_eq!(after_rejection, selected);
        assert!(malformed.is_err(), "malformed preferences must not silently reset");
        Ok(())
    }
}
