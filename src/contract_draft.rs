//! The A6 **proof-carrier draft channel** (Slice B2): when a grounded category-1 predicate
//! resolves to an *opaque* function — an ordinary `fn`, not a deductive marker — Creusot and
//! Prusti cannot see inside it and honestly report `inconclusive` (REQ032). This module drafts
//! the missing marker (`#[logic]` for Creusot, `#[pure]` for Prusti) so the operator can stage it.
//!
//! It is the one row of the A6 annotation table whose target is the *subject's source* rather than
//! the requirement item or the companion tree: "proof carriers → subject source → tool proposes
//! patch → human applies → the verifier reads it directly". The tool's write surface stops at the
//! subject working tree — it stages an uncommitted edit and never runs git (A6, D12): the draft is
//! a *proposal* the operator reviews and the verifier re-checks, never claimed correct.
//!
//! Scope is **marker-only**: it adds the annotation the compile-error already names as missing, a
//! deterministic transform over the grounding's own resolutions (each carries the fn's `file:line`),
//! so no engine is run and the whole thing is CI-testable without a verifier — the same discipline
//! as [`crate::lowering`]. Drafting semantic `#[requires]`/`#[ensures]` is a later slice.
//!
//! Implements: REQ033 (draft the missing deductive marker onto opaque predicate fns).

use crate::rust_adapter::Resolution;
use std::collections::BTreeMap;

/// The deductive transparency marker a subject's verifier reads. A subject depends on exactly one
/// contracts crate, so exactly one marker applies — [`marker_for_subject`] picks it from the
/// subject's manifest rather than guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// Creusot's logical-function marker (subject depends on `creusot-contracts`/`creusot-std`).
    Logic,
    /// Prusti's pure-function marker (subject depends on `prusti-contracts`).
    Pure,
}

impl Marker {
    /// The attribute line this marker draws, bare (no indentation).
    pub fn attribute(self) -> &'static str {
        match self {
            Marker::Logic => "#[logic]",
            Marker::Pure => "#[pure]",
        }
    }
}

/// One staged edit: insert `attribute` on its own line directly above the predicate fn at
/// `file:line`. Line is 1-based, matching [`crate::rust_adapter::CodeMatch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerDraft {
    /// Subject-relative path of the file holding the predicate fn.
    pub file: String,
    /// 1-based line of the fn signature the marker goes above.
    pub line: usize,
    /// The attribute to insert (already the right one for the subject), bare of indentation.
    pub attribute: String,
}

/// Which deductive marker the subject is set up for, read from its `Cargo.toml`. A subject that
/// depends on neither deductive contracts crate has nothing to be made transparent *for* — that is
/// REQ032's honest missing-dependency inconclusive, and this returns `None` so no draft is offered.
pub fn marker_for_subject(cargo_toml: &str) -> Option<Marker> {
    // Crude but sufficient: a dependency key appears as `name = ...` or `name.workspace = ...` at a
    // line start (inside `[dependencies]`). We do not parse the manifest — presence of the crate
    // name as a dependency key is all the signal needed, and a false hit only offers a marker the
    // operator can decline. ponytail: substring-on-key, tighten to a TOML parse if a crate name
    // ever collides with a comment or string.
    let names_a_dep = |crate_name: &str| {
        cargo_toml.lines().any(|l| {
            let l = l.trim_start();
            l.starts_with(crate_name)
                && l[crate_name.len()..]
                    .trim_start()
                    .starts_with(['=', '.', '{'])
        })
    };
    if names_a_dep("creusot-contracts") || names_a_dep("creusot-std") {
        Some(Marker::Logic)
    } else if names_a_dep("prusti-contracts") {
        Some(Marker::Pure)
    } else {
        None
    }
}

/// Plan the marker inserts for a requirement's resolved predicates. `sources` maps each subject
/// file (as it appears in a [`Resolution::Resolved`]'s `at.file`) to its full text. A predicate is
/// drafted only when it resolved to a real fn (an unresolved binding has nothing to annotate) and
/// that fn does not already carry the marker. Two predicates that resolve to the same fn yield one
/// draft — deduped by `(file, line)`.
pub fn plan_markers(
    resolutions: &BTreeMap<String, Resolution>,
    marker: Marker,
    sources: &BTreeMap<String, String>,
) -> Vec<MarkerDraft> {
    let mut seen = std::collections::BTreeSet::new();
    let mut drafts = Vec::new();
    for res in resolutions.values() {
        let Resolution::Resolved { at, .. } = res else {
            continue;
        };
        if !seen.insert((at.file.clone(), at.line)) {
            continue;
        }
        let already = sources
            .get(&at.file)
            .is_some_and(|src| already_marked(src, at.line, marker));
        if already {
            continue;
        }
        drafts.push(MarkerDraft {
            file: at.file.clone(),
            line: at.line,
            attribute: marker.attribute().to_string(),
        });
    }
    drafts
}

/// Whether the fn whose signature is on `line` (1-based) already carries `marker`. Scans the
/// contiguous run of attribute / doc-comment / blank lines directly above the signature — the only
/// place a Rust attribute for that item can sit — and looks for the marker token there.
fn already_marked(src: &str, line: usize, marker: Marker) -> bool {
    let lines: Vec<&str> = src.lines().collect();
    if line == 0 || line > lines.len() {
        return false;
    }
    let token = marker.attribute();
    // Walk upward from the line above the signature while we are still in the item's attribute run.
    for idx in (0..line - 1).rev() {
        let t = lines[idx].trim();
        if t.is_empty() || t.starts_with("///") || t.starts_with("//!") || t.starts_with("//") {
            continue;
        }
        if t.starts_with("#[") || t.starts_with("#![") {
            if t.contains(token.trim_start_matches("#[").trim_end_matches(']')) {
                return true;
            }
            continue;
        }
        // Any other code line ends the item's attribute run.
        break;
    }
    false
}

/// Apply this file's marker inserts to its source, returning the new text. Inserts run
/// bottom-up (highest line first) so an earlier insert never shifts a later target's line number.
/// Each attribute copies the fn line's own indentation so the patch reads like hand-written code.
pub fn apply_to_source(src: &str, drafts: &[MarkerDraft]) -> String {
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let mut sorted: Vec<&MarkerDraft> = drafts.iter().collect();
    sorted.sort_by_key(|d| std::cmp::Reverse(d.line));
    for d in sorted {
        if d.line == 0 || d.line > lines.len() {
            continue;
        }
        let indent: String = lines[d.line - 1]
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        lines.insert(d.line - 1, format!("{indent}{}", d.attribute));
    }
    let mut out = lines.join("\n");
    // Preserve a trailing newline if the original had one (`lines()` drops it).
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rust_adapter::{CodeMatch, ParamMode};

    fn resolved(file: &str, line: usize) -> Resolution {
        Resolution::Resolved {
            at: CodeMatch {
                file: file.to_string(),
                line,
                text: "fn p(u: &User) -> bool {".to_string(),
            },
            params: vec![ParamMode::ByRef],
        }
    }

    // Verifies: REQ033 — the marker is chosen from the subject's declared contracts crate, and a
    // subject depending on neither offers no draft (that stays REQ032's missing-dependency path).
    #[test]
    fn marker_follows_the_subjects_contracts_crate() {
        assert_eq!(
            marker_for_subject("[dependencies]\ncreusot-contracts = \"0.6\"\n"),
            Some(Marker::Logic)
        );
        assert_eq!(
            marker_for_subject("[dependencies]\nprusti-contracts = { version = \"0.2\" }\n"),
            Some(Marker::Pure)
        );
        assert_eq!(marker_for_subject("[dependencies]\nserde = \"1\"\n"), None);
    }

    // Verifies: REQ033 — a substring of a dep name is not a dependency (`creusot-std-helper` must
    // not read as `creusot-std`), and an unrelated crate does not trip a marker.
    #[test]
    fn marker_needs_the_dep_key_not_a_substring() {
        assert_eq!(
            marker_for_subject("[dependencies]\nnot-prusti-contracts-x = \"1\"\n"),
            None
        );
        assert_eq!(
            marker_for_subject("[dependencies]\ncreusot-std = \"0.6\"\n"),
            Some(Marker::Logic)
        );
    }

    // Verifies: REQ033 — an unmarked resolved predicate is drafted; an unresolved binding is not
    // (there is no fn to annotate).
    #[test]
    fn drafts_unmarked_resolved_predicates_only() {
        let mut res = BTreeMap::new();
        res.insert("logged_in".to_string(), resolved("src/lib.rs", 10));
        res.insert("missing".to_string(), Resolution::NotFound);
        let sources = BTreeMap::from([(
            "src/lib.rs".to_string(),
            "fn logged_in(u: &User) -> bool { true }\n".to_string(),
        )]);
        let drafts = plan_markers(&res, Marker::Logic, &sources);
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].file, "src/lib.rs");
        assert_eq!(drafts[0].line, 10);
        assert_eq!(drafts[0].attribute, "#[logic]");
    }

    // Verifies: REQ033 — a predicate that already carries the marker is not re-drafted (idempotent),
    // and two predicates resolving to the same fn yield a single draft.
    #[test]
    fn skips_already_marked_and_dedups_same_fn() {
        // Two source lines above the fn: a doc comment then the marker.
        let src = "/// a predicate\n#[logic]\nfn ok(u: &User) -> bool { true }\n";
        let mut res = BTreeMap::new();
        res.insert("a".to_string(), resolved("src/lib.rs", 3));
        res.insert("b".to_string(), resolved("src/lib.rs", 3)); // same fn, different symbol
        let sources = BTreeMap::from([("src/lib.rs".to_string(), src.to_string())]);
        assert!(plan_markers(&res, Marker::Logic, &sources).is_empty());

        // Same two symbols, but the fn is unmarked → exactly one draft, not two.
        let bare = BTreeMap::from([(
            "src/lib.rs".to_string(),
            "fn ok(u: &User) -> bool { true }\n".to_string(),
        )]);
        let mut same = BTreeMap::new();
        same.insert("a".to_string(), resolved("src/lib.rs", 1));
        same.insert("b".to_string(), resolved("src/lib.rs", 1));
        assert_eq!(plan_markers(&same, Marker::Logic, &bare).len(), 1);
    }

    // Verifies: REQ033 — the staged edit inserts the marker with the fn's own indentation, directly
    // above the signature, and multiple inserts in one file do not corrupt each other's line targets.
    #[test]
    fn applies_inserts_bottom_up_with_indentation() {
        let src = "mod m {\n    fn a() -> bool { true }\n    fn b() -> bool { false }\n}\n";
        let drafts = vec![
            MarkerDraft {
                file: "x".into(),
                line: 2,
                attribute: "#[logic]".into(),
            },
            MarkerDraft {
                file: "x".into(),
                line: 3,
                attribute: "#[logic]".into(),
            },
        ];
        let out = apply_to_source(src, &drafts);
        assert_eq!(
            out,
            "mod m {\n    #[logic]\n    fn a() -> bool { true }\n    #[logic]\n    fn b() -> bool { false }\n}\n"
        );
        // The applied source now reads as already-marked to the planner (round-trip / idempotence).
        assert!(already_marked(&out, 3, Marker::Logic));
    }
}
