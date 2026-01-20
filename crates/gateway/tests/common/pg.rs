use anyhow::Context as _;
use std::time::{Duration, Instant};

pub async fn wait_pg_ready(database_url: &str, timeout: Duration) -> anyhow::Result<()> {
    let start = Instant::now();
    loop {
        if start.elapsed() > timeout {
            anyhow::bail!("timed out waiting for Postgres");
        }

        if sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await
            .is_ok()
        {
            return Ok(());
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

pub fn extract_dbmate_up(sql: &str) -> anyhow::Result<String> {
    let (_, rest) = sql
        .split_once("-- migrate:up")
        .context("missing dbmate marker: -- migrate:up")?;
    let (up, _) = rest
        .split_once("-- migrate:down")
        .context("missing dbmate marker: -- migrate:down")?;
    Ok(up.trim().to_string())
}

pub async fn apply_dbmate_migrations(database_url: &str) -> anyhow::Result<()> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .context("connect to Postgres for migrations")?;

    // In gateway tests, CARGO_MANIFEST_DIR points at `crates/gateway`.
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&migrations_dir)
        .with_context(|| format!("read migrations dir {}", migrations_dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    paths.sort();

    for path in paths {
        let sql = std::fs::read_to_string(&path)
            .with_context(|| format!("read migration {}", path.display()))?;
        let up = extract_dbmate_up(&sql)?;
        for stmt in up.split(';') {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }
            sqlx::query(stmt)
                .execute(&pool)
                .await
                .with_context(|| format!("execute migration statement from {}", path.display()))?;
        }
    }

    Ok(())
}
