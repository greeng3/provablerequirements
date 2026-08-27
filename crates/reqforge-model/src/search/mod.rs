//! Full-text search (Phase 7c).
//!
//! A Tantivy index built in memory holds every artifact's
//! searchable text (title, short name, body, description, tags)
//! plus the filter fields listed in `UX-search`. `SearchIndex`
//! is rebuilt inside `world::run_discovery` alongside the UUID
//! index so the two views converge on the same snapshot of the
//! World.
//!
//! Absorbed from ReqForge (#348). The query layer (`query.rs`)
//! landed with the reports slice (#352), since it depends on `reports`.

pub mod index;
pub mod query;

pub use index::{SearchIndex, SearchIndexError, empty_index};
pub use query::{SearchError, SearchHit, SearchQuery, SearchResponse, run};
