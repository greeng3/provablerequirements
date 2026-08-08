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
        match decl {
            Decl::Sort { name, .. } => push(name),
            // A type declared on a predicate's parameter is a sort like any other (REQ059): it is
            // what the variable in that position ranges over, so it needs a binding for the same
            // reason a quantifier's sort does — nothing can range over a domain that is not known
            // to be real (R-ground-1).
            Decl::Event { params, .. } | Decl::State { params, .. } => {
                for p in params {
                    push(p.ty.trim());
                }
            }
            Decl::Identity { .. } => {}
        }
    }
    for prop in &req.require {
        if let Some(q) = &prop.quantifier {
            push(&q.sort);
        }
    }
    out
}

/// The arity the requirement declares for a vocabulary predicate — what a binding's resolved
/// observable must match, whichever world it lives in: a category-1 function's parameter count
/// (REQ025), or a category-2a TLA+ definition's (REQ028, #119). `None` when the symbol is not a
/// declared event/state predicate — a sort is applied to nothing and takes no arguments.
pub fn predicate_arity(req: &Requirement, symbol: &str) -> Option<usize> {
    req.vocabulary.iter().find_map(|d| match d {
        Decl::Event { name, params, .. } | Decl::State { name, params, .. } if name == symbol => {
            Some(params.len())
        }
        _ => None,
    })
}

/// The Rust type each parameter of a category-1 predicate is expected to take, position by
/// position: the type the sort of the argument standing in that position is bound to. The length
/// is the predicate's declared arity, so it doubles as what [`crate::rust_adapter::resolve`]
/// checks arity against (REQ057).
///
/// `None` wherever nothing can honestly be said, and every one of those cases is a real
/// limit rather than an oversight:
/// - the argument is not the quantified variable (a free variable has no sort here — #136),
/// - the property is unquantified, or its sort is not bound to a type yet,
/// - two properties apply the predicate to variables of *different* sorts in the same position,
///   which is a disagreement in the requirement, not a fact about the subject's code,
/// - **the sort is bound, but that binding did not resolve against the subject** (#198).
///
/// That last one is why `sort_resolutions` is here. The expected type is only ever the *text* an
/// operator wrote in a sort binding, and text alone is not an established type: a sort bound to
/// `Nope` yields an expected `Nope`, which duly differs from whatever the subject's parameter says,
/// and the predicate binding is parked for a disagreement with a type that does not exist. Measured
/// on a real subject: one mistake (a sort naming nothing) produced two reasons, and the second one
/// named the predicate binding — which resolved correctly, to the right function, at the right line.
/// The operator is sent to inspect a binding that needs no action, and the true reason is the one
/// printed second. So where the sort did not resolve, this says nothing and lets the sort's own
/// reason stand alone.
///
/// The returned length is always the declared arity — a silenced position becomes `None`, never a
/// shorter vector, because [`crate::rust_adapter::resolve`] checks arity against this length.
///
/// Separate from the adapter on purpose: the sort a parameter should take is the **requirement's**
/// claim, and the adapter's job is only to read what the subject wrote. Consulting an
/// already-computed resolution is not adapter work — nothing here reads the subject.
pub fn expected_param_types(
    req: &Requirement,
    bindings: &[Binding],
    sort_resolutions: &BTreeMap<String, TypeResolution>,
    symbol: &str,
) -> Vec<Option<String>> {
    let arity = predicate_arity(req, symbol).unwrap_or(0);
    let mut expected: Vec<Option<String>> = vec![None; arity];
    let mut conflicting = vec![false; arity];
    for prop in &req.require {
        let binders = req.binders(prop);
        prop.for_each_atom(&mut |atom| {
            if atom.name != symbol {
                return;
            }
            for (i, arg) in atom.args.iter().enumerate() {
                if i >= arity || conflicting[i] {
                    continue;
                }
                // The type is the one the *binder* ranges over, which is what the harness will
                // actually instantiate — an explicit `each` overrides the vocabulary's
                // declaration, and the check must follow the value that will exist, not the
                // declaration it came from.
                let Some(ty) = binders
                    .iter()
                    .find(|b| b.var == arg.trim())
                    .and_then(|b| b.sort.as_ref())
                    // A sort whose own binding did not resolve establishes no type, so this
                    // position has nothing to be compared against. Absent from the map counts as
                    // unresolved: not knowing whether it resolved is not knowing the type.
                    .filter(|sort| {
                        sort_resolutions
                            .get(sort.as_str())
                            .is_some_and(TypeResolution::is_resolved)
                    })
                    .and_then(|sort| bindings.iter().find(|b| b.symbol == *sort))
                    .map(|b| b.observable.trim().to_string())
                    .filter(|o| !o.is_empty())
                else {
                    continue;
                };
                match &expected[i] {
                    Some(seen) if *seen != ty => {
                        expected[i] = None;
                        conflicting[i] = true;
                    }
                    _ => expected[i] = Some(ty),
                }
            }
        });
    }
    expected
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
/// Every binding resolved against its category's observable world. One value rather than a tuple
/// because the three maps are produced together and travel together — a resolution run is a single
/// fact about the subject at one moment, and splitting it into positional parts only invites a
/// caller to pass two of the three.
#[derive(Debug, Default)]
pub struct Resolutions {
    /// Category-1 predicates → the state predicate each resolves to (REQ025).
    pub code: BTreeMap<String, Resolution>,
    /// Category-1 sorts → the type each resolves to (REQ026, REQ058).
    pub sorts: BTreeMap<String, TypeResolution>,
    /// Category-2a symbols → the TLA+ definition each resolves to (REQ028).
    pub model: BTreeMap<String, ModelResolution>,
    /// Category-2b symbols → the declared event each resolves to (#231). Against the operator's
    /// `monitor:` declaration, never against the subject's code: a 2b claim speaks of events in a
    /// log, and a Rust function that happens to share the name is not that event.
    pub runtime: BTreeMap<String, crate::monitor::RuntimeResolution>,
    /// Category-3 symbols → the declared step each resolves to (#241). Against the operator's `ui:`
    /// declaration, for the same reason `runtime` is: a category-3 claim speaks of what a browser
    /// does to a running page, and a Rust function sharing the name is not that step.
    pub ui: BTreeMap<String, crate::ui::UiResolution>,
    /// A fingerprint of the specs resolved against that live **outside** the subject tree, or
    /// `None` when there are none (#120). Produced here because this is the walk that already read
    /// them; consumed by the verify flow, which stamps it on the verdict so an external spec moving
    /// makes that verdict stale. Without it, a verdict proved against a sibling repo's spec would
    /// read `fresh` forever — the subject's commit does not cover a file outside the subject.
    pub spec_fingerprint: Option<String>,
}

impl Resolutions {
    /// What one binding resolved to: whether it grounds, and the operator-facing read-back.
    ///
    /// **The single answer to that question.** The CLI dry-run and the detail surface used to
    /// hand-roll the same if-else chain over the four maps, which is the #218 defect wearing a
    /// different hat: one binding read differently depending on which surface the operator looked
    /// at, and wiring a new category meant remembering both. #241 found exactly that live — `verify`
    /// said GROUNDED while `--dry-run` still printed "engine not wired yet" for the same binding.
    ///
    /// A symbol in none of the maps is reported as unresolved, never as absent: the resolver not
    /// having answered is not evidence that it grounds, the same rule [`verdict`] applies.
    pub fn describe(&self, binding: &Binding) -> (bool, String) {
        let symbol = &binding.symbol;
        let observable = &binding.observable;
        // A sort binds to a type and a predicate to a function, so the sort map is consulted first
        // — see `verdict`, where the same order keeps a `struct login` from answering for the
        // predicate `login`.
        if let Some(r) = self.sorts.get(symbol) {
            (r.is_resolved(), r.describe(symbol, observable))
        } else if let Some(r) = self.code.get(symbol) {
            (r.is_resolved(), r.describe(symbol, observable))
        } else if let Some(r) = self.model.get(symbol) {
            (r.is_resolved(), r.describe(symbol, observable))
        } else if let Some(r) = self.runtime.get(symbol) {
            (r.is_resolved(), r.describe(symbol, observable))
        } else if let Some(r) = self.ui.get(symbol) {
            (r.is_resolved(), r.describe(symbol, observable))
        } else {
            (
                false,
                format!(
                    "{symbol} → `{observable}` (category {}): no resolver answered for it, so it \
                     does not ground",
                    binding.category.as_label()
                ),
            )
        }
    }
}

pub fn resolve_bindings(
    subject: &Path,
    companion: &Path,
    requirement: &Requirement,
    bindings: &[Binding],
) -> Resolutions {
    let in_category = |cat| {
        bindings
            .iter()
            .filter(move |b| b.category == cat)
            .collect::<Vec<_>>()
    };
    // Walk and parse the subject ONCE for the whole binding set: every code lookup below reads this
    // one tree instead of starting its own walk, which is where a four-binding requirement's ten
    // full parses of the same source went (#144).
    let parsed = crate::rust_adapter::ParsedSubject::load(subject, companion);
    // Where the model lives is the operator's choice (#120): the subject tree, plus any root
    // `provreq.yml` names. Read here rather than passed in, because every caller of this already
    // has exactly the two paths it needs and none of them has an opinion about the manifest.
    let spec_paths = crate::spec_paths::SpecPaths::load(subject, companion);
    let specs = crate::tla_adapter::SubjectSpecs::load(subject, companion, &spec_paths);
    let code = in_category(BindCategory::Code);
    // Sorts first, and the order is load-bearing (#198): a predicate's parameter check compares
    // against the type its argument's sort resolved to, so it cannot run until that is known.
    // Resolved in the other order, the check compared against whatever text the sort binding held —
    // including text naming no type at all — and parked correct predicate bindings for disagreeing
    // with it.
    let sorts: BTreeMap<String, TypeResolution> = code
        .iter()
        .filter(|b| is_sort(requirement, &b.symbol))
        .map(|b| {
            (
                b.symbol.clone(),
                crate::rust_adapter::resolve_type(&parsed, &b.observable),
            )
        })
        .collect();
    let predicates = code
        .iter()
        .filter(|b| !is_sort(requirement, &b.symbol))
        .map(|b| {
            let params = expected_param_types(requirement, bindings, &sorts, &b.symbol);
            (
                b.symbol.clone(),
                crate::rust_adapter::resolve(&parsed, &b.observable, &params),
            )
        })
        .collect();
    // The arity a 2a binding must satisfy is the requirement's own, exactly as a cat-1
    // predicate's is (`expected_param_types` above computes the same number as its length). The
    // gate has already forced every use of a symbol to match its declaration, so the declared
    // arity is what the lowering will emit. A sort is applied to nothing — it becomes the set a
    // quantifier ranges over — so `unwrap_or(0)` is the right reading for a symbol that declares
    // no parameters, not a fallback.
    let model = in_category(BindCategory::Model)
        .iter()
        .map(|b| {
            let arity = predicate_arity(requirement, &b.symbol).unwrap_or(0);
            (
                b.symbol.clone(),
                crate::tla_adapter::resolve(&specs, &b.observable, arity),
            )
        })
        .collect();
    // Category 2b resolves against the operator's declaration, not the subject (#231). The trace is
    // read once for the whole binding set — `occurrences` is a dry-run aid, so an unreadable trace
    // is `None` here rather than a failure; the loud version runs at verification time.
    let monitor = crate::monitor::Monitor::load(subject, companion)
        .ok()
        .flatten();
    let counts = monitor.as_ref().and_then(crate::monitor::occurrences);
    let runtime = in_category(BindCategory::Runtime)
        .iter()
        .map(|b| {
            // A sort is not an event: a monitor binds a quantified variable from the trace's own
            // values, so there is no domain to look up (see `RuntimeResolution::TraceBound`).
            // Asking the event resolver for it would park every quantified 2b claim there is.
            let resolution = if is_sort(requirement, &b.symbol) {
                crate::monitor::RuntimeResolution::TraceBound
            } else {
                let arity = predicate_arity(requirement, &b.symbol).unwrap_or(0);
                crate::monitor::resolve(monitor.as_ref(), &b.observable, arity, counts.as_ref())
            };
            (b.symbol.clone(), resolution)
        })
        .collect();
    // Category 3 resolves against the declared steps (#241). No trace to read and nothing to count:
    // nothing has run yet and nothing may ever run, so there is no occurrence analogue to 2b's
    // vacuity warning and inventing one would be worse than silence. Note this deliberately does
    // NOT consult the WebDriver endpoint — whether a grid is reachable is a fact about the
    // operator's machine (#239), and a binding that flipped between grounded and parked as a
    // container came and went would be reporting the weather rather than the requirement.
    let ui_check = crate::ui::Ui::load(companion).ok().flatten();
    let ui = in_category(BindCategory::Ui)
        .iter()
        .map(|b| {
            let arity = predicate_arity(requirement, &b.symbol).unwrap_or(0);
            let resolution = crate::ui::resolve(
                ui_check.as_ref(),
                &b.observable,
                arity,
                is_sort(requirement, &b.symbol),
            );
            (b.symbol.clone(), resolution)
        })
        .collect();
    Resolutions {
        code: predicates,
        sorts,
        model,
        runtime,
        ui,
        spec_fingerprint: specs.external_fingerprint(),
    }
}

/// Decide grounding from one resolution run. Takes the whole [`Resolutions`] rather than its maps
/// positionally for the reason that type already documents: the maps are produced together and
/// travel together, and splitting them at a call site only invites passing three of the four.
pub fn verdict(req: &Requirement, bindings: &[Binding], resolved: &Resolutions) -> Grounding {
    let (resolutions, sort_resolutions, model_resolutions) =
        (&resolved.code, &resolved.sorts, &resolved.model);
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
            // Category 2b: against the declared event signature (#231), never the subject's code.
            BindCategory::Runtime => match resolved.runtime.get(&b.symbol) {
                Some(r) if r.is_resolved() => {}
                Some(r) => reasons.push(r.describe(&b.symbol, &b.observable)),
                None => reasons.push(format!(
                    "{}: `{}` was not resolved against the declared event signature",
                    b.symbol, b.observable
                )),
            },
            // Category 3: against the declared steps (#241), never the subject's code. A sort
            // lands on `NoDomain` here rather than resolving — see `crate::ui::binding`.
            BindCategory::Ui => match resolved.ui.get(&b.symbol) {
                Some(r) if r.is_resolved() => {}
                Some(r) => reasons.push(r.describe(&b.symbol, &b.observable)),
                None => reasons.push(format!(
                    "{}: `{}` was not resolved against the declared UI steps",
                    b.symbol, b.observable
                )),
            },
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

    /// One resolution run from the three maps a test cares about. `verdict` takes the whole run
    /// (see its doc), so the tests build one rather than passing maps positionally.
    fn run(
        code: &BTreeMap<String, Resolution>,
        sorts: &BTreeMap<String, TypeResolution>,
        model: &BTreeMap<String, ModelResolution>,
    ) -> Resolutions {
        Resolutions {
            code: code.clone(),
            sorts: sorts.clone(),
            model: model.clone(),
            ..Default::default()
        }
    }
    use crate::rust_adapter::PredicateForm;

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
            module: Some(vec![]),
        }
    }

    /// A resolved nullary predicate. This module's verdict asks only whether a binding
    /// resolved, so the parameter modes an engine would need are irrelevant here.
    fn resolved(file: &str) -> Resolution {
        Resolution::Resolved {
            at: at(file),
            params: vec![],
            form: PredicateForm::Function,
        }
    }

    /// Sort resolutions in which every named sort resolved — the ordinary case, and the one the
    /// parameter cross-check is allowed to speak in (#198). A sort absent from this map counts as
    /// unresolved, which is the whole point: the check stays silent about it.
    fn sorts_resolved(names: &[&str]) -> BTreeMap<String, TypeResolution> {
        names
            .iter()
            .map(|s| (s.to_string(), TypeResolution::Resolved(at("src/a.rs"))))
            .collect()
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
            verdict(&r, &bindings, &run(&resolutions, &sorts, &BTreeMap::new())),
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
            &run(&resolutions, &BTreeMap::new(), &BTreeMap::new()),
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
        let Grounding::Parked { reasons } = verdict(
            &r,
            &bindings,
            &run(&only_one, &BTreeMap::new(), &BTreeMap::new()),
        ) else {
            panic!("an unresolved-by-omission binding must park");
        };
        assert!(reasons.iter().any(|reason| reason.contains("has_session")));
    }

    // Verifies: REQ021 (R-ground-1) — a binding the caller did not resolve parks, never grounds by
    // default. This test used to pin MEMBERSHIP — "category 3 is the deferred one" — and its own
    // comment called category 3 the last one left. #241 wired it, so there is no unwired category
    // to rewrite around any more, and the membership reading is gone for good. What it pins now is
    // the rule that outlives every category being wired: **the caller having skipped a symbol is
    // not evidence that it grounds**, whichever world the symbol lives in. The 2b twin below says
    // the same thing about `BindCategory::Runtime`.
    #[test]
    fn a_binding_the_caller_did_not_resolve_parks_in_every_category() {
        let r = req("requirement r {
            category: 3
            vocabulary { event fired(x) }
            require { always fired(x) }
        }");
        for (category, fidelity, world) in [
            (BindCategory::Ui, Fidelity::Probed, "declared UI steps"),
            (BindCategory::Runtime, Fidelity::Observed, "declared event"),
            (BindCategory::Model, Fidelity::Definitional, "TLA+ spec"),
            (BindCategory::Code, Fidelity::Definitional, "source"),
        ] {
            let bindings = vec![Binding {
                symbol: "fired".into(),
                category,
                observable: "#submit".into(),
                fidelity,
            }];
            let Grounding::Parked { reasons } = verdict(&r, &bindings, &Resolutions::default())
            else {
                panic!("an unresolved {} binding must park", category.as_label());
            };
            assert!(
                reasons
                    .iter()
                    .any(|reason| reason.contains("fired") && reason.contains(world)),
                "category {}: {reasons:?}",
                category.as_label()
            );
        }
    }

    // Verifies: #231 (R-ground-1) — a 2b binding the caller did not resolve is treated exactly as a
    // failed resolution, not as an absent question. The caller having skipped a symbol is never
    // evidence that it grounds — the same rule the code and model arms have carried since REQ025.
    #[test]
    fn an_unresolved_runtime_binding_parks_rather_than_grounding_by_default() {
        let r = req(&MODEL_REQ.replace("category: 2a", "category: 2b"));
        let bindings = vec![Binding {
            symbol: "accepted".into(),
            category: BindCategory::Runtime,
            observable: "accepted".into(),
            fidelity: Fidelity::Observed,
        }];
        let Grounding::Parked { reasons } = verdict(&r, &bindings, &Resolutions::default()) else {
            panic!("an unresolved runtime binding must park");
        };
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("declared event signature")),
            "{reasons:?}"
        );
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

    // Verifies: REQ059 — a type declared on a predicate's parameter is a sort, so it must be bound
    // like any other. Without this the closure could never lower: the variable's domain would name
    // a type nothing had confirmed exists.
    #[test]
    fn a_declared_parameter_type_is_a_bindable_sort() {
        let r = req("requirement r {
            category: 1
            vocabulary { state proceeds(d: Decision, f: Flag) }
            require { always proceeds(d, f) }
        }");
        assert_eq!(bindable_sorts(&r), vec!["Decision", "Flag"]);
        assert!(is_sort(&r, "Decision"));
        assert!(is_bindable(&r, "Flag"));
        // …and it parks the requirement until it is bound, exactly as a quantifier's sort does.
        assert!(unbound_symbols(&r, &[]).contains(&"Decision".to_string()));
    }

    // Verifies: REQ057 + REQ059 — with every parameter's sort declared, the parameter-type
    // cross-check now covers EVERY position, not only the one an `each` binder supplied.
    #[test]
    fn declared_parameter_sorts_type_check_every_position() {
        let r = req("requirement r {
            category: 1
            vocabulary { state proceeds(d: Decision, f: Flag) }
            require { always proceeds(d, f) }
        }");
        let bindings = vec![
            code_binding("proceeds", "decide"),
            sort_binding("Decision", "InstallDecision"),
            sort_binding("Flag", "bool"),
        ];
        assert_eq!(
            expected_param_types(
                &r,
                &bindings,
                &sorts_resolved(&["Decision", "Flag"]),
                "proceeds"
            ),
            vec![
                Some("InstallDecision".to_string()),
                Some("bool".to_string())
            ]
        );
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
            &run(&resolutions, &BTreeMap::new(), &BTreeMap::new()),
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
            verdict(&r, &bindings, &run(&resolutions, &sorts, &BTreeMap::new()))
        else {
            panic!("an unresolved sort must park");
        };
        assert!(reasons.iter().any(|reason| reason.contains("NoSuchType")));
    }

    // Verifies: REQ057 — the type a parameter is expected to take comes from the sort its
    // argument ranges over, through that sort's own binding. This is the fact the adapter cannot
    // know: it reads the subject, and the sort is the requirement's claim.
    #[test]
    fn expected_param_types_follow_the_quantified_arguments_sort() {
        let r = req(CODE_REQ);
        let bindings = vec![
            code_binding("logged_in", "login"),
            sort_binding("User", "AuthUser"),
        ];
        let resolved_sorts = sorts_resolved(&["User"]);
        assert_eq!(
            expected_param_types(&r, &bindings, &resolved_sorts, "logged_in"),
            vec![Some("AuthUser".to_string())]
        );

        // An unbound sort says nothing about the parameter — the requirement parks on the
        // unbound sort itself, which is the honest reason.
        let unbound = vec![code_binding("logged_in", "login")];
        assert_eq!(
            expected_param_types(&r, &unbound, &resolved_sorts, "logged_in"),
            vec![None]
        );

        // A predicate the requirement does not declare has no parameters to speak for.
        assert!(expected_param_types(&r, &bindings, &resolved_sorts, "not_declared").is_empty());
    }

    // Verifies: REQ057 (#198) — a sort that is BOUND but did not RESOLVE says nothing about the
    // parameter either. The expected type is only the text an operator wrote, and text naming no
    // type is not an established type to compare against. Measured on a real subject: binding a
    // sort to `Nope` parked the predicate too, for disagreeing with a type that does not exist —
    // and that predicate binding had resolved correctly, to the right function, at the right line.
    #[test]
    fn a_sort_that_did_not_resolve_says_nothing_about_the_parameter() {
        let r = req(CODE_REQ);
        let bindings = vec![
            code_binding("logged_in", "login"),
            sort_binding("User", "Nope"),
        ];
        // Every way a sort binding can fail to resolve is the same answer here: nothing claimed.
        for outcome in [
            TypeResolution::NotFound,
            TypeResolution::Ambiguous(vec![at("src/a.rs"), at("src/b.rs")]),
            TypeResolution::QualifierUnmatched {
                name: "Nope".into(),
                candidates: vec![at("src/a.rs")],
            },
            TypeResolution::UnusableTypeArguments {
                reason: "…".into()
            },
        ] {
            let sorts = BTreeMap::from([("User".to_string(), outcome.clone())]);
            assert_eq!(
                expected_param_types(&r, &bindings, &sorts, "logged_in"),
                vec![None],
                "an unresolved sort ({outcome:?}) must not be compared against"
            );
        }

        // Not knowing whether the sort resolved is not knowing the type, so an absent entry is
        // silence too — never a comparison against the raw observable.
        assert_eq!(
            expected_param_types(&r, &bindings, &BTreeMap::new(), "logged_in"),
            vec![None]
        );

        // The length is still the declared arity: `resolve` checks arity against it, so silencing
        // a position must never shorten the vector.
        assert_eq!(
            expected_param_types(&r, &bindings, &BTreeMap::new(), "logged_in").len(),
            1
        );
    }

    // Verifies: REQ057 — every position the requirement cannot speak for stays `None`, so the
    // adapter never parks a binding on a type this module guessed at. An argument that is not the
    // quantified variable is exactly the free-variable case (#136), and two properties quantifying
    // the same position over different sorts is a disagreement in the requirement, not a fact
    // about the subject's code.
    #[test]
    fn a_position_the_requirement_cannot_speak_for_stays_unknown() {
        let free = req("requirement r {
            category: 1
            vocabulary { state pair(a, b) }
            require { each u: User . always pair(u, other) }
        }");
        let bindings = vec![code_binding("pair", "pair"), sort_binding("User", "User")];
        assert_eq!(
            expected_param_types(&free, &bindings, &sorts_resolved(&["User"]), "pair"),
            vec![Some("User".to_string()), None]
        );

        let conflicting = req("requirement r {
            category: 1
            vocabulary { state p(x) }
            require {
                each u: User . always p(u)
                each s: Session . always p(s)
            }
        }");
        let two_sorts = vec![
            code_binding("p", "p"),
            sort_binding("User", "User"),
            sort_binding("Session", "Session"),
        ];
        assert_eq!(
            expected_param_types(
                &conflicting,
                &two_sorts,
                &sorts_resolved(&["User", "Session"]),
                "p"
            ),
            vec![None]
        );
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
            verdict(
                &r,
                &bindings,
                &run(&BTreeMap::new(), &BTreeMap::new(), &model)
            ),
            Grounding::Grounded
        );
    }

    // Verifies: #231 — a category-2b binding grounds against the DECLARED EVENT SIGNATURE, and
    // parks carrying that adapter's own explanation. `BindCategory::Runtime` had a label and no
    // adapter; a 2b requirement could not ground at all, whatever the operator declared.
    #[test]
    fn runtime_requirement_grounds_against_the_declared_events() {
        let r = req(&MODEL_REQ.replace("category: 2a", "category: 2b"));
        let runtime_binding = |symbol: &str, observable: &str| Binding {
            symbol: symbol.into(),
            category: BindCategory::Runtime,
            observable: observable.into(),
            fidelity: Fidelity::Observed,
        };
        let bindings = vec![
            runtime_binding("accepted", "accepted"),
            runtime_binding("succeeded", "succeeded"),
            runtime_binding("Message", "Message"),
        ];
        let event = |name: &str, args: usize| crate::monitor::RuntimeResolution::Resolved {
            event: crate::monitor::Event {
                name: name.into(),
                args: vec!["id".to_string(); args],
            },
            occurrences: Some(3),
        };
        let all_declared = BTreeMap::from([
            ("accepted".to_string(), event("msg_accepted", 1)),
            ("succeeded".to_string(), event("msg_done", 1)),
            ("Message".to_string(), event("message", 0)),
        ]);
        assert_eq!(
            verdict(
                &r,
                &bindings,
                &Resolutions {
                    runtime: all_declared.clone(),
                    ..Default::default()
                }
            ),
            Grounding::Grounded
        );

        // An undeclared event parks, and the reason is the adapter's own — the operator reads one
        // account of what happened rather than a summary of it.
        let mut missing = all_declared;
        missing.insert(
            "succeeded".to_string(),
            crate::monitor::RuntimeResolution::NotDeclared {
                declared: vec!["accepted".into()],
            },
        );
        let Grounding::Parked { reasons } = verdict(
            &r,
            &bindings,
            &Resolutions {
                runtime: missing,
                ..Default::default()
            },
        ) else {
            panic!("an undeclared event must park");
        };
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("succeeded") && reason.contains("monitor.events")),
            "{reasons:?}"
        );
    }

    // Verifies: #241 — `describe` agrees with `verdict` for every category, which is what the two
    // hand-rolled read-back chains did not. Found live: `verify` reported GROUNDED for a cat-3
    // requirement while `--dry-run` printed "engine not wired yet" for the very same bindings,
    // because the CLI's chain had no `ui` arm. A binding that grounds must never read as one that
    // does not, on any surface — the #218 rule, applied to the dry-run instead of the verdict.
    #[test]
    fn describe_agrees_with_verdict_for_every_category() {
        let cases: Vec<(BindCategory, Fidelity, Resolutions)> = vec![
            (
                BindCategory::Ui,
                Fidelity::Probed,
                Resolutions {
                    ui: BTreeMap::from([(
                        "fired".to_string(),
                        crate::ui::UiResolution::Resolved {
                            alias: "fired".into(),
                            step: crate::ui::Step::Click("#submit".into()),
                        },
                    )]),
                    ..Default::default()
                },
            ),
            (
                BindCategory::Runtime,
                Fidelity::Observed,
                Resolutions {
                    runtime: BTreeMap::from([(
                        "fired".to_string(),
                        crate::monitor::RuntimeResolution::TraceBound,
                    )]),
                    ..Default::default()
                },
            ),
        ];

        let r = req("requirement r {
            category: 3
            vocabulary { state fired }
            require { always fired }
        }");
        for (category, fidelity, resolved) in cases {
            let b = Binding {
                symbol: "fired".into(),
                category,
                observable: "fired".into(),
                fidelity,
            };
            let (describes_resolved, summary) = resolved.describe(&b);
            let grounds = matches!(
                verdict(&r, std::slice::from_ref(&b), &resolved),
                Grounding::Grounded
            );
            assert_eq!(
                describes_resolved,
                grounds,
                "category {}: the read-back and the verdict disagree — {summary}",
                category.as_label()
            );
            assert!(
                !summary.contains("not wired"),
                "category {} is wired; its read-back must not say otherwise: {summary}",
                category.as_label()
            );
        }

        // And a symbol no resolver answered for reads as unresolved on both, never as absent.
        let orphan = Binding {
            symbol: "fired".into(),
            category: BindCategory::Ui,
            observable: "fired".into(),
            fidelity: Fidelity::Probed,
        };
        let (resolved_flag, summary) = Resolutions::default().describe(&orphan);
        assert!(!resolved_flag, "{summary}");
        assert!(summary.contains("does not ground"), "{summary}");
    }

    // Verifies: #241 — a category-3 binding grounds against the DECLARED STEPS. `BindCategory::Ui`
    // had a label and no adapter, so a cat-3 requirement fell through the catch-all arm and could
    // never ground, whatever the operator declared.
    #[test]
    fn ui_requirement_grounds_against_the_declared_steps() {
        let r = req("requirement u {
            category: 3
            vocabulary { state checkout state sees_total }
            require { checkout leads_to sees_total }
        }");
        let ui_binding = |symbol: &str| Binding {
            symbol: symbol.into(),
            category: BindCategory::Ui,
            observable: symbol.into(),
            fidelity: Fidelity::Probed,
        };
        let bindings = vec![ui_binding("checkout"), ui_binding("sees_total")];
        let step = |alias: &str, step: crate::ui::Step| {
            (
                alias.to_string(),
                crate::ui::UiResolution::Resolved {
                    alias: alias.into(),
                    step,
                },
            )
        };
        let declared = BTreeMap::from([
            step("checkout", crate::ui::Step::Click("#go".into())),
            step("sees_total", crate::ui::Step::TextPresent("Total".into())),
        ]);
        assert_eq!(
            verdict(
                &r,
                &bindings,
                &Resolutions {
                    ui: declared.clone(),
                    ..Default::default()
                }
            ),
            Grounding::Grounded
        );

        // An undeclared step parks, carrying the adapter's own account rather than a summary.
        let mut missing = declared;
        missing.insert(
            "sees_total".to_string(),
            crate::ui::UiResolution::NotDeclared {
                declared: vec!["checkout".into()],
            },
        );
        let Grounding::Parked { reasons } = verdict(
            &r,
            &bindings,
            &Resolutions {
                ui: missing,
                ..Default::default()
            },
        ) else {
            panic!("an undeclared step must park");
        };
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("sees_total") && reason.contains("ui.steps")),
            "{reasons:?}"
        );
    }

    // Verifies: #241 — the decision this slice turns on. Category 2b grounds a quantified claim
    // because a monitor binds the variable from its trace's own values (`TraceBound`, #232). A UI
    // driver runs a fixed script and has no such source, so the same claim must PARK here — the
    // mirror image, not a copy. Grounding it would tell the operator a claim was fine and leave the
    // refusal to a lowering they have not reached yet.
    #[test]
    fn a_quantified_ui_claim_parks_where_the_same_2b_claim_grounds() {
        let quantified = "requirement q {
            category: 3
            vocabulary { sort Item event added(i) state shown(i) }
            require { each i: Item . added(i) leads_to shown(i) }
        }";
        // 2b: the sort resolves, because the trace supplies the domain.
        assert!(
            crate::monitor::RuntimeResolution::TraceBound.is_resolved(),
            "the 2b answer this must NOT be copied from"
        );

        // 3: the same sort has nothing to draw a domain from.
        let r3 = req(quantified);
        let bindings_3 = vec![Binding {
            symbol: "Item".into(),
            category: BindCategory::Ui,
            observable: "Item".into(),
            fidelity: Fidelity::Probed,
        }];
        let resolved = resolve_ui_only(&r3, &bindings_3);
        assert_eq!(
            resolved.ui.get("Item"),
            Some(&crate::ui::UiResolution::NoDomain),
            "a cat-3 sort must not borrow 2b's TraceBound answer"
        );
        let Grounding::Parked { reasons } = verdict(&r3, &bindings_3, &resolved) else {
            panic!("a quantified category-3 claim must park");
        };
        assert!(
            reasons.iter().any(|r| r.contains("fixed script")),
            "{reasons:?}"
        );
    }

    /// Resolve only the category-3 bindings of `req`, with no `ui:` block on disk — enough to pin
    /// the sort decision without a subject tree, since a sort never reaches the declaration.
    fn resolve_ui_only(req: &Requirement, bindings: &[Binding]) -> Resolutions {
        Resolutions {
            ui: bindings
                .iter()
                .map(|b| {
                    (
                        b.symbol.clone(),
                        crate::ui::resolve(
                            None,
                            &b.observable,
                            predicate_arity(req, &b.symbol).unwrap_or(0),
                            is_sort(req, &b.symbol),
                        ),
                    )
                })
                .collect(),
            ..Default::default()
        }
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
        let Grounding::Parked { reasons } = verdict(
            &r,
            &bindings,
            &run(&BTreeMap::new(), &BTreeMap::new(), &model),
        ) else {
            panic!("an unresolved model binding must park");
        };
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("succeeded") && reason.contains("NoSuchOp")));
    }

    // Verifies: REQ028 (#119) — a 2a binding to a definition of the wrong arity parks, and the
    // reason names both counts. Grounding is where the operator can act: the binding is in front
    // of them, whereas TLC would answer with a location inside a generated module that provreq
    // deletes before the verdict is printed.
    #[test]
    fn model_requirement_parks_when_a_binding_has_the_wrong_arity() {
        let r = req(MODEL_REQ);
        let bindings = vec![
            model_binding("accepted", "queue"),
            model_binding("succeeded", "Succeeded"),
            model_binding("Message", "Message"),
        ];
        let model = BTreeMap::from([
            (
                "accepted".to_string(),
                ModelResolution::WrongArity {
                    at: spec_at(),
                    declared: 0,
                    expected: 1,
                },
            ),
            (
                "succeeded".to_string(),
                ModelResolution::Resolved(spec_at()),
            ),
            ("Message".to_string(), ModelResolution::Resolved(spec_at())),
        ]);
        let Grounding::Parked { reasons } = verdict(
            &r,
            &bindings,
            &run(&BTreeMap::new(), &BTreeMap::new(), &model),
        ) else {
            panic!("a wrong-arity model binding must park");
        };
        assert_eq!(
            reasons.len(),
            1,
            "one mistake earns one reason: {reasons:?}"
        );
        assert!(
            reasons[0].contains("takes no arguments") && reasons[0].contains("to 1 argument"),
            "{reasons:?}"
        );
    }

    // Verifies: REQ028 (#119) — the arity a 2a binding is checked against is the REQUIREMENT's,
    // read from its vocabulary. `accepted` is declared with one parameter, so a definition
    // taking none disagrees; a sort declares no parameters and is applied to none.
    #[test]
    fn the_arity_a_model_binding_must_match_comes_from_the_vocabulary() {
        let r = req(MODEL_REQ);
        assert_eq!(predicate_arity(&r, "accepted"), Some(1));
        assert_eq!(
            predicate_arity(&r, "Message"),
            None,
            "a sort, not a predicate"
        );
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
        let Grounding::Parked { reasons } = verdict(
            &r,
            &bindings,
            &run(&BTreeMap::new(), &BTreeMap::new(), &only_two),
        ) else {
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
