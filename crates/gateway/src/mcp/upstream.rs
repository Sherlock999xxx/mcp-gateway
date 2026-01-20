use super::McpState;
use super::streamable_http;
use crate::session_token::{TokenPayloadV1, UpstreamSessionBinding};
use axum::{Json, http::StatusCode, response::IntoResponse as _, response::Response};
use base64::Engine as _;
use futures::StreamExt as _;
use rmcp::{
    model::{ClientJsonRpcMessage, ClientRequest, JsonRpcRequest, JsonRpcVersion2_0, ServerResult},
    transport::streamable_http_client::StreamableHttpPostResponse,
};
use std::collections::HashMap;
use unrelated_http_tools::config::AuthConfig;

pub(super) const HOP_HEADER: &str = "x-unrelated-gateway-hop";
pub(super) const MAX_HOPS: u32 = 8;

pub(super) async fn upstream_initialize(
    http: &reqwest::Client,
    mcp_url: &str,
    init_message: &ClientJsonRpcMessage,
    headers: &reqwest::header::HeaderMap,
) -> anyhow::Result<String> {
    let resp = streamable_http::post_message(
        http,
        mcp_url.to_string().into(),
        init_message.clone(),
        None,
        headers,
    )
    .await?;
    let (_msg, session_id) = resp.expect_initialized::<reqwest::Error>().await?;
    let session_id = session_id.ok_or_else(|| anyhow::anyhow!("missing upstream session id"))?;

    // MCP handshake: client must send `notifications/initialized` after `initialize`.
    // Some upstream servers (including our adapter) treat the session as invalid until this occurs.
    let initialized: ClientJsonRpcMessage = serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;

    match streamable_http::post_message(
        http,
        mcp_url.to_string().into(),
        initialized,
        Some(session_id.clone().into()),
        headers,
    )
    .await?
    {
        StreamableHttpPostResponse::Accepted => {}
        other => {
            return Err(anyhow::anyhow!(
                "unexpected response to notifications/initialized: {other:?}"
            ));
        }
    }

    Ok(session_id)
}

pub(super) fn build_upstream_headers(
    auth: Option<&AuthConfig>,
    hop: u32,
) -> reqwest::header::HeaderMap {
    use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderName, HeaderValue};
    let mut headers = HeaderMap::new();

    // Loop guard (best-effort).
    if hop > 0
        && let Ok(v) = HeaderValue::from_str(&hop.to_string())
    {
        headers.insert(HOP_HEADER, v);
    }

    // Upstream auth (explicit; never forward caller Authorization).
    let Some(auth) = auth else {
        return headers;
    };
    match auth {
        AuthConfig::None | AuthConfig::Query { .. } => {}
        AuthConfig::Bearer { token } => {
            if let Ok(v) = HeaderValue::from_str(&format!("Bearer {token}")) {
                headers.insert(AUTHORIZATION, v);
            }
        }
        AuthConfig::Header { name, value } => {
            if let Ok(n) = HeaderName::from_bytes(name.as_bytes())
                && let Ok(v) = HeaderValue::from_str(value)
            {
                headers.insert(n, v);
            }
        }
        AuthConfig::Basic { username, password } => {
            let b64 =
                base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
            if let Ok(v) = HeaderValue::from_str(&format!("Basic {b64}")) {
                headers.insert(AUTHORIZATION, v);
            }
        }
    }
    headers
}

pub(super) fn apply_query_auth(url: &str, auth: Option<&AuthConfig>) -> String {
    let Some(AuthConfig::Query { name, value }) = auth else {
        return url.to_string();
    };
    let Ok(mut u) = reqwest::Url::parse(url) else {
        return url.to_string();
    };
    u.query_pairs_mut()
        .append_pair(name.as_str(), value.as_str());
    u.to_string()
}

pub(super) async fn proxy_to_single_upstream(
    state: &McpState,
    profile_id: &str,
    payload: &TokenPayloadV1,
    upstream_id: &str,
    message: ClientJsonRpcMessage,
    hop: u32,
) -> Result<Response, Response> {
    if hop >= MAX_HOPS {
        return Err((
            StatusCode::BAD_GATEWAY,
            "proxy loop detected (max hops exceeded)",
        )
            .into_response());
    }
    let binding = payload
        .bindings
        .iter()
        .find(|b| b.upstream == upstream_id)
        .ok_or_else(|| {
            (StatusCode::BAD_GATEWAY, "upstream session not available").into_response()
        })?;

    let Some(endpoint) = resolve_endpoint(state, profile_id, binding).await? else {
        return Err((StatusCode::BAD_GATEWAY, "upstream endpoint not found").into_response());
    };

    let endpoint_url = apply_query_auth(&endpoint.url, endpoint.auth.as_ref());
    let headers = build_upstream_headers(endpoint.auth.as_ref(), hop + 1);

    let resp = streamable_http::post_message(
        &state.http,
        endpoint_url.into(),
        message,
        Some(binding.session.clone().into()),
        &headers,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("upstream request failed: {e}"),
        )
            .into_response()
    })?;

    Ok(match resp {
        StreamableHttpPostResponse::Accepted => StatusCode::ACCEPTED.into_response(),
        StreamableHttpPostResponse::Json(msg, ..) => Json(msg).into_response(),
        StreamableHttpPostResponse::Sse(stream, ..) => super::sse_from_upstream_stream(stream),
    })
}

pub(super) async fn resolve_endpoint_url(
    state: &McpState,
    _profile_id: &str,
    binding: &UpstreamSessionBinding,
) -> Result<Option<String>, Response> {
    if let Some(ep) = state
        .endpoint_cache
        .get(&binding.upstream, &binding.endpoint)
    {
        return Ok(Some(ep.url));
    }

    let upstream = state
        .store
        .get_upstream(&binding.upstream)
        .await
        .map_err(super::internal_error_response("load upstream"))?;
    let Some(upstream) = upstream else {
        return Ok(None);
    };

    let mut endpoints: HashMap<String, crate::endpoint_cache::UpstreamEndpoint> = HashMap::new();
    for e in upstream.endpoints {
        endpoints.insert(
            e.id,
            crate::endpoint_cache::UpstreamEndpoint {
                url: e.url,
                auth: e.auth,
            },
        );
    }
    let url = endpoints.get(&binding.endpoint).map(|e| e.url.clone());
    state
        .endpoint_cache
        .put(binding.upstream.clone(), endpoints);
    Ok(url)
}

pub(super) async fn resolve_endpoint(
    state: &McpState,
    profile_id: &str,
    binding: &UpstreamSessionBinding,
) -> Result<Option<crate::endpoint_cache::UpstreamEndpoint>, Response> {
    if let Some(ep) = state
        .endpoint_cache
        .get(&binding.upstream, &binding.endpoint)
    {
        return Ok(Some(ep));
    }
    let _ = resolve_endpoint_url(state, profile_id, binding).await?;
    Ok(state
        .endpoint_cache
        .get(&binding.upstream, &binding.endpoint))
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn list_all_upstreams<T, FBuild, FExtract>(
    state: &McpState,
    profile_id: &str,
    payload: &TokenPayloadV1,
    build_request: FBuild,
    extract: FExtract,
    request_failed_message: &'static str,
    transport_failed_message: &'static str,
    hop: u32,
) -> Result<Vec<(String, T)>, Response>
where
    FBuild: Fn() -> ClientJsonRpcMessage,
    FExtract: Fn(ServerResult) -> Option<T>,
{
    if hop >= MAX_HOPS {
        return Err((
            StatusCode::BAD_GATEWAY,
            "proxy loop detected (max hops exceeded)",
        )
            .into_response());
    }
    let mut out = Vec::new();
    for binding in &payload.bindings {
        let Some(endpoint) = resolve_endpoint(state, profile_id, binding).await? else {
            continue;
        };
        let endpoint_url = apply_query_auth(&endpoint.url, endpoint.auth.as_ref());
        let headers = build_upstream_headers(endpoint.auth.as_ref(), hop + 1);
        let request = build_request();
        match streamable_http::post_message(
            &state.http,
            endpoint_url.into(),
            request,
            Some(binding.session.clone().into()),
            &headers,
        )
        .await
        {
            Ok(resp) => match read_first_response(resp).await {
                Ok(result) => {
                    if let Some(v) = extract(result) {
                        out.push((binding.upstream.clone(), v));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        upstream_id = %binding.upstream,
                        error = %e,
                        "{request_failed_message}"
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    upstream_id = %binding.upstream,
                    error = %e,
                    "{transport_failed_message}"
                );
            }
        }
    }
    Ok(out)
}

pub(super) async fn list_tools_all_upstreams(
    state: &McpState,
    profile_id: &str,
    payload: &TokenPayloadV1,
    hop: u32,
) -> Result<Vec<(String, Vec<rmcp::model::Tool>)>, Response> {
    list_all_upstreams(
        state,
        profile_id,
        payload,
        || {
            ClientJsonRpcMessage::Request(JsonRpcRequest {
                jsonrpc: JsonRpcVersion2_0,
                id: rmcp::model::RequestId::Number(1),
                request: ClientRequest::ListToolsRequest(rmcp::model::ListToolsRequest {
                    method: rmcp::model::ListToolsRequestMethod,
                    params: None,
                    extensions: rmcp::model::Extensions::default(),
                }),
            })
        },
        |result| match result {
            ServerResult::ListToolsResult(r) => Some(r.tools),
            _ => None,
        },
        "tools/list failed",
        "tools/list transport failed",
        hop,
    )
    .await
}

pub(super) async fn list_resources_all_upstreams(
    state: &McpState,
    profile_id: &str,
    payload: &TokenPayloadV1,
    hop: u32,
) -> Result<Vec<(String, Vec<rmcp::model::Resource>)>, Response> {
    list_all_upstreams(
        state,
        profile_id,
        payload,
        || {
            ClientJsonRpcMessage::Request(JsonRpcRequest {
                jsonrpc: JsonRpcVersion2_0,
                id: rmcp::model::RequestId::Number(1),
                request: ClientRequest::ListResourcesRequest(rmcp::model::ListResourcesRequest {
                    method: rmcp::model::ListResourcesRequestMethod,
                    params: None,
                    extensions: rmcp::model::Extensions::default(),
                }),
            })
        },
        |result| match result {
            ServerResult::ListResourcesResult(r) => Some(r.resources),
            _ => None,
        },
        "resources/list failed",
        "resources/list transport failed",
        hop,
    )
    .await
}

pub(super) async fn list_prompts_all_upstreams(
    state: &McpState,
    profile_id: &str,
    payload: &TokenPayloadV1,
    hop: u32,
) -> Result<Vec<(String, Vec<rmcp::model::Prompt>)>, Response> {
    list_all_upstreams(
        state,
        profile_id,
        payload,
        || {
            ClientJsonRpcMessage::Request(JsonRpcRequest {
                jsonrpc: JsonRpcVersion2_0,
                id: rmcp::model::RequestId::Number(1),
                request: ClientRequest::ListPromptsRequest(rmcp::model::ListPromptsRequest {
                    method: rmcp::model::ListPromptsRequestMethod,
                    params: None,
                    extensions: rmcp::model::Extensions::default(),
                }),
            })
        },
        |result| match result {
            ServerResult::ListPromptsResult(r) => Some(r.prompts),
            _ => None,
        },
        "prompts/list failed",
        "prompts/list transport failed",
        hop,
    )
    .await
}

pub(super) async fn read_first_response(
    resp: StreamableHttpPostResponse,
) -> anyhow::Result<ServerResult> {
    match resp {
        StreamableHttpPostResponse::Json(msg, ..) => match msg {
            rmcp::model::ServerJsonRpcMessage::Response(r) => Ok(r.result),
            rmcp::model::ServerJsonRpcMessage::Error(e) => {
                Err(anyhow::anyhow!("upstream error: {}", e.error.message))
            }
            other => Err(anyhow::anyhow!("unexpected upstream message: {other:?}")),
        },
        StreamableHttpPostResponse::Sse(mut stream, ..) => {
            while let Some(evt) = stream.next().await {
                let evt = evt?;
                let payload = evt.data.unwrap_or_default();
                if payload.trim().is_empty() {
                    continue;
                }
                let msg: rmcp::model::ServerJsonRpcMessage = serde_json::from_str(&payload)?;
                if let rmcp::model::ServerJsonRpcMessage::Response(r) = msg {
                    return Ok(r.result);
                }
            }
            Err(anyhow::anyhow!("unexpected end of sse stream"))
        }
        StreamableHttpPostResponse::Accepted => Err(anyhow::anyhow!("unexpected accepted")),
    }
}
