mod common;

use anyhow::Context as _;
use serde_json::json;
use std::time::Duration;
use tempfile::tempdir;

use common::{KillOnDrop, pick_unused_port, spawn_adapter, wait_http_ok};

#[tokio::test]
async fn map_exposes_prefixed_tools_on_collision() -> anyhow::Result<()> {
    let dir = tempdir().context("create temp dir")?;
    let cfg_path = dir.path().join("config.yaml");
    std::fs::write(
        &cfg_path,
        r"imports: []
servers:
  s1:
    type: http
    baseUrl: https://example.com
    tools:
      ping:
        method: GET
        path: /v1/ping
  s2:
    type: http
    baseUrl: https://example.com
    tools:
      ping:
        method: GET
        path: /v2/ping
",
    )
    .context("write config")?;

    let port = pick_unused_port()?;
    let child = spawn_adapter(&cfg_path, port)?;
    let _child = KillOnDrop(child);

    let base_url = format!("http://127.0.0.1:{port}");
    wait_http_ok(&format!("{base_url}/health"), Duration::from_secs(20)).await?;

    let client = reqwest::Client::new();
    let map: serde_json::Value = client
        .get(format!("{base_url}/map"))
        .send()
        .await
        .context("GET /map")?
        .error_for_status()
        .context("GET /map status")?
        .json()
        .await
        .context("GET /map json")?;

    let tools = map
        .get("tools")
        .and_then(serde_json::Value::as_object)
        .context("/map missing tools object")?;

    // Collision: both must be prefixed, unprefixed removed.
    anyhow::ensure!(
        tools.contains_key("s1:ping"),
        "expected s1:ping in /map.tools"
    );
    anyhow::ensure!(
        tools.contains_key("s2:ping"),
        "expected s2:ping in /map.tools"
    );
    anyhow::ensure!(
        !tools.contains_key("ping"),
        "expected ping to be removed on collision"
    );

    // Validate mapping shape.
    assert_eq!(
        tools.get("s1:ping").and_then(|v| v.get("server")),
        Some(&json!("s1"))
    );
    assert_eq!(
        tools.get("s1:ping").and_then(|v| v.get("original_name")),
        Some(&json!("ping"))
    );
    assert_eq!(
        tools.get("s2:ping").and_then(|v| v.get("server")),
        Some(&json!("s2"))
    );
    assert_eq!(
        tools.get("s2:ping").and_then(|v| v.get("original_name")),
        Some(&json!("ping"))
    );

    // Server counts should line up.
    let servers = map
        .get("servers")
        .and_then(serde_json::Value::as_object)
        .context("/map missing servers object")?;
    anyhow::ensure!(servers.contains_key("s1"));
    anyhow::ensure!(servers.contains_key("s2"));
    assert_eq!(
        servers.get("s1").and_then(|s| s.get("tool_count")),
        Some(&json!(1))
    );
    assert_eq!(
        servers.get("s2").and_then(|s| s.get("tool_count")),
        Some(&json!(1))
    );

    Ok(())
}
