//! Shared claim-lowering — the one place a gated category-1 PRL claim becomes a Rust boolean
//! expression, used by every cat-1 engine ([`crate::kani`], [`crate::creusot`],
//! [`crate::prusti`]).
//!
//! D2 gives the core one meaning and lowers it to each engine. The *shape* of that lowering —
//! `always`/`never` over boolean combinations of resolved predicates, optionally quantified —
//! is identical for all three; only the **assertion wrapper** differs (Kani's `assert!` over a
//! `kani::any()`, Creusot's `proof_assert! { forall<> }`, Prusti's `prusti_assert!(forall(||))`).
//! This module owns the identical part; each engine owns only its wrapper and how it runs.
//!
//! The one axis of variation folded in here is a **path prefix**: Kani's harness lives in a
//! `tests/` crate and reaches the subject through its public API (`{crate_name}::…`), while
//! Creusot's and Prusti's harnesses are in-crate modules that reach it through `crate::…`. The
//! caller passes the prefix; everything else is shared.
//!
//! Pure — the caller resolves the bindings and passes them in, so the whole lowering is testable
//! without any engine installed, which is what lets CI prove the engine-absent path continuously
//! (R-eng-2).
//!
//! What cannot be faithfully expressed — a scope, a guard, an argument that is not a variable the
//! claim ranges over — is a [`NotLowerable`], which each engine turns into an honest
//! `unknown`/`inconclusive`.
//! D2's rule: an out-of-fragment operator is a typed error surfaced to the author, never a silent
//! approximation.
//!
//! Extracted from the three engines once Prusti made a third copy (rule of three, #69).
//!
//! Implements: REQ062 (what is emitted is in the intersection of what every wired checker reads —
//! valid Rust is not sufficient when the interior of an assertion is a checker's own logic),
//! REQ066 (a boolean variable the claim ranges over lowers to itself as a condition, and a
//! non-boolean one is refused here with the reason rather than left to the checker).

use crate::grounding::Binding;
use crate::prl::ast::{Atom, Binder, Expr, Pattern, Property, Requirement, Scope};
use crate::rust_adapter::{CodeMatch, ParamMode, PredicateForm, Resolution, TypeResolution};
use std::collections::BTreeMap;

/// Why a gated category-1 requirement could not be lowered to a harness. Never an approximation —
/// the reason is the operator's to read and act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotLowerable {
    pub reason: String,
}

impl NotLowerable {
    pub(crate) fn new(reason: impl Into<String>) -> Self {
        NotLowerable {
            reason: reason.into(),
        }
    }
}

/// One lowered `require` claim: the boolean expression (with the path prefix already baked into
/// every predicate call), plus the quantifier to range it over when the claim is a ∀. Each engine
/// wraps these two in its own assertion syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredClaim {
    /// The claim as a Rust boolean expression, e.g. `crate::in_range(&u)`.
    pub claim: String,
    /// Every variable the claim ranges over, in the order a reader meets them — empty for a ground
    /// claim. `ty` is already qualified as the harness must write it (`crate::User`, or a bare
    /// `bool`), so the engine only supplies the `∀` syntax around them.
    pub quantified: Vec<Quantified>,
}

/// A quantifier lowered for a harness: the variable and the (already prefix-qualified) sort type
/// it ranges over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quantified {
    pub var: String,
    pub ty: String,
}

/// The harness function/module name for a requirement id — a valid Rust identifier, prefixed so it
/// cannot collide with the subject's own items.
pub fn harness_name(id: &str) -> String {
    let sanitized: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("provreq_{}", sanitized.to_ascii_lowercase())
}

/// Lower one `require` claim to its boolean expression plus its binders.
///
/// `prefix` is how the harness reaches the subject's items: the subject's crate name for Kani's
/// out-of-crate `tests/` harness, or `crate` for the in-crate Creusot/Prusti harnesses. The rest of
/// each path comes from where the adapter **found** the item, so `sort_resolutions` is needed for
/// the same reason `resolutions` is: a sort's module is a fact about the subject, not about the
/// binding (REQ061).
pub fn lower_property(
    req: &Requirement,
    prop: &Property,
    prefix: &str,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
    sort_resolutions: &BTreeMap<String, TypeResolution>,
) -> Result<LoweredClaim, NotLowerable> {
    let binders = req.binders(prop);
    // A variable standing as a condition has to BE a condition (REQ066). The gate cannot check
    // this — it resolves names and arities, not types — so it is checked here, where the sort's
    // real type is known, and refused with the reason rather than emitted for the compiler to
    // reject as a harness that does not build.
    if let Some(reason) = non_boolean_condition(prop, &binders, sort_resolutions) {
        return Err(NotLowerable::new(reason));
    }
    if prop.scope != Scope::Globally {
        return Err(NotLowerable::new(
            "the claim is limited to a scope (`before`/`after`/`between`), which names a \
             moment in a run — a deductive or bounded checker sees one state, not a history",
        ));
    }
    // The gate guarantees a category-1 requirement is temporal-free (REQ024), so only these two
    // can arrive. The match stays total anyway: this is public and must not depend on a caller
    // having gated first.
    let claim = match &prop.pattern {
        Pattern::Always(e) => lower_expr(e, &binders, prefix, bindings, resolutions)?,
        // `never P` is `always not P`.
        Pattern::Never(e) => format!(
            "!({})",
            lower_expr(e, &binders, prefix, bindings, resolutions)?
        ),
        other => {
            return Err(NotLowerable::new(format!(
                "`{}` is not an invariant, and the code fragment is temporal-free — the \
                 gate should have rejected it at category 1",
                pattern_verb(other)
            )))
        }
    };

    let quantified = binders
        .iter()
        .map(|b| {
            Ok(Quantified {
                var: b.var.clone(),
                ty: sort_type(b, prefix, bindings, sort_resolutions)?,
            })
        })
        .collect::<Result<Vec<_>, NotLowerable>>()?;
    Ok(LoweredClaim { claim, quantified })
}

/// How the harness names an item the subject declares: the path prefix, then the module the adapter
/// found it in, then the item (REQ061). `provreq::provision::decide_install`, not
/// `provreq::decide_install` — which is what this emitted before, correct only for a crate whose
/// every item sits in `src/lib.rs`.
///
/// A `None` module is a refusal, not a root: the item exists (grounding was right to say so) but no
/// path a harness can write reaches it — a separate crate target such as `tests/`, or a binary. That
/// is an honest `inconclusive` naming the file, rather than a guessed path that fails to compile.
fn item_path(prefix: &str, at: &CodeMatch, name: &str) -> Result<String, NotLowerable> {
    let module = at.module.as_ref().ok_or_else(|| {
        NotLowerable::new(format!(
            "`{name}` is declared in {}, which is not part of the crate a harness can import — a \
             separate target (`tests/`, `benches/`, `examples/`), a binary, or a crate root this \
             tool cannot locate. Nothing a harness writes would reach it.",
            at.file
        ))
    })?;
    Ok(std::iter::once(prefix)
        .chain(module.iter().map(String::as_str))
        .chain(std::iter::once(name))
        .collect::<Vec<_>>()
        .join("::"))
}

/// The type a binder ranges over, as the harness must write it.
///
/// Three ways this fails, and they are three different mistakes: the requirement never said what the
/// variable ranges over (REQ059), it said so but that sort is not bound to a type (which is why
/// REQ026 made sorts bindable), or it is bound but was never resolved against the subject. A
/// **primitive** is written bare, because `crate::bool` does not compile (REQ058); a declared type
/// is reached through its own module (REQ061).
/// Why a variable used as a bare condition cannot be one, or `None` when every such use is fine.
///
/// Only the language's own `bool` qualifies. A sort bound to a declared type is refused even if
/// that type happens to be an alias for `bool`: the adapter reads names, not types, so calling it
/// boolean would be this tool asserting something it did not establish.
fn non_boolean_condition(
    prop: &Property,
    binders: &[Binder],
    sort_resolutions: &BTreeMap<String, TypeResolution>,
) -> Option<String> {
    let mut refusal = None;
    prop.for_each_atom(&mut |a| {
        if refusal.is_some() || !a.args.is_empty() {
            return;
        }
        let Some(binder) = binders.iter().find(|b| b.var == a.name) else {
            return;
        };
        let Some(sort) = &binder.sort else {
            refusal = Some(format!(
                "`{}` is used as a condition, but it has no declared sort, so there is no way to \
                 tell whether it is one — declare it in the vocabulary (`state p({}: SomeSort)`)",
                a.name, a.name
            ));
            return;
        };
        refusal = match sort_resolutions.get(sort) {
            Some(TypeResolution::Primitive(ty)) if ty == "bool" => None,
            Some(TypeResolution::Primitive(ty)) => Some(format!(
                "`{}` is used as a condition, but its sort `{sort}` is `{ty}` — only a `bool` \
                 stands as a condition on its own",
                a.name
            )),
            Some(TypeResolution::Resolved(_)) => Some(format!(
                "`{}` is used as a condition, but its sort `{sort}` is a type the subject \
                 declares, not the language's `bool` — apply a predicate to `{}` instead of \
                 asserting it",
                a.name, a.name
            )),
            // Unresolved or ambiguous: `sort_type` refuses next, and names the sort. Saying it
            // twice, differently, would be worse than saying it once where it belongs.
            _ => None,
        };
    });
    refusal
}

fn sort_type(
    binder: &Binder,
    prefix: &str,
    bindings: &[Binding],
    sort_resolutions: &BTreeMap<String, TypeResolution>,
) -> Result<String, NotLowerable> {
    let sort = binder.sort.as_ref().ok_or_else(|| {
        NotLowerable::new(format!(
            "`{}` has no declared sort, so there is no domain to range over — give it one in the \
             vocabulary (`state p({}: SomeSort)`), consistently across the predicates that take it",
            binder.var, binder.var
        ))
    })?;
    let observable = bindings
        .iter()
        .find(|b| b.symbol == *sort)
        .map(|b| b.observable.trim().to_string())
        .ok_or_else(|| {
            NotLowerable::new(format!(
                "the sort `{sort}` is not bound to a type, so `{}` has no domain to range over",
                binder.var
            ))
        })?;
    match sort_resolutions.get(sort) {
        Some(TypeResolution::Primitive(name)) => Ok(name.clone()),
        Some(TypeResolution::Resolved(at)) => item_path(prefix, at, &observable),
        _ => Err(NotLowerable::new(format!(
            "the sort `{sort}` did not resolve to a type in the subject's source, so there is no \
             type to range `{}` over",
            binder.var
        ))),
    }
}

fn lower_expr(
    e: &Expr,
    binders: &[Binder],
    prefix: &str,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> Result<String, NotLowerable> {
    match e {
        Expr::Atom(a) => lower_atom(a, binders, prefix, bindings, resolutions),
        Expr::Not(inner) => Ok(format!(
            "!({})",
            lower_expr(inner, binders, prefix, bindings, resolutions)?
        )),
        Expr::And(l, r) => Ok(format!(
            "({} && {})",
            lower_expr(l, binders, prefix, bindings, resolutions)?,
            lower_expr(r, binders, prefix, bindings, resolutions)?
        )),
        Expr::Or(l, r) => Ok(format!(
            "({} || {})",
            lower_expr(l, binders, prefix, bindings, resolutions)?,
            lower_expr(r, binders, prefix, bindings, resolutions)?
        )),
    }
}

/// Lower one predicate application to a call on the subject's real function, through `prefix::`.
///
/// The call is generated from the signature the adapter actually resolved, so `&u` versus `u`
/// follows the subject's code rather than a guess. Whether the parameter's *type* matches the
/// quantifier's sort is checked at **grounding** (REQ057), by comparing written type names — so a
/// mismatch this module can reach is one no name comparison could see (a type alias, a generic
/// parameter), and it still surfaces as a harness that does not compile → `unknown`, never a wrong
/// verdict.
fn lower_atom(
    a: &Atom,
    binders: &[Binder],
    prefix: &str,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> Result<String, NotLowerable> {
    if let Some(guard) = &a.guard {
        return Err(NotLowerable::new(format!(
            "`{}` carries a `with` guard ({guard}), which the parser keeps as raw text — \
             lowering it would mean compiling text this tool never understood",
            a.name
        )));
    }
    // A bare name that is one of the claim's own variables is that variable, not a call: it is
    // already in scope as a `bool`, so the condition is the variable itself (REQ066). Checked
    // before the binding lookup because a variable has no binding of its own — its SORT is what
    // is bound, and binding the variable would be binding a name the requirement invented.
    if a.args.is_empty() {
        if let Some(binder) = binders.iter().find(|b| b.var == a.name) {
            return Ok(binder.var.clone());
        }
    }
    let binding = bindings
        .iter()
        .find(|b| b.symbol == a.name)
        .ok_or_else(|| {
            NotLowerable::new(format!(
                "`{}` is not bound to an observable, so there is nothing to call",
                a.name
            ))
        })?;
    let Some(Resolution::Resolved { params, form, .. }) = resolutions.get(&a.name) else {
        return Err(NotLowerable::new(format!(
            "`{}` did not resolve to a state predicate in the subject's source",
            a.name
        )));
    };
    if params.len() != a.args.len() {
        return Err(NotLowerable::new(format!(
            "`{}` is applied to {} argument(s) but `{}` takes {}",
            a.name,
            a.args.len(),
            binding.observable,
            params.len()
        )));
    }

    let mut args = Vec::new();
    for (arg, mode) in a.args.iter().zip(params) {
        let arg = arg.trim();
        // Only a bound variable can be instantiated. Any other term — a literal, a field access,
        // an expression — would compile to a name that exists in the requirement's world but not
        // in the harness's.
        if !binders.iter().any(|b| b.var == arg) {
            return Err(NotLowerable::new(format!(
                "`{}` is applied to `{arg}`, which is not a variable the claim ranges over — \
                 there is no value to give it",
                a.name
            )));
        }
        args.push(match mode {
            ParamMode::ByRef => format!("&{arg}"),
            ParamMode::ByValue => arg.to_string(),
        });
    }
    let Some(Resolution::Resolved { at, .. }) = resolutions.get(&a.name) else {
        unreachable!("the resolution was matched as Resolved just above")
    };
    lower_call(form, &binding.observable, &args, prefix, at)
}

/// Emit the call for one resolved predicate, in the shape its form requires (REQ055).
///
/// A method is called *on* its receiver, not passed it: lowering `fn is_ready(&self)` as
/// `prefix::is_ready(&u)` produces a harness that cannot compile, which reaches the operator as an
/// `unknown` with a compiler error rather than as the binding mistake it is.
fn lower_call(
    form: &PredicateForm,
    observable: &str,
    args: &[String],
    prefix: &str,
    at: &CodeMatch,
) -> Result<String, NotLowerable> {
    match form {
        PredicateForm::Function => Ok(format!(
            "{}({})",
            item_path(prefix, at, observable)?,
            args.join(", ")
        )),
        PredicateForm::Method { name } => {
            let (recv, rest) = args.split_first().ok_or_else(|| {
                NotLowerable::new(format!(
                    "`{name}` is an inherent method, so the predicate needs a first argument to \
                     call it on, but it is applied to none"
                ))
            })?;
            // The receiver takes its own reference: `u.is_ready()` auto-refs, and `(&u).is_ready()`
            // would double it when the method takes `&self`.
            let recv = recv.strip_prefix('&').unwrap_or(recv);
            Ok(format!("{recv}.{name}({})", rest.join(", ")))
        }
        PredicateForm::VariantTest {
            name,
            enum_name,
            variant,
            enum_module,
        } => {
            // The function and the enum are named independently: the enum it returns need not be
            // declared in the same module, and often is not (REQ061).
            let enum_at = CodeMatch {
                module: enum_module.clone(),
                ..at.clone()
            };
            // Written as a `match`, not the `matches!` macro it is sugar for: Pearlite (Creusot's
            // logic language) rejects every macro but `pearlite!`/`proof_assert!`/`seq!`, so a
            // `matches!` here reaches the operator as "unsupported expression" instead of a verdict.
            // The desugared form is what `matches!` expands to, so Kani and Prusti see no change.
            Ok(format!(
                "match {}({}) {{ {}::{variant} {{ .. }} => true, _ => false }}",
                item_path(prefix, at, name)?,
                args.join(", "),
                item_path(prefix, &enum_at, enum_name)?
            ))
        }
    }
}

fn pattern_verb(pattern: &Pattern) -> &'static str {
    match pattern {
        Pattern::Never(_) => "never",
        Pattern::Always(_) => "always",
        Pattern::Eventually(_) => "eventually",
        Pattern::LeadsTo { .. } => "leads_to",
        Pattern::Precedes { .. } => "precedes",
        Pattern::OccursAtMost { .. } => "occurs at most",
        Pattern::CanReach(_) => "can_reach",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grounding::{BindCategory, Fidelity};
    use crate::rust_adapter::CodeMatch;

    fn binding(symbol: &str, observable: &str) -> Binding {
        Binding {
            symbol: symbol.into(),
            category: BindCategory::Code,
            observable: observable.into(),
            fidelity: Fidelity::Definitional,
        }
    }

    fn resolved(params: Vec<ParamMode>, form: PredicateForm) -> Resolution {
        Resolution::Resolved {
            at: CodeMatch {
                file: "src/lib.rs".into(),
                line: 1,
                text: "…".into(),
                module: Some(vec![]),
            },
            params,
            form,
        }
    }

    /// Real PRL through the real gate, so these tests exercise the requirement an operator would
    /// actually write rather than an AST hand-built to suit them.
    fn gated(src: &str) -> Requirement {
        crate::prl::gate(src)
            .expect("test candidate should clear the gate")
            .requirement
    }

    /// `always p(u)` for each `u: Thing` — an explicit binder, the only shape that lowered before
    /// REQ059.
    const P_OF_U: &str = "requirement r { category: 1
        vocabulary { state p(u) }
        require { each u: Thing . always p(u) } }";

    /// Sort resolutions matching the fixtures' bindings: every declared sort is a type at the
    /// crate root, so a harness names it `<prefix>::<observable>` (REQ061). `bool` and friends
    /// resolve as primitives, which lower bare (REQ058).
    fn sorts(names: &[&str]) -> BTreeMap<String, TypeResolution> {
        names
            .iter()
            .map(|sort| {
                (
                    sort.to_string(),
                    TypeResolution::Resolved(CodeMatch {
                        file: "src/lib.rs".into(),
                        line: 1,
                        text: "pub struct T;".into(),
                        module: Some(vec![]),
                    }),
                )
            })
            .collect()
    }

    fn lower_one(
        req: &Requirement,
        prefix: &str,
        bindings: &[Binding],
        resolutions: &BTreeMap<String, Resolution>,
    ) -> Result<LoweredClaim, NotLowerable> {
        let declared: Vec<&str> = bindings
            .iter()
            .map(|b| b.symbol.as_str())
            .filter(|s| !resolutions.contains_key(*s))
            .collect();
        let mut by_sort = sorts(&declared);
        // A binding whose observable is a primitive resolves as one, not as a declared type.
        for b in bindings {
            if crate::rust_adapter::is_primitive(b.observable.trim()) {
                by_sort.insert(
                    b.symbol.clone(),
                    TypeResolution::Primitive(b.observable.trim().to_string()),
                );
            }
        }
        lower_property(
            req,
            &req.require[0],
            prefix,
            bindings,
            resolutions,
            &by_sort,
        )
    }

    fn lower_with(params: Vec<ParamMode>, form: PredicateForm, observable: &str) -> String {
        let bindings = vec![binding("p", observable), binding("Thing", "Thing")];
        let resolutions = BTreeMap::from([("p".to_string(), resolved(params, form))]);
        lower_one(&gated(P_OF_U), "crate", &bindings, &resolutions)
            .expect("should lower")
            .claim
    }

    // Verifies: REQ055 — a free function still lowers to a free call, unchanged.
    #[test]
    fn a_free_function_lowers_to_a_free_call() {
        assert_eq!(
            lower_with(vec![ParamMode::ByRef], PredicateForm::Function, "is_ok"),
            "crate::is_ok(&u)"
        );
    }

    // Verifies: REQ055 — a method is called *on* its receiver. Before this, an impl-block method
    // resolved green (`collect_fns` has always descended into impls) and then lowered to
    // `crate::ready(&u)` — a free call to a method, which cannot compile. The operator saw an
    // `unknown` with a compiler error instead of a working check.
    #[test]
    fn a_method_lowers_to_a_method_call_not_a_free_call() {
        let claim = lower_with(
            vec![ParamMode::ByRef],
            PredicateForm::Method {
                name: "is_ready".into(),
            },
            "Engine::is_ready",
        );
        assert_eq!(claim, "u.is_ready()");
        assert!(
            !claim.contains("crate::"),
            "a method must not be reached through a path: {claim}"
        );
    }

    // Verifies: REQ055 — the enum decision the dogfood run could not name. `{ .. }` is used
    // deliberately so unit, tuple, and struct variants all match without the binding restating
    // the variant's shape.
    #[test]
    fn a_variant_test_lowers_to_a_match_expression() {
        assert_eq!(
            lower_with(
                vec![ParamMode::ByValue],
                PredicateForm::VariantTest {
                    name: "decide".into(),
                    enum_name: "Decision".into(),
                    variant: "Proceed".into(),
                    enum_module: Some(vec![]),
                },
                "decide::Proceed",
            ),
            "match crate::decide(u) { crate::Decision::Proceed { .. } => true, _ => false }"
        );
    }

    // Verifies: REQ058 — a primitive sort is written bare, a declared one through the prefix.
    // `crate::bool` does not compile, so getting this wrong would reach the operator as an
    // `unknown` carrying a compiler error — exactly the failure grounding a primitive is for.
    #[test]
    fn a_primitive_sort_lowers_unprefixed() {
        let quantified = |sort: &str| {
            let req = gated(
                "requirement r { category: 1
                vocabulary { state p(u) }
                require { each u: S . always p(u) } }",
            );
            let bindings = vec![binding("p", "is_ok"), binding("S", sort)];
            let resolutions = BTreeMap::from([(
                "p".to_string(),
                resolved(vec![ParamMode::ByValue], PredicateForm::Function),
            )]);
            lower_one(&req, "mycrate", &bindings, &resolutions)
                .expect("should lower")
                .quantified
                .remove(0)
                .ty
        };
        assert_eq!(quantified("bool"), "bool");
        assert_eq!(quantified("u32"), "u32");
        assert_eq!(quantified("Thing"), "mycrate::Thing");
    }

    /// The REQ047 shape: a four-argument decision function, no `each` written, sorts declared on
    /// the predicate. Nothing of this form could be lowered at all before REQ059.
    const GATED_DECISION: &str = "requirement r { category: 1
        vocabulary { state supported
                     state proceeds(d: Decision, p: Flag, q: Flag, c: Flag) }
        require { always (not proceeds(d, p, q, c) or supported) } }";

    fn decision_bindings() -> Vec<Binding> {
        vec![
            binding("proceeds", "decide"),
            binding("supported", "is_supported"),
            binding("Decision", "InstallDecision"),
            binding("Flag", "bool"),
        ]
    }

    fn decision_resolutions() -> BTreeMap<String, Resolution> {
        BTreeMap::from([
            (
                "proceeds".to_string(),
                resolved(vec![ParamMode::ByValue; 4], PredicateForm::Function),
            ),
            (
                "supported".to_string(),
                resolved(vec![], PredicateForm::Function),
            ),
        ])
    }

    // Verifies: REQ059 — every variable a cat-1 claim applies a predicate to is quantified, over
    // the sort the VOCABULARY declares for that parameter. The headline of #136: a predicate of
    // arity > 1 could never be lowered before, because a property carries at most one `each`
    // binder, so there was no formulation of this requirement a harness could be built for.
    #[test]
    fn free_variables_are_closed_over_their_declared_sorts() {
        let claim = lower_one(
            &gated(GATED_DECISION),
            "mycrate",
            &decision_bindings(),
            &decision_resolutions(),
        )
        .expect("should lower");

        // In the order a reader meets them, each over its declared sort — and a primitive stays
        // bare while a declared type is reached through the prefix (REQ058).
        let binders: Vec<(String, String)> = claim
            .quantified
            .iter()
            .map(|q| (q.var.clone(), q.ty.clone()))
            .collect();
        assert_eq!(
            binders,
            vec![
                ("d".to_string(), "mycrate::InstallDecision".to_string()),
                ("p".to_string(), "bool".to_string()),
                ("q".to_string(), "bool".to_string()),
                ("c".to_string(), "bool".to_string()),
            ]
        );
        assert!(
            claim.claim.contains("mycrate::decide(d, p, q, c)"),
            "{}",
            claim.claim
        );
    }

    /// What REQ047 actually means, and could not say until REQ066: the flag ITSELF is the
    /// condition. Note what is absent — the `supported`/`is_supported` pair of GATED_DECISION,
    /// which existed only so a boolean argument could be named as a predicate. That helper was the
    /// tool bending the subject (#146).
    const GATED_FLAG_CONDITION: &str = "requirement r { category: 1
        vocabulary { state proceeds(d: Decision, p: Flag, q: Flag, c: Flag) }
        require { always (not proceeds(d, p, q, c) or p) } }";

    fn flag_bindings() -> Vec<Binding> {
        vec![
            binding("proceeds", "decide"),
            binding("Decision", "InstallDecision"),
            binding("Flag", "bool"),
        ]
    }

    // Verifies: REQ066 — a boolean variable used as an atom lowers to the variable itself, with no
    // call and no binding of its own. The claim it makes expressible is an input-to-result
    // invariant: the result depends on an ARGUMENT, which is what REQ047 is about.
    #[test]
    fn a_boolean_variable_lowers_to_itself_as_a_condition() {
        let claim = lower_one(
            &gated(GATED_FLAG_CONDITION),
            "mycrate",
            &flag_bindings(),
            &BTreeMap::from([(
                "proceeds".to_string(),
                resolved(vec![ParamMode::ByValue; 4], PredicateForm::Function),
            )]),
        )
        .expect("should lower");

        assert_eq!(
            claim.claim, "(!(mycrate::decide(d, p, q, c)) || p)",
            "the condition is the variable, not a call"
        );
        // And it is still closed over, as a `bool` — the variable has to be in scope to be one.
        assert!(
            claim
                .quantified
                .iter()
                .any(|q| q.var == "p" && q.ty == "bool"),
            "{:?}",
            claim.quantified
        );
    }

    // Verifies: REQ066 — only a `bool` stands as a condition. A variable of a declared sort is
    // refused with the reason, not emitted for the compiler to reject: `!(…) || d` where `d` is an
    // enum is a harness that does not build, and a build error is not an answer about the claim.
    #[test]
    fn a_non_boolean_variable_used_as_a_condition_does_not_lower() {
        let src = "requirement r { category: 1
            vocabulary { state proceeds(d: Decision, p: Flag, q: Flag, c: Flag) }
            require { always (not proceeds(d, p, q, c) or d) } }";
        let err = lower_one(
            &gated(src),
            "mycrate",
            &flag_bindings(),
            &BTreeMap::from([(
                "proceeds".to_string(),
                resolved(vec![ParamMode::ByValue; 4], PredicateForm::Function),
            )]),
        )
        .expect_err("an enum is not a condition");
        assert!(err.reason.contains("used as a condition"), "{}", err.reason);
        assert!(err.reason.contains("`Decision`"), "{}", err.reason);
        assert!(err.reason.contains("bool"), "{}", err.reason);
    }

    // Verifies: REQ059 — a variable the vocabulary declares no sort for does not lower, and the
    // refusal names the variable and the way out. Closing over an unknown domain would be a
    // harness quantified over a type this tool chose, which is exactly the guess R-ground-1 forbids.
    #[test]
    fn a_variable_without_a_declared_sort_does_not_lower() {
        let req = gated(
            "requirement r { category: 1
            vocabulary { state p(u) }
            require { always p(u) } }",
        );
        let bindings = vec![binding("p", "is_ok")];
        let resolutions = BTreeMap::from([(
            "p".to_string(),
            resolved(vec![ParamMode::ByValue], PredicateForm::Function),
        )]);
        let err = lower_one(&req, "crate", &bindings, &resolutions)
            .expect_err("an undeclared sort has no domain");
        assert!(
            err.reason.contains("`u` has no declared sort"),
            "{}",
            err.reason
        );
        assert!(err.reason.contains("vocabulary"), "{}", err.reason);
    }

    // Verifies: REQ059 — an explicit `each` binder still wins over the vocabulary's declaration.
    // The operator wrote that one deliberately, and a tool that silently preferred its own reading
    // would be answering a question it was not asked.
    #[test]
    fn an_explicit_binder_wins_over_the_declared_parameter_sort() {
        let req = gated(
            "requirement r { category: 1
            vocabulary { state p(u: Declared) }
            require { each u: Written . always p(u) } }",
        );
        let bindings = vec![
            binding("p", "is_ok"),
            binding("Written", "Chosen"),
            binding("Declared", "Ignored"),
        ];
        let resolutions = BTreeMap::from([(
            "p".to_string(),
            resolved(vec![ParamMode::ByValue], PredicateForm::Function),
        )]);
        let claim = lower_one(&req, "crate", &bindings, &resolutions).expect("should lower");
        assert_eq!(claim.quantified[0].ty, "crate::Chosen");
    }

    // Verifies: REQ059 — closure binds *variables*, not values. A literal is not something a claim
    // ranges over, and closing over one would emit `let true: bool = kani::any()`.
    #[test]
    fn a_literal_argument_is_not_closed_over() {
        let req = gated(
            "requirement r { category: 1
            vocabulary { state p(u: Flag) }
            require { always p(true) } }",
        );
        let bindings = vec![binding("p", "is_ok"), binding("Flag", "bool")];
        let resolutions = BTreeMap::from([(
            "p".to_string(),
            resolved(vec![ParamMode::ByValue], PredicateForm::Function),
        )]);
        let err = lower_one(&req, "crate", &bindings, &resolutions)
            .expect_err("a literal is not a variable to range over");
        assert!(
            err.reason.contains("not a variable the claim ranges over"),
            "{}",
            err.reason
        );
    }

    // Verifies: REQ061 — every path a harness writes carries the module the adapter found the item
    // in. Before this, all three shapes emitted `{prefix}::{name}`, correct only for a crate whose
    // every item sits in `src/lib.rs` — and wrong for the first real multi-module crate provreq met
    // (its own, #143).
    #[test]
    fn a_call_is_named_through_the_module_it_was_found_in() {
        let at_module = |module: &[&str]| Resolution::Resolved {
            at: CodeMatch {
                file: "src/provision.rs".into(),
                line: 79,
                text: "pub fn decide_install(".into(),
                module: Some(module.iter().map(|s| s.to_string()).collect()),
            },
            params: vec![ParamMode::ByValue],
            form: PredicateForm::Function,
        };
        let claim = |res: Resolution| {
            let bindings = vec![binding("p", "decide_install"), binding("Thing", "Thing")];
            let resolutions = BTreeMap::from([("p".to_string(), res)]);
            lower_one(&gated(P_OF_U), "subject", &bindings, &resolutions)
                .expect("should lower")
                .claim
        };
        assert_eq!(
            claim(at_module(&["provision"])),
            "subject::provision::decide_install(u)"
        );
        // An inline `mod` inside the file is one segment deeper, and the crate root has none.
        assert_eq!(
            claim(at_module(&["provision", "inner"])),
            "subject::provision::inner::decide_install(u)"
        );
        assert_eq!(claim(at_module(&[])), "subject::decide_install(u)");
    }

    // Verifies: REQ061 — the enum of a variant test is named through ITS module, which need not be
    // the function's. A single module for both would be a guess that happens to hold only when the
    // two are declared together.
    #[test]
    fn a_variant_test_names_the_enum_through_its_own_module() {
        let bindings = vec![binding("p", "decide"), binding("Thing", "Thing")];
        let resolutions = BTreeMap::from([(
            "p".to_string(),
            Resolution::Resolved {
                at: CodeMatch {
                    file: "src/provision.rs".into(),
                    line: 1,
                    text: "…".into(),
                    module: Some(vec!["provision".into()]),
                },
                params: vec![ParamMode::ByValue],
                form: PredicateForm::VariantTest {
                    name: "decide".into(),
                    enum_name: "Decision".into(),
                    variant: "Proceed".into(),
                    enum_module: Some(vec!["engine".into()]),
                },
            },
        )]);
        let claim = lower_one(&gated(P_OF_U), "subject", &bindings, &resolutions)
            .expect("should lower")
            .claim;
        assert_eq!(
            claim,
            "match subject::provision::decide(u) { subject::engine::Decision::Proceed { .. } \
             => true, _ => false }"
        );
    }

    // Verifies: REQ061 — an item no harness can name does not lower, and the refusal names the file
    // rather than emitting a path this tool invented. It RESOLVED, so grounding was green: the
    // predicate is really declared there, just not where a harness can reach it.
    #[test]
    fn an_item_with_no_module_path_does_not_lower() {
        let bindings = vec![binding("p", "ready"), binding("Thing", "Thing")];
        let resolutions = BTreeMap::from([(
            "p".to_string(),
            Resolution::Resolved {
                at: CodeMatch {
                    file: "tests/helpers.rs".into(),
                    line: 3,
                    text: "pub fn ready(u: &Thing) -> bool {".into(),
                    module: None,
                },
                params: vec![ParamMode::ByValue],
                form: PredicateForm::Function,
            },
        )]);
        let err = lower_one(&gated(P_OF_U), "subject", &bindings, &resolutions)
            .expect_err("no path reaches a separate crate target");
        assert!(err.reason.contains("tests/helpers.rs"), "{}", err.reason);
        assert!(err.reason.contains("harness"), "{}", err.reason);
    }

    // Verifies: REQ061/REQ058 — a sort is named through its own module too, and a primitive is
    // still written bare (there is no module to reach `bool` through).
    #[test]
    fn a_sort_is_named_through_its_module_and_a_primitive_stays_bare() {
        let req = gated(
            "requirement r { category: 1
            vocabulary { state p(u) }
            require { each u: S . always p(u) } }",
        );
        let bindings = vec![binding("p", "is_ok"), binding("S", "EngineStatus")];
        let resolutions = BTreeMap::from([(
            "p".to_string(),
            resolved(vec![ParamMode::ByValue], PredicateForm::Function),
        )]);
        let by_sort = BTreeMap::from([(
            "S".to_string(),
            TypeResolution::Resolved(CodeMatch {
                file: "src/engine.rs".into(),
                line: 67,
                text: "pub enum EngineStatus {".into(),
                module: Some(vec!["engine".into()]),
            }),
        )]);
        let ty = lower_property(
            &req,
            &req.require[0],
            "subject",
            &bindings,
            &resolutions,
            &by_sort,
        )
        .expect("should lower")
        .quantified
        .remove(0)
        .ty;
        assert_eq!(ty, "subject::engine::EngineStatus");

        let primitive = BTreeMap::from([(
            "S".to_string(),
            TypeResolution::Primitive("bool".to_string()),
        )]);
        let bare = lower_property(
            &req,
            &req.require[0],
            "subject",
            &[binding("p", "is_ok"), binding("S", "bool")],
            &resolutions,
            &primitive,
        )
        .expect("should lower")
        .quantified
        .remove(0)
        .ty;
        assert_eq!(bare, "bool");
    }

    // Verifies: REQ055 — a nullary method has no receiver to be called on. It cannot arise from
    // the adapter (a `self` receiver counts toward arity), but lowering is public and must not
    // depend on a caller having resolved first.
    #[test]
    fn a_nullary_method_is_not_lowerable() {
        let req = gated(
            "requirement r { category: 1
            vocabulary { state p }
            require { always p } }",
        );
        let bindings = vec![binding("p", "S::ready")];
        let resolutions = BTreeMap::from([(
            "p".to_string(),
            resolved(
                vec![],
                PredicateForm::Method {
                    name: "ready".into(),
                },
            ),
        )]);
        let err = lower_one(&req, "crate", &bindings, &resolutions)
            .expect_err("no receiver, nothing to call it on");
        assert!(err.reason.contains("first argument"), "{}", err.reason);
    }
}
