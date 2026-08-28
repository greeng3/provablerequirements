//! LLM provider adapter layer (Phase 10a).
//!
//! Infrastructure only — the plumbing Phase 10b's first LLM
//! feature (rename) will build on. Nothing in 10a actually
//! ships a user-facing feature that invokes an LLM; the
//! adapter layer exposes a generic `send_prompt` interface
//! per `LLM-promptAbstraction` and the HTTP surface exposes
//! read-only + control endpoints (provider list, retest,
//! privacy-ack, debug prompt).
//!
//! Sub-modules:
//!
//! - [`config`] — typed parse of `SystemConfig.llm`.
//! - [`provider`] — `Adapter` trait + `AdapterError` + shared
//!   request/response shapes.
//! - [`health`] — three-state machine (Healthy /
//!   Transient-degraded / Hard-disabled) per
//!   `LLM-healthTracking`.
//! - [`privacy`] — one-time-per-process ack tracker + local-
//!   endpoint detection.
//! - [`chain`] — fallback dispatcher that walks the provider
//!   list in priority order.
//! - [`adapters`] — concrete OpenAI-compatible / Anthropic /
//!   Gemini implementations (10a.2).

pub mod adapters;
pub mod chain;
pub mod config;
pub mod health;
pub mod privacy;
pub mod provider;
pub mod runtime;

pub use adapters::{BuildError, build_adapter};
pub use chain::{ChainError, SlotOutcome, run_chain};
pub use config::{ParseError, ProviderConfig, ProviderFamily, llm_config, parse_llm};
pub use health::{Clock, HealthState, HealthTracker, SystemClock};
pub use privacy::{PrivacyTracker, is_local_endpoint};
pub use provider::{
    Adapter, AdapterError, AdapterErrorKind, BoxFuture, PromptMessage, PromptRequest,
    PromptResponse, PromptRole, PromptUsage,
};
pub use runtime::{LlmRuntime, RuntimeBuildError};
