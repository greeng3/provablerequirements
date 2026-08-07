//! Resolving a category-2b binding against the declared event signature (#231).
//!
//! Grounding is where the operator confirms a binding says what they meant (REQ057). Category 1
//! resolves against the subject's Rust ([`crate::rust_adapter`]) and 2a against its TLA+
//! ([`crate::tla_adapter`]). **Category 2b resolves against neither**: a 2b claim speaks of events
//! that appear in a log, so whether `accepted` is a real observable is answered by whether the
//! trace *declares* it — not by whether some Rust function happens to be named `accepted`.
//! Resolving against the code would bind the claim to the wrong artifact entirely and would
//! silently succeed on a subject whose logging says something else.
//!
//! **Declaration decides grounding; occurrence decides evidence.** A declared event that has never
//! once been emitted still grounds — the trace is a moving artifact (#230 made it a drift axis),
//! and a binding that flipped between grounded and parked as a log grew would be reporting the
//! weather, not the requirement. But a policy over an event that never fires is **vacuously
//! satisfied**, so the read-back says so outright, before a verdict is produced from it.

use super::declaration::{Event, Monitor};
use std::collections::BTreeMap;

/// What a category-2b binding resolved to, against the declared signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeResolution {
    /// Exactly the declared event, taking the number of arguments the requirement applies it to.
    /// The only variant that grounds.
    Resolved {
        event: Event,
        /// How many times it occurs in the trace as it stands — `None` when the trace could not be
        /// read at all, which is not the same answer as zero and is not reported as one.
        occurrences: Option<usize>,
    },
    /// A **sort** in a 2b claim: the variable a quantifier ranges over. Resolved by construction,
    /// and this is not a shortcut — a monitor binds that variable from the trace's own argument
    /// values, so unlike a model checker (which needs a finite domain to enumerate) there is no
    /// domain to declare and nothing here that could be wrong. Without this a quantified 2b claim
    /// could never ground, which is every claim #232 lowers.
    TraceBound,
    /// No `monitor:` block at all, so there is no signature to resolve against.
    NoMonitor,
    /// The subject declares events, but none under that name.
    NotDeclared { declared: Vec<String> },
    /// Declared, but it carries a different number of arguments than the requirement applies the
    /// symbol to. Refused here, where the binding is: a monitor asked for a 2-argument predicate
    /// with 1 argument reports a syntax error about a signature file the operator never wrote.
    WrongArity {
        event: Event,
        declared: usize,
        expected: usize,
    },
}

impl RuntimeResolution {
    /// Whether this binding resolved — the single question [`crate::grounding::verdict`] asks.
    pub fn is_resolved(&self) -> bool {
        matches!(
            self,
            RuntimeResolution::Resolved { .. } | RuntimeResolution::TraceBound
        )
    }

    /// The operator-facing read-back (D13: "here is what your binding resolves to — is that what
    /// you meant?"). A resolved event names what it was found in and how much of it the trace has
    /// actually seen, because both are things the operator can be wrong about and only one of them
    /// is visible in the manifest.
    pub fn describe(&self, symbol: &str, observable: &str) -> String {
        match self {
            RuntimeResolution::Resolved { event, occurrences } => {
                let args = if event.args.is_empty() {
                    "no arguments".to_string()
                } else {
                    format!("({})", event.args.join(", "))
                };
                let seen = match occurrences {
                    // The vacuity warning, and the reason this grounds rather than parks: the
                    // operator is about to be told a policy holds over an event that never fired.
                    Some(0) => "\n      ⚠ declared, but it does not occur even once in the trace \
                                as it stands — a policy over an event that never fires is \
                                vacuously satisfied, and a monitor would report `not-falsified` \
                                having seen nothing of it"
                        .to_string(),
                    Some(n) => format!("\n      (occurs {n}× in the trace as it stands)"),
                    None => "\n      (the trace could not be read, so how often it occurs is \
                             unknown — not zero)"
                        .to_string(),
                };
                format!(
                    "{symbol} → `{observable}` resolves to the declared event `{}` taking {args}\
                     {seen}",
                    event.name
                )
            }
            RuntimeResolution::TraceBound => format!(
                "{symbol} → `{observable}` is the domain of a quantified variable, which a monitor \
                 binds from the trace's own values — there is no domain to declare, so nothing is \
                 checked here"
            ),
            RuntimeResolution::NoMonitor => format!(
                "{symbol}: `{observable}` cannot resolve — this subject declares no `monitor:` \
                 block in provreq.yml, so there is no trace for a category-2b claim to be observed \
                 through"
            ),
            RuntimeResolution::NotDeclared { declared } => {
                let known = if declared.is_empty() {
                    "`monitor.events` is empty".to_string()
                } else {
                    format!("declared events are: {}", declared.join(", "))
                };
                format!(
                    "{symbol}: `{observable}` is not declared under `monitor.events` in \
                     provreq.yml — {known}"
                )
            }
            RuntimeResolution::WrongArity {
                event,
                declared,
                expected,
            } => format!(
                "{symbol}: the declared event `{}` carries {declared} argument(s), but the \
                 requirement applies `{symbol}` to {expected}. A binding checked at the wrong \
                 arity is a binding that proves nothing, so this is refused here rather than left \
                 to the monitor",
                event.name
            ),
        }
    }
}

/// Resolve one 2b binding against the declared signature.
///
/// `occurrences` is the count map from [`super::trace::occurrences`], or `None` when the trace
/// could not be read — carried through rather than recomputed per binding, because one dry-run is
/// one reading of the trace.
pub fn resolve(
    monitor: Option<&Monitor>,
    observable: &str,
    arity: usize,
    occurrences: Option<&BTreeMap<String, usize>>,
) -> RuntimeResolution {
    let Some(monitor) = monitor else {
        return RuntimeResolution::NoMonitor;
    };
    let Some(event) = monitor.event(observable) else {
        return RuntimeResolution::NotDeclared {
            declared: monitor.aliases(),
        };
    };
    if event.args.len() != arity {
        return RuntimeResolution::WrongArity {
            event: event.clone(),
            declared: event.args.len(),
            expected: arity,
        };
    }
    RuntimeResolution::Resolved {
        event: event.clone(),
        occurrences: occurrences.map(|c| c.get(observable).copied().unwrap_or(0)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::declaration::TraceFormat;
    use super::*;

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
                        name: "msg_accepted".into(),
                        args: vec!["id".into()],
                    },
                ),
                (
                    "swept".to_string(),
                    Event {
                        name: "msg_swept".into(),
                        args: vec![],
                    },
                ),
            ]),
        )
    }

    // Verifies: #231 — a 2b binding resolves against the DECLARED EVENT, not the subject's code.
    // The read-back names the trace-side spelling the operator did not have to repeat, and how much
    // of it the trace has actually seen.
    #[test]
    fn a_binding_resolves_against_the_declared_event() {
        let counts = BTreeMap::from([("accepted".to_string(), 7usize)]);
        let r = resolve(Some(&monitor()), "accepted", 1, Some(&counts));
        assert!(r.is_resolved());
        let text = r.describe("accepted", "accepted");
        assert!(text.contains("`msg_accepted`"), "{text}");
        assert!(text.contains("(id)"), "{text}");
        assert!(text.contains("occurs 7×"), "{text}");
    }

    // Verifies: #231 — arity is not negotiable (REQ026/#119). An event carrying one argument cannot
    // stand in for a predicate applied to two, and the refusal happens where the binding is.
    #[test]
    fn a_wrong_arity_binding_parks_where_the_binding_is() {
        let r = resolve(Some(&monitor()), "accepted", 2, None);
        assert!(!r.is_resolved());
        let text = r.describe("accepted", "accepted");
        assert!(text.contains("carries 1 argument(s)"), "{text}");
        assert!(text.contains("applies `accepted` to 2"), "{text}");

        // The zero-argument direction is the same rule, and the one a 2a sort would hit.
        assert!(!resolve(Some(&monitor()), "swept", 1, None).is_resolved());
        assert!(resolve(Some(&monitor()), "swept", 0, None).is_resolved());
    }

    // Verifies: #231 — an undeclared event parks with a reason naming what was looked in and what
    // was found, so the operator does not have to go read the manifest to find their own typo.
    #[test]
    fn an_undeclared_event_parks_naming_what_is_declared() {
        let r = resolve(Some(&monitor()), "acepted", 1, None);
        assert!(!r.is_resolved());
        let text = r.describe("accepted", "acepted");
        assert!(text.contains("monitor.events"), "{text}");
        assert!(text.contains("accepted, swept"), "{text}");
    }

    // Verifies: #232 — a SORT in a 2b claim resolves by construction. A monitor binds a quantified
    // variable from the trace's own argument values, so there is no domain to declare — unlike a
    // model checker, which needs a finite one to enumerate. Found live: without this, `Message` was
    // looked up as an event and parked, so a quantified 2b claim could NEVER ground, which is every
    // claim the MFOTL lowering emits.
    #[test]
    fn a_sort_in_a_runtime_claim_is_bound_by_the_trace_itself() {
        let r = RuntimeResolution::TraceBound;
        assert!(r.is_resolved());
        let text = r.describe("Message", "Message");
        assert!(text.contains("no domain to declare"), "{text}");
        assert!(text.contains("nothing is checked here"), "{text}");
    }

    // Verifies: #231 — a subject with no monitor at all says so, rather than reporting the event as
    // undeclared. The two ask different things of the operator: one is a typo, the other is a
    // manifest block that was never written.
    #[test]
    fn a_subject_with_no_monitor_says_that_rather_than_not_declared() {
        let r = resolve(None, "accepted", 1, None);
        assert!(!r.is_resolved());
        assert!(r
            .describe("accepted", "accepted")
            .contains("no `monitor:` block"));
    }

    // Verifies: #231 — the decision this slice had to make. A declared event with zero occurrences
    // GROUNDS (the declaration is what binding is about, and a trace that grows must not flip a
    // binding in and out of grounded), but the read-back warns that the policy is vacuous. "Never
    // occurred" and "could not look" are different answers and never render the same.
    #[test]
    fn a_declared_event_that_never_fires_grounds_but_warns_of_vacuity() {
        let never = BTreeMap::from([("accepted".to_string(), 0usize)]);
        let r = resolve(Some(&monitor()), "accepted", 1, Some(&never));
        assert!(
            r.is_resolved(),
            "the declaration is what grounding is about; occurrence is evidence"
        );
        let text = r.describe("accepted", "accepted");
        assert!(text.contains("vacuously satisfied"), "{text}");
        assert!(text.contains("not-falsified"), "{text}");

        let unread = resolve(Some(&monitor()), "accepted", 1, None);
        let text = unread.describe("accepted", "accepted");
        assert!(text.contains("not zero"), "{text}");
        assert!(!text.contains("vacuously satisfied"), "{text}");
    }
}
