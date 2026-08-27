//! URL-artifact runtime helpers (Phase 5b).
//!
//! Today this module holds the URL-reachability check. Phase 5d's
//! bulk URL refresh + the thumbnailer's URL-fetch fallback will
//! plug in here too rather than leaking reqwest around the tree.

pub mod check;

pub use check::{CheckOutcome, UrlCheckClient, classify_status};
