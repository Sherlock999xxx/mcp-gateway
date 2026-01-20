use crate::contracts::{ContractChange, ContractEvent, ContractKind, ContractTracker};
use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::Row as _;
use sqlx::postgres::PgListener;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const CONTRACTS_CHANNEL: &str = "unrelated_gateway_contracts_v1";

#[derive(Debug, Clone)]
pub struct PgContractFanout {
    pool: PgPool,
    node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireEvent {
    origin: String,
    profile_id: String,
    kind: ContractKind,
    contract_hash: String,
    event_id: u64,
}

impl PgContractFanout {
    #[must_use]
    pub fn new(pool: PgPool, node_id: String) -> Self {
        Self { pool, node_id }
    }

    pub async fn start_listener(
        &self,
        contracts: Arc<ContractTracker>,
        shutdown: CancellationToken,
    ) -> anyhow::Result<()> {
        let mut listener = PgListener::connect_with(&self.pool)
            .await
            .context("connect PgListener")?;
        listener
            .listen(CONTRACTS_CHANNEL)
            .await
            .with_context(|| format!("LISTEN {CONTRACTS_CHANNEL}"))?;

        let node_id = self.node_id.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = shutdown.cancelled() => {
                        tracing::info!("pg fanout listener shutting down");
                        break;
                    }
                    res = listener.recv() => {
                        let notification = match res {
                            Ok(n) => n,
                            Err(e) => {
                                tracing::warn!(error = %e, "pg fanout recv error");
                                // Be conservative: exit the loop rather than spin.
                                break;
                            }
                        };

                        let payload = notification.payload();
                        let msg: WireEvent = match serde_json::from_str(payload) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::warn!(error = %e, payload = %payload, "invalid pg fanout payload");
                                continue;
                            }
                        };

                        if msg.origin == node_id {
                            continue;
                        }

                        let event = ContractEvent {
                            profile_id: msg.profile_id,
                            kind: msg.kind,
                            contract_hash: msg.contract_hash,
                            event_id: msg.event_id,
                        };
                        contracts.apply_remote_event(&event);
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn publish(&self, event: &ContractEvent) -> anyhow::Result<()> {
        let wire = WireEvent {
            origin: self.node_id.clone(),
            profile_id: event.profile_id.clone(),
            kind: event.kind,
            contract_hash: event.contract_hash.clone(),
            event_id: event.event_id,
        };
        let payload = serde_json::to_string(&wire).expect("valid json");
        sqlx::query("select pg_notify($1, $2)")
            .bind(CONTRACTS_CHANNEL)
            .bind(payload)
            .execute(&self.pool)
            .await
            .context("pg_notify")?;
        Ok(())
    }

    pub async fn persist(&self, change: &ContractChange) -> anyhow::Result<ContractEvent> {
        let profile_id = Uuid::parse_str(&change.profile_id).context("parse profile_id")?;
        let kind = change.kind.as_str();

        let row = sqlx::query(
            r"
insert into contract_events (profile_id, kind, contract_hash)
values ($1, $2, $3)
returning id
",
        )
        .bind(profile_id)
        .bind(kind)
        .bind(&change.contract_hash)
        .fetch_one(&self.pool)
        .await
        .context("insert contract event")?;

        let id: i64 = row.try_get("id")?;
        let event_id: u64 = id
            .try_into()
            .map_err(|_| anyhow::anyhow!("contract event id overflow"))?;

        Ok(ContractEvent {
            profile_id: change.profile_id.clone(),
            kind: change.kind,
            contract_hash: change.contract_hash.clone(),
            event_id,
        })
    }

    pub async fn replay(
        &self,
        profile_id: &str,
        after_event_id: u64,
        limit: i64,
    ) -> anyhow::Result<Vec<ContractEvent>> {
        let profile_id = Uuid::parse_str(profile_id).context("parse profile_id")?;
        let after: i64 = after_event_id
            .try_into()
            .map_err(|_| anyhow::anyhow!("after_event_id overflow"))?;

        let rows = sqlx::query(
            r"
select id, kind, contract_hash
from contract_events
where profile_id = $1
  and id > $2
order by id asc
limit $3
",
        )
        .bind(profile_id)
        .bind(after)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("select contract events")?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id: i64 = row.try_get("id")?;
            let kind: String = row.try_get("kind")?;
            let contract_hash: String = row.try_get("contract_hash")?;

            let kind = match kind.as_str() {
                "tools" => ContractKind::Tools,
                "resources" => ContractKind::Resources,
                "prompts" => ContractKind::Prompts,
                other => {
                    tracing::warn!(kind = %other, "unknown contract kind in db; skipping");
                    continue;
                }
            };

            let event_id: u64 = id
                .try_into()
                .map_err(|_| anyhow::anyhow!("contract event id overflow"))?;

            out.push(ContractEvent {
                profile_id: profile_id.to_string(),
                kind,
                contract_hash,
                event_id,
            });
        }

        Ok(out)
    }
}
