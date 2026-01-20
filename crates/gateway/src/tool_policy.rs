use serde::{Deserialize, Serialize};

/// Per-tool retry policy (Temporal-style fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryPolicy {
    /// Maximum number of attempts, including the initial attempt (1 => no retries).
    pub maximum_attempts: u32,
    /// Initial backoff interval in milliseconds (before the first retry).
    pub initial_interval_ms: u64,
    /// Backoff multiplier (typically >= 1.0).
    pub backoff_coefficient: f64,
    /// Optional maximum interval between retries in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_interval_ms: Option<u64>,
    /// Optional list of error category strings that should not be retried.
    ///
    /// Categories currently recognized by the Gateway:
    /// - `"timeout"`: gateway-side overall attempt timeout
    /// - `"transport"`: connect/timeouts/EOF/IO/channel errors
    /// - `"upstream_5xx"`: upstream HTTP 5xx responses
    /// - `"deserialize"`: invalid JSON-RPC response payloads
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub non_retryable_error_types: Vec<String>,
}

/// Per-profile per-tool policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPolicy {
    /// Stable tool reference in the form `"<source_id>:<original_tool_name>"`.
    pub tool: String,
    /// Optional per-tool timeout override (seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// Optional per-tool retry policy (Gateway-only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryPolicy>,
}
