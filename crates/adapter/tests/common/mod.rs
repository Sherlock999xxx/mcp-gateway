use anyhow::Context as _;
use std::process::{Child, Command};
use std::time::Duration;

pub use unrelated_test_support::KillOnDrop;

pub fn pick_unused_port() -> anyhow::Result<u16> {
    unrelated_test_support::pick_unused_port()
}

pub async fn wait_http_ok(url: &str, timeout_dur: Duration) -> anyhow::Result<()> {
    unrelated_test_support::wait_http_ok(url, timeout_dur).await
}

pub fn spawn_adapter(config_path: &std::path::Path, port: u16) -> anyhow::Result<Child> {
    let bin = env!("CARGO_BIN_EXE_unrelated-mcp-adapter");
    Command::new(bin)
        .arg("--config")
        .arg(config_path)
        .arg("--bind")
        .arg(format!("127.0.0.1:{port}"))
        .arg("--log-level")
        .arg("info")
        .spawn()
        .context("spawn adapter")
}
