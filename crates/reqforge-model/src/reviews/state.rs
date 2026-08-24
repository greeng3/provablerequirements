//! Review-state derivation.
//!
//! Computes the current `ReviewState` for an artifact from its raw
//! `review_log` history. Pure functions; no IO.
//!
//! Rules (per `REVIEW-approvedRejected`, `REVIEW-blockingTodos`,
//! `UX-reviewPane`):
//!
//! - The "current" review event is the one with the latest timestamp.
//!   On ties, the later entry in the stored array wins (the log is
//!   append-only; later-written events are intentionally stored
//!   last).
//! - A `rejected` entry opens every TODO it carries in `addedTodos`.
//!   A `todo-added` entry does the same. Any `todo-resolved` entry
//!   closes the TODO ids listed in `resolvedTodos`.
//! - Blocking TODOs are the still-open TODOs accumulated from
//!   entries newer than the most recent `approved` entry (per
//!   `REVIEW-blockingTodos`). Older open TODOs from before the last
//!   approval are implicitly forgiven by the approval.
//! - `re-request-review` is a log-only signal: it sets the derived
//!   state to `ReRequested` without resetting history. The queue
//!   sorts these into the Awaiting-review section by the
//!   re-request timestamp (per `UX-reviewQueue`).
//!
//! ## Single-reviewer now, N-of-M later
//!
//! Callers pass `&[ReviewLogEntry]` and receive a single computed
//! `DerivedReviewState`. When `REVIEW-futureMultiReviewer` lands, the
//! derivation logic inside this module changes; the serde schema and
//! the function signature stay intact.

use chrono::{DateTime, Utc};

use crate::schema::ReviewLogEntry;

/// The current review state of an artifact, derived from its log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewState {
    /// No review events recorded yet.
    NeverReviewed,
    /// Most recent state-changing event is an approval.
    Approved,
    /// Most recent state-changing event is a rejection.
    Rejected,
    /// A reviewer asked for a fresh review after the current
    /// approval/rejection (per `UX-reviewActions`).
    ReRequested,
}

/// A TODO that is currently open (un-resolved) and attached to the
/// review-log window that matters for the current state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTodo {
    pub id: String,
    pub text: String,
    /// When the TODO was added (the timestamp of the log entry that
    /// added it, not the current time).
    pub added_at: DateTime<Utc>,
    pub added_by: String,
}

/// The derived view of an artifact's review log. `last_event_at`
/// covers every log entry including `todo-added` / `todo-resolved`;
/// `last_approval_at` covers only `approved` entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedReviewState {
    pub state: ReviewState,
    pub last_approval_at: Option<DateTime<Utc>>,
    pub last_event_at: Option<DateTime<Utc>>,
    pub last_reviewer: Option<String>,
    pub blocking_todos: Vec<OpenTodo>,
}

/// Known outcome tags. The raw `ReviewLogEntry.outcome` is a free
/// string (per the schema comment); we recognise the tags below and
/// ignore everything else for state-derivation purposes so unknown
/// outcomes don't flip the state unexpectedly.
pub const OUTCOME_APPROVED: &str = "approved";
pub const OUTCOME_REJECTED: &str = "rejected";
pub const OUTCOME_TODO_ADDED: &str = "todo-added";
pub const OUTCOME_TODO_RESOLVED: &str = "todo-resolved";
pub const OUTCOME_RE_REQUEST: &str = "re-request-review";

/// Derive the current review state from the full log.
pub fn derive_review_state(log: &[ReviewLogEntry]) -> DerivedReviewState {
    if log.is_empty() {
        return DerivedReviewState {
            state: ReviewState::NeverReviewed,
            last_approval_at: None,
            last_event_at: None,
            last_reviewer: None,
            blocking_todos: Vec::new(),
        };
    }

    // Stable-sort by (timestamp, original-index) so ties preserve
    // the stored order. Using indices keeps the original log
    // untouched.
    let mut order: Vec<usize> = (0..log.len()).collect();
    order.sort_by(|&a, &b| {
        log[a]
            .timestamp
            .cmp(&log[b].timestamp)
            .then_with(|| a.cmp(&b))
    });

    let last_event_at = order.last().map(|&i| log[i].timestamp);
    let last_reviewer = order.last().map(|&i| log[i].reviewer.clone());

    let last_approval_idx = order
        .iter()
        .rev()
        .copied()
        .find(|&i| log[i].outcome == OUTCOME_APPROVED);
    let last_approval_at = last_approval_idx.map(|i| log[i].timestamp);

    let state = derive_state_tag(&order, log, last_approval_idx);

    let blocking_todos = collect_open_todos(&order, log, last_approval_idx);

    DerivedReviewState {
        state,
        last_approval_at,
        last_event_at,
        last_reviewer,
        blocking_todos,
    }
}

fn derive_state_tag(
    order: &[usize],
    log: &[ReviewLogEntry],
    last_approval_idx: Option<usize>,
) -> ReviewState {
    // Walk the timeline backwards looking for the most recent
    // state-changing outcome. `todo-added` and `todo-resolved` are
    // log-only and don't change the state on their own; they
    // surface through the blocking-TODO set.
    for &i in order.iter().rev() {
        match log[i].outcome.as_str() {
            OUTCOME_APPROVED => return ReviewState::Approved,
            OUTCOME_REJECTED => return ReviewState::Rejected,
            OUTCOME_RE_REQUEST => {
                // `re-request-review` is meaningful only after some
                // prior approval or rejection. If there is none,
                // treat it as NeverReviewed — the re-request is
                // informational.
                if last_approval_idx.is_some()
                    || order.iter().any(|&j| log[j].outcome == OUTCOME_REJECTED)
                {
                    return ReviewState::ReRequested;
                }
                return ReviewState::NeverReviewed;
            }
            _ => continue,
        }
    }
    ReviewState::NeverReviewed
}

fn collect_open_todos(
    order: &[usize],
    log: &[ReviewLogEntry],
    last_approval_idx: Option<usize>,
) -> Vec<OpenTodo> {
    // Index of the last approval in the sorted `order` (so we can
    // slice the log window newer than the last approval).
    let start = match last_approval_idx {
        None => 0,
        Some(target) => order
            .iter()
            .position(|&i| i == target)
            .map(|p| p + 1)
            .unwrap_or(0),
    };

    let mut open: Vec<OpenTodo> = Vec::new();
    for &i in &order[start..] {
        let entry = &log[i];
        if entry.outcome == OUTCOME_APPROVED {
            // Should not happen after start; belt-and-braces.
            continue;
        }
        for todo in &entry.added_todos {
            open.push(OpenTodo {
                id: todo.id.clone(),
                text: todo.text.clone(),
                added_at: entry.timestamp,
                added_by: entry.reviewer.clone(),
            });
        }
        if !entry.resolved_todos.is_empty() {
            open.retain(|t| !entry.resolved_todos.iter().any(|r| r == &t.id));
        }
    }
    open
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::AddedTodo;
    use std::collections::BTreeMap;

    fn at(ts: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(ts)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn entry(ts: &str, reviewer: &str, outcome: &str) -> ReviewLogEntry {
        ReviewLogEntry {
            timestamp: at(ts),
            reviewer: reviewer.to_owned(),
            outcome: outcome.to_owned(),
            explanation: None,
            added_todos: Vec::new(),
            resolved_todos: Vec::new(),
            overflow: BTreeMap::new(),
        }
    }

    fn with_added_todo(mut e: ReviewLogEntry, id: &str, text: &str) -> ReviewLogEntry {
        e.added_todos.push(AddedTodo {
            id: id.to_owned(),
            text: text.to_owned(),
            overflow: BTreeMap::new(),
        });
        e
    }

    fn with_resolved_todo(mut e: ReviewLogEntry, id: &str) -> ReviewLogEntry {
        e.resolved_todos.push(id.to_owned());
        e
    }

    #[test]
    fn empty_log_is_never_reviewed() {
        let d = derive_review_state(&[]);
        assert_eq!(d.state, ReviewState::NeverReviewed);
        assert!(d.blocking_todos.is_empty());
        assert!(d.last_approval_at.is_none());
        assert!(d.last_event_at.is_none());
        assert!(d.last_reviewer.is_none());
    }

    #[test]
    fn single_approval_is_approved() {
        let log = [entry("2026-04-18T00:00:00Z", "alice", "approved")];
        let d = derive_review_state(&log);
        assert_eq!(d.state, ReviewState::Approved);
        assert_eq!(d.last_approval_at, Some(at("2026-04-18T00:00:00Z")));
        assert_eq!(d.last_reviewer.as_deref(), Some("alice"));
        assert!(d.blocking_todos.is_empty());
    }

    #[test]
    fn rejection_opens_a_blocking_todo() {
        let log = [with_added_todo(
            entry("2026-04-18T00:00:00Z", "bob", "rejected"),
            "t1",
            "Add AC section",
        )];
        let d = derive_review_state(&log);
        assert_eq!(d.state, ReviewState::Rejected);
        assert_eq!(d.blocking_todos.len(), 1);
        assert_eq!(d.blocking_todos[0].id, "t1");
        assert_eq!(d.blocking_todos[0].text, "Add AC section");
    }

    #[test]
    fn approval_forgives_older_open_todos() {
        // A prior rejection leaves a TODO open; a later approval
        // drops it out of the "blocking" set because the approval
        // window starts after it.
        let log = [
            with_added_todo(
                entry("2026-04-18T00:00:00Z", "bob", "rejected"),
                "t1",
                "old TODO",
            ),
            entry("2026-04-19T00:00:00Z", "alice", "approved"),
        ];
        let d = derive_review_state(&log);
        assert_eq!(d.state, ReviewState::Approved);
        assert!(d.blocking_todos.is_empty());
    }

    #[test]
    fn todo_resolved_entry_closes_matching_open_todo() {
        let log = [
            with_added_todo(
                entry("2026-04-18T00:00:00Z", "bob", "rejected"),
                "t1",
                "first",
            ),
            with_added_todo(
                entry("2026-04-18T01:00:00Z", "bob", "todo-added"),
                "t2",
                "second",
            ),
            with_resolved_todo(
                entry("2026-04-18T02:00:00Z", "carol", "todo-resolved"),
                "t1",
            ),
        ];
        let d = derive_review_state(&log);
        assert_eq!(d.state, ReviewState::Rejected);
        let ids: Vec<&str> = d.blocking_todos.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["t2"]);
    }

    #[test]
    fn resolving_then_re_adding_same_id_leaves_it_open() {
        // resolve wipes the id; a later add with the same id
        // re-opens it. Both are blocking after the re-add.
        let log = [
            with_added_todo(
                entry("2026-04-18T00:00:00Z", "bob", "rejected"),
                "t1",
                "original",
            ),
            with_resolved_todo(
                entry("2026-04-18T01:00:00Z", "carol", "todo-resolved"),
                "t1",
            ),
            with_added_todo(
                entry("2026-04-18T02:00:00Z", "bob", "todo-added"),
                "t1",
                "reopened",
            ),
        ];
        let d = derive_review_state(&log);
        assert_eq!(d.blocking_todos.len(), 1);
        assert_eq!(d.blocking_todos[0].text, "reopened");
    }

    #[test]
    fn re_request_after_approval_is_re_requested() {
        let log = [
            entry("2026-04-18T00:00:00Z", "alice", "approved"),
            entry("2026-04-19T00:00:00Z", "bob", "re-request-review"),
        ];
        let d = derive_review_state(&log);
        assert_eq!(d.state, ReviewState::ReRequested);
        // The last approval stays in scope for "since last approval".
        assert_eq!(d.last_approval_at, Some(at("2026-04-18T00:00:00Z")));
    }

    #[test]
    fn re_request_with_no_prior_approval_or_rejection_is_never_reviewed() {
        // `re-request-review` with nothing to anchor on is
        // informational only.
        let log = [entry("2026-04-18T00:00:00Z", "alice", "re-request-review")];
        let d = derive_review_state(&log);
        assert_eq!(d.state, ReviewState::NeverReviewed);
    }

    #[test]
    fn tied_timestamps_use_insertion_order_to_break_ties() {
        // Two entries at the same timestamp; "approved" written
        // second should win the derived state.
        let log = [
            entry("2026-04-18T00:00:00Z", "bob", "rejected"),
            entry("2026-04-18T00:00:00Z", "alice", "approved"),
        ];
        let d = derive_review_state(&log);
        assert_eq!(d.state, ReviewState::Approved);
    }

    #[test]
    fn unknown_outcome_does_not_flip_state() {
        // The first entry is an approval; a later entry with an
        // outcome ReqForge doesn't recognise should be ignored for
        // state derivation. Forward-compat per the schema comment.
        let log = [
            entry("2026-04-18T00:00:00Z", "alice", "approved"),
            entry("2026-04-19T00:00:00Z", "bob", "audit-noted"),
        ];
        let d = derive_review_state(&log);
        assert_eq!(d.state, ReviewState::Approved);
    }
}
