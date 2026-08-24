//! Review workflow — state derivation, identity plumbing, and the
//! write-side validators that Phase 4 layers on top of the existing
//! `ReviewLogEntry` serde type.
//!
//! The module owns three concerns:
//!
//! - **State derivation** (`state`): given the raw `review_log` event
//!   stream, compute the current review state — `Approved`,
//!   `Rejected`, `ReRequested`, or `NeverReviewed` — plus the set of
//!   open blocking TODOs, the most-recent-approval timestamp, and
//!   the last event / reviewer for the queue. Pure functions, no IO.
//! - **Reviewer identity** (`identity`): parse the active mount's
//!   `.git/config` for a `[user] name = …` default, read
//!   `<workspace>/reviewers.json` for persisted identities, and
//!   aggregate them with the `AppState` session cache.
//! - **Write validation** (4b; `validate`): lands in Phase 4b. This
//!   module keeps the structure free so the write handler can plug
//!   in without a second reshuffle.
//!
//! `ReviewLogEntry` is intentionally an opaque event stream for
//! consumers outside this module. When `REVIEW-futureMultiReviewer`
//! lands, only the derivation inside `state.rs` changes; the serde
//! schema stays intact.

pub mod identity;
pub mod persistence;
pub mod state;
pub mod validate;

pub use identity::{ReviewerIdentityOptions, load_reviewers_json, parse_git_config_user_name};
pub use persistence::{
    LoadedSnapshot, ReviewerPersistenceError, append_reviewer_if_missing,
    load_latest_approval_snapshot, write_approval_snapshot,
};
pub use state::{DerivedReviewState, OpenTodo, ReviewState, derive_review_state};
pub use validate::{
    AddedTodoInput, ReviewAction, ReviewActionInput, ReviewValidationError, ValidatedReview,
    validate_and_build_entry,
};
