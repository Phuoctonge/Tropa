use std::path::PathBuf;

use super::{CmdResult, WithErrorCode};
#[cfg(all(windows, feature = "portable"))]
use crate::core::warp_connection::{ConnectionView, WARP_CONNECTION};
use crate::{
    config::{
        Config,
        tropa::TropaPreferences,
        warp_profiles::{ServerSelection, WarpCatalog, WarpCatalogLock},
    },
    utils::dirs,
};

#[tauri::command]
pub async fn get_tropa_preferences() -> CmdResult<TropaPreferences> {
    async {
        let _write = Config::lock_config_write().await;
        TropaPreferences::load(&dirs::app_home_dir()?.join("tropa.yaml")).await
    }
    .await
    .with_error_code("TROPA_PREFERENCES_READ_FAILED")
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TropaServer {
    parameters: crate::config::warp_profiles::WarpTransport,
    search_measurement: Option<crate::config::warp_profiles::SearchMeasurement>,
    runtime_method: Option<crate::config::warp_profiles::RuntimeMeasurementMethod>,
    network_context: crate::config::warp_context::ContextMatch,
    scanner_context: crate::config::warp_context::ContextMatch,
    id: String,
    endpoint: String,
    transport: &'static str,
    selectable: bool,
    validation: &'static str,
    rejected_at: Option<u64>,
    checked_at: Option<u64>,
    median_ms: Option<u32>,
    jitter_ms: Option<u32>,
    failed: Option<u8>,
    samples: Option<u8>,
    exit_country: Option<String>,
    node_country: Option<String>,
    node: Option<String>,
    node_location: Option<String>,
    endpoint_ping_ms: Option<u32>,
    tunnel_ping_ms: Option<u32>,
    tunnel_loss_percent: Option<u8>,
    scanner_checked_at: Option<u64>,
}

#[derive(serde::Serialize)]
pub struct TropaServers {
    selection: ServerSelection,
    servers: Vec<TropaServer>,
    policy: crate::config::warp_selection::SelectionPolicy,
    #[serde(rename = "manualSelectionAllowed")]
    manual_selection_allowed: bool,
}

fn server_view(
    mut catalog: WarpCatalog,
    policy: crate::config::warp_selection::SelectionPolicy,
    context: Option<&str>,
) -> TropaServers {
    let manual_selection_allowed = catalog.selection_allowed(&policy);
    let selection = catalog.selection.clone();
    catalog.selection = ServerSelection::Automatic;
    let mut servers: Vec<_> = catalog
        .candidates()
        .into_iter()
        .map(|entry| TropaServer {
            parameters: entry.profile.transport.clone(),
            search_measurement: entry.observation.search_measurement.clone(),
            runtime_method: entry.observation.runtime_method,
            network_context: crate::config::warp_context::compare(
                entry.observation.network_context.as_deref(),
                context,
            ),
            scanner_context: crate::config::warp_context::compare(
                entry.observation.scanner_context.as_deref(),
                context,
            ),
            id: entry.profile.id.clone(),
            endpoint: entry.profile.endpoint.to_string(),
            transport: entry.profile.transport.label(),
            selectable: catalog.profile_allowed(&entry.profile.id, &policy),
            validation: if catalog.is_rejected(&entry.profile.id) {
                "rejected"
            } else {
                "verified"
            },
            rejected_at: entry.rejected_at,
            checked_at: Some(entry.observation.checked_at),
            median_ms: Some(entry.observation.median_ms),
            jitter_ms: Some(entry.observation.jitter_ms),
            failed: Some(entry.observation.failed),
            samples: Some(entry.observation.samples),
            exit_country: entry.observation.exit_country.clone(),
            node_country: entry.observation.node_country.clone(),
            node: entry.observation.node.clone(),
            node_location: entry.observation.node_location.clone(),
            endpoint_ping_ms: entry.observation.endpoint_ping_ms,
            tunnel_ping_ms: entry.observation.tunnel_ping_ms,
            tunnel_loss_percent: entry.observation.tunnel_loss_percent,
            scanner_checked_at: entry.observation.scanner_checked_at,
        })
        .collect();
    for entry in &catalog.scanned {
        if servers.iter().any(|server| server.id == entry.profile.id) {
            continue;
        }
        servers.push(TropaServer {
            parameters: entry.profile.transport.clone(),
            search_measurement: entry.observation.search_measurement.clone(),
            runtime_method: None,
            network_context: crate::config::warp_context::ContextMatch::Unknown,
            scanner_context: crate::config::warp_context::compare(
                entry.observation.network_context.as_deref(),
                context,
            ),
            id: entry.profile.id.clone(),
            endpoint: entry.profile.endpoint.to_string(),
            transport: entry.profile.transport.label(),
            selectable: catalog.profile_allowed(&entry.profile.id, &policy),
            validation: if entry.rejected_at.is_some() {
                "rejected"
            } else {
                "untested"
            },
            rejected_at: entry.rejected_at,
            checked_at: None,
            median_ms: None,
            jitter_ms: None,
            failed: None,
            samples: None,
            exit_country: entry
                .observation
                .search_measurement
                .as_ref()
                .and_then(|measurement| measurement.exit_country.clone()),
            node: entry.observation.node.clone(),
            node_country: entry.observation.node_country.clone(),
            node_location: entry.observation.node_location.clone(),
            endpoint_ping_ms: entry.observation.endpoint_ping_ms,
            tunnel_ping_ms: entry.observation.tunnel_ping_ms,
            tunnel_loss_percent: entry.observation.tunnel_loss_percent,
            scanner_checked_at: Some(entry.observation.checked_at),
        });
    }
    TropaServers {
        selection,
        servers,
        policy,
        manual_selection_allowed,
    }
}

#[tauri::command]
pub async fn get_tropa_servers() -> CmdResult<TropaServers> {
    async {
        let _write = Config::lock_config_write().await;
        let catalog = WarpCatalog::load_current(&dirs::app_home_dir()?).await?;
        let policy = TropaPreferences::load(&dirs::app_home_dir()?.join("tropa.yaml"))
            .await?
            .selection_policy;
        Ok::<_, anyhow::Error>(server_view(
            catalog,
            policy,
            crate::config::warp_context::capture().as_deref(),
        ))
    }
    .await
    .with_error_code("TROPA_SERVERS_READ_FAILED")
}

#[tauri::command]
pub async fn select_tropa_server(selection: ServerSelection) -> CmdResult<TropaServers> {
    async {
        let _write = Config::lock_config_write().await;
        let path = dirs::app_home_dir()?.join("warp-profiles.json");
        let _catalog_write = WarpCatalogLock::acquire(&dirs::app_home_dir()?)?;
        let mut catalog = WarpCatalog::load_current(&dirs::app_home_dir()?).await?;
        let policy = TropaPreferences::load(&dirs::app_home_dir()?.join("tropa.yaml"))
            .await?
            .selection_policy;
        catalog.select_with_policy(selection, &policy)?;
        catalog.save(&path).await?;
        let policy = TropaPreferences::load(&dirs::app_home_dir()?.join("tropa.yaml"))
            .await?
            .selection_policy;
        Ok::<_, anyhow::Error>(server_view(
            catalog,
            policy,
            crate::config::warp_context::capture().as_deref(),
        ))
    }
    .await
    .with_error_code("TROPA_SERVER_SELECTION_FAILED")
}

#[tauri::command]
pub async fn set_tropa_selection_policy(
    policy: crate::config::warp_selection::SelectionPolicy,
) -> CmdResult<TropaPreferences> {
    async {
        #[cfg(all(windows, feature = "portable"))]
        let _operation = WARP_CONNECTION.idle_guard()?;
        let _write = Config::lock_config_write().await;
        let root = dirs::app_home_dir()?;
        let _catalog_write = WarpCatalogLock::acquire(&root)?;
        let path = root.join("tropa.yaml");
        let mut preferences = TropaPreferences::load(&path).await?;
        preferences.selection_policy = policy.normalized()?;
        preferences.save(&path).await?;
        Ok::<_, anyhow::Error>(preferences)
    }
    .await
    .with_error_code("TROPA_SELECTION_POLICY_FAILED")
}

#[cfg(all(windows, feature = "portable"))]
#[tauri::command]
pub async fn get_tropa_connection() -> CmdResult<ConnectionView> {
    Ok(WARP_CONNECTION.view())
}

#[cfg(all(windows, feature = "portable"))]
#[tauri::command]
pub async fn get_tropa_diagnostics() -> CmdResult<serde_json::Value> {
    async {
        let state = WARP_CONNECTION.view().diagnostic_snapshot();
        let resources = dirs::app_resources_dir()?;
        let executable = std::env::current_exe()?;
        let components = tokio::task::spawn_blocking(move || {
            crate::core::warp_diagnostics::component_report(&executable, &resources)
        })
        .await??;
        Ok::<_, anyhow::Error>(serde_json::json!({
            "schemaVersion": 1,
            "createdAt": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
            "platform": "windows-x64",
            "nativeChecksPerformedByReport": false,
            "connection": state,
            "build": components,
        }))
    }
    .await
    .with_error_code("TROPA_DIAGNOSTICS_READ_FAILED")
}

#[cfg(all(windows, feature = "portable"))]
#[tauri::command]
pub async fn connect_tropa() -> CmdResult<ConnectionView> {
    if !crate::core::runstate::RUN_STATE.state().is_admin {
        let _operation = WARP_CONNECTION.idle_guard().with_error_code("TROPA_CONNECT_BUSY")?;
        tokio::task::spawn_blocking(crate::core::warp_elevation::request_relaunch)
            .await
            .map_err(|_| {
                super::CommandFailure::coded("TROPA_ELEVATION_FAILED", "Administrator restart did not finish")
            })?
            .with_error_code("TROPA_ELEVATION_FAILED")?;
        drop(_operation);
        crate::feat::quit().await;
        return Ok(WARP_CONNECTION.view());
    }
    WARP_CONNECTION.run(false).await.with_error_code("TROPA_CONNECT_FAILED")
}

#[cfg(all(windows, feature = "portable"))]
#[tauri::command]
pub async fn refresh_tropa_servers(
    transport: Option<crate::config::warp_profiles::TransportKind>,
    advanced: Option<bool>,
) -> CmdResult<ConnectionView> {
    WARP_CONNECTION
        .search(transport, advanced.unwrap_or(false))
        .await
        .with_error_code("TROPA_SEARCH_FAILED")
}

#[cfg(all(windows, feature = "portable"))]
#[tauri::command]
pub async fn disconnect_tropa() -> CmdResult<ConnectionView> {
    WARP_CONNECTION
        .disconnect()
        .await
        .with_error_code("TROPA_DISCONNECT_FAILED")
}

#[tauri::command]
pub async fn set_tropa_game(game: Option<PathBuf>) -> CmdResult<TropaPreferences> {
    async {
        if let Some(path) = &game {
            anyhow::ensure!(
                tokio::fs::metadata(path).await?.is_file(),
                "Game executable does not exist"
            );
        }
        let _write = Config::lock_config_write().await;
        let mut preferences = TropaPreferences::load(&dirs::app_home_dir()?.join("tropa.yaml")).await?;
        preferences.game = game;
        preferences.save(&dirs::app_home_dir()?.join("tropa.yaml")).await?;
        Ok::<_, anyhow::Error>(preferences)
    }
    .await
    .with_error_code("TROPA_PREFERENCES_SAVE_FAILED")
}

#[tauri::command]
pub async fn set_tropa_routing(routing: crate::config::tropa::RoutingPreferences) -> CmdResult<TropaPreferences> {
    async {
        #[cfg(all(windows, feature = "portable"))]
        let _operation = WARP_CONNECTION.idle_guard()?;
        #[cfg(all(windows, feature = "portable"))]
        anyhow::ensure!(
            !WARP_CONNECTION.view().needs_disconnect(),
            "Disconnect before changing application routing"
        );
        let _write = Config::lock_config_write().await;
        let path = dirs::app_home_dir()?.join("tropa.yaml");
        let mut preferences = TropaPreferences::load(&path).await?;
        preferences.routing = routing.normalized().await?;
        preferences.save(&path).await?;
        Ok::<_, anyhow::Error>(preferences)
    }
    .await
    .with_error_code("TROPA_ROUTING_SAVE_FAILED")
}

#[tauri::command]
pub async fn set_tropa_close_action(close_action: crate::config::tropa::CloseAction) -> CmdResult<TropaPreferences> {
    async {
        let _write = Config::lock_config_write().await;
        let path = dirs::app_home_dir()?.join("tropa.yaml");
        let mut preferences = TropaPreferences::load(&path).await?;
        preferences.close_action = close_action;
        preferences.save(&path).await?;
        Ok::<_, anyhow::Error>(preferences)
    }
    .await
    .with_error_code("TROPA_CLOSE_ACTION_SAVE_FAILED")
}

#[cfg(test)]
#[test]
fn scanner_rows_are_visible_without_fabricated_runtime_measurements() -> anyhow::Result<()> {
    use crate::config::warp_profiles::{ScannerObservation, WarpProfile, WarpTransport};
    let mut catalog = WarpCatalog::default();
    let profile = WarpProfile::new("188.114.96.10:500".parse()?, WarpTransport::WireGuard)?;
    catalog.record_scanned(
        profile.clone(),
        ScannerObservation {
            network_context: Some("a".repeat(64)),
            checked_at: 123,
            node: Some("HEL".into()),
            node_country: Some("FI".into()),
            node_location: None,
            endpoint_ping_ms: Some(20),
            tunnel_ping_ms: Some(25),
            tunnel_loss_percent: Some(0),
            search_measurement: Some(crate::config::warp_profiles::SearchMeasurement {
                engine_build_id: format!("source:{}", "b".repeat(64)),
                stages: crate::config::warp_profiles::SearchStages {
                    handshake: Some(true),
                    http: Some(true),
                    https: Some(false),
                    udp: None,
                },
                endpoint_ping_ms: Some(20.25),
                tunnel_ping_ms: Some(25.625),
                loss_percent: Some(0.0),
                jitter_ms: Some(1.0),
                samples: 5,
                method: Some("icmp-v4-sequential-v1:1.1.1.1".into()),
                exit_country: Some("DE".into()),
            }),
        },
    )?;
    for last in 11..80 {
        let next = WarpProfile::new(format!("188.114.96.{last}:500").parse()?, WarpTransport::WireGuard)?;
        catalog.record_scanned(next, catalog.scanned[0].observation.clone())?;
    }
    let view = serde_json::to_value(server_view(catalog.clone(), Default::default(), Some(&"a".repeat(64))))?;
    assert_eq!(view["servers"].as_array().unwrap().len(), 70);
    assert_eq!(view["servers"][0]["validation"], "untested");
    assert_eq!(view["servers"][0]["nodeCountry"], "FI");
    assert_eq!(view["servers"][0]["exitCountry"], "DE");
    assert_ne!(view["servers"][0]["nodeCountry"], view["servers"][0]["exitCountry"]);
    assert_eq!(view["servers"][0]["selectable"], true);
    assert!(view["servers"][0]["medianMs"].is_null());
    assert!(view["servers"][0]["checkedAt"].is_null());
    assert_eq!(view["servers"][0]["networkContext"], "unknown");
    assert_eq!(view["servers"][0]["scannerContext"], "same");
    assert_eq!(view["servers"][0]["parameters"]["kind"], "wireGuard");
    assert_eq!(view["servers"][0]["searchMeasurement"]["endpointPingMs"], 20.25);
    assert_eq!(view["servers"][0]["searchMeasurement"]["tunnelPingMs"], 25.625);
    assert_eq!(view["servers"][0]["searchMeasurement"]["stages"]["https"], false);
    assert!(view["servers"][0]["searchMeasurement"]["stages"]["udp"].is_null());
    assert_eq!(
        view["servers"][0]["searchMeasurement"]["method"],
        "icmp-v4-sequential-v1:1.1.1.1"
    );
    assert!(view["servers"][0]["runtimeMethod"].is_null());
    assert!(!view.to_string().contains(&"a".repeat(64)));
    catalog.record_rejection(&profile.id, 124)?;
    let view = serde_json::to_value(server_view(catalog, Default::default(), None))?;
    assert_eq!(view["servers"][0]["validation"], "rejected");
    assert!(view["servers"][0]["samples"].is_null());
    assert_eq!(view["servers"][0]["scannerContext"], "unknown");
    Ok(())
}
