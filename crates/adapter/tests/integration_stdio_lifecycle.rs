mod common;
mod common_mcp;

use anyhow::Context as _;
use common::{KillOnDrop, pick_unused_port, spawn_adapter, wait_http_ok};
use common_mcp::{McpStreamableHttpSession, tool_call_body_json};
use serde_json::json;
use std::time::Duration;

fn write_stdio_config(stdio_lifecycle: &str) -> anyhow::Result<tempfile::NamedTempFile> {
    let bin = env!("CARGO_BIN_EXE_unrelated-mcp-stdio-test-server");

    let cfg = format!(
        r#"
adapter:
  stdioLifecycle: {stdio_lifecycle}
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

async fn whoami_instance_id(session: &McpStreamableHttpSession) -> anyhow::Result<String> {
    let msg = session
        .request(
            1,
            "tools/call",
            json!({"name": "whoami", "arguments": {}}),
            Duration::from_secs(5),
        )
        .await?;

    let body = tool_call_body_json(&msg)?;
    body.get("instanceId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .context("tools/call whoami missing body.instanceId")
}

#[tokio::test]
async fn stdio_lifecycle_persistent_reuses_process_across_sessions() -> anyhow::Result<()> {
    let config = write_stdio_config("persistent")?;

    let port = pick_unused_port()?;
    let adapter = spawn_adapter(config.path(), port)?;
    let _adapter = KillOnDrop(adapter);
    wait_http_ok(
        &format!("http://127.0.0.1:{port}/health"),
        Duration::from_secs(10),
    )
    .await?;

    let base_url = format!("http://127.0.0.1:{port}");
    let s1 = McpStreamableHttpSession::connect(&base_url).await?;
    let s2 = McpStreamableHttpSession::connect(&base_url).await?;

    let id1 = whoami_instance_id(&s1).await?;
    let id2 = whoami_instance_id(&s2).await?;
    assert_eq!(id1, id2, "expected persistent stdio process reuse");

    Ok(())
}

#[tokio::test]
async fn stdio_lifecycle_per_session_reuses_within_session_but_not_across_sessions()
-> anyhow::Result<()> {
    let config = write_stdio_config("per_session")?;

    let port = pick_unused_port()?;
    let adapter = spawn_adapter(config.path(), port)?;
    let _adapter = KillOnDrop(adapter);
    wait_http_ok(
        &format!("http://127.0.0.1:{port}/health"),
        Duration::from_secs(10),
    )
    .await?;

    let base_url = format!("http://127.0.0.1:{port}");
    let s1 = McpStreamableHttpSession::connect(&base_url).await?;
    let s2 = McpStreamableHttpSession::connect(&base_url).await?;

    let a1 = whoami_instance_id(&s1).await?;
    let a2 = whoami_instance_id(&s1).await?;
    assert_eq!(
        a1, a2,
        "expected per-session process reuse within a session"
    );

    let b1 = whoami_instance_id(&s2).await?;
    assert_ne!(a1, b1, "expected different process across sessions");

    Ok(())
}

#[tokio::test]
async fn stdio_lifecycle_per_call_spawns_new_process_for_each_call() -> anyhow::Result<()> {
    let config = write_stdio_config("per_call")?;

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

    let a = whoami_instance_id(&s).await?;
    let b = whoami_instance_id(&s).await?;
    assert_ne!(a, b, "expected new process per tools/call");

    Ok(())
}
