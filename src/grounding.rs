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
//! binding grounds only by resolving to a state predicate at a source location), REQ026
//! (sorts bind to real types, so a quantified variable has a domain), REQ028 (a cat-2a
//! binding grounds by resolving to a definition in a TLA+ spec).

use crate::prl::ast::{Category, Decl, Requirement};
use crate::rust_adapter::{Resolution, TypeResolution};
use crate::tla_adapter::ModelResolution;
use std::collections::BTreeMap;
use std::path::Path;

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

/// The declared vocabulary **predicates** a grounding may bind: the event/state names the
/// gate name-checks. Sorts are bound too, but separately — see [`bindable_sorts`], since a
/// predicate binds to a function and a sort binds to a type. Raw identities are still
/// unbound. `// ponytail: identities when D6 cross-category correspondence lands.`
pub fn bindable_symbols(req: &Requirement) -> Vec<String> {
    req.vocabulary
        .iter()
        .filter_map(|d| match d {
            Decl::Event { name, .. } | Decl::State { name, .. } => Some(name.clone()),
            Decl::Sort { .. } | Decl::Identity { .. } => None,
        })
        .collect()
}

/// The sorts a grounding may bind: the **types a quantified variable ranges over**
/// (`each u: User`) plus any declared `sort` in the vocabulary, deduplicated and in a
/// stable order. Peer of [`bindable_symbols`], which stays predicates-only — a predicate
/// binds to a function, a sort binds to a type, and conflating them would let one resolver
/// answer a question it was not asked.
///
/// A quantified claim whose domain is unknown is not grounded: nothing can range over a
/// sort that names no real type, so an unbound sort parks the requirement exactly as an
/// unbound predicate does (R-ground-1). REQ026.
pub fn bindable_sorts(req: &Requirement) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: &str| {
        if !s.is_empty() && !out.iter().any(|seen| seen == s) {
            out.push(s.to_string());
        }
    };
    for decl in &req.vocabulary {
        if let Decl::Sort { name, .. } = decl {
            push(name);
        }
    }
    for prop in &req.require {
        if let Some(q) = &prop.quantifier {
            push(&q.sort);
        }
    }
    out
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

/// Everything the requirement speaks of that has no binding yet — **predicates and sorts
/// alike** (REQ026). An unbound name keeps the requirement ungrounded: there is nothing to
/// observe a predicate through, and nothing for a quantified variable to range over. Pure.
pub fn unbound_symbols(req: &Requirement, bindings: &[Binding]) -> Vec<String> {
    let bound: std::collections::BTreeSet<&str> =
        bindings.iter().map(|b| b.symbol.as_str()).collect();
    bindable_symbols(req)
        .into_iter()
        .chain(bindable_sorts(req))
        .filter(|s| !bound.contains(s.as_str()))
        .collect()
}

/// Whether a name is a declared, bindable predicate **or sort** — a `--ground` for
/// anything else is a user error (you cannot ground a name the requirement does not speak
/// of).
pub fn is_bindable(req: &Requirement, symbol: &str) -> bool {
    bindable_symbols(req).iter().any(|s| s == symbol)
        || bindable_sorts(req).iter().any(|s| s == symbol)
}

/// Whether a bindable name is a **sort** rather than a predicate. Decides which resolver
/// answers for it: a predicate binds to a function, a sort binds to a type.
pub fn is_sort(req: &Requirement, symbol: &str) -> bool {
    bindable_sorts(req).iter().any(|s| s == symbol)
}

/// The grounding verdict for a requirement (R-ground-1/2). `Grounded` only when every
/// symbol is bound **and** each binding resolves against its category's observable world —
/// category 1 to a state predicate at a source location, category 2a to a definition in a
/// TLA+ spec. Any unbound symbol, any unresolved binding, or any still-deferred category
/// (2b/3) leaves it `Parked` with human-readable reasons — never a verdict, never faked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grounding {
    Grounded,
    Parked { reasons: Vec<String> },
}

/// Decide the grounding verdict from the bindings and the **already-computed** per-category
/// resolutions (keyed by symbol). Pure — the caller runs the adapters
/// ([`crate::rust_adapter`], [`crate::tla_adapter`]) and passes the results, so this stays
/// testable without a filesystem.
///
/// Each binding grounds only when it **resolves** against its category's observable world:
/// category 1 to a real state predicate (REQ025) or type (REQ026), category 2a to a real
/// TLA+ definition (REQ028). Every other outcome parks the requirement and carries the
/// adapter's own explanation as the reason, so the operator reads one account of what
/// happened rather than a summary of it. Categories 2b/3 have no observable world wired yet
/// and are honestly deferred.
/// Resolve every binding against its category's observable world, live (resolutions are never
/// stored — code moves under a binding as prose moves under a draft). The per-category map peer of
/// [`verdict`]: category-1 predicates → functions and sorts → types (REQ025/REQ026), category-2a
/// symbols → TLA+ definitions (REQ028); 2b/3 have no world wired and are absent from every map.
///
/// The predicate/sort split is kept because a coincidental cross-hit (a `struct login` standing in
/// for the predicate `login`) must never ground anything. Shared by the CLI dry-run and the serve
/// backend so both resolve bindings the one same way.
pub fn resolve_bindings(
    subject: &Path,
    companion: &Path,
    requirement: &Requirement,
    bindings: &[Binding],
) -> (
    BTreeMap<String, Resolution>,
    BTreeMap<String, TypeResolution>,
    BTreeMap<String, ModelResolution>,
) {
    let in_category = |cat| {
        bindings
            .iter()
            .filter(move |b| b.category == cat)
            .collect::<Vec<_>>()
    };
    let code = in_category(BindCategory::Code);
    let predicates = code
        .iter()
        .filter(|b| !is_sort(requirement, &b.symbol))
        .map(|b| {
            let arity = predicate_arity(requirement, &b.symbol).unwrap_or(0);
            (
                b.symbol.clone(),
                crate::rust_adapter::resolve(subject, companion, &b.observable, arity),
            )
        })
        .collect();
    let sorts = code
        .iter()
        .filter(|b| is_sort(requirement, &b.symbol))
        .map(|b| {
            (
                b.symbol.clone(),
                crate::rust_adapter::resolve_type(subject, companion, &b.observable),
            )
        })
        .collect();
    let model = in_category(BindCategory::Model)
        .iter()
        .map(|b| {
            (
                b.symbol.clone(),
                crate::tla_adapter::resolve(subject, companion, &b.observable),
            )
        })
        .collect();
    (predicates, sorts, model)
}

pub fn verdict(
    req: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
    sort_resolutions: &BTreeMap<String, TypeResolution>,
    model_resolutions: &BTreeMap<String, ModelResolution>,
) -> Grounding {
    let mut reasons = Vec::new();

    for sym in unbound_symbols(req, bindings) {
        reasons.push(format!(
            "{sym}: unbound — no observable to check it through"
        ));
    }

    for b in bindings {
        match b.category {
            // A sort binds to a type and a predicate to a function, so each is answered by
            // its own resolver — asking one for the other's name would silently succeed on
            // a coincidental match (a `struct login` is not the predicate `login`).
            BindCategory::Code if is_sort(req, &b.symbol) => {
                match sort_resolutions.get(&b.symbol) {
                    Some(r) if r.is_resolved() => {}
                    Some(r) => reasons.push(r.describe(&b.symbol, &b.observable)),
                    None => reasons.push(format!(
                        "{} (sort): `{}` was not resolved against the subject's source",
                        b.symbol, b.observable
                    )),
                }
            }
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
            // Category 2a: predicates and sorts alike resolve through the one model resolver,
            // because TLA+ does not distinguish an action from a set from a variable at the
            // name level (see [`crate::tla_adapter`]).
            BindCategory::Model => match model_resolutions.get(&b.symbol) {
                Some(r) if r.is_resolved() => {}
                Some(r) => reasons.push(r.describe(&b.symbol, &b.observable)),
                None => reasons.push(format!(
                    "{}: `{}` was not resolved against the subject's TLA+ spec",
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

    // Verifies: REQ021/REQ026 — an unbound name is reported and drops off once bound, and
    // that covers SORTS as well as predicates: `CODE_REQ` quantifies `each u: User`, so the
    // sort `User` is a name the requirement speaks of and must be bound too.
    #[test]
    fn unbound_symbols_tracks_predicates_and_sorts() {
        let r = req(CODE_REQ);
        let none: Vec<Binding> = vec![];
        assert_eq!(
            unbound_symbols(&r, &none),
            vec!["logged_in", "has_session", "User"]
        );

        let one = vec![code_binding("logged_in", "login")];
        assert_eq!(unbound_symbols(&r, &one), vec!["has_session", "User"]);

        let all = vec![
            code_binding("logged_in", "login"),
            code_binding("has_session", "has_session"),
            sort_binding("User", "User"),
        ];
        assert!(unbound_symbols(&r, &all).is_empty());
    }

    fn code_binding(symbol: &str, observable: &str) -> Binding {
        Binding {
            symbol: symbol.into(),
            category: BindCategory::Code,
            observable: observable.into(),
            fidelity: Fidelity::Definitional,
        }
    }

    fn sort_binding(symbol: &str, observable: &str) -> Binding {
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

    /// A resolved nullary predicate. This module's verdict asks only whether a binding
    /// resolved, so the parameter modes an engine would need are irrelevant here.
    fn resolved(file: &str) -> Resolution {
        Resolution::Resolved {
            at: at(file),
            params: vec![],
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
            sort_binding("User", "User"),
        ];
        let resolutions = BTreeMap::from([
            ("logged_in".to_string(), resolved("src/a.rs")),
            ("has_session".to_string(), resolved("src/a.rs")),
        ]);
        let sorts =
            BTreeMap::from([("User".to_string(), TypeResolution::Resolved(at("src/a.rs")))]);
        assert_eq!(
            verdict(&r, &bindings, &resolutions, &sorts, &BTreeMap::new()),
            Grounding::Grounded
        );
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
            ("logged_in".to_string(), resolved("src/a.rs")),
            ("has_session".to_string(), Resolution::NotFound),
        ]);
        let Grounding::Parked { reasons } = verdict(
            &r,
            &bindings,
            &resolutions,
            &BTreeMap::new(),
            &BTreeMap::new(),
        ) else {
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
        let only_one = BTreeMap::from([("logged_in".to_string(), resolved("src/a.rs"))]);
        let Grounding::Parked { reasons } =
            verdict(&r, &bindings, &only_one, &BTreeMap::new(), &BTreeMap::new())
        else {
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
        let Grounding::Parked { reasons } = verdict(
            &r,
            &bindings,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        ) else {
            panic!("a deferred category must park");
        };
        assert!(reasons.iter().any(|reason| reason.contains("deferred")));
    }

    // Verifies: REQ026 — the sorts a quantifier ranges over are bindable, alongside any
    // declared `sort`. Predicates stay out of this list; they bind to functions.
    #[test]
    fn bindable_sorts_are_quantifier_sorts_and_declared_sorts() {
        assert_eq!(bindable_sorts(&req(CODE_REQ)), vec!["User"]);
        let with_decl = req("requirement r {
            category: 1
            vocabulary { sort Message state sent(m) }
            require { each m: Message . always sent(m) }
        }");
        assert_eq!(bindable_sorts(&with_decl), vec!["Message"]);
        assert!(!bindable_sorts(&with_decl).contains(&"sent".to_string()));
        assert!(is_sort(&with_decl, "Message"));
        assert!(!is_sort(&with_decl, "sent"));
    }

    // Verifies: REQ026 — an UNBOUND sort parks the requirement. A quantified claim whose
    // domain names nothing is not grounded, however well its predicates resolve.
    #[test]
    fn unbound_sort_parks_even_when_every_predicate_resolves() {
        let r = req(CODE_REQ);
        let bindings = vec![
            code_binding("logged_in", "login"),
            code_binding("has_session", "has_session"),
        ];
        let resolutions = BTreeMap::from([
            ("logged_in".to_string(), resolved("src/a.rs")),
            ("has_session".to_string(), resolved("src/a.rs")),
        ]);
        let Grounding::Parked { reasons } = verdict(
            &r,
            &bindings,
            &resolutions,
            &BTreeMap::new(),
            &BTreeMap::new(),
        ) else {
            panic!("an unbound sort must park");
        };
        assert!(
            reasons.iter().any(|reason| reason.contains("User")),
            "the unbound sort must be named: {reasons:?}"
        );
    }

    // Verifies: REQ026 — a BOUND sort that does not resolve parks too, carrying the
    // adapter's own explanation.
    #[test]
    fn unresolved_sort_parks() {
        let r = req(CODE_REQ);
        let bindings = vec![
            code_binding("logged_in", "login"),
            code_binding("has_session", "has_session"),
            sort_binding("User", "NoSuchType"),
        ];
        let resolutions = BTreeMap::from([
            ("logged_in".to_string(), resolved("src/a.rs")),
            ("has_session".to_string(), resolved("src/a.rs")),
        ]);
        let sorts = BTreeMap::from([("User".to_string(), TypeResolution::NotFound)]);
        let Grounding::Parked { reasons } =
            verdict(&r, &bindings, &resolutions, &sorts, &BTreeMap::new())
        else {
            panic!("an unresolved sort must park");
        };
        assert!(reasons.iter().any(|reason| reason.contains("NoSuchType")));
    }

    // Verifies: REQ025 — the arity checked against comes from the requirement's own
    // vocabulary declaration, which is what makes a wrong binding detectable.
    #[test]
    fn predicate_arity_comes_from_the_vocabulary() {
        let r = req(CODE_REQ);
        assert_eq!(predicate_arity(&r, "logged_in"), Some(1));
        assert_eq!(predicate_arity(&r, "not_declared"), None);
    }

    // A category-2a model requirement: a liveness claim the model world can express (the code
    // fragment cannot, which is exactly why it declares 2a).
    const MODEL_REQ: &str = "requirement r {
        category: 2a
        vocabulary { sort Message event accepted(m) state succeeded(m) }
        require { each m: Message . accepted(m) leads_to succeeded(m) }
    }";

    fn model_binding(symbol: &str, observable: &str) -> Binding {
        Binding {
            symbol: symbol.into(),
            category: BindCategory::Model,
            observable: observable.into(),
            fidelity: Fidelity::Definitional,
        }
    }

    // Verifies: REQ028 — a category-2a requirement grounds when every symbol (predicates AND
    // sorts alike) resolves to a definition in the subject's TLA+ spec. This is the model
    // world's analog of the cat-1 grounding.
    #[test]
    fn model_requirement_grounds_when_every_binding_resolves() {
        let r = req(MODEL_REQ);
        let bindings = vec![
            model_binding("accepted", "Accept"),
            model_binding("succeeded", "Succeeded"),
            model_binding("Message", "Message"),
        ];
        let model = BTreeMap::from([
            ("accepted".to_string(), ModelResolution::Resolved(spec_at())),
            (
                "succeeded".to_string(),
                ModelResolution::Resolved(spec_at()),
            ),
            ("Message".to_string(), ModelResolution::Resolved(spec_at())),
        ]);
        assert_eq!(
            verdict(&r, &bindings, &BTreeMap::new(), &BTreeMap::new(), &model),
            Grounding::Grounded
        );
    }

    // Verifies: REQ028 — a 2a binding to a name the spec does not define parks the
    // requirement, carrying the adapter's own explanation (never a verdict, R-ground-1).
    #[test]
    fn model_requirement_parks_when_a_binding_does_not_resolve() {
        let r = req(MODEL_REQ);
        let bindings = vec![
            model_binding("accepted", "Accept"),
            model_binding("succeeded", "NoSuchOp"),
            model_binding("Message", "Message"),
        ];
        let model = BTreeMap::from([
            ("accepted".to_string(), ModelResolution::Resolved(spec_at())),
            ("succeeded".to_string(), ModelResolution::NotFound),
            ("Message".to_string(), ModelResolution::Resolved(spec_at())),
        ]);
        let Grounding::Parked { reasons } =
            verdict(&r, &bindings, &BTreeMap::new(), &BTreeMap::new(), &model)
        else {
            panic!("an unresolved model binding must park");
        };
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("succeeded") && reason.contains("NoSuchOp")));
    }

    // Verifies: REQ028 — a 2a symbol the caller never resolved is NOT treated as grounded,
    // exactly as for cat-1: absence of evidence is not evidence of grounding.
    #[test]
    fn model_requirement_parks_when_a_binding_was_never_resolved() {
        let r = req(MODEL_REQ);
        let bindings = vec![
            model_binding("accepted", "Accept"),
            model_binding("succeeded", "Succeeded"),
            model_binding("Message", "Message"),
        ];
        let only_two = BTreeMap::from([
            ("accepted".to_string(), ModelResolution::Resolved(spec_at())),
            ("Message".to_string(), ModelResolution::Resolved(spec_at())),
        ]);
        let Grounding::Parked { reasons } =
            verdict(&r, &bindings, &BTreeMap::new(), &BTreeMap::new(), &only_two)
        else {
            panic!("an unresolved-by-omission model binding must park");
        };
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("succeeded") && reason.contains("TLA+")));
    }

    fn spec_at() -> crate::tla_adapter::SpecMatch {
        crate::tla_adapter::SpecMatch {
            file: "spec.tla".into(),
            line: 1,
            text: "Accept(m) == TRUE".into(),
        }
    }
}
