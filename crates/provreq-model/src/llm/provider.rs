//! Adapter trait + shared request/response/error shapes.
//!
//! Per `LLM-promptAbstraction`, adapters expose a single
//! generic `send_prompt` method. Feature code (Phase 10b's
//! rename, and whatever comes after) builds its own prompt
//! and hands the adapter a `PromptRequest`. This keeps the
//! adapter layer feature-agnostic — new LLM-backed features
//! don't grow new methods on the trait, they just build a
//! different prompt.

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

use super::config::ProviderFamily;

/// Shorthand for the boxed future returned by `send_prompt`.
/// Manual boxing (rather than `async fn` in the trait) keeps
/// `dyn Adapter` object-safe without pulling in `async-trait`
/// — the fallback chain in [`super::chain`] walks a
/// `&[Box<dyn Adapter>]` so dyn-safety matters.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Generic prompt input. A `system` prime + an ordered
/// conversation of `messages`. The conversation is always
/// user/assistant alternation starting with a user turn —
/// adapters that want a different shape (e.g. Gemini's
/// `contents` array) translate at the boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRequest {
    /// Optional system prompt. Anthropic accepts this as a
    /// top-level field; OpenAI and Gemini inject it as the
    /// first message.
    pub system: Option<String>,
    pub messages: Vec<PromptMessage>,
    /// Upper bound on output tokens. All three provider
    /// families support this; adapters clamp to their API's
    /// local ceiling if the caller overshoots.
    pub max_tokens: u32,
    /// 0.0 – 2.0 per OpenAI; Anthropic caps at 1.0 and
    /// Gemini at 2.0. Adapters clamp at their local ceiling.
    pub temperature: f32,
    /// Per-request timeout in milliseconds. `None` falls back
    /// to the adapter family's default (30 s — fine for short
    /// prompts like rename suggestion). Long-prompt features
    /// (Phase 12 link analysis on a project of 100+ artifacts
    /// with a local 32B model) need a much higher budget; they
    /// pass an explicit value here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: PromptRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptRole {
    User,
    Assistant,
}

/// Successful response from an adapter. `text` is the
/// concatenation of the model's output text parts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptResponse {
    pub text: String,
    /// Token usage when the provider reports it. Both
    /// OpenAI and Anthropic do; Gemini's numbers are
    /// best-effort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<PromptUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Failure modes surfaced by adapters. The split matters —
/// `HealthTracker::record_failure` reads the `kind()` to
/// decide whether to mark the provider transient-degraded
/// (retryable) or hard-disabled (won't retry this process).
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// Request exceeded the adapter's deadline. Transient.
    #[error("{family}: request timed out after {ms}ms")]
    Timeout { family: &'static str, ms: u64 },
    /// HTTP 429. Transient — the provider is asking us to
    /// slow down.
    #[error("{family}: rate-limited by provider (HTTP 429)")]
    RateLimited { family: &'static str },
    /// HTTP 5xx from the provider. Transient.
    #[error("{family}: provider returned HTTP {status}")]
    ServerError { family: &'static str, status: u16 },
    /// HTTP 401/403 or a missing API key env var. Permanent
    /// for this process — we will not retry without operator
    /// intervention (fix the env, restart).
    #[error("{family}: authentication failed ({detail})")]
    Auth {
        family: &'static str,
        detail: String,
    },
    /// HTTP 404 on a known endpoint, or an explicit
    /// "model not found" response. Permanent — the config
    /// names a model the provider doesn't serve.
    #[error("{family}: model '{model}' not found at provider")]
    ModelNotFound { family: &'static str, model: String },
    /// DNS, TCP, or TLS failure. Permanent for this process
    /// — if the endpoint is wrong or unreachable at process
    /// start, it will stay that way until restart.
    #[error("{family}: connection failed ({detail})")]
    Connection {
        family: &'static str,
        detail: String,
    },
    /// Provider returned an HTTP 200 but the body didn't
    /// decode or didn't contain the expected completion
    /// shape. Permanent — probably a version skew we can't
    /// paper over.
    #[error("{family}: provider response could not be parsed ({detail})")]
    Malformed {
        family: &'static str,
        detail: String,
    },
}

/// Broad classification of an adapter error, used by the
/// health tracker to decide transient-vs-permanent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterErrorKind {
    /// Provider is temporarily unhappy — back off and retry
    /// on a doubling schedule.
    Transient,
    /// Provider is permanently unusable for this process —
    /// disable it and don't retry until restart.
    Permanent,
}

impl AdapterError {
    pub fn kind(&self) -> AdapterErrorKind {
        match self {
            Self::Timeout { .. } | Self::RateLimited { .. } | Self::ServerError { .. } => {
                AdapterErrorKind::Transient
            }
            Self::Auth { .. }
            | Self::ModelNotFound { .. }
            | Self::Connection { .. }
            | Self::Malformed { .. } => AdapterErrorKind::Permanent,
        }
    }

    pub fn family(&self) -> &'static str {
        match self {
            Self::Timeout { family, .. }
            | Self::RateLimited { family }
            | Self::ServerError { family, .. }
            | Self::Auth { family, .. }
            | Self::ModelNotFound { family, .. }
            | Self::Connection { family, .. }
            | Self::Malformed { family, .. } => family,
        }
    }
}

/// The single surface every provider family implements.
///
/// `send_prompt` is intentionally the only mutating method
/// — health tracking and retest scheduling live outside the
/// adapter so they stay generic across families.
pub trait Adapter: Send + Sync {
    /// Which provider family this adapter implements. Used
    /// in error messages and the provider-list endpoint.
    fn family(&self) -> ProviderFamily;

    /// The configured model identifier (e.g. `gpt-4o-mini`,
    /// `claude-haiku-4-5`, `gemini-2.0-flash`).
    fn model(&self) -> &str;

    /// The effective endpoint URL the adapter talks to —
    /// either the operator-supplied `endpoint` or the
    /// family's published default.
    fn endpoint(&self) -> &str;

    /// Whether the adapter has the API key it needs to run.
    /// Surfaced directly in the provider-list endpoint as
    /// `apiKeyAvailable`. A `false` here is also an `Auth`
    /// failure at send time — it's cheaper to report it
    /// up front.
    fn api_key_available(&self) -> bool;

    fn send_prompt<'a>(
        &'a self,
        req: &'a PromptRequest,
    ) -> BoxFuture<'a, Result<PromptResponse, AdapterError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_errors_classified_transient() {
        assert_eq!(
            AdapterError::Timeout {
                family: "anthropic",
                ms: 30_000,
            }
            .kind(),
            AdapterErrorKind::Transient
        );
        assert_eq!(
            AdapterError::RateLimited {
                family: "openai-compatible"
            }
            .kind(),
            AdapterErrorKind::Transient
        );
        assert_eq!(
            AdapterError::ServerError {
                family: "gemini",
                status: 503,
            }
            .kind(),
            AdapterErrorKind::Transient
        );
    }

    #[test]
    fn permanent_errors_classified_permanent() {
        assert_eq!(
            AdapterError::Auth {
                family: "anthropic",
                detail: "401".into(),
            }
            .kind(),
            AdapterErrorKind::Permanent
        );
        assert_eq!(
            AdapterError::ModelNotFound {
                family: "gemini",
                model: "nope".into(),
            }
            .kind(),
            AdapterErrorKind::Permanent
        );
        assert_eq!(
            AdapterError::Connection {
                family: "openai-compatible",
                detail: "dns".into(),
            }
            .kind(),
            AdapterErrorKind::Permanent
        );
        assert_eq!(
            AdapterError::Malformed {
                family: "anthropic",
                detail: "missing field".into(),
            }
            .kind(),
            AdapterErrorKind::Permanent
        );
    }

    #[test]
    fn family_accessor_returns_configured_family() {
        let err = AdapterError::Timeout {
            family: "gemini",
            ms: 1_000,
        };
        assert_eq!(err.family(), "gemini");
    }
}
