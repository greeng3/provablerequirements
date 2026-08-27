//! ReqForge's artifact model, storage, and schema migration — absorbed rather than reimplemented.
//!
//! This is the first slice of phase 2 of the absorb (#304, #305). It is **ReqForge's code, moved,
//! not provreq's code inspired by it**: the argument for merging rather than porting was that the
//! expensive part is the model and the 766 tests covering it, and a port inherits none of them.
//! Changes here should therefore be the minimum needed to make the crate stand alone. Anything
//! that looks like an improvement is a change to code whose tests were written elsewhere, and is
//! better made deliberately, later, than in transit.
//!
//! **Why these nine modules and no others.** They are the transitive closure of the artifact model.
//! Not one of them imports axum or tokio — the framing carried since #290, that the model was
//! entangled with a web server, was true of the *crate* and false of the *modules*, which is what
//! made slicing viable at all.
//!
//! `write` is in the closure rather than deferred to a later slice because
//! [`schema_migration`]'s lazy write-back writes, and [`system`]'s readonly detection inspects
//! [`write::AtomicWriteError`]. That is the design: migration with lazy write-back inherently
//! writes, and lazy write-back is exactly why this model is worth having for `verdicts.yml`, which
//! is evidence rather than configuration.
//!
//! What is deliberately absent: `http`, `llm`, `reports`, and `exports`. Later slices. Keeping
//! reqwest out also means ReqForge's 0.13 and provreq's 0.12 do not have to be reconciled yet.
//!
//! [`doorstop`] is the one-way importer (slice 2, #309): the permanent boundary that lets provreq
//! read Doorstop items — its own ~70 included — into this model. It is in the closure early because
//! it is what makes Doorstop an import format rather than a storage format.

pub mod doorstop;
pub mod frontmatter;
pub mod index;
pub mod links;
pub mod load;
pub mod mount;
// The review log proper — state derivation, snapshot persistence, and write-side validators
// (slice 3, #311). It is where a verification admission belongs; `ReqforgeSource::annotate` can now
// be wired to it, though it still refuses until a slice does so deliberately.
pub mod reviews;
// Source-tag scanner — resolves `Implements:`/`Verifies:`-style code tags against a `World` (slice
// 2 of the reports cluster, #331/#350). ReqForge's own scanner, the reports cluster's input;
// distinct from provreq's native `src/trace/` (#334) and not wired into the verify flow.
pub mod scan;
pub mod schema;
pub mod schema_migration;
// Full-text search index + the discovery snapshot it rides on — the first slice of the
// reports/data cluster (#331/#348), the data layer Phase 5's UI renders. `search::query` (which
// depends on `reports`) lands with the reports slice.
pub mod search;
pub mod system;
pub mod world;
pub mod write;
