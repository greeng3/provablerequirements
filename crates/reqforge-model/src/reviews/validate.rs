//! Write-side validation for review actions (Phase 4b).
//!
//! The write handler calls `validate_and_build_entry` with the
//! current artifact (or its derived state) and a parsed
//! [`ReviewActionInput`]. The validator either returns a ready-to-
//! append `ReviewLogEntry` or a typed error the handler converts to
//! a 4xx response.
//!
//! One HTTP call = one log entry, per the Phase 4 action/entry
//! contract. `reject-with-TODOs` is a single `rejected` entry
//! carrying `addedTodos`; it does NOT also emit `todo-added` entries.
//! For Phase 4 only a single TODO is accepted on a rejection;
//! multiple TODOs remain a forward-compatible storage concern
//! (`addedTodos: Vec<…>`) but are rejected at the validator here so
//! the UI and the write path stay in lockstep.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::reviews::state::{
    DerivedReviewState, OUTCOME_APPROVED, OUTCOME_RE_REQUEST, OUTCOME_REJECTED, OUTCOME_TODO_ADDED,
    OUTCOME_TODO_RESOLVED, ReviewState,
};
use crate::schema::{AddedTodo, ReviewLogEntry};

/// Parsed, validator-friendly input for a review action. Matches
/// the wire DTO shape 1:1 but with an enum tag so the handler can
/// reason about the action before validation runs.
#[derive(Debug, Clone)]
pub struct ReviewActionInput {
    pub reviewer: String,
    pub action: ReviewAction,
    pub explanation: Option<String>,
}

/// One of the five Phase 4 actions from `UX-reviewActions`.
#[derive(Debug, Clone)]
pub enum ReviewAction {
    Approve,
    /// Reject with an attached TODO. Phase 4 accepts exactly one
    /// TODO per rejection; multi-TODO-on-rejection is deferred.
    RejectWithTodo(AddedTodoInput),
    AddTodo(AddedTodoInput),
    ResolveTodo {
        id: String,
    },
    ReRequestReview,
}

/// The wire shape for a newly-added TODO. The id is server-allocated
/// (UUID v7) if the client doesn't supply one — typical flow is that
/// the client doesn't, since ids matter only for later resolution.
#[derive(Debug, Clone)]
pub struct AddedTodoInput {
    pub id: Option<String>,
    pub text: String,
}

/// Outcome of `validate_and_build_entry`: the ready-to-append log
/// entry, plus any side-effect breadcrumbs the handler needs.
#[derive(Debug)]
pub struct ValidatedReview {
    pub entry: ReviewLogEntry,
    /// True when the action is `approve`; the handler uses this to
    /// know whether to write a review snapshot.
    pub is_approval: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReviewValidationError {
    #[error("reviewer identity must not be empty")]
    EmptyReviewer,

    #[error("cannot approve while blocking TODOs are open: {open:?}")]
    ApproveWithOpenTodos { open: Vec<String> },

    #[error("re-request-review requires a prior approval or rejection; artifact has neither")]
    ReRequestWithoutHistory,

    #[error("todo id '{0}' is not currently open")]
    ResolveTodoUnknown(String),

    #[error("todo text must not be empty")]
    EmptyTodoText,
}

/// Validate and build the log entry to append for a review action.
///
/// `now` is passed in rather than read directly from `Utc::now()` so
/// callers (including tests) can freeze time.
pub fn validate_and_build_entry(
    derived: &DerivedReviewState,
    input: ReviewActionInput,
    now: DateTime<Utc>,
) -> Result<ValidatedReview, ReviewValidationError> {
    let reviewer = input.reviewer.trim().to_owned();
    if reviewer.is_empty() {
        return Err(ReviewValidationError::EmptyReviewer);
    }

    match input.action {
        ReviewAction::Approve => {
            if !derived.blocking_todos.is_empty() {
                let open = derived
                    .blocking_todos
                    .iter()
                    .map(|t| t.id.clone())
                    .collect();
                return Err(ReviewValidationError::ApproveWithOpenTodos { open });
            }
            Ok(ValidatedReview {
                entry: base_entry(now, reviewer, OUTCOME_APPROVED, input.explanation),
                is_approval: true,
            })
        }
        ReviewAction::RejectWithTodo(todo) => {
            let todo = build_added_todo(todo)?;
            let mut entry = base_entry(now, reviewer, OUTCOME_REJECTED, input.explanation);
            entry.added_todos.push(todo);
            Ok(ValidatedReview {
                entry,
                is_approval: false,
            })
        }
        ReviewAction::AddTodo(todo) => {
            let todo = build_added_todo(todo)?;
            let mut entry = base_entry(now, reviewer, OUTCOME_TODO_ADDED, input.explanation);
            entry.added_todos.push(todo);
            Ok(ValidatedReview {
                entry,
                is_approval: false,
            })
        }
        ReviewAction::ResolveTodo { id } => {
            if !derived.blocking_todos.iter().any(|t| t.id == id) {
                return Err(ReviewValidationError::ResolveTodoUnknown(id));
            }
            let mut entry = base_entry(now, reviewer, OUTCOME_TODO_RESOLVED, input.explanation);
            entry.resolved_todos.push(id);
            Ok(ValidatedReview {
                entry,
                is_approval: false,
            })
        }
        ReviewAction::ReRequestReview => {
            if matches!(
                derived.state,
                ReviewState::NeverReviewed | ReviewState::ReRequested
            ) && derived.last_approval_at.is_none()
            {
                return Err(ReviewValidationError::ReRequestWithoutHistory);
            }
            Ok(ValidatedReview {
                entry: base_entry(now, reviewer, OUTCOME_RE_REQUEST, input.explanation),
                is_approval: false,
            })
        }
    }
}

fn base_entry(
    now: DateTime<Utc>,
    reviewer: String,
    outcome: &str,
    explanation: Option<String>,
) -> ReviewLogEntry {
    ReviewLogEntry {
        timestamp: now,
        reviewer,
        outcome: outcome.to_owned(),
        explanation: explanation.filter(|s| !s.trim().is_empty()),
        added_todos: Vec::new(),
        resolved_todos: Vec::new(),
        overflow: std::collections::BTreeMap::new(),
    }
}

fn build_added_todo(input: AddedTodoInput) -> Result<AddedTodo, ReviewValidationError> {
    let text = input.text.trim();
    if text.is_empty() {
        return Err(ReviewValidationError::EmptyTodoText);
    }
    let id = input.id.unwrap_or_else(|| Uuid::now_v7().to_string());
    Ok(AddedTodo {
        id,
        text: text.to_owned(),
        overflow: std::collections::BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reviews::OpenTodo;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn derived(state: ReviewState) -> DerivedReviewState {
        DerivedReviewState {
            state,
            last_approval_at: None,
            last_event_at: None,
            last_reviewer: None,
            blocking_todos: Vec::new(),
        }
    }

    fn derived_with_open_todo(id: &str) -> DerivedReviewState {
        DerivedReviewState {
            state: ReviewState::Rejected,
            last_approval_at: None,
            last_event_at: Some(now()),
            last_reviewer: Some("bob".to_owned()),
            blocking_todos: vec![OpenTodo {
                id: id.to_owned(),
                text: "fix it".to_owned(),
                added_at: now(),
                added_by: "bob".to_owned(),
            }],
        }
    }

    fn input(reviewer: &str, action: ReviewAction) -> ReviewActionInput {
        ReviewActionInput {
            reviewer: reviewer.to_owned(),
            action,
            explanation: None,
        }
    }

    #[test]
    fn empty_reviewer_is_rejected() {
        let err = validate_and_build_entry(
            &derived(ReviewState::NeverReviewed),
            input("  ", ReviewAction::Approve),
            now(),
        )
        .unwrap_err();
        assert_eq!(err, ReviewValidationError::EmptyReviewer);
    }

    #[test]
    fn approve_succeeds_with_no_open_todos() {
        let v = validate_and_build_entry(
            &derived(ReviewState::NeverReviewed),
            input("alice", ReviewAction::Approve),
            now(),
        )
        .unwrap();
        assert_eq!(v.entry.outcome, "approved");
        assert!(v.is_approval);
    }

    #[test]
    fn approve_fails_when_blocking_todos_open() {
        let err = validate_and_build_entry(
            &derived_with_open_todo("t1"),
            input("alice", ReviewAction::Approve),
            now(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ReviewValidationError::ApproveWithOpenTodos { ref open } if open == &vec!["t1".to_owned()]
        ));
    }

    #[test]
    fn reject_with_todo_embeds_todo_on_rejected_entry() {
        let v = validate_and_build_entry(
            &derived(ReviewState::NeverReviewed),
            input(
                "bob",
                ReviewAction::RejectWithTodo(AddedTodoInput {
                    id: Some("t42".to_owned()),
                    text: "Add AC".to_owned(),
                }),
            ),
            now(),
        )
        .unwrap();
        assert_eq!(v.entry.outcome, "rejected");
        assert_eq!(v.entry.added_todos.len(), 1);
        assert_eq!(v.entry.added_todos[0].id, "t42");
        assert_eq!(v.entry.added_todos[0].text, "Add AC");
        assert!(!v.is_approval);
    }

    #[test]
    fn reject_fills_todo_id_when_client_omits_it() {
        let v = validate_and_build_entry(
            &derived(ReviewState::NeverReviewed),
            input(
                "bob",
                ReviewAction::RejectWithTodo(AddedTodoInput {
                    id: None,
                    text: "Add AC".to_owned(),
                }),
            ),
            now(),
        )
        .unwrap();
        assert!(!v.entry.added_todos[0].id.is_empty());
    }

    #[test]
    fn reject_with_empty_todo_text_fails() {
        let err = validate_and_build_entry(
            &derived(ReviewState::NeverReviewed),
            input(
                "bob",
                ReviewAction::RejectWithTodo(AddedTodoInput {
                    id: None,
                    text: "   ".to_owned(),
                }),
            ),
            now(),
        )
        .unwrap_err();
        assert_eq!(err, ReviewValidationError::EmptyTodoText);
    }

    #[test]
    fn add_todo_emits_todo_added_entry() {
        let v = validate_and_build_entry(
            &derived(ReviewState::Approved),
            input(
                "bob",
                ReviewAction::AddTodo(AddedTodoInput {
                    id: None,
                    text: "Address later".to_owned(),
                }),
            ),
            now(),
        )
        .unwrap();
        assert_eq!(v.entry.outcome, "todo-added");
        assert_eq!(v.entry.added_todos.len(), 1);
    }

    #[test]
    fn resolve_todo_succeeds_when_id_is_open() {
        let v = validate_and_build_entry(
            &derived_with_open_todo("t1"),
            input(
                "alice",
                ReviewAction::ResolveTodo {
                    id: "t1".to_owned(),
                },
            ),
            now(),
        )
        .unwrap();
        assert_eq!(v.entry.outcome, "todo-resolved");
        assert_eq!(v.entry.resolved_todos, vec!["t1".to_owned()]);
    }

    #[test]
    fn resolve_todo_fails_for_unknown_id() {
        let err = validate_and_build_entry(
            &derived_with_open_todo("t1"),
            input(
                "alice",
                ReviewAction::ResolveTodo {
                    id: "t9".to_owned(),
                },
            ),
            now(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            ReviewValidationError::ResolveTodoUnknown("t9".to_owned())
        );
    }

    #[test]
    fn re_request_fails_without_any_prior_history() {
        let err = validate_and_build_entry(
            &derived(ReviewState::NeverReviewed),
            input("alice", ReviewAction::ReRequestReview),
            now(),
        )
        .unwrap_err();
        assert_eq!(err, ReviewValidationError::ReRequestWithoutHistory);
    }

    #[test]
    fn re_request_succeeds_after_approval() {
        let mut d = derived(ReviewState::Approved);
        d.last_approval_at = Some(now());
        let v = validate_and_build_entry(&d, input("alice", ReviewAction::ReRequestReview), now())
            .unwrap();
        assert_eq!(v.entry.outcome, "re-request-review");
    }

    #[test]
    fn re_request_succeeds_after_rejection_too() {
        let d = derived(ReviewState::Rejected);
        let v = validate_and_build_entry(&d, input("alice", ReviewAction::ReRequestReview), now())
            .unwrap();
        assert_eq!(v.entry.outcome, "re-request-review");
    }

    #[test]
    fn explanation_whitespace_is_dropped() {
        let v = validate_and_build_entry(
            &derived(ReviewState::NeverReviewed),
            ReviewActionInput {
                reviewer: "alice".to_owned(),
                action: ReviewAction::Approve,
                explanation: Some("   ".to_owned()),
            },
            now(),
        )
        .unwrap();
        assert!(v.entry.explanation.is_none());
    }
}
