//! Doorstop-UID → ReqForge-name mapping (INTEROP-doorstopIdNormalization).
//!
//! The artifact name is the doorstop UID **verbatim**. Nothing in the model parses a name
//! structurally — links resolve by UUID and the link hint carries `collectionPrefix`/`artifactName`
//! as separate fields — so a name never has to encode the prefix, and keeping the source's own
//! identifier has three payoffs: a subject migrating onto ReqForge keeps every id (the filename
//! stem is the id its verdicts, drafts, and code references are keyed on), imported artifacts trace
//! back to their origin by name, and two distinct UIDs can never collide the way the old
//! dash→underscore normalisation could (`DES-rocket-nozzle` and `DES-rocket_nozzle`).
//!
//! This module only validates the UID's shape and hands the name back; the caller stores the same
//! UID under `legacy.doorstopUid` and decides what to do with the name.

/// Split a doorstop UID into its `prefix` + `NANU` (suffix
/// after the sep). Returns the NANU exactly as it appears in
/// the UID — no stripping, no padding changes. `None` when the
/// UID doesn't start with the expected prefix + sep (which
/// callers treat as a structural problem in the doorstop
/// tree).
pub fn parse_doorstop_uid<'a>(uid: &'a str, prefix: &str, sep: &str) -> Option<&'a str> {
    if !uid.starts_with(prefix) {
        return None;
    }
    let rest = &uid[prefix.len()..];
    if sep.is_empty() {
        return Some(rest);
    }
    rest.strip_prefix(sep)
}

/// The ReqForge artifact name for an imported doorstop item — the UID verbatim. Returns `None` when
/// the UID doesn't parse against the document's `prefix`/`sep` (empty or missing NANU), which the
/// caller surfaces as a warning in the import report.
pub fn reqforge_name_from_uid(uid: &str, prefix: &str, sep: &str) -> Option<String> {
    let nanu = parse_doorstop_uid(uid, prefix, sep)?;
    if nanu.is_empty() {
        return None;
    }
    Some(uid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_the_source_uid_verbatim() {
        // The name is the doorstop uid unchanged — a no-sep tree keeps its padded numeric uid,
        // and a `-`-sep tree keeps its dashed uid. This is what lets a subject migrate onto
        // ReqForge without any id churn: the filename stem stays exactly what it was.
        assert_eq!(
            reqforge_name_from_uid("REQ001", "REQ", "").as_deref(),
            Some("REQ001")
        );
        assert_eq!(
            reqforge_name_from_uid("REQ-001", "REQ", "-").as_deref(),
            Some("REQ-001")
        );
    }

    #[test]
    fn multi_word_nanu_is_preserved_not_underscored() {
        // The old importer turned NANU dashes into underscores; nothing in the model parses a
        // name structurally (links resolve by uuid), so the mangling only lost fidelity and could
        // collide `DES-rocket-nozzle` with a hypothetical `DES-rocket_nozzle`.
        assert_eq!(
            reqforge_name_from_uid("DES-rocket-nozzle", "DES", "-").as_deref(),
            Some("DES-rocket-nozzle")
        );
    }

    #[test]
    fn uid_without_expected_prefix_is_none() {
        assert!(reqforge_name_from_uid("XYZ-001", "REQ", "-").is_none());
    }

    #[test]
    fn uid_with_prefix_but_no_nanu_is_none() {
        // Prefix alone isn't an item.
        assert!(reqforge_name_from_uid("REQ-", "REQ", "-").is_none());
        assert!(reqforge_name_from_uid("REQ", "REQ", "").is_none());
    }

    #[test]
    fn empty_sep_treats_rest_as_nanu() {
        assert_eq!(
            reqforge_name_from_uid("REQ001", "REQ", "").as_deref(),
            Some("REQ001")
        );
    }
}
