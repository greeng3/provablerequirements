//! Resolving a category-3 binding against the declared steps (#241).
//!
//! Grounding is where the operator confirms a binding says what they meant (REQ057). Category 1
//! resolves against the subject's Rust, 2a against its TLA+, 2b against the declared event
//! signature (#231). **Category 3 resolves against the declared steps** — never the subject's
//! code, for the same reason #231 gives one category further in: a category-3 claim speaks of what
//! a browser does to a running page, so whether `checkout` is a real observable is answered by
//! whether the operator declared a step called `checkout`, not by whether some Rust function
//! happens to share the name.
//!
//! **A sort does not resolve here, and that is the interesting part.** Category 2b answers the same
//! question with [`crate::monitor::RuntimeResolution::TraceBound`]: a monitor binds a quantified
//! variable from the trace's own argument values, so there is no domain to declare and nothing that
//! could be wrong. *That reason does not exist here.* A driver runs a fixed script against one
//! deployment; there is no set of values for a variable to range over and nothing to draw a domain
//! from. Copying `TraceBound` across would ground a claim nothing can drive, and the operator would
//! only find out at lowering — after the read-back had already told them it was fine.

use super::declaration::{Step, Ui};

/// What a category-3 binding resolved to, against the declared steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiResolution {
    /// Exactly the declared step, applied to no arguments. The only variant that grounds.
    Resolved { alias: String, step: Step },
    /// No `ui:` block at all, so there are no steps to resolve against. Kept distinct from
    /// [`UiResolution::NotDeclared`] because they are different mistakes with different fixes:
    /// one operator has not configured a UI check, the other mistyped a step in the one they have.
    NoUi,
    /// The subject declares steps, but none under that name.
    NotDeclared { declared: Vec<String> },
    /// A **sort** in a category-3 claim — the variable a quantifier ranges over. Refused, because
    /// nothing in a UI check can supply its domain. See this module's docs: this is the deliberate
    /// mirror-image of 2b's `TraceBound`, not an oversight, and getting it backwards would ground a
    /// claim no driver can express.
    NoDomain,
    /// A declared step applied to arguments. A step is a fixed action on a fixed selector — there
    /// is no parameter to pass — so the requirement using the symbol with arguments means one of
    /// the two is wrong, and the operator can see that here.
    TakesNoArguments { alias: String, expected: usize },
}

impl UiResolution {
    /// Whether this binding resolved — the single question [`crate::grounding::verdict`] asks.
    pub fn is_resolved(&self) -> bool {
        matches!(self, UiResolution::Resolved { .. })
    }

    /// The operator-facing read-back (D13: "here is what your binding resolves to — is that what
    /// you meant?"). A resolved step names the action it will actually take, because the alias is
    /// the operator's word and the action is what the browser will do with it — and a check that
    /// clicks the wrong selector is exactly the mistake this read-back exists to catch.
    pub fn describe(&self, symbol: &str, observable: &str) -> String {
        match self {
            UiResolution::Resolved { alias, step } => format!(
                "{symbol} → `{observable}` resolves to the declared step `{alias}` — \
                 {}\n      (what a driver will do; nothing has run, so there is nothing yet to \
                 report about it)",
                describe_action(step)
            ),
            UiResolution::NoUi => format!(
                "{symbol}: `{observable}` cannot resolve — this subject declares no `ui:` block in \
                 provreq.yml, so there is no deployment for a category-3 claim to be checked \
                 against"
            ),
            UiResolution::NotDeclared { declared } => {
                let known = if declared.is_empty() {
                    "`ui.steps` is empty".to_string()
                } else {
                    format!("declared steps are: {}", declared.join(", "))
                };
                format!(
                    "{symbol}: `{observable}` is not declared under `ui.steps` in provreq.yml — \
                     {known}"
                )
            }
            UiResolution::NoDomain => format!(
                "{symbol}: `{observable}` is the domain of a quantified variable, and a category-3 \
                 check has nothing to draw one from — a driver runs a fixed script against one \
                 deployment, so there is no set of values for the variable to range over. A \
                 monitor can bind such a variable from its trace (category 2b); a UI probe cannot. \
                 Restate the claim without the quantifier, or declare category 2b and monitor a \
                 log that carries the values"
            ),
            UiResolution::TakesNoArguments { alias, expected } => format!(
                "{symbol}: the declared step `{alias}` takes no arguments — it is a fixed action on \
                 a fixed selector — but the requirement applies `{symbol}` to {expected}. A binding \
                 checked at the wrong arity is a binding that proves nothing, so this is refused \
                 here rather than left to the driver"
            ),
        }
    }
}

/// What the driver will actually do, in the operator's terms.
fn describe_action(step: &Step) -> String {
    match step {
        Step::Goto(path) => format!("navigate to `{path}`"),
        Step::Click(selector) => format!("click the first element matching `{selector}`"),
        Step::TextPresent(text) => format!("check that the page contains `{text}`"),
    }
}

/// Resolve one category-3 binding against the declared steps.
///
/// `is_sort` is passed in rather than re-derived, because the requirement's vocabulary is the
/// caller's to read and this module's job is the steps.
pub fn resolve(ui: Option<&Ui>, observable: &str, arity: usize, is_sort: bool) -> UiResolution {
    if is_sort {
        return UiResolution::NoDomain;
    }
    let Some(ui) = ui else {
        return UiResolution::NoUi;
    };
    let Some(step) = ui.step(observable) else {
        return UiResolution::NotDeclared {
            declared: ui.aliases(),
        };
    };
    if arity != 0 {
        return UiResolution::TakesNoArguments {
            alias: observable.to_string(),
            expected: arity,
        };
    }
    UiResolution::Resolved {
        alias: observable.to_string(),
        step: step.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    // Verifies: #241 — a binding resolves against the DECLARATION, never the subject's code. The
    // alias is the operator's handle; the action is what the browser will do with it.
    #[test]
    fn a_declared_step_resolves_and_the_readback_names_the_action() {
        let r = resolve(Some(&ui()), "checkout", 0, false);
        assert!(r.is_resolved());

        let text = r.describe("checkout", "checkout");
        assert!(text.contains("declared step `checkout`"), "{text}");
        assert!(
            text.contains("click the first element matching `button[data-test=checkout]`"),
            "a read-back that hides the selector cannot catch a check that clicks the wrong \
             thing: {text}"
        );
    }

    // Verifies: #241 — the decision this slice exists to get right. 2b answers this with
    // `TraceBound` because a monitor binds the variable from the trace's own values. A driver has
    // no such source, so copying that answer across would ground a claim nothing can drive.
    #[test]
    fn a_sort_does_not_resolve_because_nothing_can_supply_its_domain() {
        let r = resolve(Some(&ui()), "Cart", 0, true);
        assert_eq!(r, UiResolution::NoDomain);
        assert!(!r.is_resolved(), "a quantified cat-3 claim must not ground");

        let text = r.describe("c", "Cart");
        assert!(text.contains("fixed script"), "{text}");
        assert!(
            text.contains("category 2b"),
            "the read-back must name the category that CAN bind it, or the operator is told no \
             with nowhere to go: {text}"
        );
    }

    // Verifies: #241 — "no UI check configured" and "that step is not declared" are different
    // mistakes with different fixes, so they never render the same.
    #[test]
    fn an_absent_block_and_an_undeclared_step_are_different_answers() {
        let none = resolve(None, "checkout", 0, false);
        assert_eq!(none, UiResolution::NoUi);
        assert!(none.describe("c", "checkout").contains("no `ui:` block"));

        let missing = resolve(Some(&ui()), "chekout", 0, false);
        let text = missing.describe("c", "chekout");
        assert!(text.contains("not declared under `ui.steps`"), "{text}");
        assert!(
            text.contains("checkout") && text.contains("open_cart") && text.contains("sees_total"),
            "an error that does not say what IS declared sends the operator back to the manifest \
             to find their own typo: {text}"
        );

        let empty = Ui::new("http://x.test", None, BTreeMap::new());
        assert!(
            resolve(Some(&empty), "checkout", 0, false)
                .describe("c", "checkout")
                .contains("`ui.steps` is empty")
        );
    }

    // Verifies: #241 — a step is a fixed action on a fixed selector, so there is no parameter to
    // pass. Refused where the binding is, the same call #231 makes about a declared event's arity.
    #[test]
    fn a_step_applied_to_arguments_is_refused_at_the_binding() {
        let r = resolve(Some(&ui()), "checkout", 1, false);
        assert_eq!(
            r,
            UiResolution::TakesNoArguments {
                alias: "checkout".into(),
                expected: 1,
            }
        );
        assert!(!r.is_resolved());

        let text = r.describe("checkout", "checkout");
        assert!(text.contains("takes no arguments"), "{text}");
        assert!(text.contains("applies `checkout` to 1"), "{text}");
    }
}
