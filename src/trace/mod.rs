//! Requirement-trace scanning — provreq's native reader for the `Implements:` /
//! `Verifies:` code tags (Phase 4a, #334). It replaces the interim
//! `scripts/traceability.py` (retired in slice g) and adds what that reader cannot do:
//! resolving each tag to the **source symbol it annotates**, so a later slice can run and
//! rate that artifact by name.
//!
//! Two portable stages, neither a per-language parser:
//!   1. [`carve`] extracts comment runs per a per-language comment grammar
//!      ([`languages`]); [`tags`] parses `Implements:`/`Verifies:` tags out of them. Both
//!      are absorbed from ReqForge's proven scanner, adapted to provreq's id grammar
//!      (which does not require a hyphen — `REQ021`, not only `REQ-021`).
//!   2. [`resolve`] forward-scans from a tag to the declaration that follows it, over a
//!      small per-language declaration table — the portable spine that generalises to
//!      other languages by a table entry, not a new parser.
//!
//! The walk is provreq's own ([`crate::subject_tree`] pruning + `WalkDir`), not ReqForge's,
//! so an AppleDouble `._foo.rs` sidecar is never scanned as a second source (#294/#307).
//!
//! Implements: REQ075

pub mod carve;
pub mod languages;
pub mod resolve;
pub mod run;
pub mod tags;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// What a tag claims about the requirement it names. ReqForge's six-verb catalog collapses
/// to the two that speak about *code*: `Implements:` (with its `Requirements:` alias, and
/// ReqForge's own `Satisfies:`) says this source realises the requirement; `Verifies:` says
/// it checks it. The other four verbs are requirement-to-requirement links, never code tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceKind {
    Implements,
    Verifies,
}

impl TraceKind {
    /// The kind a parsed canonical verb maps to, or `None` for the four
    /// requirement-to-requirement verbs that do not belong on code.
    fn from_verb(verb: &str) -> Option<Self> {
        match verb {
            "Satisfies" => Some(TraceKind::Implements),
            "Verifies" => Some(TraceKind::Verifies),
            _ => None,
        }
    }
}

/// One requirement-trace tag found in the subject's source, resolved to the symbol it
/// annotates where one can be found. `req_id` is the id as written; reconciling it against
/// the subject's declared requirements — known vs orphan — is a downstream concern (the
/// report, slice d), not this scan's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub req_id: String,
    pub kind: TraceKind,
    /// Subject-relative path of the file the tag sits in.
    pub file: PathBuf,
    /// 1-based line of the tag.
    pub line: usize,
    /// The declaration the tag annotates — a test/function name a runner can address, or a
    /// test title for `it('…')`/`test('…')`. `None` when no declaration follows (a
    /// module-level `//!` tag, or a tag with no resolvable carrier); the tag still carries
    /// its file and line, and an unresolved tag is never evidence that the requirement holds.
    pub symbol: Option<String>,
}

/// Whether the walk skips this entry: the companion side-store, a pruned directory, or an
/// operating-system resource file. The same rule [`crate::rust_adapter::ParsedSubject::load`]
/// uses, so a code tag and a binding resolve against exactly the same set of files.
fn is_skipped(entry: &DirEntry, companion_root: &Path) -> bool {
    entry.path() == companion_root
        || crate::subject_tree::is_pruned_dir(entry.path(), entry.depth())
        || crate::subject_tree::is_pruned_file(entry.path())
}

/// Scan `subject` for `Implements:`/`Verifies:` tags and resolve each to the symbol it
/// annotates. `companion` is provreq's side-store root, pruned from the walk.
///
/// `known_prefixes` (upper-cased, e.g. `{"REQ"}`) is the set of requirement-id prefixes the
/// subject actually declares. Only tags whose id carries one of these prefixes are kept — the
/// defence against a coincidental `category-1` in prose being read as an id, exactly as the
/// Python reader's `valid_prefixes` did. An empty set keeps nothing. Whether a kept id names
/// a *real* requirement (vs an orphan number) is a downstream concern (the report, slice d).
///
/// Results are ordered by (file, line).
pub fn scan(subject: &Path, companion: &Path, known_prefixes: &BTreeSet<String>) -> Vec<Tag> {
    let mut out: Vec<Tag> = Vec::new();
    for entry in WalkDir::new(subject)
        .into_iter()
        .filter_entry(|e| !is_skipped(e, companion))
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(file_name) = entry.path().file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(language) = languages::language_for(file_name) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let rel = entry
            .path()
            .strip_prefix(subject)
            .unwrap_or(entry.path())
            .to_path_buf();
        let lines: Vec<&str> = text.lines().collect();
        for raw in tags::parse_tags(&carve::extract_comment_runs(&text, language)) {
            let Some(kind) = TraceKind::from_verb(&raw.verb) else {
                continue;
            };
            if !known_prefixes.contains(&tags::id_prefix(&raw.raw_id)) {
                continue;
            }
            out.push(Tag {
                req_id: raw.raw_id,
                kind,
                file: rel.clone(),
                line: raw.line,
                symbol: resolve::symbol_for(&lines, raw.line, language),
            });
        }
    }
    out.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn subject(rel: &str, body: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
        tmp
    }

    fn companion(tmp: &tempfile::TempDir) -> PathBuf {
        tmp.path().join(".provreq-companion")
    }

    fn req_prefixes() -> BTreeSet<String> {
        BTreeSet::from(["REQ".to_owned()])
    }

    // Verifies: REQ075 — provreq's OWN tag layout resolves: a `// Verifies:` line comment,
    // continued, above `#[test]` then the fn, binds to the function name.
    #[test]
    fn verifies_tag_resolves_through_attribute_to_the_fn() {
        let src = "\
mod tests {
    // Verifies: REQ021 — the bindable symbols are exactly the declared
    // predicates (not sorts or the quantifier variable).
    #[test]
    fn bindable_symbols_are_declared_predicates() {}
}
";
        let tmp = subject("src/grounding.rs", src);
        let tags = scan(tmp.path(), &companion(&tmp), &req_prefixes());
        let tag = tags
            .iter()
            .find(|t| t.req_id == "REQ021")
            .expect("the REQ021 tag must be found");
        assert_eq!(tag.kind, TraceKind::Verifies);
        assert_eq!(
            tag.symbol.as_deref(),
            Some("bindable_symbols_are_declared_predicates"),
            "the tag resolves past `#[test]` to the fn it annotates"
        );
    }

    // Verifies: REQ075 — provreq's id grammar has no required hyphen (`REQ021`); the
    // absorbed parser must accept it, or every provreq tag would be dropped.
    #[test]
    fn a_hyphenless_id_is_accepted() {
        let tmp = subject(
            "src/a.rs",
            "// Implements: REQ021\nfn login() -> bool { true }\n",
        );
        let tags = scan(tmp.path(), &companion(&tmp), &req_prefixes());
        assert!(
            tags.iter().any(|t| t.req_id == "REQ021"),
            "a hyphenless id must be scanned: {tags:?}"
        );
    }

    // Verifies: REQ075 — Implements: and its aliases all read as one kind; the four
    // requirement-to-requirement verbs never surface as code tags.
    #[test]
    fn implements_aliases_map_to_one_kind() {
        let tmp = subject(
            "src/a.rs",
            "// Requirements: REQ001\nfn a() {}\n// Satisfies: REQ002\nfn b() {}\n",
        );
        let tags = scan(tmp.path(), &companion(&tmp), &req_prefixes());
        assert!(tags
            .iter()
            .all(|t| t.kind == TraceKind::Implements && t.req_id.starts_with("REQ")));
        assert_eq!(tags.len(), 2, "both alias forms count: {tags:?}");
    }

    // Verifies: REQ075 — a module-level `//!` tag names no carrier, so it resolves to
    // `symbol: None` rather than latching onto the next unrelated item; it is still reported.
    #[test]
    fn a_module_level_tag_resolves_to_no_symbol() {
        let src = "//! Implements: REQ030\n\nuse std::path::Path;\n\nfn helper() {}\n";
        let tmp = subject("src/m.rs", src);
        let tags = scan(tmp.path(), &companion(&tmp), &req_prefixes());
        let tag = tags
            .iter()
            .find(|t| t.req_id == "REQ030")
            .expect("the module tag is reported");
        assert_eq!(
            tag.symbol, None,
            "a module-doc tag names no carrier: {tag:?}"
        );
    }

    // Verifies: REQ075 — the walk is provreq's own: an AppleDouble `._*.rs` sidecar is NOT
    // scanned as a second source (#294/#307), unlike ReqForge's extension-only matcher.
    #[test]
    fn an_appledouble_sidecar_is_not_scanned() {
        let tmp = subject("src/a.rs", "// Verifies: REQ021\nfn a() {}\n");
        fs::write(
            tmp.path().join("src/._a.rs"),
            "// Verifies: REQ999\nfn a() {}\n",
        )
        .unwrap();
        let tags = scan(tmp.path(), &companion(&tmp), &req_prefixes());
        assert!(
            tags.iter().all(|t| t.req_id != "REQ999"),
            "the sidecar must not be scanned: {tags:?}"
        );
        assert!(tags.iter().any(|t| t.req_id == "REQ021"));
    }

    // Verifies: REQ075 — provreq's real module-doc form: comma-separated ids each trailed by
    // parenthetical prose that itself contains id-shaped noise (`category-1`). The prefix
    // filter keeps only the REQ ids and never the prose token.
    #[test]
    fn parenthetical_prose_ids_are_filtered_by_prefix() {
        let src = "//! Implements: REQ021 (grounding schema + category-1 dry-run), REQ025 (a cat-1 binding)\n\nfn f() {}\n";
        let tmp = subject("src/g.rs", src);
        let tags = scan(tmp.path(), &companion(&tmp), &req_prefixes());
        let ids: Vec<&str> = tags.iter().map(|t| t.req_id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["REQ021", "REQ025"],
            "only the REQ ids survive: {ids:?}"
        );
    }
}
