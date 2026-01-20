use base64::Engine as _;
use rmcp::model::RequestId;
use sha2::Digest as _;

pub(super) const PROXIED_REQUEST_ID_PREFIX: &str = "unrelated.proxy";
pub(super) const PROXIED_REQUEST_ID_PREFIX_READABLE: &str = "unrelated.proxy.r";
pub(super) const RESOURCE_URN_PREFIX: &str = "urn:unrelated-mcp-gateway:resource:";

pub(super) fn make_proxied_request_id(
    ns: crate::store::RequestIdNamespacing,
    upstream_id: &str,
    original: &RequestId,
) -> RequestId {
    // Encode both parts so parsing is unambiguous even if upstream ids or original ids contain
    // arbitrary characters.
    let original_json = original.clone().into_json_value();
    let original_json = serde_json::to_vec(&original_json).unwrap_or_default();
    let original_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(original_json);

    match ns {
        crate::store::RequestIdNamespacing::Opaque => {
            let upstream_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(upstream_id);
            RequestId::String(
                format!("{PROXIED_REQUEST_ID_PREFIX}.{upstream_b64}.{original_b64}").into(),
            )
        }
        crate::store::RequestIdNamespacing::Readable => RequestId::String(
            format!("{PROXIED_REQUEST_ID_PREFIX_READABLE}.{upstream_id}.{original_b64}").into(),
        ),
    }
}

pub(super) fn parse_proxied_request_id(id: &RequestId) -> Option<(String, RequestId)> {
    let RequestId::String(s) = id else {
        return None;
    };
    let s = s.as_ref();

    // IMPORTANT: check readable first, since its prefix is a strict extension of the opaque prefix.
    // If we check opaque first, "unrelated.proxy.r.*" would incorrectly match the opaque branch.
    let (upstream_id, original_b64) =
        if let Some(rest) = s.strip_prefix(&format!("{PROXIED_REQUEST_ID_PREFIX_READABLE}.")) {
            // Readable: unrelated.proxy.r.<upstream_id>.<b64(original)>
            let (upstream_id, original_b64) = rest.rsplit_once('.')?;
            (upstream_id.to_string(), original_b64)
        } else if let Some(rest) = s.strip_prefix(&format!("{PROXIED_REQUEST_ID_PREFIX}.")) {
            // Opaque: unrelated.proxy.<b64(upstream)>.<b64(original)>
            let (upstream_b64, original_b64) = rest.split_once('.')?;
            let upstream_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(upstream_b64.as_bytes())
                .ok()?;
            let upstream_id = String::from_utf8(upstream_bytes).ok()?;
            (upstream_id, original_b64)
        } else {
            return None;
        };

    let original_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(original_b64.as_bytes())
        .ok()?;
    let original_json: serde_json::Value = serde_json::from_slice(&original_bytes).ok()?;
    let original: RequestId = serde_json::from_value(original_json).ok()?;
    Some((upstream_id, original))
}

pub(super) fn parse_resource_collision_urn(uri: &str) -> Option<(&str, &str)> {
    uri.strip_prefix(RESOURCE_URN_PREFIX)
        .and_then(|rest| rest.split_once(':'))
}

pub(super) fn resource_collision_urn(upstream_id: &str, original_uri: &str) -> String {
    let hash = hex::encode(sha2::Sha256::digest(original_uri.as_bytes()));
    format!("{RESOURCE_URN_PREFIX}{upstream_id}:{hash}")
}
