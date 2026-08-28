//! One-time-per-process privacy acknowledgement tracker.
//!
//! Per `LLM-privacyWarning`: before routing a prompt to a
//! non-local provider, operators must acknowledge that
//! artifact content will be sent outside the host. Local
//! endpoints (localhost, RFC 1918 ranges, IPv6 loopback)
//! bypass the warning since no data leaves the host.
//!
//! The acknowledgement is held as a [`HashSet`] of provider
//! indices — per-provider, in-memory, cleared on restart.
//! This is intentional: operators re-confirm every time the
//! server comes up, which gives them a chance to rethink the
//! config after a break.

use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::sync::Mutex;

use url::{Host, Url};

pub struct PrivacyTracker {
    acknowledged: Mutex<HashSet<usize>>,
}

impl PrivacyTracker {
    pub fn new() -> Self {
        Self {
            acknowledged: Mutex::new(HashSet::new()),
        }
    }

    /// Whether the operator has acknowledged that prompts
    /// to this provider may leave the host.
    pub fn is_acknowledged(&self, index: usize) -> bool {
        self.acknowledged
            .lock()
            .expect("privacy tracker poisoned")
            .contains(&index)
    }

    /// Record that the operator has acknowledged the warning
    /// for this provider slot. Idempotent.
    pub fn acknowledge(&self, index: usize) {
        self.acknowledged
            .lock()
            .expect("privacy tracker poisoned")
            .insert(index);
    }

    /// For the debug-prompt endpoint and for Phase 10b
    /// consumers — decides whether to require ack before
    /// calling `send_prompt`. A local endpoint is always
    /// fine; everything else requires prior ack.
    pub fn requires_ack(&self, index: usize, endpoint: &str) -> bool {
        if is_local_endpoint(endpoint) {
            return false;
        }
        !self.is_acknowledged(index)
    }
}

impl Default for PrivacyTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether an endpoint URL points at a host that stays on
/// the operator's machine/LAN. Matches:
///
/// - `localhost` (case-insensitive)
/// - IPv4 loopback (`127.0.0.0/8`)
/// - IPv6 loopback (`::1`)
/// - RFC 1918 private ranges (`10.0.0.0/8`, `172.16.0.0/12`,
///   `192.168.0.0/16`)
///
/// Anything we can't parse is treated as NOT local — when
/// in doubt, require the privacy warning. That's the safe
/// direction: a false positive on "might leak" is annoying;
/// a false negative is a data-exfiltration.
pub fn is_local_endpoint(endpoint: &str) -> bool {
    let Ok(url) = Url::parse(endpoint) else {
        return false;
    };
    let Some(host) = url.host() else {
        return false;
    };
    match host {
        Host::Domain(d) => d.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(v4) => is_private_ipv4(v4),
        Host::Ipv6(v6) => v6.is_loopback(),
    }
}

fn is_private_ipv4(v4: Ipv4Addr) -> bool {
    if v4.is_loopback() {
        return true;
    }
    let o = v4.octets();
    if o[0] == 10 {
        return true;
    }
    if o[0] == 172 && (16..=31).contains(&o[1]) {
        return true;
    }
    if o[0] == 192 && o[1] == 168 {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localhost_variants_are_local() {
        assert!(is_local_endpoint("http://localhost:1234/v1"));
        assert!(is_local_endpoint("https://LOCALHOST"));
        assert!(is_local_endpoint("http://127.0.0.1:11434"));
        assert!(is_local_endpoint("http://127.5.6.7"));
    }

    #[test]
    fn rfc1918_ranges_are_local() {
        assert!(is_local_endpoint("http://10.0.0.1"));
        assert!(is_local_endpoint("http://10.255.255.255"));
        assert!(is_local_endpoint("http://172.16.0.1"));
        assert!(is_local_endpoint("http://172.31.0.1"));
        assert!(is_local_endpoint("http://192.168.1.1"));
    }

    #[test]
    fn ipv6_loopback_is_local() {
        assert!(is_local_endpoint("http://[::1]:8080"));
    }

    #[test]
    fn public_internet_is_not_local() {
        assert!(!is_local_endpoint("https://api.openai.com"));
        assert!(!is_local_endpoint("https://api.anthropic.com"));
        assert!(!is_local_endpoint(
            "https://generativelanguage.googleapis.com"
        ));
        assert!(!is_local_endpoint("http://8.8.8.8"));
    }

    #[test]
    fn near_rfc1918_but_outside_ranges_is_not_local() {
        // 172.15.x.x and 172.32.x.x are public, even though
        // 172.16–31 are private.
        assert!(!is_local_endpoint("http://172.15.0.1"));
        assert!(!is_local_endpoint("http://172.32.0.1"));
        // 11.x and 192.167/192.169 are public.
        assert!(!is_local_endpoint("http://11.0.0.1"));
        assert!(!is_local_endpoint("http://192.167.1.1"));
        assert!(!is_local_endpoint("http://192.169.1.1"));
    }

    #[test]
    fn garbage_endpoints_are_not_local() {
        assert!(!is_local_endpoint(""));
        assert!(!is_local_endpoint("not a url"));
        assert!(!is_local_endpoint("file:///etc/passwd"));
    }

    #[test]
    fn acknowledge_is_per_slot_and_idempotent() {
        let t = PrivacyTracker::new();
        assert!(!t.is_acknowledged(0));
        t.acknowledge(0);
        t.acknowledge(0);
        assert!(t.is_acknowledged(0));
        assert!(!t.is_acknowledged(1));
    }

    #[test]
    fn requires_ack_skips_local_endpoints() {
        let t = PrivacyTracker::new();
        assert!(!t.requires_ack(0, "http://localhost:11434"));
        assert!(!t.requires_ack(0, "http://192.168.1.1"));
    }

    #[test]
    fn requires_ack_gates_remote_endpoints_until_acknowledged() {
        let t = PrivacyTracker::new();
        assert!(t.requires_ack(0, "https://api.openai.com"));
        t.acknowledge(0);
        assert!(!t.requires_ack(0, "https://api.openai.com"));
    }
}
