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
//! What cannot be faithfully expressed — a scope, a guard, an argument that is not the quantified
//! variable — is a [`NotLowerable`], which each engine turns into an honest `unknown`/`inconclusive`.
//! D2's rule: an out-of-fragment operator is a typed error surfaced to the author, never a silent
//! approximation.
//!
//! Extracted from the three engines once Prusti made a third copy (rule of three, #69).

use crate::grounding::Binding;
use crate::prl::ast::{Atom, Expr, Pattern, Property, Quantifier, Scope};
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
    /// `Some` when the property is quantified. `ty` is already qualified with the prefix
    /// (`crate::User` / `mycrate::User`), so the engine only supplies the `∀` syntax around it.
    pub quantified: Option<Quantified>,
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
    prop: &Property,
    prefix: &str,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> Result<LoweredClaim, NotLowerable> {
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
        Pattern::Always(e) => {
            lower_expr(e, prop.quantifier.as_ref(), prefix, bindings, resolutions)?
        }
        // `never P` is `always not P`.
        Pattern::Never(e) => format!(
            "!({})",
            lower_expr(e, prop.quantifier.as_ref(), prefix, bindings, resolutions)?
        ),
        other => {
            return Err(NotLowerable::new(format!(
                "`{}` is not an invariant, and the code fragment is temporal-free — the \
                 gate should have rejected it at category 1",
                pattern_verb(other)
            )))
        }
    };

    let quantified = match &prop.quantifier {
        Some(q) => Some(Quantified {
            var: q.var.clone(),
            ty: format!("{prefix}::{}", sort_target(q, bindings)?),
        }),
        None => None,
    };
    Ok(LoweredClaim { claim, quantified })
}

/// The sort's bound Rust type (bare, unprefixed). An unbound sort cannot be ranged over — which is
/// exactly why REQ026 made sorts bindable.
fn sort_target(q: &Quantifier, bindings: &[Binding]) -> Result<String, NotLowerable> {
    bindings
        .iter()
        .find(|b| b.symbol == q.sort)
        .map(|b| b.observable.clone())
        .ok_or_else(|| {
            NotLowerable::new(format!(
                "the sort `{}` is not bound to a type, so `{}` has no domain to range over",
                q.sort, q.var
            ))
        })
}

fn lower_expr(
    e: &Expr,
    quantifier: Option<&Quantifier>,
    prefix: &str,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> Result<String, NotLowerable> {
    match e {
        Expr::Atom(a) => lower_atom(a, quantifier, prefix, bindings, resolutions),
        Expr::Not(inner) => Ok(format!(
            "!({})",
            lower_expr(inner, quantifier, prefix, bindings, resolutions)?
        )),
        Expr::And(l, r) => Ok(format!(
            "({} && {})",
            lower_expr(l, quantifier, prefix, bindings, resolutions)?,
            lower_expr(r, quantifier, prefix, bindings, resolutions)?
        )),
        Expr::Or(l, r) => Ok(format!(
            "({} || {})",
            lower_expr(l, quantifier, prefix, bindings, resolutions)?,
            lower_expr(r, quantifier, prefix, bindings, resolutions)?
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
    quantifier: Option<&Quantifier>,
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
        // Only the quantified variable can be instantiated. Any other term would compile to a
        // name that exists in the requirement's world but not in the harness's.
        match quantifier {
            Some(q) if q.var == arg => {}
            _ => {
                return Err(NotLowerable::new(format!(
                    "`{}` is applied to `{arg}`, which is not the quantified variable — \
                     there is no value to give it",
                    a.name
                )))
            }
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

    /// `always p(u)` for each `u: Thing`, the shape every cat-1 invariant over a sort takes.
    fn always_p_of_u() -> Property {
        Property {
            quantifier: Some(Quantifier {
                var: "u".into(),
                sort: "Thing".into(),
            }),
            pattern: Pattern::Always(Expr::Atom(Atom {
                name: "p".into(),
                args: vec!["u".into()],
                guard: None,
                line: 1,
            })),
            scope: Scope::Globally,
            line: 1,
        }
    }

    fn lower_with(params: Vec<ParamMode>, form: PredicateForm, observable: &str) -> String {
        let bindings = vec![binding("p", observable), binding("Thing", "Thing")];
        let resolutions = BTreeMap::from([("p".to_string(), resolved(params, form))]);
        lower_property(&always_p_of_u(), "crate", &bindings, &resolutions)
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

    // Verifies: REQ055 — a nullary method has no receiver to be called on. It cannot arise from
    // the adapter (a `self` receiver counts toward arity), but lowering is public and must not
    // depend on a caller having resolved first.
    #[test]
    fn a_nullary_method_is_not_lowerable() {
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
        let prop = Property {
            quantifier: None,
            pattern: Pattern::Always(Expr::Atom(Atom {
                name: "p".into(),
                args: vec![],
                guard: None,
                line: 1,
            })),
            scope: Scope::Globally,
            line: 1,
        };
        let err = lower_property(&prop, "crate", &bindings, &resolutions)
            .expect_err("no receiver, nothing to call it on");
        assert!(err.reason.contains("first argument"), "{}", err.reason);
    }
}
