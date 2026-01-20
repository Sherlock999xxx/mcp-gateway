use crate::profile_http::{
    DataPlaneAuthSettings, DataPlaneLimitsSettings, NullableString, NullableU64,
    default_data_plane_auth_mode, default_true, resolve_nullable_u64, validate_tool_allowlist,
    validate_tool_timeout_and_policies,
};
use crate::store::{
    AdminProfile, AdminStore, AdminTenant, AdminUpstream, DataPlaneAuthMode, McpProfileSettings,
    OidcPrincipalBinding, TenantSecretMetadata, ToolSourceKind, UpstreamEndpoint,
};
use crate::tenant::{IssueTenantTokenRequest, IssueTenantTokenResponse, now_unix_secs};
use crate::tenant_token::{TenantSigner, TenantTokenPayloadV1};
use crate::tool_policy::ToolPolicy;
use axum::{
    Extension, Json, Router,
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use unrelated_http_tools::config::AuthConfig;
use unrelated_http_tools::config::HttpServerConfig;
use unrelated_openapi_tools::config::ApiServerConfig;
use unrelated_tool_transforms::TransformPipeline;
use uuid::{Uuid, Version};

const OIDC_NOT_CONFIGURED_MSG: &str = "JWT/OIDC is unavailable because OIDC is not configured on the Gateway (missing UNRELATED_GATEWAY_OIDC_ISSUER). Configure OIDC or choose a different mode.";
type BoxResponse = Box<axum::response::Response>;

#[derive(Clone)]
pub struct AdminState {
    pub store: Option<Arc<dyn AdminStore>>,
    pub admin_token: Option<String>,
    /// Enable the fresh-install bootstrap endpoint.
    ///
    /// When false, `/bootstrap/v1/tenant` is disabled.
    pub bootstrap_enabled: bool,
    pub tenant_signer: TenantSigner,
    pub shared_source_ids: Arc<std::collections::HashSet<String>>,
    pub oidc_issuer: Option<String>,
}

pub fn router() -> Router {
    Router::new()
        // Bootstrap (fresh install)
        .route("/bootstrap/v1/tenant/status", get(bootstrap_tenant_status))
        .route("/bootstrap/v1/tenant", post(bootstrap_tenant))
        .route("/admin/v1/tenants", post(put_tenant).get(list_tenants))
        .route(
            "/admin/v1/tenants/{tenant_id}",
            get(get_tenant).delete(delete_tenant),
        )
        .route(
            "/admin/v1/tenants/{tenant_id}/tool-sources",
            get(list_tool_sources),
        )
        .route(
            "/admin/v1/tenants/{tenant_id}/tool-sources/{source_id}",
            get(get_tool_source)
                .put(put_tool_source)
                .delete(delete_tool_source),
        )
        .route("/admin/v1/tenants/{tenant_id}/secrets", get(list_secrets))
        .route(
            "/admin/v1/tenants/{tenant_id}/secrets/{name}",
            put(put_secret).delete(delete_secret),
        )
        .route(
            "/admin/v1/tenants/{tenant_id}/oidc-principals",
            get(list_oidc_principals).put(put_oidc_principal),
        )
        .route(
            "/admin/v1/tenants/{tenant_id}/oidc-principals/{subject}",
            delete(delete_oidc_principal),
        )
        .route(
            "/admin/v1/upstreams",
            post(put_upstream).get(list_upstreams),
        )
        .route(
            "/admin/v1/upstreams/{upstream_id}",
            get(get_upstream).delete(delete_upstream),
        )
        .route("/admin/v1/profiles", post(put_profile).get(list_profiles))
        .route(
            "/admin/v1/profiles/{profile_id}",
            get(get_profile).delete(delete_profile),
        )
        .route("/admin/v1/tenant-tokens", post(issue_tenant_token))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapTenantRequest {
    /// First tenant id to create as the initial tenant.
    tenant_id: String,
    /// Optional tenant token TTL (seconds). Defaults to 365 days.
    #[serde(default)]
    ttl_seconds: Option<u64>,
    /// If true (default), create a starter profile for the new tenant.
    #[serde(default = "default_true")]
    create_profile: bool,
    /// Starter profile name when `createProfile` is true.
    #[serde(default)]
    profile_name: Option<String>,
    /// Starter profile description when `createProfile` is true.
    #[serde(default)]
    profile_description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapTenantResponse {
    ok: bool,
    tenant_id: String,
    token: String,
    exp_unix_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_plane_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapTenantStatusResponse {
    bootstrap_enabled: bool,
    can_bootstrap: bool,
    tenant_count: usize,
}

async fn bootstrap_tenant_status(
    Extension(state): Extension<Arc<AdminState>>,
) -> impl IntoResponse {
    // Mirror the bootstrap endpoint behavior: hidden unless explicitly enabled.
    if !state.bootstrap_enabled {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    let tenants = match store.list_tenants().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    Json(BootstrapTenantStatusResponse {
        bootstrap_enabled: true,
        can_bootstrap: tenants.is_empty(),
        tenant_count: tenants.len(),
    })
    .into_response()
}

async fn bootstrap_tenant(
    Extension(state): Extension<Arc<AdminState>>,
    Json(req): Json<BootstrapTenantRequest>,
) -> impl IntoResponse {
    // Safety: only enabled explicitly.
    if !state.bootstrap_enabled {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    let tenant_id = req.tenant_id.trim();
    if tenant_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "tenantId is required").into_response();
    }

    // Only allow bootstrapping on an empty DB.
    let existing = match store.list_tenants().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    if !existing.is_empty() {
        return (StatusCode::CONFLICT, "already bootstrapped").into_response();
    }

    if let Err(e) = store.put_tenant(tenant_id, true).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    let mut profile_id: Option<String> = None;
    let mut data_plane_path: Option<String> = None;
    if req.create_profile {
        let pid = Uuid::new_v4().to_string();
        let name = req
            .profile_name
            .as_deref()
            .unwrap_or("Starter profile")
            .trim();
        if name.is_empty() {
            return (StatusCode::BAD_REQUEST, "profileName must be non-empty").into_response();
        }
        let description = req.profile_description.as_deref();

        // Create an empty (no upstreams/sources) profile; UI can attach sources later.
        if let Err(e) = store
            .put_profile(
                &pid,
                tenant_id,
                name,
                description,
                true, // enabled
                true, // allow_partial_upstreams
                &[],  // upstream_ids
                &[],  // source_ids
                &TransformPipeline::default(),
                &[],
                DataPlaneAuthMode::ApiKeyInitializeOnly,
                true,  // accept_x_api_key
                false, // rate_limit_enabled
                None,
                false, // quota_enabled
                None,
                None, // tool_call_timeout_secs
                &[],
                &McpProfileSettings::default(),
            )
            .await
        {
            if e.to_string().contains("profiles_tenant_name_ci_uq") {
                return (
                    StatusCode::CONFLICT,
                    "profile name already exists for this tenant (case-insensitive)",
                )
                    .into_response();
            }
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }

        data_plane_path = Some(format!("/{pid}/mcp"));
        profile_id = Some(pid);
    }

    let ttl = req.ttl_seconds.unwrap_or(31_536_000);
    let now = match now_unix_secs() {
        Ok(n) => n,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let exp = now.saturating_add(ttl).max(now + 1);

    let payload = TenantTokenPayloadV1 {
        tenant_id: tenant_id.to_string(),
        exp_unix_secs: exp,
    };
    let token = match state.tenant_signer.sign_v1(&payload) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    Json(BootstrapTenantResponse {
        ok: true,
        tenant_id: tenant_id.to_string(),
        token,
        exp_unix_secs: exp,
        profile_id,
        data_plane_path,
    })
    .into_response()
}

fn authz(headers: &HeaderMap, expected: Option<&str>) -> Result<(), impl IntoResponse> {
    let Some(expected) = expected else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Admin API disabled (UNRELATED_GATEWAY_ADMIN_TOKEN not set)",
        ));
    };
    let got = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    let want = format!("Bearer {expected}");
    if got == want {
        Ok(())
    } else {
        Err((StatusCode::UNAUTHORIZED, "Unauthorized"))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PutTenantRequest {
    id: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PutUpstreamRequest {
    id: String,
    #[serde(default = "default_true")]
    enabled: bool,
    endpoints: Vec<PutEndpoint>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PutEndpoint {
    id: String,
    url: String,
    #[serde(default)]
    auth: Option<AuthConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PutProfileRequest {
    #[serde(default)]
    id: Option<String>,
    tenant_id: String,
    /// Human-friendly profile name (unique per tenant, case-insensitive).
    ///
    /// If omitted, defaults to the existing profile name when updating.
    #[serde(default)]
    name: Option<String>,
    /// Optional human-friendly description (PUT semantics).
    ///
    /// - omitted => keep existing description
    /// - null => clear description
    /// - string => set description
    #[serde(default)]
    description: Option<NullableString>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_true")]
    allow_partial_upstreams: bool,
    upstreams: Vec<String>,
    /// Local tool sources attached to this profile (shared + tenant-owned).
    #[serde(default)]
    sources: Vec<String>,
    /// Per-profile tool transforms (renames/defaults).
    #[serde(default)]
    transforms: TransformPipeline,
    /// Per-profile tool allowlist.
    ///
    /// Semantics:
    /// - omitted / `null` / `[]` => no allowlist configured (allow all tools)
    /// - otherwise entries should be `"<source_id>:<original_tool_name>"`.
    #[serde(default)]
    tools: Option<Vec<String>>,

    /// Optional per-profile data-plane auth settings.
    #[serde(default)]
    data_plane_auth: Option<DataPlaneAuthSettings>,

    /// Optional per-profile data-plane limits (rate limits and quotas).
    #[serde(default)]
    data_plane_limits: Option<DataPlaneLimitsSettings>,

    /// Optional per-profile default timeout override for `tools/call` (seconds).
    #[serde(default)]
    tool_call_timeout_secs: Option<NullableU64>,
    /// Optional per-profile per-tool policies (timeouts + retry policy).
    #[serde(default)]
    tool_policies: Option<Vec<ToolPolicy>>,

    /// Optional per-profile MCP proxy behavior settings (capabilities allow/deny, notification filters, namespacing).
    #[serde(default)]
    mcp: Option<McpProfileSettings>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OkResponse {
    ok: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateProfileResponse {
    ok: bool,
    id: String,
    data_plane_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TenantsResponse {
    tenants: Vec<TenantResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TenantResponse {
    id: String,
    enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpstreamsResponse {
    upstreams: Vec<UpstreamResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpstreamResponse {
    id: String,
    enabled: bool,
    endpoints: Vec<UpstreamEndpointResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpstreamEndpointResponse {
    id: String,
    url: String,
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<AuthConfig>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfilesResponse {
    profiles: Vec<ProfileResponse>,
}

fn is_profile_mcp_endpoint_url(profile_id: &str, url: &str) -> bool {
    let Ok(u) = reqwest::Url::parse(url) else {
        return false;
    };
    let want = format!("/{profile_id}/mcp");
    u.path() == want || u.path() == format!("{want}/")
}

async fn validate_no_self_upstream_loop(
    store: &dyn AdminStore,
    profile_id: &str,
    upstream_ids: &[String],
) -> Result<(), axum::response::Response> {
    for upstream_id in upstream_ids {
        let Some(upstream) = store
            .get_upstream(upstream_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?
        else {
            continue;
        };
        for ep in upstream.endpoints {
            if is_profile_mcp_endpoint_url(profile_id, &ep.url) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!(
                        "upstream endpoint '{}' points to this profile's MCP endpoint (self-loop)",
                        ep.url
                    ),
                )
                    .into_response());
            }
        }
    }
    Ok(())
}

async fn put_tenant(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Json(req): Json<PutTenantRequest>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };
    if let Err(e) = store.put_tenant(&req.id, req.enabled).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    (StatusCode::CREATED, Json(OkResponse { ok: true })).into_response()
}

async fn list_tenants(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.list_tenants().await {
        Ok(tenants) => Json(TenantsResponse {
            tenants: tenants.into_iter().map(tenant_to_response).collect(),
        })
        .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_tenant(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.get_tenant(&tenant_id).await {
        Ok(Some(t)) => Json(tenant_to_response(t)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "tenant not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_tenant(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.delete_tenant(&tenant_id).await {
        Ok(true) => Json(OkResponse { ok: true }).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "tenant not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn put_upstream(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Json(req): Json<PutUpstreamRequest>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    let endpoints: Vec<UpstreamEndpoint> = req
        .endpoints
        .into_iter()
        .map(|e| UpstreamEndpoint {
            id: e.id,
            url: e.url,
            auth: e.auth,
        })
        .collect();

    if let Err(e) = store.put_upstream(&req.id, req.enabled, &endpoints).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    (StatusCode::CREATED, Json(OkResponse { ok: true })).into_response()
}

async fn list_upstreams(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.list_upstreams().await {
        Ok(upstreams) => Json(UpstreamsResponse {
            upstreams: upstreams.into_iter().map(upstream_to_response).collect(),
        })
        .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_upstream(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path(upstream_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.get_upstream(&upstream_id).await {
        Ok(Some(u)) => Json(upstream_to_response(u)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "upstream not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_upstream(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path(upstream_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.delete_upstream(&upstream_id).await {
        Ok(true) => Json(OkResponse { ok: true }).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "upstream not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn parse_or_generate_profile_uuid(id: Option<&str>) -> Result<Uuid, &'static str> {
    let Some(id) = id else {
        return Ok(Uuid::new_v4());
    };
    match Uuid::parse_str(id) {
        Ok(u) if u.get_version() == Some(Version::Random) => Ok(u),
        _ => Err("profile id must be a UUIDv4 (random)"),
    }
}

fn resolve_data_plane_auth_settings(
    req: Option<DataPlaneAuthSettings>,
    existing: Option<&AdminProfile>,
    is_update: bool,
) -> DataPlaneAuthSettings {
    match req {
        Some(v) => v,
        None => {
            if is_update {
                existing.map_or(
                    DataPlaneAuthSettings {
                        mode: default_data_plane_auth_mode(),
                        accept_x_api_key: default_true(),
                    },
                    |p| DataPlaneAuthSettings {
                        mode: p.data_plane_auth_mode,
                        accept_x_api_key: p.accept_x_api_key,
                    },
                )
            } else {
                DataPlaneAuthSettings {
                    mode: default_data_plane_auth_mode(),
                    accept_x_api_key: default_true(),
                }
            }
        }
    }
}

fn resolve_data_plane_limits_settings(
    req: Option<DataPlaneLimitsSettings>,
    existing: Option<&AdminProfile>,
    is_update: bool,
) -> Result<DataPlaneLimitsSettings, &'static str> {
    let limits = match req {
        Some(v) => v,
        None => {
            if is_update {
                existing.map_or(
                    DataPlaneLimitsSettings {
                        rate_limit_enabled: false,
                        rate_limit_tool_calls_per_minute: None,
                        quota_enabled: false,
                        quota_tool_calls: None,
                    },
                    |p| DataPlaneLimitsSettings {
                        rate_limit_enabled: p.rate_limit_enabled,
                        rate_limit_tool_calls_per_minute: p.rate_limit_tool_calls_per_minute,
                        quota_enabled: p.quota_enabled,
                        quota_tool_calls: p.quota_tool_calls,
                    },
                )
            } else {
                DataPlaneLimitsSettings {
                    rate_limit_enabled: false,
                    rate_limit_tool_calls_per_minute: None,
                    quota_enabled: false,
                    quota_tool_calls: None,
                }
            }
        }
    };
    limits.validate()?;
    Ok(limits)
}

fn resolve_tool_call_timeout_secs(
    req: Option<NullableU64>,
    existing: Option<&AdminProfile>,
) -> Option<u64> {
    resolve_nullable_u64(req, existing.and_then(|p| p.tool_call_timeout_secs))
}

fn resolve_tool_policies(
    req: Option<Vec<ToolPolicy>>,
    existing: Option<&AdminProfile>,
) -> Vec<ToolPolicy> {
    req.or_else(|| existing.map(|p| p.tool_policies.clone()))
        .unwrap_or_default()
}

fn resolve_mcp_settings(
    req: Option<McpProfileSettings>,
    existing: Option<&AdminProfile>,
) -> McpProfileSettings {
    req.or_else(|| existing.map(|p| p.mcp.clone()))
        .unwrap_or_default()
}

async fn load_existing_profile_for_update(
    store: &dyn AdminStore,
    profile_id: &str,
    is_update: bool,
) -> Result<Option<AdminProfile>, BoxResponse> {
    if !is_update {
        return Ok(None);
    }
    store
        .get_profile(profile_id)
        .await
        .map_err(|e| Box::new((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()))
}

fn resolve_profile_name(
    req_name: Option<String>,
    existing: Option<&AdminProfile>,
) -> Result<String, BoxResponse> {
    let name = match (req_name, existing) {
        (Some(n), _) => n,
        (None, Some(p)) => p.name.clone(),
        (None, None) => {
            return Err(Box::new(
                (StatusCode::BAD_REQUEST, "name is required").into_response(),
            ));
        }
    };
    if name.trim().is_empty() {
        return Err(Box::new(
            (StatusCode::BAD_REQUEST, "name is required").into_response(),
        ));
    }
    Ok(name)
}

fn resolve_profile_description(
    req_description: Option<&NullableString>,
    existing: Option<&AdminProfile>,
) -> Option<String> {
    match req_description {
        None => existing.and_then(|p| p.description.clone()),
        Some(NullableString::Null) => None,
        Some(NullableString::Value(v)) => Some(v.clone()),
    }
}

fn validate_oidc_configured_if_needed(
    oidc_issuer: Option<&str>,
    mode: DataPlaneAuthMode,
) -> Result<(), BoxResponse> {
    if mode == DataPlaneAuthMode::JwtEveryRequest && oidc_issuer.is_none() {
        return Err(Box::new(
            (StatusCode::BAD_REQUEST, OIDC_NOT_CONFIGURED_MSG).into_response(),
        ));
    }
    Ok(())
}

struct PutProfileStoreInputs<'a> {
    profile_id: &'a str,
    name: &'a str,
    description: Option<&'a str>,
    enabled_tools: &'a [String],
    data_plane_auth: DataPlaneAuthSettings,
    data_plane_limits: DataPlaneLimitsSettings,
    tool_call_timeout_secs: Option<u64>,
    tool_policies: &'a [ToolPolicy],
    mcp: &'a McpProfileSettings,
}

async fn put_profile_in_store(
    store: &dyn AdminStore,
    req: &PutProfileRequest,
    input: PutProfileStoreInputs<'_>,
) -> Result<(), BoxResponse> {
    store
        .put_profile(
            input.profile_id,
            &req.tenant_id,
            input.name,
            input.description,
            req.enabled,
            req.allow_partial_upstreams,
            &req.upstreams,
            &req.sources,
            &req.transforms,
            input.enabled_tools,
            input.data_plane_auth.mode,
            input.data_plane_auth.accept_x_api_key,
            input.data_plane_limits.rate_limit_enabled,
            input.data_plane_limits.rate_limit_tool_calls_per_minute,
            input.data_plane_limits.quota_enabled,
            input.data_plane_limits.quota_tool_calls,
            input.tool_call_timeout_secs,
            input.tool_policies,
            input.mcp,
        )
        .await
        .map_err(|e| {
            if e.to_string().contains("profiles_tenant_name_ci_uq") {
                Box::new(
                    (
                        StatusCode::CONFLICT,
                        "profile name already exists for this tenant (case-insensitive)",
                    )
                        .into_response(),
                )
            } else {
                Box::new((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
            }
        })?;
    Ok(())
}

async fn put_profile(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Json(req): Json<PutProfileRequest>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    let profile_uuid = match parse_or_generate_profile_uuid(req.id.as_deref()) {
        Ok(u) => u,
        Err(msg) => return (StatusCode::BAD_REQUEST, msg).into_response(),
    };
    let profile_id = profile_uuid.to_string();
    let is_update = req.id.is_some();

    let existing =
        match load_existing_profile_for_update(store.as_ref(), &profile_id, is_update).await {
            Ok(p) => p,
            Err(resp) => return *resp,
        };
    let name = match resolve_profile_name(req.name.clone(), existing.as_ref()) {
        Ok(n) => n,
        Err(resp) => return *resp,
    };
    let description = resolve_profile_description(req.description.as_ref(), existing.as_ref());

    let enabled_tools = req.tools.as_deref().unwrap_or(&[]);
    let data_plane_auth =
        resolve_data_plane_auth_settings(req.data_plane_auth.clone(), existing.as_ref(), is_update);
    if let Err(resp) =
        validate_oidc_configured_if_needed(state.oidc_issuer.as_deref(), data_plane_auth.mode)
    {
        return *resp;
    }
    let data_plane_limits = match resolve_data_plane_limits_settings(
        req.data_plane_limits.clone(),
        existing.as_ref(),
        is_update,
    ) {
        Ok(v) => v,
        Err(msg) => return (StatusCode::BAD_REQUEST, msg).into_response(),
    };

    // Tool call timeouts + per-tool policies (timeouts + retry policy).
    let tool_call_timeout_secs =
        resolve_tool_call_timeout_secs(req.tool_call_timeout_secs, existing.as_ref());
    let tool_policies = resolve_tool_policies(req.tool_policies.clone(), existing.as_ref());
    let mcp = resolve_mcp_settings(req.mcp.clone(), existing.as_ref());
    if let Err(msg) = validate_tool_timeout_and_policies(tool_call_timeout_secs, &tool_policies) {
        return (StatusCode::BAD_REQUEST, msg).into_response();
    }
    if let Err(msg) = validate_tool_allowlist(enabled_tools) {
        return (StatusCode::BAD_REQUEST, msg).into_response();
    }

    if let Err(resp) =
        validate_no_self_upstream_loop(store.as_ref(), &profile_id, &req.upstreams).await
    {
        return resp;
    }

    let store_input = PutProfileStoreInputs {
        profile_id: &profile_id,
        name: &name,
        description: description.as_deref(),
        enabled_tools,
        data_plane_auth,
        data_plane_limits,
        tool_call_timeout_secs,
        tool_policies: &tool_policies,
        mcp: &mcp,
    };
    if let Err(resp) = put_profile_in_store(store.as_ref(), &req, store_input).await {
        return *resp;
    }
    (
        StatusCode::CREATED,
        Json(CreateProfileResponse {
            ok: true,
            data_plane_path: format!("/{profile_id}/mcp"),
            id: profile_id,
        }),
    )
        .into_response()
}

async fn list_profiles(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }

    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.list_profiles().await {
        Ok(profiles) => Json(ProfilesResponse {
            profiles: profiles
                .into_iter()
                .map(profile_to_admin_response)
                .collect(),
        })
        .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_profile(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }

    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    // Avoid leaking details / DB errors on obviously-invalid ids.
    if Uuid::parse_str(&profile_id)
        .ok()
        .and_then(|u| (u.get_version() == Some(Version::Random)).then_some(u))
        .is_none()
    {
        return (StatusCode::NOT_FOUND, "profile not found").into_response();
    }

    match store.get_profile(&profile_id).await {
        Ok(Some(profile)) => Json(profile_to_admin_response(profile)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "profile not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_profile(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    if Uuid::parse_str(&profile_id)
        .ok()
        .and_then(|u| (u.get_version() == Some(Version::Random)).then_some(u))
        .is_none()
    {
        return (StatusCode::NOT_FOUND, "profile not found").into_response();
    }

    match store.delete_profile(&profile_id).await {
        Ok(true) => Json(OkResponse { ok: true }).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "profile not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileResponse {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    tenant_id: String,
    enabled: bool,
    allow_partial_upstreams: bool,
    upstreams: Vec<String>,
    sources: Vec<String>,
    transforms: TransformPipeline,
    tools: Vec<String>,
    data_plane_auth: DataPlaneAuthSettings,
    data_plane_limits: DataPlaneLimitsSettings,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_timeout_secs: Option<u64>,
    tool_policies: Vec<ToolPolicy>,
    mcp: McpProfileSettings,
}

fn tenant_to_response(t: AdminTenant) -> TenantResponse {
    TenantResponse {
        id: t.id,
        enabled: t.enabled,
    }
}

fn upstream_to_response(u: AdminUpstream) -> UpstreamResponse {
    UpstreamResponse {
        id: u.id,
        enabled: u.enabled,
        endpoints: u
            .endpoints
            .into_iter()
            .map(|e| UpstreamEndpointResponse {
                id: e.id,
                url: e.url,
                enabled: e.enabled,
                auth: e.auth,
            })
            .collect(),
    }
}

fn profile_to_admin_response(profile: AdminProfile) -> ProfileResponse {
    ProfileResponse {
        id: profile.id,
        name: profile.name,
        description: profile.description,
        tenant_id: profile.tenant_id,
        enabled: profile.enabled,
        allow_partial_upstreams: profile.allow_partial_upstreams,
        upstreams: profile.upstream_ids,
        sources: profile.source_ids,
        transforms: profile.transforms,
        tools: profile.enabled_tools,
        data_plane_auth: DataPlaneAuthSettings {
            mode: profile.data_plane_auth_mode,
            accept_x_api_key: profile.accept_x_api_key,
        },
        data_plane_limits: DataPlaneLimitsSettings {
            rate_limit_enabled: profile.rate_limit_enabled,
            rate_limit_tool_calls_per_minute: profile.rate_limit_tool_calls_per_minute,
            quota_enabled: profile.quota_enabled,
            quota_tool_calls: profile.quota_tool_calls,
        },
        tool_call_timeout_secs: profile.tool_call_timeout_secs,
        tool_policies: profile.tool_policies,
        mcp: profile.mcp,
    }
}

async fn issue_tenant_token(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Json(req): Json<IssueTenantTokenRequest>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.get_tenant(&req.tenant_id).await {
        Ok(Some(t)) if t.enabled => {}
        Ok(Some(_)) => return (StatusCode::BAD_REQUEST, "tenant is disabled").into_response(),
        Ok(None) => return (StatusCode::NOT_FOUND, "tenant not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }

    let ttl = req.ttl_seconds.unwrap_or(31_536_000);
    let now = match now_unix_secs() {
        Ok(n) => n,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let exp = now.saturating_add(ttl).max(now + 1);

    let payload = TenantTokenPayloadV1 {
        tenant_id: req.tenant_id.clone(),
        exp_unix_secs: exp,
    };
    let token = match state.tenant_signer.sign_v1(&payload) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    Json(IssueTenantTokenResponse {
        ok: true,
        tenant_id: req.tenant_id,
        token,
        exp_unix_secs: exp,
    })
    .into_response()
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum PutToolSourceBody {
    Http {
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(flatten)]
        config: HttpServerConfig,
    },
    Openapi {
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(flatten)]
        config: ApiServerConfig,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolSourceResponse {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolSourcesResponse {
    sources: Vec<ToolSourceResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PutSecretBody {
    value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SecretsResponse {
    secrets: Vec<TenantSecretMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PutOidcPrincipalRequest {
    subject: String,
    /// If set, the principal is scoped to this profile. If omitted, principal is tenant-wide.
    #[serde(default)]
    profile_id: Option<String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteOidcPrincipalQuery {
    #[serde(default)]
    profile_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OidcPrincipalsResponse {
    principals: Vec<OidcPrincipalBinding>,
}

fn is_valid_source_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains(':')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn tool_source_kind_str(k: ToolSourceKind) -> &'static str {
    match k {
        ToolSourceKind::Http => "http",
        ToolSourceKind::Openapi => "openapi",
    }
}

async fn list_tool_sources(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.list_tool_sources(&tenant_id).await {
        Ok(list) => {
            let sources = list
                .into_iter()
                .map(|s| ToolSourceResponse {
                    id: s.id,
                    tool_type: tool_source_kind_str(s.kind).to_string(),
                    enabled: s.enabled,
                })
                .collect();
            Json(ToolSourcesResponse { sources }).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_tool_source(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path((tenant_id, source_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.get_tool_source(&tenant_id, &source_id).await {
        Ok(Some(s)) => Json(ToolSourceResponse {
            id: s.id,
            tool_type: tool_source_kind_str(s.kind).to_string(),
            enabled: s.enabled,
        })
        .into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "tool source not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn put_tool_source(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path((tenant_id, source_id)): Path<(String, String)>,
    Json(body): Json<PutToolSourceBody>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    if !is_valid_source_id(&source_id) {
        return (
            StatusCode::BAD_REQUEST,
            "invalid source id (allowed: [a-zA-Z0-9_-], must not contain ':')",
        )
            .into_response();
    }
    if state.shared_source_ids.contains(&source_id) {
        return (
            StatusCode::BAD_REQUEST,
            "source id collides with a shared catalog source id",
        )
            .into_response();
    }
    if store
        .get_upstream(&source_id)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        return (
            StatusCode::BAD_REQUEST,
            "source id collides with an upstream id",
        )
            .into_response();
    }

    // Ensure tenant exists.
    match store.get_tenant(&tenant_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, "tenant not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }

    let (enabled, kind, spec) = match body {
        PutToolSourceBody::Http { enabled, config } => (
            enabled,
            ToolSourceKind::Http,
            serde_json::to_value(&config).map_err(|e| e.to_string()),
        ),
        PutToolSourceBody::Openapi { enabled, config } => (
            enabled,
            ToolSourceKind::Openapi,
            serde_json::to_value(&config).map_err(|e| e.to_string()),
        ),
    };

    let spec = match spec {
        Ok(v) => v,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    if let Err(e) = store
        .put_tool_source(&tenant_id, &source_id, enabled, kind, spec)
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    Json(OkResponse { ok: true }).into_response()
}

async fn delete_tool_source(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path((tenant_id, source_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.delete_tool_source(&tenant_id, &source_id).await {
        Ok(true) => Json(OkResponse { ok: true }).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "tool source not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn list_secrets(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.list_secrets(&tenant_id).await {
        Ok(secrets) => Json(SecretsResponse { secrets }).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn put_secret(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path((tenant_id, name)): Path<(String, String)>,
    Json(req): Json<PutSecretBody>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    if name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "secret name is required").into_response();
    }
    if req.value.is_empty() {
        return (StatusCode::BAD_REQUEST, "secret value is required").into_response();
    }

    if let Err(e) = store.put_secret(&tenant_id, &name, &req.value).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    Json(OkResponse { ok: true }).into_response()
}

async fn delete_secret(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path((tenant_id, name)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };

    match store.delete_secret(&tenant_id, &name).await {
        Ok(true) => Json(OkResponse { ok: true }).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "secret not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn is_valid_oidc_subject(subject: &str) -> bool {
    // For simplicity and to avoid path confusion, disallow '/'.
    // Cognito/Entra commonly use UUID-like subjects, so this is fine for the current scope.
    let s = subject.trim();
    !s.is_empty() && !s.contains('/')
}

async fn list_oidc_principals(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };
    let Some(issuer) = state.oidc_issuer.as_deref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "OIDC not configured (set UNRELATED_GATEWAY_OIDC_ISSUER)",
        )
            .into_response();
    };

    // Ensure tenant exists.
    match store.get_tenant(&tenant_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, "tenant not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }

    match store.list_oidc_principals(&tenant_id, issuer).await {
        Ok(principals) => Json(OidcPrincipalsResponse { principals }).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn put_oidc_principal(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
    Json(req): Json<PutOidcPrincipalRequest>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };
    let Some(issuer) = state.oidc_issuer.as_deref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "OIDC not configured (set UNRELATED_GATEWAY_OIDC_ISSUER)",
        )
            .into_response();
    };

    let subject = req.subject.trim().to_string();
    if !is_valid_oidc_subject(&subject) {
        return (StatusCode::BAD_REQUEST, "invalid OIDC subject").into_response();
    }

    // Ensure tenant exists.
    match store.get_tenant(&tenant_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, "tenant not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }

    if let Some(profile_id) = req.profile_id.as_deref() {
        // Validate UUID and cross-tenant correctness.
        if Uuid::parse_str(profile_id)
            .ok()
            .and_then(|u| (u.get_version() == Some(Version::Random)).then_some(u))
            .is_none()
        {
            return (StatusCode::NOT_FOUND, "profile not found").into_response();
        }
        match store.get_profile(profile_id).await {
            Ok(Some(p)) if p.tenant_id == tenant_id => {}
            Ok(_) => return (StatusCode::NOT_FOUND, "profile not found").into_response(),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }

    if let Err(e) = store
        .put_oidc_principal(
            &tenant_id,
            issuer,
            &subject,
            req.profile_id.as_deref(),
            req.enabled,
        )
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    Json(OkResponse { ok: true }).into_response()
}

async fn delete_oidc_principal(
    Extension(state): Extension<Arc<AdminState>>,
    headers: HeaderMap,
    Path((tenant_id, subject)): Path<(String, String)>,
    Query(q): Query<DeleteOidcPrincipalQuery>,
) -> impl IntoResponse {
    if let Err(resp) = authz(&headers, state.admin_token.as_deref()) {
        return resp.into_response();
    }
    let Some(store) = &state.store else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Admin store unavailable").into_response();
    };
    let Some(issuer) = state.oidc_issuer.as_deref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "OIDC not configured (set UNRELATED_GATEWAY_OIDC_ISSUER)",
        )
            .into_response();
    };

    let subject = subject.trim().to_string();
    if !is_valid_oidc_subject(&subject) {
        return (StatusCode::BAD_REQUEST, "invalid OIDC subject").into_response();
    }

    // Ensure tenant exists.
    match store.get_tenant(&tenant_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, "tenant not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }

    match store
        .delete_oidc_principal(&tenant_id, issuer, &subject, q.profile_id.as_deref())
        .await
    {
        Ok(0) => (StatusCode::NOT_FOUND, "oidc principal not found").into_response(),
        Ok(_) => Json(OkResponse { ok: true }).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
