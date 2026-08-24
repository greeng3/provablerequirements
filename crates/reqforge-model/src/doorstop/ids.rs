//! Doorstop-UID → ReqForge-name normalisation per
//! INTEROP-doorstopIdNormalization.
//!
//! Rules applied here:
//!   - The NANU portion — everything after the prefix + sep —
//!     keeps its numeric padding (so `REQ001` → `REQ-001`, not
//!     `REQ-1`).
//!   - If the NANU contains `-` characters (possible when the
//!     doorstop doc's `sep` is `-` and the NANU is multi-word
//!     like `DES-rocket-nozzle`), every `-` in the NANU is
//!     replaced with `_` on import.
//!   - The original doorstop UID is preserved verbatim by the
//!     caller under `legacy.doorstopUid` — this module only
//!     computes names.
//!
//! This module is intentionally pure; the callers decide what
//! to do with the normalised name.

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

/// Normalise a doorstop NANU into the ReqForge artifact name
/// component (the part after `<prefix>-`). The ReqForge UID
/// itself is built by the caller as `format!("{prefix}-{name}")`
/// using the standard `-` separator regardless of the doorstop
/// document's `sep`.
pub fn normalize_item_name(nanu: &str) -> String {
    nanu.replace('-', "_")
}

/// Build the full ReqForge artifact name for an imported
/// doorstop item. Returns `None` when the UID doesn't parse
/// (caller emits a warning in the import report).
pub fn reqforge_name_from_uid(uid: &str, prefix: &str, sep: &str) -> Option<String> {
    let nanu = parse_doorstop_uid(uid, prefix, sep)?;
    if nanu.is_empty() {
        return None;
    }
    Some(format!("{prefix}-{}", normalize_item_name(nanu)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padded_numeric_nanu_survives() {
        assert_eq!(
            reqforge_name_from_uid("REQ001", "REQ", "").as_deref(),
            Some("REQ-001")
        );
        assert_eq!(
            reqforge_name_from_uid("REQ-001", "REQ", "-").as_deref(),
            Some("REQ-001")
        );
    }

    #[test]
    fn dashes_in_multi_word_nanu_become_underscores() {
        assert_eq!(
            reqforge_name_from_uid("DES-rocket-nozzle", "DES", "-").as_deref(),
            Some("DES-rocket_nozzle")
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
            Some("REQ-001")
        );
    }

    #[test]
    fn normalize_item_name_preserves_underscores_and_alphanums() {
        assert_eq!(normalize_item_name("already_snake"), "already_snake");
        assert_eq!(normalize_item_name("abc123"), "abc123");
        assert_eq!(normalize_item_name("a-b-c"), "a_b_c");
    }
}
