//! Phase 12a: LLM-assisted link suggestions.
//!
//! On-disk surface: per-Project sidecars under
//! `artifacts/.suggestions/`. Two files:
//!
//! - `pending.json` — the active proposal queue an analysis run
//!   produces. Survives container restart so the operator can
//!   work through suggestions across sessions.
//! - `declined.json` — proposals the operator has rejected.
//!   Re-runs filter on `(from, to, linkType)` so previously
//!   declined triples don't re-surface; the Rejected tab in the
//!   UI gives explicit access to revisit them via Reinstate.
//!
//! Both files are written via [`crate::write::atomic_write`] so
//! crashes mid-write can't corrupt them.

pub mod declined;
pub mod engine;
pub mod errors;
pub mod pending;
pub mod types;

pub use engine::{ParseError, ProposalError, build_prompt, parse_suggestions, propose_links};
pub use errors::SuggestionError;
pub use types::{DeclineRecord, Suggestion};
