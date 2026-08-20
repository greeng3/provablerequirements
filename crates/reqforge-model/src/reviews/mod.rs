//! A sliver of ReqForge's `reviews` module — `identity` only.
//!
//! The review log proper (`persistence`, `state`, `validate`, and the rest of this module's 1,405
//! lines) belongs to a later slice of #304, and it is the home an admission will eventually get:
//! `ReqforgeSource::annotate` refuses today precisely so that nothing invents a convention before
//! that slice decides one.
//!
//! `identity` is here because [`crate::load::project`] calls `parse_git_config_user_name` to work
//! out who is running, and it depends on nothing beyond `serde` and [`crate::schema::Overflow`].
//! Taking 278 lines rather than 1,405 keeps this slice to the artifact model, which is what it is
//! for. The re-export below mirrors ReqForge's own `mod.rs`, so the call site resolves unchanged
//! and the later slice can drop the remaining submodules in beside this one.

pub mod identity;

pub use identity::{ReviewerIdentityOptions, load_reviewers_json, parse_git_config_user_name};
