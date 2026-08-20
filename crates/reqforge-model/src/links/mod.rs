//! Link-type catalog and resolution utilities.
//!
//! Phase 3a introduces:
//!
//! - A hard-coded built-in catalog covering the seven baseline
//!   link types defined in `TRACE-linkCatalog`. Built-in types are
//!   part of the product spec rather than user data, so they live
//!   in code rather than in a JSON fixture — there's no extension
//!   point to keep open, because the System config handles that.
//! - An effective-catalog resolver that merges built-ins with any
//!   System-declared extras (`TRACE-linkExtensibility`). Built-ins
//!   always win on same-name collisions.
//!
//! Downstream code consumes `Vec<LinkType>` uniformly — callers
//! don't need to know whether a type came from the built-ins or
//! the System config except via the `source` field.

pub mod catalog;
pub mod validate;

pub use catalog::{LinkType, LinkTypeSource, builtin_catalog, effective_catalog};
pub use validate::{LinkValidationError, LinkWriteInput, ValidatedLinks, validate_links};
