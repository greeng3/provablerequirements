//! D13 grounding — the binding schema and the grounded/parked decision. Binds PRL
//! vocabulary symbols to real observables and **dry-runs** the category-1 (code-state)
//! bindings against the subject's real source, so the operator can confirm *"here is what
//! your binding resolves to — is that what you meant?"* before any engine is trusted.
//!
//! Category 1's observable world is the subject's own source tree, and resolving against
//! it is [`crate::rust_adapter`]'s job (R-eng-4, the per-language adapter) — this module
//! owns the category-independent schema and the verdict, not the language. Categories
//! 2a/2b/3 (model / runtime / UI) carry the same binding schema but their dry-run is
//! **deferred** until the engines/telemetry are wired — a deferred or unresolved grounding
//! never fakes a verdict and never grounds the requirement (R-ground-1); the requirement
//! stays `admitted-but-ungrounded`, parked (R-ground-2).
//!
//! Bindings persist on the draft; **resolutions do not** — they are recomputed live on
//! every dry-run, because code moves under a binding exactly as prose moves under a draft.
//!
//! Implements: REQ021 (grounding binding schema + category-1 dry-run), REQ025 (a cat-1
//! binding grounds only by resolving to a state predicate at a source location).

use crate::prl::ast::{Category, Decl, Requirement};
use crate::rust_adapter::Resolution;
use std::collections::BTreeMap;

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
/// declared predicate; `observable` is the concrete anchor — for category 1 the **name of
/// a function** that stands for the predicate, resolved against the subject's real syntax
/// tree (REQ025), not a text to search for; `fidelity` feeds verdict strength (D5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Binding {
    pub symbol: String,
    pub category: BindCategory,
    pub observable: String,
    pub fidelity: Fidelity,
}

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

/// The arity the requirement declares for a vocabulary predicate — what a category-1
/// binding's resolved function must match (REQ025). `None` when the symbol is not a
/// declared event/state predicate.
pub fn predicate_arity(req: &Requirement, symbol: &str) -> Option<usize> {
    req.vocabulary.iter().find_map(|d| match d {
        Decl::Event { name, params, .. } | Decl::State { name, params, .. } if name == symbol => {
            Some(params.len())
        }
        _ => None,
    })
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
/// symbol is bound in category 1 **and** each such binding resolves to a real state
/// predicate at a real source location. Any unbound symbol, any deferred (non-code)
/// category, or any unresolved code binding leaves it `Parked` with human-readable
/// reasons — never a verdict, never faked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grounding {
    Grounded,
    Parked { reasons: Vec<String> },
}

/// Decide the grounding verdict from the bindings and the **already-computed** category-1
/// resolutions (keyed by symbol). Pure — the caller runs [`crate::rust_adapter::resolve`]
/// and passes the results, so this stays testable without a filesystem.
///
/// A category-1 binding grounds only when its predicate **resolves** to a real state
/// predicate at a real source location (REQ025). Every other outcome — not found,
/// ambiguous, wrong arity, not boolean — parks the requirement and carries the adapter's
/// own explanation as the reason, so the operator reads one account of what happened
/// rather than a summary of it.
pub fn verdict(
    req: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> Grounding {
    let mut reasons = Vec::new();

    for sym in unbound_symbols(req, bindings) {
        reasons.push(format!(
            "{sym}: unbound — no observable to check it through"
        ));
    }

    for b in bindings {
        match b.category {
            BindCategory::Code => match resolutions.get(&b.symbol) {
                Some(r) if r.is_resolved() => {}
                // An absent resolution is treated exactly as a failed one: the caller not
                // having resolved a symbol is not evidence that it grounds.
                Some(r) => reasons.push(r.describe(&b.symbol, &b.observable)),
                None => reasons.push(format!(
                    "{}: `{}` was not resolved against the subject's source",
                    b.symbol, b.observable
                )),
            },
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

    fn at(file: &str) -> crate::rust_adapter::CodeMatch {
        crate::rust_adapter::CodeMatch {
            file: file.into(),
            line: 1,
            text: "fn f() -> bool { true }".into(),
        }
    }

    // Verifies: REQ021/REQ025 (R-ground-1/2) — a requirement grounds only when every
    // symbol is bound in category 1 and each binding RESOLVES to a real state predicate.
    #[test]
    fn verdict_is_grounded_only_when_every_code_binding_resolves() {
        let r = req(CODE_REQ);
        let bindings = vec![
            code_binding("logged_in", "login"),
            code_binding("has_session", "has_session"),
        ];
        let resolutions = BTreeMap::from([
            (
                "logged_in".to_string(),
                Resolution::Resolved(at("src/a.rs")),
            ),
            (
                "has_session".to_string(),
                Resolution::Resolved(at("src/a.rs")),
            ),
        ]);
        assert_eq!(verdict(&r, &bindings, &resolutions), Grounding::Grounded);
    }

    // Verifies: REQ025 (R-ground-2) — a binding that does not resolve parks the
    // requirement (never a verdict), carrying the adapter's own explanation.
    #[test]
    fn verdict_parks_when_a_binding_does_not_resolve() {
        let r = req(CODE_REQ);
        let bindings = vec![
            code_binding("logged_in", "login"),
            code_binding("has_session", "nonexistent"),
        ];
        let resolutions = BTreeMap::from([
            (
                "logged_in".to_string(),
                Resolution::Resolved(at("src/a.rs")),
            ),
            ("has_session".to_string(), Resolution::NotFound),
        ]);
        let Grounding::Parked { reasons } = verdict(&r, &bindings, &resolutions) else {
            panic!("an unresolved binding must park, never ground");
        };
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("has_session") && reason.contains("nonexistent")));
    }

    // Verifies: REQ025 — a symbol the caller never resolved is NOT treated as grounded.
    // Absence of evidence is not evidence of grounding.
    #[test]
    fn verdict_parks_when_a_binding_was_never_resolved() {
        let r = req(CODE_REQ);
        let bindings = vec![
            code_binding("logged_in", "login"),
            code_binding("has_session", "has_session"),
        ];
        let only_one = BTreeMap::from([(
            "logged_in".to_string(),
            Resolution::Resolved(at("src/a.rs")),
        )]);
        let Grounding::Parked { reasons } = verdict(&r, &bindings, &only_one) else {
            panic!("an unresolved-by-omission binding must park");
        };
        assert!(reasons.iter().any(|reason| reason.contains("has_session")));
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

    // Verifies: REQ025 — the arity checked against comes from the requirement's own
    // vocabulary declaration, which is what makes a wrong binding detectable.
    #[test]
    fn predicate_arity_comes_from_the_vocabulary() {
        let r = req(CODE_REQ);
        assert_eq!(predicate_arity(&r, "logged_in"), Some(1));
        assert_eq!(predicate_arity(&r, "not_declared"), None);
    }
}
