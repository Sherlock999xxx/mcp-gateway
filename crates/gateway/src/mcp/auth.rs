use super::McpState;
use crate::session_token::{TokenAuthV1, TokenOidcV1};
use crate::store::DataPlaneAuthMode;
use axum::{http::HeaderMap, http::StatusCode, response::IntoResponse as _, response::Response};

fn extract_api_key_secret(headers: &HeaderMap, accept_x_api_key: bool) -> Option<String> {
    if accept_x_api_key && let Some(v) = headers.get("x-api-key").and_then(|h| h.to_str().ok()) {
        let v = v.trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }

    let authz = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())?;
    let token = authz.strip_prefix("Bearer ").map(str::trim)?;
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

fn extract_bearer_jwt(headers: &HeaderMap) -> Option<String> {
    let authz = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())?;
    let token = authz.strip_prefix("Bearer ").map(str::trim)?;
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

pub(super) fn unauthorized(msg: &'static str) -> Response {
    (StatusCode::UNAUTHORIZED, msg).into_response()
}

pub(super) async fn authorize_jwt_request(
    state: &McpState,
    profile: &crate::store::Profile,
    headers: &HeaderMap,
) -> Result<TokenOidcV1, Response> {
    let Some(jwt) = extract_bearer_jwt(headers) else {
        return Err(unauthorized("Unauthorized: bearer token is required"));
    };
    let Some(oidc) = state.oidc.as_ref() else {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "OIDC is not configured (missing UNRELATED_GATEWAY_OIDC_ISSUER)",
        )
            .into_response());
    };

    let claims = match oidc.validate(&jwt).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "oidc jwt validation failed");
            return Err(unauthorized("Unauthorized: invalid bearer token"));
        }
    };

    // We intentionally avoid claim-based RBAC. We only use an identifier for lookup.
    // Prefer `sub` (OIDC) and fall back to `oid` (Entra ID).
    let subject = claims
        .get("sub")
        .and_then(serde_json::Value::as_str)
        .or_else(|| claims.get("oid").and_then(serde_json::Value::as_str))
        .ok_or_else(|| unauthorized("Unauthorized: bearer token missing subject"))?;

    let allowed = state
        .store
        .is_oidc_principal_allowed(&profile.tenant_id, &profile.id, oidc.issuer(), subject)
        .await
        .map_err(super::internal_error_response("check oidc principal"))?;

    if !allowed {
        return Err(unauthorized("Unauthorized"));
    }

    Ok(TokenOidcV1 {
        issuer: oidc.issuer().to_string(),
        subject: subject.to_string(),
    })
}

async fn enforce_jwt_every_request_in_session(
    state: &McpState,
    profile: &crate::store::Profile,
    headers: &HeaderMap,
    session_oidc: Option<&TokenOidcV1>,
) -> Result<(), Response> {
    let principal = authorize_jwt_request(state, profile, headers).await?;
    let session = session_oidc.ok_or_else(|| {
        unauthorized("Unauthorized: missing OIDC binding in session; re-initialize required")
    })?;
    if session.issuer != principal.issuer || session.subject != principal.subject {
        return Err(unauthorized(
            "Unauthorized: session token principal does not match bearer token",
        ));
    }
    Ok(())
}

pub(super) async fn authenticate_api_key_on_initialize(
    state: &McpState,
    profile: &crate::store::Profile,
    headers: &HeaderMap,
) -> Result<TokenAuthV1, Response> {
    let Some(secret) = extract_api_key_secret(headers, profile.accept_x_api_key) else {
        return Err(unauthorized(
            "Unauthorized: API key is required for initialize",
        ));
    };

    let api_key = state
        .store
        .authenticate_api_key(&profile.tenant_id, &profile.id, &secret)
        .await
        .map_err(super::internal_error_response("authenticate api key"))?
        .ok_or_else(|| unauthorized("Unauthorized: invalid API key"))?;

    // Best-effort metering on initialize (request count + last_used_at).
    state
        .store
        .touch_api_key(&api_key.tenant_id, &api_key.api_key_id)
        .await
        .map_err(super::internal_error_response("touch api key"))?;

    Ok(TokenAuthV1 {
        tenant_id: api_key.tenant_id,
        api_key_id: api_key.api_key_id,
    })
}

pub(super) async fn enforce_data_plane_auth(
    state: &McpState,
    profile: &crate::store::Profile,
    headers: &HeaderMap,
    session_auth: Option<&TokenAuthV1>,
    session_oidc: Option<&TokenOidcV1>,
) -> Result<(), Response> {
    match profile.data_plane_auth_mode {
        DataPlaneAuthMode::Disabled => Ok(()),
        DataPlaneAuthMode::ApiKeyInitializeOnly => {
            enforce_api_key_initialize_only(state, profile, session_auth).await
        }
        DataPlaneAuthMode::ApiKeyEveryRequest => {
            enforce_api_key_every_request(state, profile, headers, session_auth).await
        }
        DataPlaneAuthMode::JwtEveryRequest => {
            enforce_jwt_every_request_in_session(state, profile, headers, session_oidc).await
        }
    }
}

async fn enforce_api_key_initialize_only(
    state: &McpState,
    profile: &crate::store::Profile,
    session_auth: Option<&TokenAuthV1>,
) -> Result<(), Response> {
    let auth = session_auth.ok_or_else(|| {
        unauthorized("Unauthorized: missing API key in session; re-initialize required")
    })?;

    if auth.tenant_id != profile.tenant_id {
        return Err(unauthorized("Unauthorized"));
    }

    let active = state
        .store
        .is_api_key_active(&auth.tenant_id, &auth.api_key_id)
        .await
        .map_err(super::internal_error_response("check api key active"))?;

    if !active {
        return Err(unauthorized("Unauthorized: API key revoked"));
    }

    state
        .store
        .touch_api_key(&auth.tenant_id, &auth.api_key_id)
        .await
        .map_err(super::internal_error_response("touch api key"))?;

    Ok(())
}

async fn enforce_api_key_every_request(
    state: &McpState,
    profile: &crate::store::Profile,
    headers: &HeaderMap,
    session_auth: Option<&TokenAuthV1>,
) -> Result<(), Response> {
    let auth = session_auth.ok_or_else(|| {
        unauthorized("Unauthorized: missing API key in session; re-initialize required")
    })?;

    if auth.tenant_id != profile.tenant_id {
        return Err(unauthorized("Unauthorized"));
    }

    let Some(secret) = extract_api_key_secret(headers, profile.accept_x_api_key) else {
        return Err(unauthorized("Unauthorized: API key header is required"));
    };

    let api_key = state
        .store
        .authenticate_api_key(&profile.tenant_id, &profile.id, &secret)
        .await
        .map_err(super::internal_error_response("authenticate api key"))?
        .ok_or_else(|| unauthorized("Unauthorized: invalid API key"))?;

    if api_key.api_key_id != auth.api_key_id {
        return Err(unauthorized("Unauthorized"));
    }

    state
        .store
        .touch_api_key(&api_key.tenant_id, &api_key.api_key_id)
        .await
        .map_err(super::internal_error_response("touch api key"))?;

    Ok(())
}
