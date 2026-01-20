//! Outbound HTTP safety controls (SSRF protection, limits, redaction).
//!
//! This module is intentionally policy-only. Consumers choose a policy:
//! - Adapter (standalone): typically permissive
//! - Gateway (multi-tenant): typically restrictive

use crate::runtime::HttpToolsError;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::net::lookup_host;
use url::Url;

#[derive(Debug, Clone)]
pub enum RedirectPolicy {
    /// Do not follow redirects.
    None,
    /// Follow redirects, but re-check the destination URL on each hop.
    Checked,
}

#[derive(Debug, Clone)]
pub struct OutboundHttpSafety {
    /// If set, only these hosts are allowed (case-insensitive).
    pub allowed_hosts: Option<HashSet<String>>,
    /// If true, allow private/loopback/link-local/reserved destination IPs.
    pub allow_private_networks: bool,
    /// Maximum response body size (bytes). `None` = unlimited.
    pub max_response_bytes: Option<usize>,
    /// Redirect behavior.
    pub redirects: RedirectPolicy,
}

impl OutboundHttpSafety {
    /// Most permissive policy (intended for the Adapter).
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            allowed_hosts: None,
            allow_private_networks: true,
            max_response_bytes: None,
            redirects: RedirectPolicy::Checked,
        }
    }

    /// Safer default policy for multi-tenant environments (intended for the Gateway).
    #[must_use]
    pub fn gateway_default() -> Self {
        Self {
            allowed_hosts: None,
            allow_private_networks: false,
            max_response_bytes: Some(1024 * 1024), // 1 MiB
            redirects: RedirectPolicy::None,
        }
    }

    /// Validate a URL before making an outbound request.
    ///
    /// This rejects non-`http(s)` schemes and applies host/IP restrictions.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is disallowed by the policy (unsupported scheme, host not in
    /// allowlist, or hostname resolves to a disallowed IP range).
    pub async fn check_url(&self, url: &Url) -> Result<(), HttpToolsError> {
        let scheme = url.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(HttpToolsError::Http(format!(
                "Outbound HTTP blocked: unsupported URL scheme '{scheme}'"
            )));
        }

        let Some(host) = url.host_str() else {
            return Err(HttpToolsError::Http(
                "Outbound HTTP blocked: missing URL host".to_string(),
            ));
        };

        if let Some(allowed) = &self.allowed_hosts
            && !allowed.contains(&host.to_ascii_lowercase())
        {
            return Err(HttpToolsError::Http(format!(
                "Outbound HTTP blocked: host '{host}' not in allowlist"
            )));
        }

        if self.allow_private_networks {
            return Ok(());
        }

        // IP literal?
        if let Ok(ip) = host.parse::<IpAddr>() {
            return if is_denied_ip(ip) {
                Err(HttpToolsError::Http(format!(
                    "Outbound HTTP blocked: destination IP '{ip}' is not allowed"
                )))
            } else {
                Ok(())
            };
        }

        // Resolve hostname and validate every resolved address.
        let port = url.port_or_known_default().unwrap_or(443);
        let addrs = lookup_host((host, port)).await.map_err(|e| {
            HttpToolsError::Http(format!("DNS lookup failed for host '{host}': {e}"))
        })?;

        let mut saw_any = false;
        for addr in addrs {
            saw_any = true;
            if is_denied_ip(addr.ip()) {
                return Err(HttpToolsError::Http(format!(
                    "Outbound HTTP blocked: host '{host}' resolved to disallowed IP '{}'",
                    addr.ip()
                )));
            }
        }

        if !saw_any {
            return Err(HttpToolsError::Http(format!(
                "DNS lookup returned no addresses for host '{host}'"
            )));
        }

        Ok(())
    }
}

#[must_use]
pub fn redact_url(url: &Url) -> String {
    let mut u = url.clone();
    // Best-effort: drop credentials + query + fragment.
    let _ = u.set_username("");
    let _ = u.set_password(None);
    u.set_query(None);
    u.set_fragment(None);
    u.to_string()
}

#[must_use]
pub fn sanitize_reqwest_error(e: &reqwest::Error) -> String {
    let mut msg = e.to_string();
    if let Some(u) = e.url() {
        msg = msg.replace(u.as_str(), &redact_url(u));
    }
    msg
}

fn is_denied_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_denied_ipv4(v4),
        IpAddr::V6(v6) => is_denied_ipv6(v6),
    }
}

fn is_denied_ipv4(ip: Ipv4Addr) -> bool {
    // Disallow:
    // - loopback
    // - private
    // - link-local (incl. metadata IPs like 169.254.169.254)
    // - unspecified/broadcast
    // - multicast
    // - CGNAT (100.64.0.0/10)
    // - reserved (240.0.0.0/4)
    if ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
    {
        return true;
    }

    // Carrier-grade NAT range.
    let oct = ip.octets();
    if oct[0] == 100 && (64..=127).contains(&oct[1]) {
        return true;
    }

    // Reserved / future use.
    if oct[0] >= 240 {
        return true;
    }

    false
}

fn is_denied_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn restrictive_policy_blocks_loopback() {
        let safety = OutboundHttpSafety::gateway_default();
        let url = Url::parse("http://127.0.0.1:1234/").expect("url");
        let err = safety.check_url(&url).await.unwrap_err();
        assert!(err.to_string().contains("blocked"));
    }

    #[tokio::test]
    async fn permissive_policy_allows_loopback() {
        let safety = OutboundHttpSafety::permissive();
        let url = Url::parse("http://127.0.0.1:1234/").expect("url");
        safety.check_url(&url).await.expect("allowed");
    }
}
