//! Lowering a metric deadline to MFOTL — the formula and signature MonPoly reads (#232).
//!
//! **Where this lives, and why it is not in [`crate::lowering`].** That module is the one copy of
//! the *category-1* lowering: a claim becoming a **Rust boolean expression**, shared by Kani,
//! Creusot and Prusti because all three assert Rust. A temporal logic is not that. TLA+ already
//! sets the precedent — [`crate::tlc`] owns its own `[]`/`~>` lowering — and MFOTL follows it. What
//! stays shared is the discipline and its vocabulary: one [`NotLowerable`], and nothing
//! approximated to make it fit.
//!
//! # The two rules, both measured against the real engine
//!
//! **1. Emit no leading `ALWAYS`.** MonPoly evaluates at every time point implicitly, so the
//! quantification over time an `always` expresses is already there. Emitting one is not merely
//! redundant — it is refused outright:
//!
//! ```text
//! ALWAYS FORALL id. accepted(id) IMPLIES EVENTUALLY[0,30] succeeded(id)
//! → The formula contains an unbounded future temporal operator. It is hence not monitorable.
//! ```
//!
//! **2. Emit the NEGATION — the violation pattern, not the policy.** MonPoly reports what
//! *matches*. Handed the policy it prints `true` at every satisfying time point; handed the
//! violation pattern it prints **nothing** on a clean trace and the offending tuple on a dirty one.
//! Measured on the same violating log:
//!
//! ```text
//! policy    FORALL id. accepted(id) IMPLIES EVENTUALLY[0,30] succeeded(id)
//!           → @200. (time point 1): true   @300. … true   @305. … true
//!             (and the ONE violating time point, 0, is absent)
//! violation accepted(id) AND NOT EVENTUALLY[0,30] succeeded(id)
//!           → @100. (time point 0): (2)
//! ```
//!
//! Read the policy form as "output means violation" and every satisfied policy reports as refuted.
//! **The adapter (#233) inverts the reading back, and that inversion is the most dangerous thing in
//! the 2b arc** — which is why [`tests`] drives the real engine in both directions rather than
//! asserting on a string and arguing about the rest.
//!
//! **3. The quantified variable stays FREE.** `¬∀x. P(x)` is `∃x. ¬P(x)`, so a violation is
//! witnessed by the offending value — but binding it with `EXISTS` throws that witness away.
//! Measured on the same violating log: the free form prints `(2)`, and `EXISTS id. …` prints a bare
//! `true`. The tuple is the counterexample, so the variable is left free.
//!
//! Implements: #232 (`leads_to … within T` lowers to the negated MFOTL violation pattern).

use super::declaration::Monitor;
use crate::grounding::Binding;
use crate::lowering::NotLowerable;
use crate::prl::ast::Scope;
use crate::prl::ast::{Atom, Expr, Pattern, Property, Requirement};
use std::collections::BTreeMap;

/// What MonPoly needs to run: the predicate signature and the violation formula. Never one without
/// the other — a formula naming a predicate the signature does not declare is a parse error, and
/// the operator would be reading half a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorClaim {
    /// The `.sig` file's contents: one `pred(arg:type)` per event the formula names.
    pub signature: String,
    /// The MFOTL **violation** pattern — matches exactly where the requirement is broken.
    pub formula: String,
    /// The deadline in seconds, kept so the adapter and the read-back can say what was checked
    /// without re-parsing the formula.
    pub within_seconds: u64,
}

/// Every argument is declared `string`, deliberately.
///
// ponytail: the declaration (#230) records argument NAMES, not types, and this lowering never emits
// a comparison or an arithmetic term — arguments only ever bind variables across two predicates. So
// the type is unobservable here, and `string` is the one that cannot fail on a real log: measured,
// a numeric-looking value still matched and still produced its witness tuple under `string`. If a
// future pattern needs `<` or arithmetic, the declaration grows a type then, not now.
const ARG_TYPE: &str = "string";

/// Lower one `require` claim to MonPoly's inputs.
///
/// Refuses rather than approximates, the rule the 2a lowering already follows: a scope, a guard, a
/// pattern with no metric reading, an argument that is not a variable the claim ranges over, or a
/// deadline MonPoly's interval syntax cannot express.
pub fn lower(
    req: &Requirement,
    prop: &Property,
    monitor: &Monitor,
    bindings: &[Binding],
) -> Result<MonitorClaim, NotLowerable> {
    if prop.scope != Scope::Globally {
        return Err(NotLowerable::new(
            "the claim is limited to a scope (`before`/`after`/`between`) — a monitor reads a \
             trace from its start, and the Dwyer-scope encoding is deferred rather than lowered \
             wrongly",
        ));
    }
    let (from, to, within) = match &prop.pattern {
        Pattern::LeadsTo {
            from,
            to,
            within: Some(within),
        } => (from, to, within),
        Pattern::LeadsTo { within: None, .. } => {
            return Err(NotLowerable::new(
                "`leads_to` with no `within` is a qualitative claim — it says the response \
                 eventually comes, with no deadline, and a monitor over a finite trace can never \
                 refute that. It belongs to a model checker (category 2a), not here",
            ))
        }
        other => {
            return Err(NotLowerable::new(format!(
                "`{}` carries no metric deadline, so there is nothing here MonPoly can decide \
                 that a model checker cannot decide better; lowering it would be an approximation",
                pattern_verb(other)
            )))
        }
    };
    let within_seconds = deadline_seconds(within)?;

    let vars: Vec<String> = req.binders(prop).iter().map(|b| b.var.clone()).collect();
    let mut used = BTreeMap::new();
    let trigger = lower_expr(from, &vars, monitor, bindings, &mut used)?;
    let response = lower_expr(to, &vars, monitor, bindings, &mut used)?;

    Ok(MonitorClaim {
        // Rule 2: the negation. `from` happened and the response did NOT arrive inside the
        // deadline — which is exactly where the requirement is broken, and nowhere else.
        formula: format!("({trigger}) AND NOT EVENTUALLY[0,{within_seconds}] ({response})"),
        signature: signature(&used),
        within_seconds,
    })
}

/// The `.sig` contents for the events the formula actually names — not every declared event. A
/// signature declaring predicates the log never carries is not an error, but it invites the reader
/// to think the claim is about more than it is.
fn signature(used: &BTreeMap<String, Vec<String>>) -> String {
    used.iter()
        .map(|(name, args)| {
            let params = args
                .iter()
                .map(|a| format!("{a}:{ARG_TYPE}"))
                .collect::<Vec<_>>()
                .join(",");
            format!("{name}({params})\n")
        })
        .collect()
}

/// A deadline in whole seconds.
///
/// Sub-second is refused rather than rounded, and that is measured rather than assumed: MonPoly's
/// interval grammar takes integers, and `EVENTUALLY[0,0.5]` is a **parse error**, not a slower
/// check. Rounding `500ms` up to a second would silently widen the very bound the requirement is
/// about.
fn deadline_seconds(raw: &str) -> Result<u64, NotLowerable> {
    let text = raw.trim().to_ascii_lowercase().replace(' ', "");
    let split = text.find(|c: char| c.is_alphabetic()).ok_or_else(|| {
        NotLowerable::new(format!(
            "the deadline `{raw}` names no unit — write it as `30s`, `5m`, or `2h`, so the \
             interval provreq emits cannot mean something other than what was written"
        ))
    })?;
    let (value, unit) = text.split_at(split);
    let value: u64 = value.parse().map_err(|_| {
        NotLowerable::new(format!(
            "the deadline `{raw}` is not a whole number of {unit} — provreq emits an integer \
             interval, because MonPoly's own interval syntax takes one"
        ))
    })?;
    let seconds = match unit {
        "s" | "sec" | "secs" | "seconds" => value,
        "m" | "min" | "mins" | "minutes" => value * 60,
        "h" | "hr" | "hrs" | "hours" => value * 3600,
        "ms" => {
            return Err(NotLowerable::new(format!(
                "the deadline `{raw}` is sub-second, and MonPoly's interval syntax takes whole \
                 units — `EVENTUALLY[0,0.5]` does not parse. Rounding it would widen the bound the \
                 requirement is about, so it is refused instead"
            )))
        }
        other => {
            return Err(NotLowerable::new(format!(
                "the deadline `{raw}` is in `{other}`, which provreq does not know how to convert \
                 to the seconds a monitored trace is timed in — write `s`, `m`, or `h`"
            )))
        }
    };
    Ok(seconds)
}

fn lower_expr(
    e: &Expr,
    vars: &[String],
    monitor: &Monitor,
    bindings: &[Binding],
    used: &mut BTreeMap<String, Vec<String>>,
) -> Result<String, NotLowerable> {
    match e {
        Expr::Atom(a) => lower_atom(a, vars, monitor, bindings, used),
        Expr::Not(inner) => Ok(format!(
            "NOT ({})",
            lower_expr(inner, vars, monitor, bindings, used)?
        )),
        Expr::And(l, r) => Ok(format!(
            "({}) AND ({})",
            lower_expr(l, vars, monitor, bindings, used)?,
            lower_expr(r, vars, monitor, bindings, used)?
        )),
        Expr::Or(l, r) => Ok(format!(
            "({}) OR ({})",
            lower_expr(l, vars, monitor, bindings, used)?,
            lower_expr(r, vars, monitor, bindings, used)?
        )),
    }
}

/// One predicate application: the event's **trace-side name** applied to the claim's own variables.
///
/// The binding names the alias the operator declared (#231); the `name:` inside it is what the log
/// actually calls the event, and that is what MonPoly must read. Arity is re-checked here even
/// though grounding checks it, for the reason [`crate::tlc`]'s total match exists: this is public
/// and must not depend on a caller having grounded first.
fn lower_atom(
    a: &Atom,
    vars: &[String],
    monitor: &Monitor,
    bindings: &[Binding],
    used: &mut BTreeMap<String, Vec<String>>,
) -> Result<String, NotLowerable> {
    if let Some(guard) = &a.guard {
        return Err(NotLowerable::new(format!(
            "`{}` carries a `with` guard ({guard}), which the parser keeps as raw text — lowering \
             it would mean emitting MFOTL this tool never understood",
            a.name
        )));
    }
    let binding = bindings
        .iter()
        .find(|b| b.symbol == a.name)
        .ok_or_else(|| {
            NotLowerable::new(format!(
                "`{}` is not bound to a declared event, so there is nothing for a monitor to look \
                 for in the trace",
                a.name
            ))
        })?;
    let event = monitor.event(&binding.observable).ok_or_else(|| {
        NotLowerable::new(format!(
            "`{}` is bound to `{}`, which `monitor.events` in provreq.yml does not declare",
            a.name, binding.observable
        ))
    })?;
    if event.args.len() != a.args.len() {
        return Err(NotLowerable::new(format!(
            "the declared event `{}` carries {} argument(s), but the claim applies `{}` to {} — a \
             formula written at the wrong arity is one MonPoly would reject, or worse, silently \
             read as being about something else",
            event.name,
            event.args.len(),
            a.name,
            a.args.len()
        )));
    }
    // Rule 3: the claim's variables go in FREE. An argument that is not one of them has nothing to
    // bind it, and MonPoly would read a bare token as a value to match literally.
    for arg in &a.args {
        if !vars.iter().any(|v| v == arg.trim()) {
            return Err(NotLowerable::new(format!(
                "`{}` is applied to `{}`, which is not a variable the claim ranges over — a \
                 monitor would read it as a literal value to match, which is not what was written",
                a.name,
                arg.trim()
            )));
        }
    }
    used.insert(event.name.clone(), event.args.clone());
    let args = a
        .args
        .iter()
        .map(|arg| arg.trim().to_string())
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!("{}({args})", event.name))
}

/// Locate MonPoly for the real-engine tests: `MONPOLY_BIN` if set, else `monpoly` on `PATH` —
/// the same shape as [`crate::tlc`]'s `TLA2TOOLS_JAR`.
#[cfg(test)]
fn monpoly_bin() -> String {
    std::env::var("MONPOLY_BIN").unwrap_or_else(|_| "monpoly".to_string())
}

fn pattern_verb(p: &Pattern) -> &'static str {
    match p {
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
    use super::super::declaration::{Event, TraceFormat};
    use super::*;
    use crate::grounding::{BindCategory, Fidelity};

    const DEADLINE: &str = "requirement delivered {
        category: 2b
        vocabulary { sort Message event accepted(m: Message) event succeeded(m: Message) }
        require { each m: Message . accepted(m) leads_to succeeded(m) within 30s }
    }";

    fn requirement(text: &str) -> Requirement {
        crate::prl::gate(text)
            .unwrap_or_else(|e| panic!("the fixture must gate: {e:?}"))
            .requirement
    }

    fn monitor() -> Monitor {
        Monitor::new(
            "logs/events.jsonl".into(),
            TraceFormat::Jsonl,
            "ts",
            "event",
            BTreeMap::from([
                (
                    "accepted".to_string(),
                    Event {
                        name: "accepted".into(),
                        args: vec!["id".into()],
                    },
                ),
                (
                    "succeeded".to_string(),
                    Event {
                        name: "succeeded".into(),
                        args: vec!["id".into()],
                    },
                ),
            ]),
        )
    }

    fn bindings() -> Vec<Binding> {
        ["accepted", "succeeded"]
            .iter()
            .map(|s| Binding {
                symbol: (*s).into(),
                category: BindCategory::Runtime,
                observable: (*s).into(),
                fidelity: Fidelity::Observed,
            })
            .collect()
    }

    fn lowered(text: &str) -> Result<MonitorClaim, NotLowerable> {
        let req = requirement(text);
        let prop = req.require[0].clone();
        lower(&req, &prop, &monitor(), &bindings())
    }

    // Verifies: #232 — the two rules that make a formula monitorable at all. No leading `ALWAYS`
    // (MonPoly refuses an unbounded future operator outright), and the NEGATION rather than the
    // policy (MonPoly reports what matches, so the violation pattern is what makes silence mean
    // "clean"). Both are pinned against the real engine by the two `#[ignore]`d tests below; this
    // one pins the emitted text so a refactor cannot quietly drop either rule.
    #[test]
    fn a_deadline_lowers_to_the_negated_violation_pattern() {
        let c = lowered(DEADLINE).expect("a metric deadline is exactly what 2b lowers");
        assert_eq!(
            c.formula,
            "(accepted(m)) AND NOT EVENTUALLY[0,30] (succeeded(m))"
        );
        assert_eq!(c.within_seconds, 30);
        assert!(!c.formula.contains("ALWAYS"), "rule 1: {}", c.formula);
        assert!(
            c.formula.contains("NOT EVENTUALLY"),
            "rule 2: {}",
            c.formula
        );
        // Rule 3: the variable stays free. `EXISTS`/`FORALL` would throw the witness tuple away.
        assert!(
            !c.formula.contains("EXISTS") && !c.formula.contains("FORALL"),
            "rule 3: {}",
            c.formula
        );
        // The signature declares the trace-side names, not the aliases, and only what is used.
        assert_eq!(c.signature, "accepted(id:string)\nsucceeded(id:string)\n");
    }

    // Verifies: #232 — the deadline is converted, and anything MonPoly's interval grammar cannot
    // express is refused rather than approximated. Sub-second is the one that matters: rounding
    // `500ms` up to a second would silently widen the bound the requirement is about.
    #[test]
    fn a_deadline_converts_or_is_refused_with_its_reason() {
        for (within, seconds) in [("30s", 30), ("5m", 300), ("2h", 7200), ("1 min", 60)] {
            let c = lowered(&DEADLINE.replace("30s", within)).expect(within);
            assert_eq!(c.within_seconds, seconds, "{within}");
            assert!(c.formula.contains(&format!("[0,{seconds}]")), "{within}");
        }
        for (within, names) in [
            ("500ms", "sub-second"),
            ("30", "names no unit"),
            ("2days", "does not know how to convert"),
        ] {
            let err = lowered(&DEADLINE.replace("30s", within))
                .expect_err("an inexpressible deadline is never approximated");
            assert!(err.reason.contains(names), "{within}: {}", err.reason);
        }
    }

    // Verifies: #232 — what is not faithfully expressible is an honest refusal with a reason, the
    // rule the 2a lowering already follows. A `leads_to` with NO deadline is the interesting one:
    // it is not a 2b claim at all, and a monitor over a finite trace could never refute it.
    #[test]
    fn what_carries_no_metric_deadline_is_refused_with_the_reason() {
        let no_deadline = DEADLINE.replace(" within 30s", "");
        let err = lowered(&no_deadline).expect_err("a qualitative leads_to is not 2b's to decide");
        assert!(err.reason.contains("no deadline"), "{}", err.reason);

        let invariant = "requirement r {
            category: 2b
            vocabulary { sort Message event accepted(m: Message) }
            require { each m: Message . always accepted(m) }
        }";
        let err = lowered(invariant).expect_err("an invariant carries no deadline");
        assert!(err.reason.contains("`always`"), "{}", err.reason);
    }

    // Verifies: #232 — an argument that is not a variable the claim ranges over is refused. MonPoly
    // would read a bare token as a literal value to match, so the formula would silently be about
    // something other than what was written.
    #[test]
    fn an_argument_that_is_not_a_claim_variable_is_refused() {
        let literal = DEADLINE.replace("accepted(m) leads_to", "accepted(seven) leads_to");
        let err = lowered(&literal).expect_err("a literal argument is not lowered");
        assert!(err.reason.contains("not a variable"), "{}", err.reason);
    }

    // Verifies: #232 — an unbound symbol, or one bound to an event the manifest does not declare,
    // is refused here rather than emitted for MonPoly to reject with a message about a signature
    // file the operator never wrote.
    #[test]
    fn an_undeclared_event_is_refused_before_the_engine_sees_it() {
        let req = requirement(DEADLINE);
        let prop = req.require[0].clone();
        let mut wrong = bindings();
        wrong[1].observable = "nope".into();
        let err = lower(&req, &prop, &monitor(), &wrong).expect_err("undeclared cannot lower");
        assert!(err.reason.contains("does not declare"), "{}", err.reason);

        let err = lower(&req, &prop, &monitor(), &wrong[..1]).expect_err("unbound cannot lower");
        assert!(err.reason.contains("not bound"), "{}", err.reason);
    }

    /// Run MonPoly over a log with the lowered claim, returning its stdout.
    #[cfg(test)]
    fn run_monpoly(claim: &MonitorClaim, log: &str) -> String {
        let dir = tempfile::tempdir().expect("tempdir");
        let sig = dir.path().join("subject.sig");
        let formula = dir.path().join("claim.mfotl");
        let trace = dir.path().join("trace.log");
        std::fs::write(&sig, &claim.signature).expect("sig");
        std::fs::write(&formula, format!("{}\n", claim.formula)).expect("formula");
        std::fs::write(&trace, log).expect("log");
        let out = std::process::Command::new(monpoly_bin())
            .args(["-sig", sig.to_str().unwrap()])
            .args(["-formula", formula.to_str().unwrap()])
            .args(["-log", trace.to_str().unwrap()])
            .output()
            .expect("MonPoly must be installed for a real-engine test");
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    }

    // Verifies: #232 against the REAL ENGINE, in the direction that must stay silent. This is half
    // of the pair the arc's most dangerous bug needs: if the lowering ever emits the policy instead
    // of its negation, this test sees output where there must be none.
    //
    // Deliberately not asserted on a string: MonPoly's refusal message ("not monitorable") and its
    // witness lines both arrive on a ZERO exit code, so silence is the only signal that means
    // "clean" — and this test is what proves the emitted formula earns it.
    #[test]
    #[ignore = "requires MonPoly (MONPOLY_BIN or `monpoly` on PATH)"]
    fn real_monpoly_says_nothing_when_the_deadline_is_met() {
        let claim = lowered(DEADLINE).expect("lowers");
        let clean =
            "@100 accepted (1)\n@110 succeeded (1)\n@300 accepted (9)\n@305 succeeded (9)\n";
        let out = run_monpoly(&claim, clean);
        assert!(
            out.trim().is_empty(),
            "a met deadline must produce NO output — anything here would be read as a violation \
             by the adapter: {out}"
        );
    }

    // Verifies: #232 against the REAL ENGINE, in the direction that must speak. The other half of
    // the pair — and it checks the WITNESS, not merely that something was printed: the offending
    // id is what makes a refutation re-checkable (D9), and it is exactly what an `EXISTS` binder
    // would have thrown away.
    #[test]
    #[ignore = "requires MonPoly (MONPOLY_BIN or `monpoly` on PATH)"]
    fn real_monpoly_reports_the_offending_tuple_when_the_deadline_is_missed() {
        let claim = lowered(DEADLINE).expect("lowers");
        // id 2 is accepted at 100 and only succeeds at 200 — 100s later, well past the 30s bound.
        // id 9 is well inside it, so exactly one time point may match.
        let dirty =
            "@100 accepted (2)\n@200 succeeded (2)\n@300 accepted (9)\n@305 succeeded (9)\n";
        let out = run_monpoly(&claim, dirty);
        assert!(
            out.contains("(2)"),
            "the violating id must be the witness, not a bare `true`: {out}"
        );
        assert!(
            !out.contains("not monitorable"),
            "the emitted formula must be one MonPoly will actually monitor: {out}"
        );
        assert_eq!(
            out.lines().filter(|l| l.starts_with('@')).count(),
            1,
            "exactly the one violating time point matches: {out}"
        );
    }
}
