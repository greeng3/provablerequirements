//! Write-side validation for artifact links (Phase 3b).
//!
//! Runs inside the `update_artifact` handler before any file is
//! written. Checks three invariants per `TRACE-linkCrud` and
//! `TRACE-linkExtensibility`:
//!
//! - Type name is non-empty and present in the effective catalog.
//! - No artifact links to itself.
//! - Every link has a hint by write time — the server fills it from
//!   the UUID index when possible so clients with a stale index
//!   don't have to; when the target is unmounted, the client hint
//!   is the only option and must be supplied.
//!
//! Unresolved target UUIDs are *not* rejected: cross-repo authoring
//! has to work even when the other repo isn't currently mounted
//! (per `TRACE-crossRepoLinks`).

use uuid::Uuid;

use crate::index::UuidIndex;
use crate::links::LinkType;
use crate::schema::{Link, LinkHint};

/// Parsed + validated replacement for an artifact's `links` array.
#[derive(Debug, Clone)]
pub struct ValidatedLinks(pub Vec<Link>);

/// Request-side shape for one link. `hint` is optional on input —
/// the server prefers the canonical location from the UUID index,
/// falling back to the client hint when the target is unmounted.
#[derive(Debug, Clone)]
pub struct LinkWriteInput {
    pub target_uuid: Uuid,
    pub type_name: String,
    pub hint: Option<LinkHint>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LinkValidationError {
    #[error("link type must not be empty")]
    EmptyType,

    #[error("link type '{0}' is not in the effective catalog")]
    UnknownType(String),

    #[error("artifact cannot link to itself")]
    SelfLink,

    #[error(
        "link to {target_uuid} is not resolvable and no hint was supplied — \
         cross-repo links require a hint"
    )]
    UnresolvedWithoutHint { target_uuid: Uuid },
}

/// Validate every write-request link and populate hints, returning
/// the exact `Vec<Link>` that should be persisted.
pub fn validate_links(
    self_uuid: Uuid,
    inputs: &[LinkWriteInput],
    catalog: &[LinkType],
    index: &UuidIndex,
) -> Result<ValidatedLinks, LinkValidationError> {
    let mut out = Vec::with_capacity(inputs.len());
    for input in inputs {
        if input.type_name.is_empty() {
            return Err(LinkValidationError::EmptyType);
        }
        if input.target_uuid == self_uuid {
            return Err(LinkValidationError::SelfLink);
        }
        if !catalog.iter().any(|t| t.name == input.type_name) {
            return Err(LinkValidationError::UnknownType(input.type_name.clone()));
        }
        let hint = resolve_hint(input, index)?;
        out.push(Link {
            target_uuid: input.target_uuid,
            type_name: input.type_name.clone(),
            hint,
            overflow: std::collections::BTreeMap::new(),
        });
    }
    Ok(ValidatedLinks(out))
}

fn resolve_hint(
    input: &LinkWriteInput,
    index: &UuidIndex,
) -> Result<LinkHint, LinkValidationError> {
    if let Some(location) = index.get(&input.target_uuid) {
        return Ok(LinkHint {
            project_slug: location.project_slug.clone(),
            collection_prefix: location.collection_prefix.clone(),
            artifact_name: location.artifact_name.clone(),
            overflow: std::collections::BTreeMap::new(),
        });
    }
    input
        .hint
        .clone()
        .ok_or(LinkValidationError::UnresolvedWithoutHint {
            target_uuid: input.target_uuid,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::builtin_catalog;

    fn empty_index() -> UuidIndex {
        // The validator's hit-path (auto-populating hints from the
        // index) is covered by integration tests that go through
        // `build_uuid_index` on a real loaded project. Here we only
        // exercise the miss-path and pure-logic branches.
        UuidIndex::new()
    }

    fn self_id() -> Uuid {
        Uuid::parse_str("0194f6d0-0001-7000-8000-000000000099").unwrap()
    }

    fn target_id() -> Uuid {
        Uuid::parse_str("0194f6d0-0001-7000-8000-000000000001").unwrap()
    }

    fn input_for(target: Uuid, type_name: &str, hint: Option<LinkHint>) -> LinkWriteInput {
        LinkWriteInput {
            target_uuid: target,
            type_name: type_name.to_owned(),
            hint,
        }
    }

    fn fake_hint() -> LinkHint {
        LinkHint {
            project_slug: "other-repo".to_owned(),
            collection_prefix: "REQ".to_owned(),
            artifact_name: "REQ-elsewhere".to_owned(),
            overflow: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn rejects_empty_type() {
        let err = validate_links(
            self_id(),
            &[input_for(target_id(), "", Some(fake_hint()))],
            builtin_catalog(),
            &empty_index(),
        )
        .unwrap_err();
        assert_eq!(err, LinkValidationError::EmptyType);
    }

    #[test]
    fn rejects_unknown_type() {
        let err = validate_links(
            self_id(),
            &[input_for(target_id(), "invented-type", Some(fake_hint()))],
            builtin_catalog(),
            &empty_index(),
        )
        .unwrap_err();
        assert!(matches!(err, LinkValidationError::UnknownType(t) if t == "invented-type"));
    }

    #[test]
    fn rejects_self_link() {
        let err = validate_links(
            self_id(),
            &[input_for(self_id(), "derives-from", Some(fake_hint()))],
            builtin_catalog(),
            &empty_index(),
        )
        .unwrap_err();
        assert_eq!(err, LinkValidationError::SelfLink);
    }

    #[test]
    fn rejects_unresolved_without_hint() {
        // target_id() is not in an empty index; no hint supplied.
        let err = validate_links(
            self_id(),
            &[input_for(target_id(), "derives-from", None)],
            builtin_catalog(),
            &empty_index(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            LinkValidationError::UnresolvedWithoutHint { .. }
        ));
    }

    #[test]
    fn accepts_unresolved_target_with_hint() {
        let result = validate_links(
            self_id(),
            &[input_for(target_id(), "derives-from", Some(fake_hint()))],
            builtin_catalog(),
            &empty_index(),
        )
        .unwrap();
        assert_eq!(result.0.len(), 1);
        assert_eq!(result.0[0].hint.project_slug, "other-repo");
    }

    #[test]
    fn accepts_empty_link_list() {
        let result = validate_links(self_id(), &[], builtin_catalog(), &empty_index()).unwrap();
        assert!(result.0.is_empty());
    }
}
