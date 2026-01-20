use super::McpState;
use super::streamable_http;
use crate::session_token::TokenPayloadV1;
use crate::tool_policy::RetryPolicy;
use crate::tools_cache::{CachedToolsSurface, ToolRoute, ToolRouteKind, profile_fingerprint};
use axum::{Json, http::StatusCode, response::IntoResponse as _, response::Response};
use rmcp::model::GetMeta as _;
use rmcp::{
    model::{ClientJsonRpcMessage, ErrorCode, JsonRpcRequest, JsonRpcVersion2_0, RequestId},
    transport::streamable_http_client::StreamableHttpPostResponse,
};
use std::borrow::Cow;

pub(super) async fn route_and_proxy_tools_call(
    state: &McpState,
    profile_id: &str,
    profile: &crate::store::Profile,
    payload: &TokenPayloadV1,
    token: String,
    message: &mut ClientJsonRpcMessage,
    hop: u32,
) -> Result<Response, Response> {
    let Some((tool_name, req_id, args_value)) = super::extract_call_tool(message) else {
        return Err((StatusCode::BAD_REQUEST, "invalid tools/call request").into_response());
    };

    let (mut surface, built_now) =
        get_or_build_tools_surface_for_call(state, profile_id, profile, payload, &token, hop)
            .await?;

    // Resolve tool owner via cached routing table.
    let missing = surface.routes.get(&tool_name).is_none();
    if missing && !built_now {
        // JIT refresh on miss: invalidate and rebuild once.
        state.tools_cache.invalidate(&token);
        surface = Box::pin(super::surface::build_tools_surface(
            state, profile_id, profile, payload, hop,
        ))
        .await?;
        state.tools_cache.put(
            profile_id,
            token.clone(),
            profile_fingerprint(profile),
            surface.clone(),
        );
    }

    let route = match resolve_tool_route(&surface, &tool_name) {
        Ok(r) => r,
        Err(ToolRouteLookupError::Ambiguous) => {
            return Err(super::jsonrpc_error_response(
                req_id.clone(),
                ErrorCode::INVALID_PARAMS,
                format!("ambiguous tool name '{tool_name}'; use '<source_id>:{tool_name}'"),
            ));
        }
        Err(ToolRouteLookupError::Unknown) => {
            return Err(super::jsonrpc_error_response(
                req_id.clone(),
                ErrorCode::INVALID_PARAMS,
                format!("unknown tool: {tool_name}"),
            ));
        }
    };

    // Validate incoming args against the *advertised* (post-transform) tool schema.
    if let Some(tool_def) = surface.tools.iter().find(|t| t.name == tool_name)
        && let Err((msg, data)) = validate_tool_arguments(tool_def, &args_value)
    {
        return Err(super::jsonrpc_error_response_with_data(
            req_id.clone(),
            ErrorCode::INVALID_PARAMS,
            msg,
            Some(data),
        ));
    }

    // Rewrite exposed arguments (post-transform surface) back into original tool args.
    let args = build_transformed_call_args(profile, &route.original_name, args_value);

    let tool_ref = stable_tool_ref(&route.source_id, &route.original_name);
    let timeout_secs = tool_call_timeout_secs_for(profile, &tool_ref);
    let timeout = std::time::Duration::from_secs(timeout_secs);

    if let Some(resp) = execute_local_tool_call(
        state,
        profile,
        &route,
        &args,
        req_id.clone(),
        timeout,
        timeout_secs,
    )
    .await?
    {
        return Ok(resp);
    }

    // Rewrite name before proxying.
    if let Some(call) = super::as_call_tool_mut(message) {
        call.name = Cow::Owned(route.original_name.clone());
        call.arguments = Some(args);
    }

    proxy_upstream_tool_call_with_retry(UpstreamToolCall {
        state,
        profile_id,
        profile,
        payload,
        route: &route,
        req_id: &req_id,
        message: message.clone(),
        timeout,
        timeout_secs,
        hop,
    })
    .await
}

async fn get_or_build_tools_surface_for_call(
    state: &McpState,
    profile_id: &str,
    profile: &crate::store::Profile,
    payload: &TokenPayloadV1,
    token: &str,
    hop: u32,
) -> Result<(CachedToolsSurface, bool), Response> {
    let fp = profile_fingerprint(profile);
    let mut surface = state.tools_cache.get(token, &fp);
    let mut built_now = false;
    if surface.is_none() {
        surface = Some(
            Box::pin(super::surface::build_tools_surface(
                state, profile_id, profile, payload, hop,
            ))
            .await?,
        );
        state.tools_cache.put(
            profile_id,
            token.to_string(),
            fp,
            surface.clone().expect("surface"),
        );
        built_now = true;
    }
    Ok((surface.expect("surface"), built_now))
}

#[derive(Debug, Clone, Copy)]
enum ToolRouteLookupError {
    Ambiguous,
    Unknown,
}

fn resolve_tool_route(
    surface: &CachedToolsSurface,
    tool_name: &str,
) -> Result<ToolRoute, ToolRouteLookupError> {
    surface.routes.get(tool_name).cloned().ok_or_else(|| {
        if surface.ambiguous_names.contains(tool_name) {
            ToolRouteLookupError::Ambiguous
        } else {
            ToolRouteLookupError::Unknown
        }
    })
}

fn build_transformed_call_args(
    profile: &crate::store::Profile,
    original_tool_name: &str,
    args_value: serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    let mut args = match args_value {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    profile
        .transforms
        .apply_call_transforms(original_tool_name, &mut args);
    args
}

async fn execute_local_tool_call(
    state: &McpState,
    profile: &crate::store::Profile,
    route: &ToolRoute,
    args: &serde_json::Map<String, serde_json::Value>,
    req_id: RequestId,
    timeout: std::time::Duration,
    timeout_secs: u64,
) -> Result<Option<Response>, Response> {
    if route.kind == ToolRouteKind::SharedLocal {
        let fut = state.catalog.call_tool(
            &route.source_id,
            &route.original_name,
            serde_json::Value::Object(args.clone()),
        );
        let result = match tokio::time::timeout(timeout, fut).await {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                return Err(super::jsonrpc_error_response(
                    req_id,
                    ErrorCode::INTERNAL_ERROR,
                    e.to_string(),
                ));
            }
            Err(_) => {
                return Err(super::jsonrpc_error_response(
                    req_id,
                    ErrorCode::INTERNAL_ERROR,
                    format!("tool call timed out after {timeout_secs}s"),
                ));
            }
        };
        let msg = rmcp::model::ServerJsonRpcMessage::Response(rmcp::model::JsonRpcResponse {
            jsonrpc: JsonRpcVersion2_0,
            id: req_id,
            result: rmcp::model::ServerResult::CallToolResult(result),
        });
        return Ok(Some(super::sse_single_message(&msg)));
    }

    if route.kind == ToolRouteKind::TenantLocal {
        let fut = Box::pin(state.tenant_catalog.call_tool(
            state.store.as_ref(),
            &profile.tenant_id,
            &route.source_id,
            &route.original_name,
            serde_json::Value::Object(args.clone()),
        ));
        let result = match tokio::time::timeout(timeout, fut).await {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                return Err(super::jsonrpc_error_response(
                    req_id,
                    ErrorCode::INTERNAL_ERROR,
                    e.to_string(),
                ));
            }
            Err(_) => {
                return Err(super::jsonrpc_error_response(
                    req_id,
                    ErrorCode::INTERNAL_ERROR,
                    format!("tool call timed out after {timeout_secs}s"),
                ));
            }
        };
        let msg = rmcp::model::ServerJsonRpcMessage::Response(rmcp::model::JsonRpcResponse {
            jsonrpc: JsonRpcVersion2_0,
            id: req_id,
            result: rmcp::model::ServerResult::CallToolResult(result),
        });
        return Ok(Some(super::sse_single_message(&msg)));
    }

    Ok(None)
}

fn inject_timeout_budget_meta(msg: &mut ClientJsonRpcMessage, remaining: std::time::Duration) {
    if let ClientJsonRpcMessage::Request(JsonRpcRequest { request, .. }) = msg {
        let ms: u64 = remaining.as_millis().try_into().unwrap_or(u64::MAX);
        let meta = request.get_meta_mut();
        meta.insert(
            "unrelated".to_string(),
            serde_json::json!({ "timeoutMs": ms }),
        );
    }
}

struct UpstreamToolCall<'a> {
    state: &'a McpState,
    profile_id: &'a str,
    profile: &'a crate::store::Profile,
    payload: &'a TokenPayloadV1,
    route: &'a ToolRoute,
    req_id: &'a RequestId,
    message: ClientJsonRpcMessage,
    timeout: std::time::Duration,
    timeout_secs: u64,
    hop: u32,
}

fn upstream_request_timed_out_error(id: RequestId, timeout_secs: u64) -> Response {
    super::jsonrpc_error_response(
        id,
        ErrorCode::INTERNAL_ERROR,
        format!("upstream request timed out after {timeout_secs}s"),
    )
}

fn find_upstream_binding<'a>(
    call: &'a UpstreamToolCall<'_>,
) -> Option<&'a crate::session_token::UpstreamSessionBinding> {
    call.payload
        .bindings
        .iter()
        .find(|b| b.upstream == call.route.source_id)
}

async fn resolve_upstream_endpoint_url(
    call: &UpstreamToolCall<'_>,
    binding: &crate::session_token::UpstreamSessionBinding,
) -> Result<crate::endpoint_cache::UpstreamEndpoint, Response> {
    super::upstream::resolve_endpoint(call.state, call.profile_id, binding)
        .await?
        .ok_or_else(|| {
            super::jsonrpc_error_response(
                call.req_id.clone(),
                ErrorCode::INTERNAL_ERROR,
                "upstream endpoint not found".to_string(),
            )
        })
}

async fn post_upstream_with_retry(
    call: &UpstreamToolCall<'_>,
    binding: &crate::session_token::UpstreamSessionBinding,
    endpoint_url: &str,
    headers: &reqwest::header::HeaderMap,
    retry: Option<&RetryPolicy>,
    max_attempts: u32,
    deadline: std::time::Instant,
) -> Result<StreamableHttpPostResponse, Response> {
    let mut attempt: u32 = 1;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(upstream_request_timed_out_error(
                call.req_id.clone(),
                call.timeout_secs,
            ));
        }

        let mut msg = call.message.clone();
        inject_timeout_budget_meta(&mut msg, remaining);

        let fut = streamable_http::post_message(
            &call.state.http,
            endpoint_url.to_owned().into(),
            msg,
            Some(binding.session.clone().into()),
            headers,
        );

        match tokio::time::timeout(remaining, fut).await {
            Ok(Ok(r)) => return Ok(r),
            Ok(Err(e)) => {
                let retryable = should_retry_upstream_error(retry, &e);
                let msg = format!("upstream request failed: {e}");
                if !retryable || attempt >= max_attempts {
                    return Err(super::jsonrpc_error_response(
                        call.req_id.clone(),
                        ErrorCode::INTERNAL_ERROR,
                        msg,
                    ));
                }
            }
            Err(_) => {
                let msg = format!("upstream request timed out after {}s", call.timeout_secs);
                let timeout_retryable =
                    retry.is_some_and(|p| !retry_policy_disallows(p, "timeout"));
                if attempt >= max_attempts || !timeout_retryable {
                    return Err(super::jsonrpc_error_response(
                        call.req_id.clone(),
                        ErrorCode::INTERNAL_ERROR,
                        msg,
                    ));
                }
            }
        }

        if let Some(policy) = retry {
            let delay = retry_delay(policy, attempt);
            if !delay.is_zero() {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    return Err(upstream_request_timed_out_error(
                        call.req_id.clone(),
                        call.timeout_secs,
                    ));
                }
                if delay >= remaining {
                    return Err(upstream_request_timed_out_error(
                        call.req_id.clone(),
                        call.timeout_secs,
                    ));
                }
                tokio::time::sleep(delay).await;
            }
        }
        attempt = attempt.saturating_add(1);
    }
}

async fn proxy_upstream_tool_call_with_retry(
    call: UpstreamToolCall<'_>,
) -> Result<Response, Response> {
    let tool_ref = stable_tool_ref(&call.route.source_id, &call.route.original_name);
    let retry = tool_retry_policy_for(call.profile, &tool_ref);
    let max_attempts: u32 = retry.as_ref().map_or(1, |r| r.maximum_attempts.max(1));

    let binding = find_upstream_binding(&call).ok_or_else(|| {
        super::jsonrpc_error_response(
            call.req_id.clone(),
            ErrorCode::INTERNAL_ERROR,
            "upstream session not available".to_string(),
        )
    })?;
    let endpoint = resolve_upstream_endpoint_url(&call, binding).await?;
    if call.hop >= super::upstream::MAX_HOPS {
        return Err(super::jsonrpc_error_response(
            call.req_id.clone(),
            ErrorCode::INTERNAL_ERROR,
            "proxy loop detected (max hops exceeded)".to_string(),
        ));
    }
    let endpoint_url = super::upstream::apply_query_auth(&endpoint.url, endpoint.auth.as_ref());
    let headers = super::upstream::build_upstream_headers(endpoint.auth.as_ref(), call.hop + 1);

    let deadline = std::time::Instant::now() + call.timeout;
    let resp = post_upstream_with_retry(
        &call,
        binding,
        &endpoint_url,
        &headers,
        retry.as_ref(),
        max_attempts,
        deadline,
    )
    .await?;

    match resp {
        StreamableHttpPostResponse::Accepted => Ok(StatusCode::ACCEPTED.into_response()),
        StreamableHttpPostResponse::Json(msg, ..) => Ok(Json(msg).into_response()),
        StreamableHttpPostResponse::Sse(stream, ..) => {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(upstream_request_timed_out_error(
                    call.req_id.clone(),
                    call.timeout_secs,
                ));
            }
            Ok(super::sse_from_upstream_stream_with_timeout(
                stream, remaining,
            ))
        }
    }
}

fn stable_tool_ref(source_id: &str, original_tool_name: &str) -> String {
    format!("{source_id}:{original_tool_name}")
}

fn tool_call_timeout_secs_for(profile: &crate::store::Profile, tool_ref: &str) -> u64 {
    let max = crate::timeouts::tool_call_timeout_max_secs();
    let mut secs = crate::timeouts::tool_call_timeout_default_secs();
    if let Some(v) = profile.tool_call_timeout_secs
        && v > 0
    {
        secs = v.min(max);
    }
    if let Some(p) = profile.tool_policies.iter().find(|p| p.tool == tool_ref)
        && let Some(v) = p.timeout_secs
        && v > 0
    {
        secs = v.min(max);
    }
    secs.max(1)
}

fn tool_retry_policy_for(profile: &crate::store::Profile, tool_ref: &str) -> Option<RetryPolicy> {
    profile
        .tool_policies
        .iter()
        .find(|p| p.tool == tool_ref)
        .and_then(|p| p.retry.clone())
}

fn retry_policy_disallows(policy: &RetryPolicy, category: &str) -> bool {
    policy
        .non_retryable_error_types
        .iter()
        .any(|t| t == category)
}

pub(super) fn retry_delay(policy: &RetryPolicy, attempt: u32) -> std::time::Duration {
    // attempt starts at 1 for the initial try; delay after attempt 1 is `initial_interval`.
    if attempt == 0 {
        return std::time::Duration::from_millis(0);
    }
    let exp = attempt.saturating_sub(1).min(30);
    let coeff = policy.backoff_coefficient;
    if !coeff.is_finite() || coeff <= 0.0 {
        return std::time::Duration::from_millis(0);
    }
    let mult = coeff.powi(i32::try_from(exp).unwrap_or(30));
    if !mult.is_finite() || mult <= 0.0 {
        return std::time::Duration::from_millis(0);
    }

    let mut d = std::time::Duration::from_millis(policy.initial_interval_ms).mul_f64(mult);
    if let Some(max_ms) = policy.maximum_interval_ms {
        d = d.min(std::time::Duration::from_millis(max_ms));
    }
    d
}

fn upstream_error_category(
    e: &rmcp::transport::streamable_http_client::StreamableHttpError<reqwest::Error>,
) -> Option<&'static str> {
    use rmcp::transport::streamable_http_client::StreamableHttpError;
    match e {
        StreamableHttpError::Client(err) => {
            if err.status().is_some_and(|s| s.is_server_error()) {
                return Some("upstream_5xx");
            }
            if err.is_timeout() || err.is_connect() {
                return Some("transport");
            }
            None
        }
        StreamableHttpError::UnexpectedServerResponse(msg) => {
            let s = msg.as_ref();
            if s.contains("http 5") {
                return Some("upstream_5xx");
            }
            None
        }
        // Likely transient / transport-ish.
        StreamableHttpError::Io(_)
        | StreamableHttpError::Sse(_)
        | StreamableHttpError::UnexpectedEndOfStream
        | StreamableHttpError::TokioJoinError(_)
        | StreamableHttpError::TransportChannelClosed => Some("transport"),
        // Might be transient, but often indicates a server bug; still allow retry if configured.
        StreamableHttpError::Deserialize(_) => Some("deserialize"),

        // Default: not retryable.
        _ => None,
    }
}

fn should_retry_upstream_error(
    policy: Option<&RetryPolicy>,
    e: &rmcp::transport::streamable_http_client::StreamableHttpError<reqwest::Error>,
) -> bool {
    let Some(category) = upstream_error_category(e) else {
        return false;
    };
    if policy.is_some_and(|p| retry_policy_disallows(p, category)) {
        return false;
    }
    true
}

pub(super) fn validate_tool_arguments(
    tool: &rmcp::model::Tool,
    args: &serde_json::Value,
) -> Result<(), (String, serde_json::Value)> {
    let schema = serde_json::Value::Object((*tool.input_schema).clone());
    let props = schema
        .get("properties")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let required: Vec<String> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();

    let args_obj = args.as_object().cloned().unwrap_or_default();
    let valid_params: Vec<String> = props.keys().cloned().collect();
    let valid_param_refs: Vec<&str> = valid_params.iter().map(String::as_str).collect();

    let mut violations: Vec<serde_json::Value> = Vec::new();

    // Unknown parameters (suggestions).
    for k in args_obj.keys() {
        if props.contains_key(k) {
            continue;
        }
        let suggestions = find_similar_strings(k, &valid_param_refs);
        violations.push(serde_json::json!({
            "type": "invalid-parameter",
            "parameter": k,
            "suggestions": suggestions,
            "validParameters": valid_params,
        }));
    }

    // Missing required parameters.
    for r in &required {
        if !args_obj.contains_key(r) {
            violations.push(serde_json::json!({
                "type": "missing-required-parameter",
                "parameter": r,
            }));
        }
    }

    // JSON Schema validation (types/constraints).
    if let Ok(compiled) = jsonschema::validator_for(&schema) {
        for e in compiled.iter_errors(args) {
            // Filter out "required" errors; we already report them with a nicer shape.
            if matches!(
                e.kind(),
                jsonschema::error::ValidationErrorKind::Required { .. }
            ) {
                continue;
            }
            let instance_path = e.instance_path().to_string();
            violations.push(serde_json::json!({
                "type": "constraint-violation",
                "message": e.to_string(),
                "instancePath": instance_path,
            }));
        }
    }

    if violations.is_empty() {
        return Ok(());
    }

    // Message: optimize for unknown-parameter typos (even if there are other violations too).
    let msg = if let Some(v) = violations
        .iter()
        .find(|v| v.get("type").and_then(|t| t.as_str()) == Some("invalid-parameter"))
    {
        let p = v.get("parameter").and_then(|v| v.as_str()).unwrap_or("?");
        let suggestion = v
            .get("suggestions")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str());
        if let Some(s) = suggestion {
            format!("Invalid params: unknown parameter '{p}' (did you mean '{s}'?)")
        } else {
            format!("Invalid params: unknown parameter '{p}'")
        }
    } else {
        format!(
            "Invalid params: validation failed with {} error(s)",
            violations.len()
        )
    };

    Err((
        msg,
        serde_json::json!({
            "type": "validation-errors",
            "violations": violations,
        }),
    ))
}

fn find_similar_strings(unknown: &str, known: &[&str]) -> Vec<String> {
    let mut candidates: Vec<(f64, String)> = Vec::new();
    for k in known {
        let score = strsim::jaro(unknown, k);
        if score > 0.7 {
            candidates.push((score, (*k).to_string()));
        }
    }
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    candidates.into_iter().map(|(_, s)| s).collect()
}
