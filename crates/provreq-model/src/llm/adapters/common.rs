//! Shared helpers for the concrete adapters.
//!
//! Per `LLM-secretsViaEnv` (Phase 13): API keys live directly in
//! the System config; adapters read them from their own
//! `api_key` field, no env-var indirection. This file no longer
//! exposes a key-reading helper — the helpers below are
//! transport-layer only.

use reqwest::Error as ReqwestError;

use crate::llm::provider::AdapterError;

/// Map a `reqwest::Error` onto the appropriate `AdapterError`
/// variant. Called by every adapter on transport-layer
/// failures (before any HTTP status is known). The detail
/// string is reqwest's full chained error message so the
/// operator sees the underlying cause (URL parse failure, DNS
/// miss, TLS handshake error, etc.) rather than just an
/// opaque "builder error".
pub fn classify_reqwest_error(
    family: &'static str,
    timeout_ms: u64,
    err: ReqwestError,
) -> AdapterError {
    if err.is_timeout() {
        return AdapterError::Timeout {
            family,
            ms: timeout_ms,
        };
    }
    let detail = format_error_chain(&err);
    if err.is_connect() {
        return AdapterError::Connection { family, detail };
    }
    if err.is_decode() {
        return AdapterError::Malformed { family, detail };
    }
    if err.is_builder() {
        // URL malformed, headers invalid, etc. — the operator
        // typed something the request builder couldn't accept.
        // Surface as Auth (the closest "operator config" arm)
        // with the chained message so they can see the cause.
        return AdapterError::Auth { family, detail };
    }
    // Default for request-send / redirect / body errors that
    // happen before we see an HTTP status — same category as
    // a dead socket.
    AdapterError::Connection { family, detail }
}

/// Walk a `reqwest::Error`'s source chain and stitch the
/// messages together with " — " separators. Reqwest's `Display`
/// impl alone usually only prints the topmost message, which
/// for builder errors collapses to "builder error" — useless
/// to an operator. This helper recovers the underlying cause
/// (e.g. "relative URL without a base").
fn format_error_chain(err: &ReqwestError) -> String {
    let mut parts = vec![err.to_string()];
    let mut source: Option<&dyn std::error::Error> = std::error::Error::source(err);
    while let Some(s) = source {
        parts.push(s.to_string());
        source = s.source();
    }
    parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" — ")
}

/// Ergonomic wrapper around the endpoint URL. Trims trailing
/// slashes once at construction so every `.join()` yields a
/// canonical URL even if the operator wrote `https://x/` vs
/// `https://x`.
#[derive(Clone, Debug)]
pub struct HttpEndpoint {
    base: String,
}

impl HttpEndpoint {
    pub fn new(mut s: String) -> Self {
        while s.ends_with('/') {
            s.pop();
        }
        Self { base: s }
    }

    pub fn as_str(&self) -> &str {
        &self.base
    }

    pub fn join(&self, path: &str) -> String {
        let mut out = String::with_capacity(self.base.len() + path.len());
        out.push_str(&self.base);
        if !path.starts_with('/') {
            out.push('/');
        }
        out.push_str(path);
        out
    }
}
