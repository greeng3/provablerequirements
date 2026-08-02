//! The A6 **proof-carrier draft channel** (Slice B2): when a grounded category-1 predicate
//! resolves to an *opaque* function — an ordinary `fn`, not a deductive marker — Creusot and
//! Prusti cannot see inside it and honestly report `inconclusive` (REQ032). This module drafts
//! the missing marker so the operator can stage it.
//!
//! The channel is **Prusti-only** ([`Marker::drafts_markers`], #158): `#[pure]` makes a function
//! transparent in place, while `#[logic]` would remove it from the program and break every call
//! site. Creusot's route to the same predicate is a mirror ([`crate::mirror_draft`]).
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

    /// Whether **semantic contracts** (`#[requires]`/`#[ensures]` drafted from the requirement) are
    /// part of this dialect's route to a proof. They are for Prusti and are not for Creusot (#164).
    ///
    /// Creusot reaches an ordinary program function through a `#[logic]` **mirror**
    /// ([`crate::mirror_draft`]), and once it does, a drafted contract clause is all risk:
    ///
    /// * It cannot help. The harness is a `proof_assert!` over the mirrors and calls no program
    ///   function, so an `#[ensures]` on one is discharged where nothing reads it.
    /// * It can produce a **false `proven`**. The mirror's linking `#[ensures]` is discharged
    ///   *assuming the function's preconditions*, so a drafted `#[requires]` narrows the domain on
    ///   which the mirror was ever checked while the harness's `forall` ranges over all of it.
    ///   Measured against the real prover in
    ///   `crate::creusot::tests::a_precondition_on_a_mirrored_function_can_prove_something_false`.
    ///
    /// Prusti has no such exposure: its `#[pure]` functions are spec-callable, so there are no
    /// mirrors and no links for a precondition to weaken. Contracts are its whole mechanism.
    pub fn drafts_contracts(self) -> bool {
        match self {
            Marker::Logic => false,
            Marker::Pure => true,
        }
    }

    /// Whether **drafting this marker onto the subject's own predicate function** is a route to a
    /// proof in this dialect. It is for Prusti and is not for Creusot (#158).
    ///
    /// The premise of the marker channel is that the marker makes an ordinary function transparent
    /// *in place*. Prusti's `#[pure]` does exactly that: the function stays in the program, keeps
    /// its callers, and becomes callable from a specification as well.
    ///
    /// `#[logic]` does not. It declares a **logical** function, which moves the item out of the
    /// program namespace — so every real call site stops compiling. Measured on this repo:
    /// `#[logic]` on `provision::decide_install` and `EngineStatus::is_ready` gave `E0425`/`E0599`
    /// at six call sites. That is not a prover bug; it is what `#[logic]` means, and it applies to
    /// exactly the case a category-1 predicate normally resolves to — an ordinary function the
    /// subject calls.
    ///
    /// Creusot reaches such a function through a `#[logic]` **mirror** instead
    /// ([`crate::mirror_draft`], REQ068): the program function is left alone and gains only a
    /// linking postcondition. So there is nothing left for the marker channel to do there, and
    /// staging one would break the subject's build before any prover ran.
    pub fn drafts_markers(self) -> bool {
        match self {
            Marker::Logic => false,
            Marker::Pure => true,
        }
    }

    /// The `use` line that brings this dialect's attributes into scope.
    ///
    /// Creusot's own guide says to glob `creusot_std::prelude::*`, and that is right for a file
    /// **written** for Creusot — but staging imports into a file the subject already had, and that
    /// prelude deliberately shadows `vec!`, `Clone`, `PartialEq` and `Default` with Creusot's
    /// versions. Its source says so outright: rustc "will either shadow the old identifier or
    /// complain about the ambiguity". Measured — staging into `src/engine.rs`, which uses `vec!`
    /// eleven times, gave `error[E0659]: 'vec' is ambiguous` and no prover ever ran. So import the
    /// **macros** module instead: it carries every attribute provreq stages (`requires`, `ensures`,
    /// `logic`, `pearlite`, `proof_assert`) and shadows nothing the subject already uses.
    pub fn prelude_import(self) -> &'static str {
        match self {
            Marker::Logic => "use creusot_std::macros::*;",
            Marker::Pure => "use prusti_contracts::*;",
        }
    }
}

/// Ensure `src` imports the dialect's prelude, returning the text unchanged when it already does.
///
/// Staging a contract into a file writes `#[requires]`/`#[ensures]`/`#[logic]`/`pearlite!` into it,
/// and none of those are in scope unless the file imports them. Measured: staging into
/// `src/engine.rs`, which names `creusot_std` nowhere, failed with *cannot find attribute `ensures`
/// in this scope* — a staged edit that cannot compile, before any prover saw the claim. The subject
/// having the dependency is not enough; each *file* needs the import.
///
/// Placement is the fiddly part and is why this is a function rather than a `push_str`. A `use` is
/// an item, and Rust requires inner attributes (`#![…]`) and inner doc comments (`//!`) to precede
/// every item in a file — so prepending blindly turns a documented module into a syntax error. The
/// import therefore goes after that leading block, which is also where a human would put it.
pub fn ensure_prelude(src: &str, marker: Marker) -> String {
    let import = marker.prelude_import();
    if src.lines().any(|l| l.trim() == import) {
        return src.to_string();
    }
    let lines: Vec<&str> = src.lines().collect();
    // Skip the leading run of inner attributes, inner docs, blank lines and ordinary comments —
    // everything that may (or must) precede the first item.
    //
    // An **outer** doc comment (`///`) stops the scan even though it also starts with `//`: it
    // belongs to the item beneath it, so stepping past it silently re-attaches the operator's
    // documentation to provreq's import. Measured on a live subject, whose `Status` enum lost its
    // own doc comment to the `use` line inserted under it.
    let at = lines
        .iter()
        .position(|l| {
            let t = l.trim_start();
            let skippable = t.is_empty()
                || t.starts_with("//!")
                || t.starts_with("#![")
                || (t.starts_with("//") && !t.starts_with("///"))
                || t.starts_with("/*");
            !skippable
        })
        .unwrap_or(lines.len());
    let mut out: Vec<String> = lines[..at].iter().map(|s| s.to_string()).collect();
    out.push(import.to_string());
    out.extend(lines[at..].iter().map(|s| s.to_string()));
    let mut text = out.join("\n");
    if src.ends_with('\n') || src.is_empty() {
        text.push('\n');
    }
    text
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
    use crate::rust_adapter::{CodeMatch, ParamMode, PredicateForm};

    // Verifies: a file that does not import the dialect's prelude gets the import. Measured
    // failure this guards: staging into `src/engine.rs` (which names `creusot_std` nowhere) gave
    // `cannot find attribute 'ensures' in this scope` — a staged edit that cannot compile.
    #[test]
    fn staging_into_a_file_without_the_prelude_adds_the_import() {
        let src = "pub struct S;\n";
        let out = ensure_prelude(src, Marker::Logic);
        assert_eq!(out, "use creusot_std::macros::*;\npub struct S;\n");
    }

    // Verifies: the import lands AFTER inner docs and inner attributes. Rust requires those to
    // precede every item, and a `use` is an item — prepending blindly turns a documented module
    // into a syntax error, which is precisely the file shape this codebase uses everywhere.
    #[test]
    fn the_import_goes_after_inner_docs_and_attributes_not_before_them() {
        let src = "//! Module docs.\n//! More docs.\n#![allow(unused)]\n\npub struct S;\n";
        let out = ensure_prelude(src, Marker::Logic);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "//! Module docs.");
        assert_eq!(lines[2], "#![allow(unused)]");
        assert_eq!(
            lines[4], "use creusot_std::macros::*;",
            "the import follows the leading block: {out}"
        );
        assert_eq!(lines[5], "pub struct S;");
    }

    // Verifies: an existing import is not duplicated — staging runs fresh from the original source
    // on every repair round, so a duplicate would accumulate one per round.
    #[test]
    fn an_existing_prelude_import_is_not_duplicated() {
        let src = "//! Docs.\nuse creusot_std::macros::*;\npub struct S;\n";
        assert_eq!(ensure_prelude(src, Marker::Logic), src);
    }

    // Verifies (#164): contracts are a Prusti-only channel. On the Creusot route a drafted
    // `#[requires]` on a mirrored function weakens the very link that makes a mirror trustworthy —
    // measured as a real `Holds` for a claim that is false of the program, in
    // `crate::creusot::tests::a_precondition_on_a_mirrored_function_can_prove_something_false`.
    // The rule lives on the dialect because that is what decides it, not a flag the operator sets.
    #[test]
    fn contracts_are_drafted_for_prusti_and_never_for_creusot() {
        assert!(
            !Marker::Logic.drafts_contracts(),
            "Creusot reaches a program function through a mirror; a drafted clause cannot help it \
             and a drafted precondition can prove something false"
        );
        assert!(
            Marker::Pure.drafts_contracts(),
            "Prusti's `#[pure]` fns are spec-callable, so contracts ARE the mechanism there"
        );
    }

    // Verifies (#158): the marker channel is Prusti-only. `#[logic]` declares a LOGICAL function, so
    // staging it onto the program function a predicate resolved to takes that item out of the
    // program namespace — measured on this repo as `E0425`/`E0599` at six call sites, a working
    // tree that compiles in no configuration at all. Creusot's route is a mirror (REQ068), which
    // leaves the program function alone.
    #[test]
    fn markers_are_drafted_for_prusti_and_never_for_creusot() {
        assert!(
            !Marker::Logic.drafts_markers(),
            "`#[logic]` on a called program function breaks every call site; Creusot's route to it \
             is a mirror"
        );
        assert!(
            Marker::Pure.drafts_markers(),
            "`#[pure]` makes a function transparent in place — callers and body both survive"
        );
    }

    // Verifies (#158): a staged marker arrives with the attribute in scope. The channel wrote a bare
    // `#[pure]` into a file that imports nothing, giving `cannot find attribute` — a proposal that
    // cannot parse is not reviewable. Order matters: the markers go in first (bottom-up), then the
    // import, which adds a line above them all.
    #[test]
    fn a_staged_marker_arrives_with_its_attribute_in_scope() {
        let src = "//! Docs.\n\npub fn ready(n: u32) -> bool {\n    n > 0\n}\n";
        let drafts = [MarkerDraft {
            file: "src/lib.rs".into(),
            line: 3,
            attribute: Marker::Pure.attribute().to_string(),
        }];
        let out = ensure_prelude(&apply_to_source(src, &drafts), Marker::Pure);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "//! Docs.");
        assert_eq!(lines[2], "use prusti_contracts::*;");
        assert_eq!(
            lines[3], "#[pure]",
            "the marker still sits above its fn: {out}"
        );
        assert_eq!(lines[4], "pub fn ready(n: u32) -> bool {");
    }

    // Verifies: the import goes BEFORE an outer doc comment, not between it and its item. `///`
    // also starts with `//`, so the ordinary-comment skip stepped past it — measured on a live
    // subject, where a documented `pub enum Status` had provreq's `use` line inserted between the
    // doc comment and the enum, silently re-attaching the operator's documentation to the import.
    #[test]
    fn the_import_does_not_separate_an_outer_doc_comment_from_its_item() {
        let src = "/// Whether the engine is present.\npub enum Status { Ready }\n";
        let out = ensure_prelude(src, Marker::Logic);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "use creusot_std::macros::*;");
        assert_eq!(lines[1], "/// Whether the engine is present.");
        assert_eq!(lines[2], "pub enum Status { Ready }");
    }

    // Verifies: an ordinary leading comment is still skipped past — a licence header is attached to
    // nothing, so the import belongs after it, where a human would put it.
    #[test]
    fn an_ordinary_leading_comment_is_still_skipped() {
        let src = "// SPDX-License-Identifier: MIT\n\npub struct S;\n";
        let out = ensure_prelude(src, Marker::Logic);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "// SPDX-License-Identifier: MIT");
        assert_eq!(lines[2], "use creusot_std::macros::*;");
    }

    // Verifies: the Creusot import does NOT glob the prelude. That prelude deliberately shadows
    // `vec!`, `Clone`, `PartialEq` and `Default`, so globbing it into a subject file that already
    // uses any of them is `error[E0659]: 'vec' is ambiguous` — measured on `src/engine.rs`, which
    // killed a live run before the prover was reached.
    #[test]
    fn the_creusot_import_does_not_glob_the_shadowing_prelude() {
        let out = ensure_prelude("pub struct S;\n", Marker::Logic);
        assert!(
            !out.contains("prelude::*"),
            "the prelude shadows std names the subject already uses: {out}"
        );
        assert!(out.contains("macros::*"), "attributes still come in: {out}");
    }

    // Verifies: the import follows the dialect, never a hardcoded Creusot one.
    #[test]
    fn the_import_follows_the_subjects_dialect() {
        assert!(
            ensure_prelude("pub struct S;\n", Marker::Pure).starts_with("use prusti_contracts::*;")
        );
    }

    fn resolved(file: &str, line: usize) -> Resolution {
        Resolution::Resolved {
            at: CodeMatch {
                file: file.to_string(),
                line,
                text: "fn p(u: &User) -> bool {".to_string(),
                module: Some(vec![]),
            },
            params: vec![ParamMode::ByRef],
            form: PredicateForm::Function,
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
