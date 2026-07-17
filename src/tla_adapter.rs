//! The category-2a adapter: resolve a PRL vocabulary symbol to **a definition in the
//! subject's TLA+ spec** — the shape the design's Adapters list requires for the model world
//! ("2a (model) a direct model variable/action reference"), and the shape a model checker
//! can consume.
//!
//! The observable world here is a **model**, not the subject's own code: category 1 resolves
//! against the subject's Rust ([`crate::rust_adapter`]); category 2a resolves against a TLA+
//! spec the operator wrote to model the system. Both are per-observable-world adapters
//! (R-eng-4); this one owns TLA+, and [`crate::grounding`] owns the category-independent
//! schema and verdict.
//!
//! **One resolver, because TLA+ has one kind of name.** Category 1 keeps predicates
//! (→ functions) and sorts (→ types) apart, because Rust makes them syntactically distinct
//! and a `struct login` must never satisfy the predicate `login`. TLA+ draws no such
//! line: an action `Accept(m) == …`, a state operator `Succeeded(m) == …`, a data set
//! `Message == 1..N`, a `VARIABLE status`, and a `CONSTANT MaxLen` are all just *named
//! definitions*. So a 2a binding resolves by one question — does the spec define this name? —
//! which is both smaller than cat-1's split and more faithful to the language.
//!
//! **Structural extraction, not SANY.** There is no TLA+ parser crate the way `syn` parses
//! Rust, so this reads the definitions a spec declares (`VARIABLES`/`CONSTANTS` declarations
//! and top-level operator definitions) structurally. That limit is real — a name introduced
//! by `LET`/`INSTANCE`, or a multi-line declaration, is not seen — and [`ModelResolution::describe`]
//! states it in the operator's read-back rather than letting a resolved binding imply more
//! than was checked, exactly as the Rust adapter is honest that `syn` sees no types.
//!
//! **Existence only.** Whether the definition has the right arity or the right shape is the
//! engine's question (as instantiability was for cat-1 sorts, REQ026) and is deferred; a
//! binding here confirms the model element the requirement names actually exists.
//!
//! Implements: REQ028 (a cat-2a binding resolves to a definition in a TLA+ spec).

use std::path::Path;
use walkdir::WalkDir;

/// Where a definition lives in the subject's model: file (relative to the subject root),
/// 1-based line, and that line's own text — so the operator confirms against the real spec
/// rather than a definition this tool reconstructed. Peer of [`crate::rust_adapter::CodeMatch`];
/// kept separate so the two adapters stay independent (a third observable world earns a
/// shared type, not before).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecMatch {
    pub file: String,
    pub line: usize,
    pub text: String,
}

/// What resolving one cat-2a binding against the subject's TLA+ found. Fewer variants than
/// [`crate::rust_adapter::Resolution`] on purpose: arity and return-shape are Rust-type
/// questions that do not arise for a bare TLA+ name, so an enum carrying them would misstate
/// the state space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelResolution {
    /// Exactly one definition of that name in the subject's TLA+. The only variant that
    /// grounds.
    Resolved(SpecMatch),
    /// No definition of that name anywhere in the subject's TLA+.
    NotFound,
    /// Several definitions share the name. Never guessed between — the operator must
    /// disambiguate, because picking one silently would bind the requirement to whichever
    /// spec was walked first.
    Ambiguous(Vec<SpecMatch>),
}

impl ModelResolution {
    /// Whether this binding resolved — the single question [`crate::grounding::verdict`]
    /// asks. Only [`ModelResolution::Resolved`] grounds; everything else parks the
    /// requirement (R-ground-1).
    pub fn is_resolved(&self) -> bool {
        matches!(self, ModelResolution::Resolved(_))
    }

    /// The operator-facing read-back for one binding (D13: "here is what your binding
    /// resolves to — is that what you meant?"). A resolved definition names the limit of
    /// what was checked, so a green line never implies more than a structural existence
    /// check.
    pub fn describe(&self, symbol: &str, observable: &str) -> String {
        match self {
            ModelResolution::Resolved(at) => format!(
                "{symbol} → `{observable}` resolves to {}:{}  {}\n      (existence only — a \
                 structural read of the spec, so arity/shape and names introduced by \
                 LET/INSTANCE are not checked here)",
                at.file, at.line, at.text
            ),
            ModelResolution::NotFound => format!(
                "{symbol}: no definition `{observable}` in the subject's TLA+ — the model \
                 does not name it, so nothing observes it"
            ),
            ModelResolution::Ambiguous(ats) => {
                let places = ats
                    .iter()
                    .map(|a| format!("{}:{}", a.file, a.line))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{symbol}: `{observable}` is ambiguous — {} definitions share the name \
                     ({places}); qualify it, because binding to one silently would pick \
                     whichever spec was walked first",
                    ats.len()
                )
            }
        }
    }
}

/// Resolve a PRL symbol to a definition named `observable` in the subject's TLA+ (REQ028).
/// Read-only over the subject and recomputed live — the model moves under a binding exactly
/// as code and prose do, so a resolution is never stored.
pub fn resolve(subject_root: &Path, companion_root: &Path, observable: &str) -> ModelResolution {
    let name = observable.trim();
    if name.is_empty() {
        return ModelResolution::NotFound;
    }
    let found = find_definitions(subject_root, companion_root, name);
    match found.len() {
        0 => ModelResolution::NotFound,
        1 => ModelResolution::Resolved(found.into_iter().next().expect("len checked")),
        _ => ModelResolution::Ambiguous(found),
    }
}

/// Whether a directory is pruned from the walk: the VCS metadata dir, or the companion tree
/// (whose own files could hold a spurious self-hit). Mirrors the Rust adapter's skip rule so
/// the two observable worlds agree on which of the subject's files count.
fn is_skipped_dir(path: &Path, companion_root: &Path) -> bool {
    if path == companion_root {
        return true;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == ".git")
        .unwrap_or(false)
}

/// Every TLA+ definition named `name` across the subject's `.tla` files.
fn find_definitions(subject_root: &Path, companion_root: &Path, name: &str) -> Vec<SpecMatch> {
    let mut out = Vec::new();
    for entry in WalkDir::new(subject_root)
        .into_iter()
        .filter_entry(|e| !is_skipped_dir(e.path(), companion_root))
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|x| x != "tla") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let rel = entry
            .path()
            .strip_prefix(subject_root)
            .unwrap_or(entry.path())
            .display()
            .to_string();
        collect_definitions(&text, name, &rel, &mut out);
    }
    out
}

/// Scan one spec's text for a definition named `name`. Comments are stripped first so a name
/// mentioned in prose is never mistaken for a definition.
fn collect_definitions(text: &str, name: &str, rel: &str, out: &mut Vec<SpecMatch>) {
    for (idx, raw) in text.lines().enumerate() {
        let line = strip_comment(raw);
        if defines_name(line, name) {
            out.push(SpecMatch {
                file: rel.to_string(),
                line: idx + 1,
                text: raw.trim().to_string(),
            });
        }
    }
}

/// Drop a `\*` line comment, so `x == 1  \* not Accept` is read as `x == 1`. Block comments
/// `(* … *)` are a documented gap (see the module docs); a name buried only inside one is not
/// resolved, which errs toward NotFound, never toward a false resolve.
fn strip_comment(line: &str) -> &str {
    match line.find("\\*") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Whether `line` declares or defines `name`: a `VARIABLE(S)`/`CONSTANT(S)` entry, or an
/// operator definition `name == …` / `name(args) == …` / `name[x \in S] == …`.
fn defines_name(line: &str, name: &str) -> bool {
    let line = line.trim_start();
    if let Some(rest) = declaration_names(line) {
        return rest.split(',').any(|tok| identifier(tok) == Some(name));
    }
    operator_name(line) == Some(name)
}

/// The declared names after a `VARIABLE(S)`/`CONSTANT(S)` keyword, as raw comma-separated
/// text; `None` when the line is not such a declaration.
fn declaration_names(line: &str) -> Option<&str> {
    for kw in ["VARIABLES", "VARIABLE", "CONSTANTS", "CONSTANT"] {
        if let Some(rest) = line.strip_prefix(kw) {
            // The keyword must be a whole word, not a prefix of a longer identifier.
            if rest.starts_with(|c: char| c.is_whitespace()) {
                return Some(rest);
            }
        }
    }
    None
}

/// The operator name a `name … ==` definition introduces, or `None` when the line is not an
/// operator definition. Handles the plain, applied (`(args)`), and function (`[x \in S]`)
/// forms; an infix-operator definition (`a \oplus b == …`) is a documented gap.
fn operator_name(line: &str) -> Option<&str> {
    let (head, _) = line.split_once("==")?;
    // Everything left of `==`, minus an argument list or function-domain suffix, must be a
    // single identifier — otherwise it is an expression that merely contains `==`, not a
    // definition.
    let head = head.trim();
    let name_part = head
        .split_once('(')
        .map(|(n, _)| n)
        .or_else(|| head.split_once('[').map(|(n, _)| n))
        .unwrap_or(head)
        .trim();
    identifier(name_part)
}

/// `Some(tok)` when the trimmed token is exactly one TLA+ identifier (a letter followed by
/// letters/digits/underscores), else `None`. This is what keeps `x + y` or `Foo.bar` from
/// reading as a name.
fn identifier(tok: &str) -> Option<&str> {
    let tok = tok.trim();
    let mut chars = tok.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if tok.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some(tok)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A subject tree with `spec.tla` holding `src`, plus a companion dir the walk skips.
    fn subject(src: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("spec.tla"), src).unwrap();
        tmp
    }

    fn resolve_in(tmp: &tempfile::TempDir, observable: &str) -> ModelResolution {
        resolve(
            tmp.path(),
            &tmp.path().join("ProvableRequirements"),
            observable,
        )
    }

    const SPEC: &str = "\
---- MODULE Msg ----
EXTENDS Naturals
CONSTANT MaxLen
VARIABLES queue, status

Accept(m) == queue' = Append(queue, m)
Succeeded(m) == status[m] = \"Succeeded\"
Message == 1..MaxLen
Init == queue = <<>>
====
";

    // Verifies: REQ028 — an operator definition resolves to a real location in the spec,
    // which is what makes a 2a binding groundable against the model.
    #[test]
    fn resolves_an_operator_definition_to_its_location() {
        let tmp = subject(SPEC);
        let ModelResolution::Resolved(at) = resolve_in(&tmp, "Accept") else {
            panic!("Accept should resolve");
        };
        assert_eq!(at.file, "spec.tla");
        assert_eq!(at.line, 6);
        assert!(at.text.contains("Accept(m) =="));
    }

    // Verifies: REQ028 — TLA+ has one kind of name, so a VARIABLE, a CONSTANT, and a data-set
    // operator all resolve through the same resolver. A cat-1-style predicate/sort split
    // would wrongly reject two of these.
    #[test]
    fn a_variable_a_constant_and_a_set_all_resolve() {
        let tmp = subject(SPEC);
        assert!(resolve_in(&tmp, "queue").is_resolved(), "VARIABLE");
        assert!(
            resolve_in(&tmp, "status").is_resolved(),
            "VARIABLE (2nd on the line)"
        );
        assert!(resolve_in(&tmp, "MaxLen").is_resolved(), "CONSTANT");
        assert!(
            resolve_in(&tmp, "Message").is_resolved(),
            "set-defining operator"
        );
    }

    // Verifies: REQ028 — a name the spec does not define parks the requirement (R-ground-1),
    // never grounds on a coincidental text match.
    #[test]
    fn an_undefined_name_does_not_resolve() {
        let tmp = subject(SPEC);
        assert_eq!(resolve_in(&tmp, "Rejected"), ModelResolution::NotFound);
        assert!(resolve_in(&tmp, "Rejected")
            .describe("rejected", "Rejected")
            .contains("does not name it"));
    }

    // Verifies: REQ028 — a name only mentioned inside a `\*` comment is not a definition.
    #[test]
    fn a_name_only_in_a_comment_does_not_resolve() {
        let tmp = subject("VARIABLES queue  \\* Accept is handled elsewhere\nInit == queue = 0\n");
        assert_eq!(resolve_in(&tmp, "Accept"), ModelResolution::NotFound);
    }

    // Verifies: REQ028 — a keyword that is only a prefix of a longer identifier is not a
    // declaration (`CONSTANTS` must not make `CONSTANThing` resolve, nor swallow a var named
    // `VARIABLEs_note`). Guards the whole-word check.
    #[test]
    fn a_keyword_prefix_is_not_a_declaration() {
        let tmp = subject("CONSTANTing == 1\nVARIABLESuspect == 2\n");
        // These are operator definitions named CONSTANTing / VARIABLESuspect, not decls.
        assert!(resolve_in(&tmp, "CONSTANTing").is_resolved());
        assert_eq!(resolve_in(&tmp, "CONSTANT"), ModelResolution::NotFound);
        assert_eq!(resolve_in(&tmp, "ing"), ModelResolution::NotFound);
    }

    // Verifies: REQ028 — an expression that merely contains `==` (an equality inside a
    // definition's body) is not mistaken for a definition of some compound name.
    #[test]
    fn an_equality_expression_is_not_a_definition() {
        let tmp = subject("Inv == queue = 0 /\\ status = \"ok\"\n");
        assert!(resolve_in(&tmp, "Inv").is_resolved());
        // `queue = 0 /\ status` is not a name — the body must not resolve as one.
        assert_eq!(resolve_in(&tmp, "queue"), ModelResolution::NotFound);
    }

    // Verifies: REQ028 — the function-definition form `Name[x \in S] == …` resolves.
    #[test]
    fn a_function_definition_resolves() {
        let tmp = subject("q == [x \\in 1..3 |-> x * 2]\nDouble[x \\in Nat] == x + x\n");
        assert!(resolve_in(&tmp, "Double").is_resolved());
    }

    // Verifies: REQ028 — two specs defining the same name are never silently disambiguated;
    // binding to one would depend on walk order, which is not this tool's call.
    #[test]
    fn duplicate_definitions_are_ambiguous_never_guessed() {
        let tmp = subject("Accept(m) == TRUE\n");
        std::fs::write(tmp.path().join("other.tla"), "Accept(m) == FALSE\n").unwrap();
        let ModelResolution::Ambiguous(ats) = resolve_in(&tmp, "Accept") else {
            panic!("two definitions must be ambiguous");
        };
        assert_eq!(ats.len(), 2);
    }

    // Verifies: REQ028 — the walk skips the companion tree and `.git`, the same discipline as
    // the Rust adapter, so a stray spec there cannot create a spurious ambiguity.
    #[test]
    fn the_walk_skips_the_companion_and_git() {
        let tmp = subject("Accept(m) == TRUE\n");
        let companion = tmp.path().join("ProvableRequirements");
        std::fs::create_dir_all(&companion).unwrap();
        std::fs::write(companion.join("shadow.tla"), "Accept(m) == FALSE\n").unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        std::fs::write(tmp.path().join(".git/x.tla"), "Accept(m) == FALSE\n").unwrap();

        let ModelResolution::Resolved(at) = resolve(tmp.path(), &companion, "Accept") else {
            panic!("the companion/.git copies must not create an ambiguity");
        };
        assert_eq!(at.file, "spec.tla");
    }

    // Verifies: REQ028 — a non-`.tla` file is not searched; a model observable is a TLA+
    // definition, not any text that resembles one.
    #[test]
    fn non_tla_files_are_not_searched() {
        let tmp = subject("Accept(m) == TRUE\n");
        std::fs::write(tmp.path().join("README.md"), "Accept(m) == FALSE\n").unwrap();
        assert!(resolve_in(&tmp, "Accept").is_resolved());
    }

    // Verifies: REQ028 — an empty observable resolves to nothing, guarding a degenerate
    // binding rather than matching the first definition it meets.
    #[test]
    fn empty_observable_resolves_to_nothing() {
        let tmp = subject(SPEC);
        assert_eq!(resolve_in(&tmp, "   "), ModelResolution::NotFound);
    }
}
