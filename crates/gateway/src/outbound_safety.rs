use std::collections::HashSet;
use unrelated_http_tools::safety::OutboundHttpSafety;

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn env_csv_set(name: &str) -> Option<HashSet<String>> {
    let raw = std::env::var(name).ok()?;
    let set: HashSet<String> = raw
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    (!set.is_empty()).then_some(set)
}

/// Outbound HTTP safety policy for the Gateway.
///
/// Default is restrictive (SSRF hardening). For local development/testing you can opt into
/// allowing private networks.
///
/// Env:
/// - `UNRELATED_GATEWAY_OUTBOUND_ALLOW_PRIVATE_NETWORKS=1` to allow RFC1918/loopback/link-local.
/// - `UNRELATED_GATEWAY_OUTBOUND_ALLOWED_HOSTS=host1,host2` to restrict hosts (case-insensitive).
#[must_use]
pub fn gateway_outbound_http_safety() -> OutboundHttpSafety {
    let mut safety = OutboundHttpSafety::gateway_default();

    if env_flag("UNRELATED_GATEWAY_OUTBOUND_ALLOW_PRIVATE_NETWORKS") {
        safety.allow_private_networks = true;
    }

    if let Some(set) = env_csv_set("UNRELATED_GATEWAY_OUTBOUND_ALLOWED_HOSTS") {
        safety.allowed_hosts = Some(set);
    }

    safety
}
