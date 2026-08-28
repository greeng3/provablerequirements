//! Core types for Phase 12a's link-suggestion surface.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One LLM-proposed link between two artifacts.
///
/// `id` is minted once at proposal time (UUIDv7) so accept /
/// reject / reinstate URLs are stable across page reloads. The
/// triple `(from, to, link_type)` is the conceptual key used to
/// dedupe re-runs against the declined sidecar.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: Uuid,
    pub from: Uuid,
    pub to: Uuid,
    pub link_type: String,
    /// 0.0 – 1.0; clamped on read.
    pub confidence: f32,
    pub rationale: String,
}

/// A previously-declined suggestion plus the rejection timestamp.
/// Serialized flat — the suggestion's fields appear at the top
/// level of the JSON object alongside `declinedAt`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclineRecord {
    #[serde(flatten)]
    pub suggestion: Suggestion,
    pub declined_at: DateTime<Utc>,
}
