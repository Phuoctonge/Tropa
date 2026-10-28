use super::warp_profiles::{TransportKind, WarpObservation, WarpTransport};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub(crate) struct SelectionPolicy {
    pub(crate) allowed_transports: Vec<TransportKind>,
    pub(crate) include_nodes: Vec<String>,
    pub(crate) exclude_nodes: Vec<String>,
    pub(crate) include_node_countries: Vec<String>,
    pub(crate) exclude_node_countries: Vec<String>,
}

impl SelectionPolicy {
    pub(crate) fn for_search(&self, transport: Option<TransportKind>) -> Result<Self> {
        let mut scoped = self.clone();
        if let Some(transport) = transport {
            ensure!(
                self.allows_kind(transport),
                crate::core::warp_failure::WarpFailure::new(
                    crate::core::warp_failure::FailureStage::Scanner,
                    crate::core::warp_failure::FailureReason::PolicyRestricted,
                )
                .for_transport(transport)
            );
            scoped.allowed_transports = vec![transport];
        }
        Ok(scoped)
    }

    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub(crate) fn normalized(mut self) -> Result<Self> {
        ensure!(
            self.allowed_transports.len() <= 64,
            "Слишком много режимов подключения."
        );
        let mut transports = Vec::new();
        for transport in self.allowed_transports {
            if !transports.contains(&transport) {
                transports.push(transport);
            }
        }
        self.allowed_transports = transports;
        for (list, width) in [
            (&mut self.include_nodes, 3),
            (&mut self.exclude_nodes, 3),
            (&mut self.include_node_countries, 2),
            (&mut self.exclude_node_countries, 2),
        ] {
            ensure!(list.len() <= 64, "В одном ограничении допускается не больше 64 кодов.");
            for code in list.iter_mut() {
                *code = code.trim().to_ascii_uppercase();
                ensure!(
                    code.len() == width && code.bytes().all(|b| b.is_ascii_uppercase()),
                    "Укажите коды узлов из трёх букв и стран из двух букв."
                );
            }
            list.sort();
            list.dedup();
        }
        for (include, exclude) in [
            (&self.include_nodes, &self.exclude_nodes),
            (&self.include_node_countries, &self.exclude_node_countries),
        ] {
            ensure!(
                include.is_empty() || include.iter().any(|code| !exclude.contains(code)),
                "Ограничения исключают все разрешённые значения. Измените список."
            );
        }
        Ok(self)
    }

    pub(crate) fn allows_transport(&self, transport: &WarpTransport) -> bool {
        self.allows_kind(transport.kind())
    }

    pub(crate) fn allows_kind(&self, transport: TransportKind) -> bool {
        self.allowed_transports.is_empty() || self.allowed_transports.contains(&transport)
    }

    pub(crate) fn has_geography(&self) -> bool {
        !self.include_nodes.is_empty()
            || !self.exclude_nodes.is_empty()
            || !self.include_node_countries.is_empty()
            || !self.exclude_node_countries.is_empty()
    }

    fn matches_code(include: &[String], exclude: &[String], value: Option<&str>, allow_unknown: bool) -> bool {
        if include.is_empty() && exclude.is_empty() {
            return true;
        }
        match value {
            Some(value) => {
                !exclude.iter().any(|v| v == value) && (include.is_empty() || include.iter().any(|v| v == value))
            }
            None => allow_unknown,
        }
    }

    pub(crate) fn matches_metadata(&self, node: Option<&str>, country: Option<&str>, allow_unknown: bool) -> bool {
        Self::matches_code(&self.include_nodes, &self.exclude_nodes, node, allow_unknown)
            && Self::matches_code(
                &self.include_node_countries,
                &self.exclude_node_countries,
                country,
                allow_unknown,
            )
    }

    pub(crate) fn allows_observation(
        &self,
        transport: &WarpTransport,
        observation: &WarpObservation,
        allow_unknown: bool,
    ) -> bool {
        self.allows_transport(transport)
            && self.matches_metadata(
                observation.node.as_deref(),
                observation.node_country.as_deref(),
                allow_unknown,
            )
    }

    pub(crate) fn scanner_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for (flag, codes) in [
            ("-node", &self.include_nodes),
            ("-exclude-node", &self.exclude_nodes),
            ("-country", &self.include_node_countries),
            ("-exclude-country", &self.exclude_node_countries),
        ] {
            if !codes.is_empty() {
                args.extend([flag.to_owned(), codes.join(",")]);
            }
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_shot_search_can_narrow_but_not_override_connection_policy() -> Result<()> {
        let policy = SelectionPolicy {
            allowed_transports: vec![TransportKind::WireGuard, TransportKind::MasqueH3],
            include_node_countries: vec!["DE".into()],
            exclude_nodes: vec!["DME".into()],
            ..Default::default()
        };
        let scoped = policy.for_search(Some(TransportKind::MasqueH3))?;
        assert_eq!(scoped.allowed_transports, [TransportKind::MasqueH3]);
        assert!(!scoped.allows_kind(TransportKind::WireGuard));
        assert!(scoped.matches_metadata(Some("FRA"), Some("DE"), false));
        assert!(!scoped.matches_metadata(Some("DME"), Some("DE"), false));
        assert!(!scoped.matches_metadata(Some("AMS"), Some("NL"), false));
        assert_eq!(
            policy.allowed_transports,
            [TransportKind::WireGuard, TransportKind::MasqueH3]
        );
        assert_eq!(policy.for_search(None)?, policy);
        let error = policy.for_search(Some(TransportKind::MasqueH2)).err().unwrap();
        let failure = error.downcast_ref::<crate::core::warp_failure::WarpFailure>().unwrap();
        assert_eq!(
            failure.reason,
            crate::core::warp_failure::FailureReason::PolicyRestricted
        );
        Ok(())
    }

    #[test]
    fn exclusions_override_inclusions_and_unknown_metadata_needs_revalidation() -> Result<()> {
        let nodes = SelectionPolicy {
            include_nodes: vec!["hel".into(), "ARN".into()],
            exclude_nodes: vec!["arn".into(), "DME".into()],
            ..Default::default()
        }
        .normalized()?;
        assert!(!nodes.matches_metadata(Some("ARN"), Some("SE"), false));
        assert!(!nodes.matches_metadata(Some("DME"), Some("RU"), false));
        assert!(nodes.matches_metadata(Some("HEL"), Some("FI"), false));
        assert!(!nodes.matches_metadata(None, Some("FI"), false));
        assert!(nodes.matches_metadata(None, Some("FI"), true));
        assert_eq!(
            nodes.scanner_args(),
            vec!["-node", "ARN,HEL", "-exclude-node", "ARN,DME"]
        );
        let countries = SelectionPolicy {
            include_node_countries: vec!["de".into(), "NL".into()],
            exclude_nodes: vec!["ARN".into()],
            ..Default::default()
        }
        .normalized()?;
        assert!(countries.matches_metadata(Some("FRA"), Some("DE"), false));
        assert!(countries.matches_metadata(Some("AMS"), Some("NL"), false));
        assert!(!countries.matches_metadata(Some("HEL"), Some("FI"), false));
        assert!(!countries.matches_metadata(Some("ARN"), Some("NL"), false));
        assert!(!countries.matches_metadata(Some("FRA"), None, false));
        assert_eq!(
            countries.scanner_args(),
            vec!["-exclude-node", "ARN", "-country", "DE,NL"]
        );
        Ok(())
    }

    #[test]
    fn contradictory_and_unbounded_input_is_rejected_before_scanning() {
        assert!(
            SelectionPolicy {
                include_nodes: vec!["dme".into()],
                exclude_nodes: vec!["DME".into()],
                ..Default::default()
            }
            .normalized()
            .is_err()
        );
        assert!(
            SelectionPolicy {
                include_nodes: vec!["DME,-best".into()],
                ..Default::default()
            }
            .normalized()
            .is_err()
        );
        assert!(
            SelectionPolicy {
                include_nodes: vec!["HEL".into(); 65],
                ..Default::default()
            }
            .normalized()
            .is_err()
        );
    }
}
