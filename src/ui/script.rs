//! Lowering a category-3 claim to a step script (#243).
//!
//! The fact that decides everything here: **a UI driver makes the trace it then judges.** A model
//! checker explores a state graph it did not build; a monitor reads a log the subject wrote (#230 —
//! provreq never runs the subject). A driver performs the actions *and* observes the result, so a
//! claim whose outcome is decided by the script rather than by the deployment is not a check at
//! all — it passes because provreq made it pass.
//!
//! Slice 1's step vocabulary already splits along the line that matters:
//!
//! | step | kind | who decides the outcome |
//! |---|---|---|
//! | `goto` | action | the driver |
//! | `click` | action | the driver |
//! | `text_present` | **observation** | the deployment |
//!
//! So the rule this module enforces: **what a claim asserts must be an observation.** An
//! action-consequent claim is refused rather than lowered, because a vacuously-passing UI check is
//! worse than none — it yields a `not-falsified` verdict indistinguishable from the real thing.
//!
//! **#232's "emit the negation" rule does NOT carry over, and that is worth saying outright**,
//! because anyone arriving from the 2b arc will look for it. MonPoly had to be handed the violation
//! pattern: the policy form prints `true` at every satisfying point and omits the violating one, a
//! measured fact about how that engine reports. A driver asserts directly and reports its own
//! pass/fail, so a [`UiClaim`] carries the claim as written.
//!
//! Implements: #243.

use super::declaration::{Step, Ui};
use crate::grounding::Binding;
use crate::lowering::NotLowerable;
use crate::prl::ast::{Atom, Expr, Pattern, Property, Requirement, Scope};

/// What a driver needs to run: the actions to perform, in order, and the observation that decides
/// the verdict. Never one without the other — a script with nothing to assert would report a
/// `not-falsified` that means only "the clicks worked".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiClaim {
    /// The actions to perform, in order: any `after` setup first, then the claim's antecedent.
    pub steps: Vec<Step>,
    /// The observation the deployment answers with. The only thing that can falsify the claim.
    pub expect: Step,
    /// The observation must be **absent** — `never p`, or a negated operand. Not an engine-facing
    /// inversion (see the module docs on #232); the driver asserts whichever way this says.
    pub expect_absent: bool,
}

impl UiClaim {
    /// The script as the operator will read it back — the ordered actions and what is then checked.
    /// A script assembled from bindings is not obvious from the requirement text, so D12 has to
    /// show it rather than assert that it exists.
    pub fn describe(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, action_phrase(s)))
            .collect();
        lines.push(format!(
            "{}. then check the page {} {}",
            self.steps.len() + 1,
            if self.expect_absent {
                "does NOT contain"
            } else {
                "contains"
            },
            match &self.expect {
                Step::TextPresent(t) => format!("`{t}`"),
                other => format!("`{}`", other.argument()),
            }
        ));
        lines
    }
}

fn action_phrase(step: &Step) -> String {
    match step {
        Step::Goto(path) => format!("navigate to `{path}`"),
        Step::Click(sel) => format!("click the first element matching `{sel}`"),
        Step::TextPresent(t) => format!("check that the page contains `{t}`"),
    }
}

/// Lower one grounded category-3 property to a step script.
///
/// Mirrors [`crate::monitor::lower`]: an out-of-fragment pattern is a typed [`NotLowerable`] the
/// engine turns into an honest `unknown`, never a silent approximation (D2).
pub fn lower(
    req: &Requirement,
    prop: &Property,
    ui: &Ui,
    bindings: &[Binding],
) -> Result<UiClaim, NotLowerable> {
    // A quantified claim never reaches here — it parks at grounding, because a driver has no source
    // for the variable's domain (`UiResolution::NoDomain`, #241). Checked anyway: this module must
    // not be the place that first assumes it.
    if !req.binders(prop).is_empty() {
        return Err(NotLowerable::new(
            "the claim ranges over a variable, and a driver runs a fixed script against one \
             deployment — there is no set of values for it to range over (see the binding's own \
             account of this)",
        ));
    }

    let setup = match &prop.scope {
        Scope::Globally => Vec::new(),
        // `after x` is the one Dwyer scope a driver can honour exactly, and it is how a script says
        // "get there first". Unlike a monitor — which reads a trace from its start and so defers
        // every scope (#232) — a driver is *already* performing actions, so a setup action is the
        // same kind of thing it does anyway.
        Scope::After(atom) => vec![action_for(atom, ui, bindings)?],
        Scope::Before(_) | Scope::Between(_, _) => {
            return Err(NotLowerable::new(
                "the claim is limited to a `before`/`between` scope — a driver's run starts when it \
                 opens the page, so there is no \"before\" for it to check in, and no way to stop \
                 at a boundary it has already passed. `after <step>` is the scope a script can \
                 honour",
            ));
        }
    };

    match &prop.pattern {
        Pattern::LeadsTo { from, to, .. } => {
            let action = action_for(
                single_atom(from, "the antecedent of `leads_to`")?,
                ui,
                bindings,
            )?;
            let (atom, absent) = observation_operand(to, "the consequent of `leads_to`")?;
            let expect = observation_for(atom, ui, bindings)?;
            Ok(UiClaim {
                steps: setup.into_iter().chain([action]).collect(),
                expect,
                expect_absent: absent,
            })
        }
        // `always p` and `eventually p` collapse to the same script over a single run with a fixed
        // number of steps: perform the setup, then look. The DIFFERENCE between them — "at every
        // point" versus "at some point" — is a claim about a run that could have gone other ways,
        // and one scripted run cannot tell them apart. Rather than lower both to the same thing and
        // let the verdict imply a distinction it never checked, only `always` is accepted: it is
        // the reading a single observation actually supports.
        Pattern::Always(e) | Pattern::Never(e) => {
            let (atom, absent) = observation_operand(e, "the operand")?;
            let expect = observation_for(atom, ui, bindings)?;
            let negated = matches!(prop.pattern, Pattern::Never(_)) != absent;
            Ok(UiClaim {
                steps: setup,
                expect,
                expect_absent: negated,
            })
        }
        Pattern::Eventually(_) => Err(NotLowerable::new(
            "`eventually` says the page comes to show it at some point, and a script looks once, \
             at the point it was told to. Refusing here rather than checking `always` and calling \
             it `eventually` — the two differ only over runs a fixed script does not take. Write \
             it as `always` if one look is what you meant, or as `<action> leads_to <observation>` \
             if something has to happen first",
        )),
        Pattern::Precedes { .. } => Err(NotLowerable::new(
            "`precedes` orders two things in time, and in a UI check the script IS the order — it \
             would be answered by what you wrote here rather than by the deployment, and would \
             hold every time. Order the actions with `after <step>` and assert what the page then \
             shows",
        )),
        Pattern::OccursAtMost { .. } => Err(NotLowerable::new(
            "`occurs at most` counts over a run, and a script performs a fixed number of actions — \
             the count is what you wrote, not something the deployment decided. A log carries real \
             counts: declare category 2b and monitor one",
        )),
        Pattern::CanReach(_) => Err(NotLowerable::new(
            "`can_reach` needs a model's state graph, which a driver of one live deployment does \
             not have (category 2a)",
        )),
    }
}

/// The single atom of an operand, or a refusal. A boolean combination has no step-script reading:
/// `and` would leave the running order unstated and `or` would leave which branch was checked
/// unstated — the same objection slice 1 makes to a step declaring two actions.
fn single_atom<'a>(expr: &'a Expr, role: &str) -> Result<&'a Atom, NotLowerable> {
    match expr {
        Expr::Atom(a) => Ok(a),
        Expr::Not(_) => Err(NotLowerable::new(format!(
            "{role} is negated — a driver can only *do* an action, not un-do one"
        ))),
        Expr::And(_, _) | Expr::Or(_, _) => Err(NotLowerable::new(format!(
            "{role} combines several terms, and a step script has no reading for that: `and` \
             leaves the order they run in unstated, and `or` leaves which one was checked \
             unstated. Name one step, and order any others with `after <step>`"
        ))),
    }
}

/// The atom an observation asserts, and whether it is asserted absent. `not p` is honest here in a
/// way it is not for an action: the deployment can answer "the page does not show this".
fn observation_operand<'a>(expr: &'a Expr, role: &str) -> Result<(&'a Atom, bool), NotLowerable> {
    match expr {
        Expr::Not(inner) => Ok((single_atom(inner, role)?, true)),
        _ => Ok((single_atom(expr, role)?, false)),
    }
}

/// The declared step an atom's binding names, whatever its kind.
fn step_for<'a>(atom: &Atom, ui: &'a Ui, bindings: &[Binding]) -> Result<&'a Step, NotLowerable> {
    let binding = bindings
        .iter()
        .find(|b| b.symbol == atom.name)
        .ok_or_else(|| {
            NotLowerable::new(format!(
                "`{}` has no binding, so there is no step for it to run",
                atom.name
            ))
        })?;
    ui.step(&binding.observable).ok_or_else(|| {
        NotLowerable::new(format!(
            "`{}` binds to `{}`, which is not declared under `ui.steps`",
            atom.name, binding.observable
        ))
    })
}

/// A step used as something the driver **does**.
fn action_for(atom: &Atom, ui: &Ui, bindings: &[Binding]) -> Result<Step, NotLowerable> {
    let step = step_for(atom, ui, bindings)?;
    match step {
        Step::Goto(_) | Step::Click(_) => Ok(step.clone()),
        Step::TextPresent(t) => Err(NotLowerable::new(format!(
            "`{}` is used as something to do, but it is declared as `text_present: \"{t}\"` — an \
             observation, which a driver reads rather than performs",
            atom.name
        ))),
    }
}

/// A step used as what the claim **asserts**. The rule this module exists for.
fn observation_for(atom: &Atom, ui: &Ui, bindings: &[Binding]) -> Result<Step, NotLowerable> {
    let step = step_for(atom, ui, bindings)?;
    match step {
        Step::TextPresent(_) => Ok(step.clone()),
        Step::Goto(_) | Step::Click(_) => Err(NotLowerable::new(format!(
            "`{}` is what this claim asserts, but it is declared as `{}` — an action the driver \
             performs itself. A claim the driver satisfies by doing it cannot be falsified: it \
             would report `not-falsified` having checked nothing the deployment decided. Assert a \
             `text_present` step instead",
            atom.name,
            step.key()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grounding::{BindCategory, Fidelity};
    use std::collections::BTreeMap;

    fn ui() -> Ui {
        Ui::new(
            "http://localhost:8080",
            None,
            BTreeMap::from([
                ("open_cart".to_string(), Step::Goto("/cart".into())),
                (
                    "checkout".to_string(),
                    Step::Click("button[data-test=checkout]".into()),
                ),
                (
                    "sees_total".to_string(),
                    Step::TextPresent("Order total".into()),
                ),
            ]),
        )
    }

    fn bindings(symbols: &[&str]) -> Vec<Binding> {
        symbols
            .iter()
            .map(|s| Binding {
                symbol: (*s).to_string(),
                category: BindCategory::Ui,
                observable: (*s).to_string(),
                fidelity: Fidelity::Probed,
            })
            .collect()
    }

    fn lower_src(src: &str, symbols: &[&str]) -> Result<UiClaim, NotLowerable> {
        let req = crate::prl::parser::parse(src).expect("should parse");
        let prop = req.require[0].clone();
        lower(&req, &prop, &ui(), &bindings(symbols))
    }

    const VOCAB: &str = "vocabulary { state open_cart state checkout state sees_total }";

    // Verifies: #243 — the shape of the whole slice. An action antecedent and an observation
    // consequent lower to a script the operator can read back.
    #[test]
    fn an_action_then_an_observation_lowers_to_a_script() {
        let claim = lower_src(
            &format!(
                "requirement u {{ category: 3 {VOCAB} require {{ checkout leads_to sees_total }} }}"
            ),
            &["checkout", "sees_total"],
        )
        .expect("lowers");

        assert_eq!(
            claim,
            UiClaim {
                steps: vec![Step::Click("button[data-test=checkout]".into())],
                expect: Step::TextPresent("Order total".into()),
                expect_absent: false,
            }
        );
        assert_eq!(
            claim.describe(),
            vec![
                "1. click the first element matching `button[data-test=checkout]`".to_string(),
                "2. then check the page contains `Order total`".to_string(),
            ]
        );
    }

    // Verifies: #243 — THE rule. A driver performs its own actions, so a claim asserting one holds
    // because provreq made it hold. That is a `not-falsified` indistinguishable from a real one,
    // which is worse than no check at all.
    #[test]
    fn a_claim_asserting_an_action_is_refused_as_unfalsifiable() {
        let err = lower_src(
            &format!(
                "requirement u {{ category: 3 {VOCAB} require {{ open_cart leads_to checkout }} }}"
            ),
            &["open_cart", "checkout"],
        )
        .expect_err("an action consequent cannot be falsified");

        assert!(err.reason.contains("checkout"), "{}", err.reason);
        assert!(err.reason.contains("cannot be falsified"), "{}", err.reason);
        assert!(
            err.reason.contains("text_present"),
            "the refusal must say what to assert instead: {}",
            err.reason
        );
    }

    // Verifies: #243 — the converse mistake, caught with its own words rather than a generic one.
    #[test]
    fn an_observation_used_as_an_action_is_refused() {
        let err = lower_src(
            &format!("requirement u {{ category: 3 {VOCAB} require {{ sees_total leads_to sees_total }} }}"),
            &["sees_total"],
        )
        .expect_err("an observation is not something to do");
        assert!(err.reason.contains("observation"), "{}", err.reason);
        assert!(
            err.reason.contains("rather than performs"),
            "{}",
            err.reason
        );
    }

    // Verifies: #243 — `after` is the ordering mechanism, taken from the language rather than
    // invented. It runs first, and it must itself be an action.
    #[test]
    fn an_after_scope_becomes_the_setup_step() {
        let claim = lower_src(
            &format!(
                "requirement u {{ category: 3 {VOCAB} \
                 require {{ checkout leads_to sees_total after open_cart }} }}"
            ),
            &["open_cart", "checkout", "sees_total"],
        )
        .expect("lowers");

        assert_eq!(
            claim.steps,
            vec![
                Step::Goto("/cart".into()),
                Step::Click("button[data-test=checkout]".into())
            ],
            "the `after` step runs first"
        );
        assert_eq!(claim.describe()[0], "1. navigate to `/cart`");
    }

    // Verifies: #243 — `never p` asserts the absence of an observation, which a deployment can
    // genuinely answer. No engine-facing inversion is involved (see the module docs on #232).
    #[test]
    fn never_asserts_the_observation_is_absent() {
        let claim = lower_src(
            &format!("requirement u {{ category: 3 {VOCAB} require {{ never sees_total }} }}"),
            &["sees_total"],
        )
        .expect("lowers");
        assert!(claim.expect_absent);
        assert!(claim.steps.is_empty());
        assert_eq!(
            claim.describe(),
            vec!["1. then check the page does NOT contain `Order total`".to_string()]
        );

        // And a doubly-negated `never not p` is back to asserting presence — the flags compose
        // rather than one silently winning.
        let double = lower_src(
            &format!(
                "requirement u {{ category: 3 {VOCAB} require {{ never (not sees_total) }} }}"
            ),
            &["sees_total"],
        )
        .expect("lowers");
        assert!(!double.expect_absent);
    }

    // Verifies: #243 — the patterns a fixed script cannot honestly express, each refused with the
    // reason and a way forward rather than a generic "out of fragment".
    #[test]
    fn the_patterns_a_single_scripted_run_cannot_decide_are_refused() {
        let cases = [
            ("checkout precedes sees_total", "the script IS the order"),
            (
                "checkout occurs at most 2 times",
                "not something the deployment decided",
            ),
            ("eventually sees_total", "a script looks once"),
        ];
        for (claim, expected) in cases {
            let err = lower_src(
                &format!("requirement u {{ category: 3 {VOCAB} require {{ {claim} }} }}"),
                &["checkout", "sees_total"],
            )
            .expect_err("must refuse");
            assert!(
                err.reason.contains(expected),
                "`{claim}` refused with the wrong reason: {}",
                err.reason
            );
        }
    }

    // Verifies: #243 — a boolean combination has no step-script reading, and the refusal says which
    // ambiguity it is: `and` leaves the order unstated, `or` leaves the branch unstated. Same
    // objection slice 1 makes to a step declaring two actions.
    #[test]
    fn a_boolean_combination_has_no_script_reading() {
        let err = lower_src(
            &format!(
                "requirement u {{ category: 3 {VOCAB} \
                 require {{ (open_cart and checkout) leads_to sees_total }} }}"
            ),
            &["open_cart", "checkout", "sees_total"],
        )
        .expect_err("must refuse");
        assert!(err.reason.contains("leaves the order"), "{}", err.reason);
        assert!(
            err.reason.contains("after <step>"),
            "the refusal must name the mechanism that DOES order steps: {}",
            err.reason
        );
    }

    // Verifies: #243 — an unbound or undeclared symbol is refused here too, rather than reaching a
    // driver as a missing step. Grounding catches these first; this module does not assume it did.
    #[test]
    fn a_symbol_with_no_step_behind_it_is_refused_naming_it() {
        let unbound = lower_src(
            &format!(
                "requirement u {{ category: 3 {VOCAB} require {{ checkout leads_to sees_total }} }}"
            ),
            &["checkout"],
        )
        .expect_err("sees_total has no binding");
        assert!(unbound.reason.contains("sees_total"), "{}", unbound.reason);
        assert!(unbound.reason.contains("no binding"), "{}", unbound.reason);
    }
}
