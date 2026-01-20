use crate::store::Profile;
use parking_lot::RwLock;
use rmcp::model::Tool;
use serde_json::json;
use sha2::Digest as _;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRouteKind {
    Upstream,
    SharedLocal,
    TenantLocal,
}

#[derive(Debug, Clone)]
pub struct ToolRoute {
    pub kind: ToolRouteKind,
    pub source_id: String,
    pub original_name: String,
}

#[derive(Debug, Clone)]
pub struct CachedToolsSurface {
    pub tools: Arc<Vec<Tool>>,
    pub routes: Arc<HashMap<String, ToolRoute>>,
    /// Tool names (post-transform) that were ambiguous and therefore require prefixing.
    pub ambiguous_names: Arc<HashSet<String>>,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    profile_id: String,
    profile_fingerprint: String,
    expires_at: Instant,
    surface: CachedToolsSurface,
}

#[derive(Clone)]
pub struct ToolSurfaceCache {
    ttl: Duration,
    inner: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl ToolSurfaceCache {
    #[must_use]
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn get(
        &self,
        session_token: &str,
        profile_fingerprint: &str,
    ) -> Option<CachedToolsSurface> {
        let now = Instant::now();
        let mut map = self.inner.write();
        let expires_at = map.get(session_token)?.expires_at;
        if expires_at <= now {
            map.remove(session_token);
            return None;
        }
        if map.get(session_token)?.profile_fingerprint != profile_fingerprint {
            // Profile changed: invalidate.
            map.remove(session_token);
            return None;
        }
        Some(map.get(session_token)?.surface.clone())
    }

    pub fn put(
        &self,
        profile_id: &str,
        session_token: String,
        profile_fingerprint: String,
        surface: CachedToolsSurface,
    ) {
        let expires_at = Instant::now() + self.ttl;
        self.inner.write().insert(
            session_token,
            CacheEntry {
                profile_id: profile_id.to_string(),
                profile_fingerprint,
                expires_at,
                surface,
            },
        );
    }

    pub fn invalidate(&self, session_token: &str) {
        self.inner.write().remove(session_token);
    }

    /// Best-effort cache invalidation for HA deployments.
    ///
    /// Removes all cached entries for sessions belonging to a given profile.
    pub fn invalidate_profile(&self, profile_id: &str) {
        let mut map = self.inner.write();
        map.retain(|_, v| v.profile_id != profile_id);
    }
}

#[must_use]
pub fn profile_fingerprint(profile: &Profile) -> String {
    // We only include fields that influence the exposed tool surface and routing behavior.
    let v = json!({
        "profileId": profile.id,
        "tenantId": profile.tenant_id,
        "allowPartialUpstreams": profile.allow_partial_upstreams,
        "sourceIds": profile.source_ids,
        "enabledTools": profile.enabled_tools,
        "transforms": profile.transforms,
    });
    let s = serde_json::to_string(&v).expect("profile fingerprint json serializes");
    hex::encode(sha2::Sha256::digest(s.as_bytes()))
}
