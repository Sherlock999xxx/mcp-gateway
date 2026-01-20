mod common;
mod common_mcp;

use anyhow::Context as _;
use common::{KillOnDrop, pick_unused_port, spawn_adapter, wait_http_ok};
use common_mcp::McpStreamableHttpSession;
use serde_json::json;
use std::time::Duration;

fn write_stdio_config() -> anyhow::Result<tempfile::NamedTempFile> {
    let bin = env!("CARGO_BIN_EXE_unrelated-mcp-stdio-list-changed-test-server");

    let cfg = format!(
        r#"
adapter:
  stdioLifecycle: persistent
servers:
  s1:
    type: stdio
    command: "{bin}"
    args: []
"#
    );

    let file = tempfile::NamedTempFile::new().context("create temp config")?;
    std::fs::write(file.path(), cfg).context("write temp config")?;
    Ok(file)
}

fn tool_names(msg: &serde_json::Value) -> anyhow::Result<Vec<String>> {
    let tools = msg
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(serde_json::Value::as_array)
        .context("tools/list missing result.tools")?;
    Ok(tools
        .iter()
        .filter_map(|t| {
            t.get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect())
}

#[tokio::test]
async fn stdio_persistent_refreshes_registry_on_upstream_tools_list_changed() -> anyhow::Result<()>
{
    let config = write_stdio_config()?;

    let port = pick_unused_port()?;
    let adapter = spawn_adapter(config.path(), port)?;
    let _adapter = KillOnDrop(adapter);
    wait_http_ok(
        &format!("http://127.0.0.1:{port}/health"),
        Duration::from_secs(10),
    )
    .await?;

    let base_url = format!("http://127.0.0.1:{port}");
    let s = McpStreamableHttpSession::connect(&base_url).await?;

    let before = s
        .request(1, "tools/list", json!({}), Duration::from_secs(5))
        .await?;
    let before_names = tool_names(&before)?;
    assert!(
        before_names.contains(&"toggle_extra_tool".to_string()),
        "expected toggle_extra_tool in initial tools/list"
    );
    assert!(
        before_names.contains(&"whoami".to_string()),
        "expected whoami in initial tools/list"
    );
    assert!(
        !before_names.contains(&"hello".to_string()),
        "did not expect hello in initial tools/list"
    );

    // Trigger tool list mutation in the upstream; server emits notifications/tools/list_changed.
    let _ = s
        .request(
            2,
            "tools/call",
            json!({"name": "toggle_extra_tool", "arguments": {}}),
            Duration::from_secs(5),
        )
        .await?;

    // Poll until the Adapter refresh loop has rebuilt the registry.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let msg = s
                .request(3, "tools/list", json!({}), Duration::from_secs(5))
                .await?;
            let names = tool_names(&msg)?;
            if names.contains(&"hello".to_string()) {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .context("timeout waiting for adapter registry refresh")??;

    Ok(())
}
