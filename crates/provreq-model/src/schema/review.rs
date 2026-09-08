//! Review log entry and TODO types — per `FORMAT-reviewLogSchema`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::schema::Overflow;

/// A single entry in an artifact's review log.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewLogEntry {
    pub timestamp: DateTime<Utc>,
    pub reviewer: String,

    /// Outcome tag — one of "approved", "rejected", "todo-added",
    /// "todo-resolved", or another tag the review workflow
    /// recognises. Kept as `String` so future outcome tags don't
    /// break back-compat.
    pub outcome: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_todos: Vec<AddedTodo>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved_todos: Vec<String>,

    #[serde(flatten)]
    pub overflow: Overflow,
}

/// A TODO newly added by a review log entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AddedTodo {
    pub id: String,
    pub text: String,

    #[serde(flatten)]
    pub overflow: Overflow,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_minimal_review_entry() {
        let raw = json!({
            "timestamp": "2026-04-18T00:00:00Z",
            "reviewer": "alice",
            "outcome": "approved"
        });
        let entry: ReviewLogEntry = serde_json::from_value(raw).unwrap();
        assert_eq!(entry.reviewer, "alice");
        assert_eq!(entry.outcome, "approved");
        assert!(entry.explanation.is_none());
        assert!(entry.added_todos.is_empty());
        assert!(entry.resolved_todos.is_empty());
    }

    #[test]
    fn deserializes_entry_with_todos_and_explanation() {
        let raw = json!({
            "timestamp": "2026-04-18T10:20:30Z",
            "reviewer": "bob",
            "outcome": "todo-added",
            "explanation": "Missing acceptance criteria.",
            "addedTodos": [
                { "id": "t1", "text": "Add AC section" }
            ],
            "resolvedTodos": ["t0"]
        });
        let entry: ReviewLogEntry = serde_json::from_value(raw).unwrap();
        assert_eq!(
            entry.explanation.as_deref(),
            Some("Missing acceptance criteria.")
        );
        assert_eq!(entry.added_todos.len(), 1);
        assert_eq!(entry.added_todos[0].id, "t1");
        assert_eq!(entry.resolved_todos, vec!["t0"]);
    }

    #[test]
    fn preserves_unknown_fields() {
        let raw = json!({
            "timestamp": "2026-04-18T00:00:00Z",
            "reviewer": "alice",
            "outcome": "approved",
            "severity": "low"
        });
        let entry: ReviewLogEntry = serde_json::from_value(raw.clone()).unwrap();
        let round_tripped = serde_json::to_value(&entry).unwrap();
        assert_eq!(round_tripped, raw);
    }
}
