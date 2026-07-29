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

use crate::grounding::Binding;
use crate::prl::ast::{Atom, Binder, Expr, Pattern, Property, Requirement, Scope};
use crate::rust_adapter::{ParamMode, PredicateForm, Resolution};
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

/// Lower one `require` claim to its boolean expression plus optional quantifier.
///
/// `prefix` is how the harness reaches the subject's items: the subject's crate name for Kani's
/// out-of-crate `tests/` harness, or `crate` for the in-crate Creusot/Prusti harnesses.
pub fn lower_property(
    req: &Requirement,
    prop: &Property,
    prefix: &str,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> Result<LoweredClaim, NotLowerable> {
    let binders = req.binders(prop);
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
                ty: qualify(&sort_target(b, bindings)?, prefix),
            })
        })
        .collect::<Result<Vec<_>, NotLowerable>>()?;
    Ok(LoweredClaim { claim, quantified })
}

/// A sort's type as the harness must write it. A type the subject declares is reached through the
/// harness's path prefix; a **primitive** is written bare, because `crate::bool` does not compile
/// — and a harness that does not compile reaches the operator as an `unknown` with a compiler
/// error, which is the failure this tool exists to move earlier (REQ058).
fn qualify(target: &str, prefix: &str) -> String {
    if crate::rust_adapter::is_primitive(target) {
        target.to_string()
    } else {
        format!("{prefix}::{target}")
    }
}

/// The bound Rust type of a binder's sort (bare, unprefixed). Two ways a variable can fail to have
/// a domain, and they are different mistakes: the requirement never said what it ranges over
/// (REQ059 — the vocabulary declares no type for that parameter, or two applications disagree), or
/// it said so and that sort is not bound to a type (which is exactly why REQ026 made sorts
/// bindable).
fn sort_target(binder: &Binder, bindings: &[Binding]) -> Result<String, NotLowerable> {
    let sort = binder.sort.as_ref().ok_or_else(|| {
        NotLowerable::new(format!(
            "`{}` has no declared sort, so there is no domain to range over — give it one in the \
             vocabulary (`state p({}: SomeSort)`), consistently across the predicates that take it",
            binder.var, binder.var
        ))
    })?;
    bindings
        .iter()
        .find(|b| b.symbol == *sort)
        .map(|b| b.observable.clone())
        .ok_or_else(|| {
            NotLowerable::new(format!(
                "the sort `{sort}` is not bound to a type, so `{}` has no domain to range over",
                binder.var
            ))
        })
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
    lower_call(form, &binding.observable, &args, prefix)
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
) -> Result<String, NotLowerable> {
    match form {
        PredicateForm::Function => Ok(format!("{prefix}::{observable}({})", args.join(", "))),
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
        } => Ok(format!(
            "matches!({prefix}::{name}({}), {prefix}::{enum_name}::{variant} {{ .. }})",
            args.join(", ")
        )),
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

    fn lower_one(
        req: &Requirement,
        prefix: &str,
        bindings: &[Binding],
        resolutions: &BTreeMap<String, Resolution>,
    ) -> Result<LoweredClaim, NotLowerable> {
        lower_property(req, &req.require[0], prefix, bindings, resolutions)
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
    fn a_variant_test_lowers_to_a_matches_expression() {
        assert_eq!(
            lower_with(
                vec![ParamMode::ByValue],
                PredicateForm::VariantTest {
                    name: "decide".into(),
                    enum_name: "Decision".into(),
                    variant: "Proceed".into(),
                },
                "decide::Proceed",
            ),
            "matches!(crate::decide(u), crate::Decision::Proceed { .. })"
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
