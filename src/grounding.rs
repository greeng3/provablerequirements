//! D13 grounding — first slice. Binds PRL vocabulary symbols to real observables
//! and **dry-runs** the category-1 (code-state) bindings against the subject's real
//! source, so the operator can confirm *"here are the spans matching this binding — is
//! that what you meant?"* before any engine is trusted.
//!
//! Only category 1 has a real dry-run in this slice: its observable is the subject's
//! own source tree, which is already on disk. Categories 2a/2b/3 (model / runtime /
//! UI) carry the same binding schema but their dry-run is **deferred** until the
//! engines/telemetry are wired — a deferred or no-match grounding never fakes a verdict
//! and never grounds the requirement (R-ground-1); the requirement stays
//! `admitted-but-ungrounded`, parked (R-ground-2).
//!
//! Bindings persist on the draft; **matches do not** — they are recomputed live on
//! every dry-run, because code moves under a binding exactly as prose moves under a
//! draft.
//!
//! Implements: REQ021 (grounding binding schema + category-1 dry-run).

use crate::prl::ast::{Category, Decl, Requirement};
use std::collections::BTreeMap;
use std::path::Path;
use walkdir::WalkDir;

/// D5 binding fidelity — a verdict is never stronger than its weakest binding. This
/// slice records it; the Step-4 verdict engine consumes it. `definitional` = true by
/// construction (model vars), `observed` = a runtime observation that can be wrong,
/// `probed` = a flaky UI probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fidelity {
    Definitional,
    Observed,
    Probed,
}

impl Fidelity {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "definitional" => Some(Fidelity::Definitional),
            "observed" => Some(Fidelity::Observed),
            "probed" => Some(Fidelity::Probed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Fidelity::Definitional => "definitional",
            Fidelity::Observed => "observed",
            Fidelity::Probed => "probed",
        }
    }
}

/// Which observable world a binding lives in (D4). Only [`BindCategory::Code`] has a
/// real dry-run in this slice. Serializable peer of the parse-only [`Category`], so the
/// AST stays a pure parse artifact with no serde.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum BindCategory {
    Code,
    Model,
    Runtime,
    Ui,
}

impl BindCategory {
    pub fn as_label(&self) -> &'static str {
        match self {
            BindCategory::Code => "1",
            BindCategory::Model => "2a",
            BindCategory::Runtime => "2b",
            BindCategory::Ui => "3",
        }
    }

    /// The default binding fidelity for this category (D5). Category 1 code-state is a
    /// static structural fact (`definitional`); runtime is `observed`; UI is `probed`.
    /// The operator can override with `--fidelity`.
    pub fn default_fidelity(&self) -> Fidelity {
        match self {
            BindCategory::Code | BindCategory::Model => Fidelity::Definitional,
            BindCategory::Runtime => Fidelity::Observed,
            BindCategory::Ui => Fidelity::Probed,
        }
    }
}

impl From<Category> for BindCategory {
    fn from(c: Category) -> Self {
        match c {
            Category::Code => BindCategory::Code,
            Category::Model => BindCategory::Model,
            Category::Runtime => BindCategory::Runtime,
            Category::Ui => BindCategory::Ui,
        }
    }
}

/// One vocabulary symbol bound to one concrete observable (D4). `symbol` names a
/// declared predicate; `observable` is the concrete anchor (for category 1, a code
/// search term); `fidelity` feeds verdict strength (D5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Binding {
    pub symbol: String,
    pub category: BindCategory,
    pub observable: String,
    pub fidelity: Fidelity,
}

/// One code-state span the dry-run matched: file (relative to the subject root), 1-based
/// line, and the trimmed line text — enough for the operator to eyeball the binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeMatch {
    pub file: String,
    pub line: usize,
    pub text: String,
}

/// Cap on spans reported per dry-run — enough to confirm a binding, not a full grep.
/// `// ponytail: fixed cap; if operators need paging, count-and-page then.`
pub const DRY_RUN_MATCH_CAP: usize = 20;

/// The declared vocabulary symbols a grounding may bind: the event/state **predicates**
/// the gate name-checks. Sorts (types) and raw identities are not bound in this slice.
/// `// ponytail: predicates only; sort/type existence when cat-1 needs it.`
pub fn bindable_symbols(req: &Requirement) -> Vec<String> {
    req.vocabulary
        .iter()
        .filter_map(|d| match d {
            Decl::Event { name, .. } | Decl::State { name, .. } => Some(name.clone()),
            Decl::Sort { .. } | Decl::Identity { .. } => None,
        })
        .collect()
}

/// The requirement's primary binding category — its first declared category, or
/// [`BindCategory::Code`] when none is declared (this slice's only real dry-run world).
/// `// ponytail: one binding category per requirement; per-category multi-binding when
/// D6 cross-category coherence lands.`
pub fn default_category(req: &Requirement) -> BindCategory {
    req.category
        .first()
        .copied()
        .map(BindCategory::from)
        .unwrap_or(BindCategory::Code)
}

/// Symbols that are declared vocabulary but have no binding yet — an unbound symbol
/// keeps the requirement ungrounded (there is nothing to observe it through). Pure.
pub fn unbound_symbols(req: &Requirement, bindings: &[Binding]) -> Vec<String> {
    let bound: std::collections::BTreeSet<&str> =
        bindings.iter().map(|b| b.symbol.as_str()).collect();
    bindable_symbols(req)
        .into_iter()
        .filter(|s| !bound.contains(s.as_str()))
        .collect()
}

/// Whether a symbol name is a declared, bindable predicate — a `--ground` for anything
/// else is a user error (you cannot ground a symbol the requirement does not speak of).
pub fn is_bindable(req: &Requirement, symbol: &str) -> bool {
    bindable_symbols(req).iter().any(|s| s == symbol)
}

/// The grounding verdict for a requirement (R-ground-1/2). `Grounded` only when every
/// symbol is bound in category 1 **and** each such binding matched ≥1 real span. Any
/// unbound symbol, any deferred (non-code) category, or any no-match code binding leaves
/// it `Parked` with human-readable reasons — never a verdict, never faked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grounding {
    Grounded,
    Parked { reasons: Vec<String> },
}

/// Decide the grounding verdict from the bindings and the **already-computed** category-1
/// match counts (keyed by symbol). Pure — the caller runs [`dry_run_code`] and passes the
/// counts, so this stays testable without a filesystem.
pub fn verdict(
    req: &Requirement,
    bindings: &[Binding],
    code_match_counts: &BTreeMap<String, usize>,
) -> Grounding {
    let mut reasons = Vec::new();

    for sym in unbound_symbols(req, bindings) {
        reasons.push(format!(
            "{sym}: unbound — no observable to check it through"
        ));
    }

    for b in bindings {
        match b.category {
            BindCategory::Code if code_match_counts.get(&b.symbol).copied().unwrap_or(0) == 0 => {
                reasons.push(format!(
                    "{}: no code span matches `{}` — wrong binding, or the requirement is \
                     ahead of the code (parked)",
                    b.symbol, b.observable
                ))
            }
            BindCategory::Code => {}
            other => reasons.push(format!(
                "{}: category {} dry-run deferred — engine not wired yet",
                b.symbol,
                other.as_label()
            )),
        }
    }

    if reasons.is_empty() {
        Grounding::Grounded
    } else {
        Grounding::Parked { reasons }
    }
}

/// Dry-run a category-1 (code-state) observable against the subject's real source: walk
/// the tree, skipping `.git` and the companion tree, and collect up to
/// [`DRY_RUN_MATCH_CAP`] lines containing `observable` (substring —
/// `// ponytail: substring, regex when operators need it`). Live: recomputed every call,
/// never persisted, because code drifts under a binding.
pub fn dry_run_code(
    subject_root: &Path,
    companion_root: &Path,
    observable: &str,
) -> Vec<CodeMatch> {
    let needle = observable.trim();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(subject_root)
        .into_iter()
        .filter_entry(|e| !is_skipped_dir(e.path(), companion_root))
    {
        if out.len() >= DRY_RUN_MATCH_CAP {
            break;
        }
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        // Non-UTF-8 / binary files read-fail or lossily differ — skip them silently;
        // a code-state binding names source text, not bytes.
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let rel = entry
            .path()
            .strip_prefix(subject_root)
            .unwrap_or(entry.path())
            .display()
            .to_string();
        for (i, line) in text.lines().enumerate() {
            if line.contains(needle) {
                out.push(CodeMatch {
                    file: rel.clone(),
                    line: i + 1,
                    text: line.trim().to_string(),
                });
                if out.len() >= DRY_RUN_MATCH_CAP {
                    break;
                }
            }
        }
    }
    out
}

/// Whether a directory should be pruned from the dry-run walk: the VCS metadata dir or
/// the companion tree (whose `drafts.yml` holds the observables themselves — matching
/// there would be a spurious self-hit).
fn is_skipped_dir(path: &Path, companion_root: &Path) -> bool {
    if path == companion_root {
        return true;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == ".git")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prl::gate;

    fn req(src: &str) -> Requirement {
        gate(src)
            .expect("test candidate should clear the gate")
            .requirement
    }

    // The CODE-fragment reading of "a logged-in user always has a session": an INVARIANT
    // (`always`, i.e. `logged_in ⇒ has_session` at every state), NOT the liveness
    // `leads_to` this fixture used before REQ024. Category 1 is temporal-free, so a
    // deductive prover can check the implication as a state predicate but has nothing to
    // say about a future-time obligation. The same prose has both readings — the declared
    // category is what picks one, which is exactly what the fragment check now enforces.
    const CODE_REQ: &str = "requirement r {
        category: 1
        vocabulary { state logged_in(u), has_session(u) }
        require { each u: User . always (not logged_in(u) or has_session(u)) }
    }";

    // Verifies: REQ021 — the bindable symbols are exactly the declared event/state
    // predicates (not sorts or the quantifier variable).
    #[test]
    fn bindable_symbols_are_declared_predicates() {
        let syms = bindable_symbols(&req(CODE_REQ));
        assert_eq!(syms, vec!["logged_in", "has_session"]);
    }

    // Verifies: REQ021 — a category-1 requirement defaults its bindings to the Code
    // world with definitional fidelity.
    #[test]
    fn category_and_fidelity_default_from_the_requirement() {
        let cat = default_category(&req(CODE_REQ));
        assert_eq!(cat, BindCategory::Code);
        assert_eq!(cat.default_fidelity(), Fidelity::Definitional);
    }

    // Verifies: REQ021 — you cannot ground a symbol the requirement does not declare.
    #[test]
    fn is_bindable_rejects_undeclared_symbols() {
        let r = req(CODE_REQ);
        assert!(is_bindable(&r, "logged_in"));
        assert!(!is_bindable(&r, "not_a_symbol"));
    }

    // Verifies: REQ021 — an unbound declared symbol is reported, and drops off once bound.
    #[test]
    fn unbound_symbols_tracks_coverage() {
        let r = req(CODE_REQ);
        let none: Vec<Binding> = vec![];
        assert_eq!(unbound_symbols(&r, &none), vec!["logged_in", "has_session"]);

        let one = vec![Binding {
            symbol: "logged_in".into(),
            category: BindCategory::Code,
            observable: "fn log_in".into(),
            fidelity: Fidelity::Definitional,
        }];
        assert_eq!(unbound_symbols(&r, &one), vec!["has_session"]);
    }

    fn code_binding(symbol: &str, observable: &str) -> Binding {
        Binding {
            symbol: symbol.into(),
            category: BindCategory::Code,
            observable: observable.into(),
            fidelity: Fidelity::Definitional,
        }
    }

    // Verifies: REQ021 (R-ground-1/2) — a requirement grounds only when every symbol is
    // bound in category 1 and each binding matched a real span.
    #[test]
    fn verdict_is_grounded_only_when_every_code_binding_matches() {
        let r = req(CODE_REQ);
        let bindings = vec![
            code_binding("logged_in", "fn log_in"),
            code_binding("has_session", "struct Session"),
        ];
        let mut counts = BTreeMap::new();
        counts.insert("logged_in".to_string(), 2);
        counts.insert("has_session".to_string(), 1);
        assert_eq!(verdict(&r, &bindings, &counts), Grounding::Grounded);
    }

    // Verifies: REQ021 (R-ground-2) — a code binding with no match parks the requirement
    // (never a verdict), and names the two causes.
    #[test]
    fn verdict_parks_on_no_match() {
        let r = req(CODE_REQ);
        let bindings = vec![
            code_binding("logged_in", "fn log_in"),
            code_binding("has_session", "fn nonexistent"),
        ];
        let mut counts = BTreeMap::new();
        counts.insert("logged_in".to_string(), 3);
        counts.insert("has_session".to_string(), 0);
        let Grounding::Parked { reasons } = verdict(&r, &bindings, &counts) else {
            panic!("a no-match binding must park, never ground");
        };
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("has_session") && reason.contains("no code span")));
    }

    // Verifies: REQ021 (R-ground-1) — a non-code binding is honestly deferred, never
    // silently grounded, because its engine has not run.
    #[test]
    fn verdict_defers_non_code_categories() {
        let r = req("requirement r {
            category: 2b
            vocabulary { event fired(x) }
            require { always fired(x) }
        }");
        let bindings = vec![Binding {
            symbol: "fired".into(),
            category: BindCategory::Runtime,
            observable: "queue.events".into(),
            fidelity: Fidelity::Observed,
        }];
        let Grounding::Parked { reasons } = verdict(&r, &bindings, &BTreeMap::new()) else {
            panic!("a deferred category must park");
        };
        assert!(reasons.iter().any(|reason| reason.contains("deferred")));
    }

    // Verifies: REQ021 — the dry-run finds real spans in the subject tree, skips the
    // companion tree and .git, respects the cap, and reports relative file + line.
    #[test]
    fn dry_run_matches_real_source_and_skips_companion() {
        let tmp = tempfile::tempdir().unwrap();
        let subject = tmp.path();
        std::fs::create_dir_all(subject.join("src")).unwrap();
        std::fs::write(
            subject.join("src/auth.rs"),
            "fn log_in(u: User) {}\n// unrelated\nfn log_out() {}\n",
        )
        .unwrap();
        // A companion sibling whose drafts.yml mentions the same term must NOT match.
        let companion = subject.join("requirements-provreq");
        std::fs::create_dir_all(&companion).unwrap();
        std::fs::write(companion.join("drafts.yml"), "observable: fn log_in\n").unwrap();
        // .git content must be skipped too.
        std::fs::create_dir_all(subject.join(".git")).unwrap();
        std::fs::write(subject.join(".git/config"), "fn log_in\n").unwrap();

        let matches = dry_run_code(subject, &companion, "fn log_in");
        assert_eq!(matches.len(), 1, "only the real source line should match");
        assert_eq!(matches[0].file, "src/auth.rs");
        assert_eq!(matches[0].line, 1);
        assert!(matches[0].text.contains("log_in"));
    }

    // Verifies: REQ021 — an empty observable never matches (guards a degenerate binding).
    #[test]
    fn dry_run_empty_observable_matches_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.rs"), "anything\n").unwrap();
        let companion = tmp.path().join("companion");
        assert!(dry_run_code(tmp.path(), &companion, "   ").is_empty());
    }
}
