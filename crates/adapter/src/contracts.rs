use parking_lot::RwLock;
use rmcp::{
    model::{Prompt, Resource, Tool},
    service::{Peer, RoleServer},
};
use serde_json::Value;
use sha2::Digest as _;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContractHashes {
    pub tools: String,
    pub resources: String,
    pub prompts: String,
}

#[derive(Debug, Default)]
struct SurfaceHashes {
    tools: Option<String>,
    resources: Option<String>,
    prompts: Option<String>,
}

pub(crate) fn compute_contract_hashes(
    tools: &[Tool],
    resources: &[Resource],
    prompts: &[Prompt],
) -> ContractHashes {
    ContractHashes {
        tools: tools_contract_hash(tools),
        resources: resources_contract_hash(resources),
        prompts: prompts_contract_hash(prompts),
    }
}

/// Best-effort contract hashing + `list_changed` notifications for the Adapter.
///
/// The Adapter can refresh its aggregated registry at runtime (e.g. when stdio backends restart).
/// When that changes the exposed surfaces, we broadcast `notifications/*/list_changed` to all
/// connected sessions.
#[derive(Debug, Default)]
pub struct ContractNotifier {
    peers: RwLock<HashMap<String, Peer<RoleServer>>>,
    hashes: RwLock<SurfaceHashes>,
}

impl ContractNotifier {
    pub fn observe_peer(&self, session_id: &str, peer: Peer<RoleServer>) {
        self.peers.write().insert(session_id.to_string(), peer);
    }

    pub fn get_peer(&self, session_id: &str) -> Option<Peer<RoleServer>> {
        self.peers.read().get(session_id).cloned()
    }

    pub fn forget_peer(&self, session_id: &str) {
        self.peers.write().remove(session_id);
    }

    pub async fn update_and_notify(
        &self,
        tools: &[Tool],
        resources: &[Resource],
        prompts: &[Prompt],
    ) {
        let ContractHashes {
            tools: new_tools,
            resources: new_resources,
            prompts: new_prompts,
        } = compute_contract_hashes(tools, resources, prompts);

        let (notify_tools, notify_resources, notify_prompts) = {
            let mut hashes = self.hashes.write();

            let notify_tools =
                hashes.tools.as_deref() != Some(&new_tools) && hashes.tools.is_some();
            let notify_resources =
                hashes.resources.as_deref() != Some(&new_resources) && hashes.resources.is_some();
            let notify_prompts =
                hashes.prompts.as_deref() != Some(&new_prompts) && hashes.prompts.is_some();

            hashes.tools = Some(new_tools);
            hashes.resources = Some(new_resources);
            hashes.prompts = Some(new_prompts);

            (notify_tools, notify_resources, notify_prompts)
        };

        if notify_tools {
            self.notify_tool_list_changed().await;
        }
        if notify_resources {
            self.notify_resource_list_changed().await;
        }
        if notify_prompts {
            self.notify_prompt_list_changed().await;
        }
    }

    async fn notify_tool_list_changed(&self) {
        let peers: Vec<(String, Peer<RoleServer>)> = self
            .peers
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let mut dead: Vec<String> = Vec::new();
        for (session_id, peer) in peers {
            if let Err(e) = peer.notify_tool_list_changed().await {
                tracing::debug!(mcp_session_id = %session_id, error = %e, "failed to send tools list_changed");
                dead.push(session_id);
            }
        }

        if !dead.is_empty() {
            let mut map = self.peers.write();
            for id in dead {
                map.remove(&id);
            }
        }
    }

    async fn notify_resource_list_changed(&self) {
        let peers: Vec<(String, Peer<RoleServer>)> = self
            .peers
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let mut dead: Vec<String> = Vec::new();
        for (session_id, peer) in peers {
            if let Err(e) = peer.notify_resource_list_changed().await {
                tracing::debug!(
                    mcp_session_id = %session_id,
                    error = %e,
                    "failed to send resources list_changed"
                );
                dead.push(session_id);
            }
        }

        if !dead.is_empty() {
            let mut map = self.peers.write();
            for id in dead {
                map.remove(&id);
            }
        }
    }

    async fn notify_prompt_list_changed(&self) {
        let peers: Vec<(String, Peer<RoleServer>)> = self
            .peers
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let mut dead: Vec<String> = Vec::new();
        for (session_id, peer) in peers {
            if let Err(e) = peer.notify_prompt_list_changed().await {
                tracing::debug!(mcp_session_id = %session_id, error = %e, "failed to send prompts list_changed");
                dead.push(session_id);
            }
        }

        if !dead.is_empty() {
            let mut map = self.peers.write();
            for id in dead {
                map.remove(&id);
            }
        }
    }
}

fn tools_contract_hash(tools: &[Tool]) -> String {
    let mut entries: Vec<(String, String, Value, Value, Value)> = tools
        .iter()
        .map(|t| {
            let name = t.name.to_string();
            let description = t.description.as_deref().unwrap_or_default().to_string();
            let input_schema = canonicalize_json(&Value::Object(t.input_schema.as_ref().clone()));
            let output_schema = t.output_schema.as_ref().map_or(Value::Null, |s| {
                canonicalize_json(&Value::Object(s.as_ref().clone()))
            });
            let annotations = serde_json::to_value(&t.annotations).unwrap_or(Value::Null);
            let annotations = canonicalize_json(&annotations);
            (name, description, input_schema, output_schema, annotations)
        })
        .collect();

    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let v = Value::Array(
        entries
            .into_iter()
            .map(
                |(name, description, input_schema, output_schema, annotations)| {
                    serde_json::json!({
                        "name": name,
                        "description": description,
                        "inputSchema": input_schema,
                        "outputSchema": output_schema,
                        "annotations": annotations,
                    })
                },
            )
            .collect(),
    );

    let serialized = serde_json::to_string(&canonicalize_json(&v)).expect("valid json");
    hex::encode(sha2::Sha256::digest(serialized.as_bytes()))
}

fn resources_contract_hash(resources: &[Resource]) -> String {
    let mut entries: Vec<(String, Value)> = resources
        .iter()
        .map(|r| {
            let uri = r.uri.clone();
            let v = serde_json::to_value(r).expect("resource serializes");
            (uri, canonicalize_json(&v))
        })
        .collect();

    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let v = Value::Array(entries.into_iter().map(|(_k, v)| v).collect());
    let serialized = serde_json::to_string(&canonicalize_json(&v)).expect("valid json");
    hex::encode(sha2::Sha256::digest(serialized.as_bytes()))
}

fn prompts_contract_hash(prompts: &[Prompt]) -> String {
    let mut entries: Vec<(String, Value)> = prompts
        .iter()
        .map(|p| {
            let name = p.name.clone();
            let v = serde_json::to_value(p).expect("prompt serializes");
            (name, canonicalize_json(&v))
        })
        .collect();

    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let v = Value::Array(entries.into_iter().map(|(_k, v)| v).collect());
    let serialized = serde_json::to_string(&canonicalize_json(&v)).expect("valid json");
    hex::encode(sha2::Sha256::digest(serialized.as_bytes()))
}

fn canonicalize_json(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                if let Some(val) = map.get(&k) {
                    out.insert(k, canonicalize_json(val));
                }
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(canonicalize_json).collect()),
        other => other.clone(),
    }
}
