//! Shared helpers for the concrete adapters.
//!
//! Per `LLM-secretsViaEnv` (Phase 13): API keys live directly in
//! the System config; adapters read them from their own
//! `api_key` field, no env-var indirection. This file no longer
//! exposes a key-reading helper — the helpers below are
//! transport-layer only.

use reqwest::Error as ReqwestError;
use reqwest::StatusCode;

use crate::llm::provider::AdapterError;

/// Map a non-2xx HTTP status onto an [`AdapterError`], surfacing the provider's own error `body`.
/// Shared by every adapter so the three never diverge on how a rejected request reads. The status
/// families are the same across Anthropic/OpenAI/Gemini; the body — a `{"error":{"message":..}}`
/// shape for all three — carries the actionable message a bare status hides (e.g. an invalid model
/// id on a 400).
pub fn status_to_error(
    family: &'static str,
    status: StatusCode,
    model: &str,
    body: &str,
) -> AdapterError {
    match status.as_u16() {
        401 | 403 => AdapterError::Auth {
            family,
            detail: with_body(&format!("HTTP {}", status.as_u16()), body),
        },
        404 => AdapterError::ModelNotFound {
            family,
            model: model.to_owned(),
        },
        429 => AdapterError::RateLimited { family },
        500..=599 => AdapterError::ServerError {
            family,
            status: status.as_u16(),
        },
        // A 4xx (or any other non-2xx): the request was rejected. Surface the provider's error body.
        _ => AdapterError::Rejected {
            family,
            status: status.as_u16(),
            detail: body_message(body),
        },
    }
}

/// Pull the operator-facing `error.message` out of a provider error body (all three families use
/// `{"error":{"message":..}}`), falling back to the raw body. Capped so a large body can't flood
/// the surface it renders on.
fn body_message(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "no error body".to_string();
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed)
        && let Some(msg) = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
    {
        return truncate(msg);
    }
    truncate(trimmed)
}

/// Append a provider error body to an existing detail, when there is one.
fn with_body(prefix: &str, body: &str) -> String {
    if body.trim().is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}: {}", body_message(body))
    }
}

/// Cap a message by chars (so a multibyte body can't panic the slice) so it can't flood the UI.
fn truncate(s: &str) -> String {
    const MAX: usize = 400;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        let capped: String = s.chars().take(MAX).collect();
        format!("{capped}…")
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::provider::AdapterErrorKind;
    use reqwest::StatusCode;

    // A 400 must surface the provider's own error message (e.g. an invalid model id), not a
    // generic "unexpected HTTP" — that message is the whole point of reading the body.
    #[test]
    fn rejects_a_400_with_the_providers_error_message() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"model: claude-bogus is not a valid model"}}"#;
        let err = status_to_error("anthropic", StatusCode::BAD_REQUEST, "claude-bogus", body);
        assert!(matches!(err, AdapterError::Rejected { status: 400, .. }));
        assert!(
            err.to_string()
                .contains("claude-bogus is not a valid model"),
            "surfaces the body message: {err}"
        );
        // A 4xx bad request is permanent — retrying won't fix a bad model id.
        assert_eq!(err.kind(), AdapterErrorKind::Permanent);
    }

    #[test]
    fn empty_body_still_names_the_status() {
        let err = status_to_error("openai", StatusCode::from_u16(418).unwrap(), "m", "");
        assert!(err.to_string().contains("418"), "{err}");
    }

    #[test]
    fn a_401_body_is_appended_to_the_auth_detail() {
        let body = r#"{"error":{"message":"invalid x-api-key"}}"#;
        let err = status_to_error("openai", StatusCode::UNAUTHORIZED, "m", body);
        assert!(matches!(err, AdapterError::Auth { .. }));
        assert!(err.to_string().contains("invalid x-api-key"), "{err}");
    }
}
