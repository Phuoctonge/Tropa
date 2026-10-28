use crate::{core::warp_connection::WARP_CONNECTION, utils::dirs};

async fn split_process_checks(expect_warp: bool) -> anyhow::Result<serde_json::Value> {
    use crate::config::tropa::{RoutingMode, TropaPreferences};
    use anyhow::ensure;
    use sha2::{Digest, Sha256};
    let root = dirs::app_home_dir()?;
    let selected = root.join("smoke/selected/curl.exe");
    let direct = root.join("smoke/direct/curl.exe");
    let preferences = TropaPreferences::load(&root.join("tropa.yaml")).await?;
    ensure!(
        preferences.routing.mode == RoutingMode::Selected,
        "Split smoke requires selected mode"
    );
    ensure!(
        preferences.routing.applications == vec![selected.clone()],
        "Split smoke requires its exact selected fixture"
    );
    let system = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").ok_or_else(|| anyhow::anyhow!("Missing Windows directory"))?,
    )
    .join("System32/curl.exe");
    let expected = Sha256::digest(tokio::fs::read(system).await?);
    for path in [&selected, &direct] {
        ensure!(
            Sha256::digest(tokio::fs::read(path).await?) == expected,
            "Smoke fixture differs from Windows curl"
        );
    }
    if expect_warp {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let runtime: serde_json::Value =
            serde_yaml_ng::from_str(&tokio::fs::read_to_string(root.join("secrets/tropa-runtime.yaml")).await?)?;
        let port = u16::try_from(
            runtime["listeners"][0]["port"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("Missing verification listener"))?,
        )?;
        let response = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            let mut stream = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).await?;
            stream
                .write_all(b"CONNECT www.cloudflare.com:443 HTTP/1.1\r\nHost: www.cloudflare.com:443\r\n\r\n")
                .await?;
            let mut response = [0u8; 512];
            let size = stream.read(&mut response).await?;
            Ok::<_, anyhow::Error>(String::from_utf8_lossy(&response[..size]).into_owned())
        })
        .await??;
        ensure!(
            response.starts_with("HTTP/1.1 407") || response.starts_with("HTTP/1.0 407"),
            "Verification listener did not require authentication"
        );
    }
    async fn trace(path: &std::path::Path) -> anyhow::Result<serde_json::Value> {
        let mut command = tokio::process::Command::new(path);
        command
            .args([
                "--noproxy",
                "*",
                "--ipv4",
                "--connect-timeout",
                "5",
                "--max-time",
                "15",
                "--max-filesize",
                "16384",
                "--silent",
                "https://www.cloudflare.com/cdn-cgi/trace",
            ])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x0800_0000);
        let output = tokio::time::timeout(std::time::Duration::from_secs(20), command.output()).await??;
        ensure!(output.status.success(), "Smoke HTTPS request failed");
        let body = std::str::from_utf8(&output.stdout)?;
        let field = |key: &str| body.lines().find_map(|line| line.strip_prefix(key));
        let warp = field("warp=").ok_or_else(|| anyhow::anyhow!("Missing WARP trace flag"))?;
        let ip = field("ip=").ok_or_else(|| anyhow::anyhow!("Missing trace address"))?;
        let ip_hash: String = Sha256::digest(ip.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Ok(serde_json::json!({"warp":warp,"ipSha256":ip_hash,"colo":field("colo=")}))
    }
    let (selected, direct) = tokio::join!(trace(&selected), trace(&direct));
    let (selected, direct) = (selected?, direct?);
    let selected_on = matches!(selected["warp"].as_str(), Some("on" | "plus"));
    let selected_expected = if expect_warp {
        selected_on
    } else {
        selected["warp"] == "off"
    };
    Ok(serde_json::json!({"passed":selected_expected && direct["warp"] == "off", "selected":selected,"direct":direct}))
}

fn split_result(result: Option<anyhow::Result<serde_json::Value>>) -> Option<serde_json::Value> {
    result.map(|result| result.unwrap_or_else(|error| serde_json::json!({"passed":false,"error":error.to_string()})))
}

pub(crate) async fn run_if_requested() -> bool {
    if !std::env::args().any(|arg| arg == "--tropa-smoke") {
        return false;
    }
    let split = std::env::args().any(|arg| arg == "--tropa-split-smoke");
    let split_before = if split {
        split_result(Some(split_process_checks(false).await))
    } else {
        None
    };
    let connected = if split_before.as_ref().is_some_and(|report| report["passed"] != true) {
        Err(anyhow::anyhow!("Split smoke baseline failed"))
    } else {
        WARP_CONNECTION.run(false).await
    };
    let snapshot = WARP_CONNECTION.view();
    let split_connected = if split && connected.is_ok() {
        split_result(Some(split_process_checks(true).await))
    } else {
        None
    };
    if connected.is_ok() {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    let disconnected = WARP_CONNECTION.disconnect().await;
    let split_after = if split && disconnected.is_ok() {
        split_result(Some(split_process_checks(false).await))
    } else {
        None
    };
    if let Ok(root) = dirs::app_home_dir() {
        let result = serde_json::json!({
            "connected":connected.is_ok(),
            "splitBefore":split_before,
            "splitConnected":split_connected,
            "splitAfter":split_after,
            "connection":snapshot,
            "connectError":connected.err().map(|error|error.to_string()),
            "disconnected":disconnected.is_ok(),
            "disconnectError":disconnected.err().map(|error|error.to_string()),
            "final":WARP_CONNECTION.view()
        });
        let _ = tokio::fs::write(root.join("tropa-smoke-result.json"), result.to_string()).await;
    }
    crate::feat::quit().await;
    true
}
