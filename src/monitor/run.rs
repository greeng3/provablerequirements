//! Running MonPoly and reading its answer honestly (#233).
//!
//! # ⚠️ The exit code carries no information — it is 0 for everything
//!
//! Measured in slice 0, and the trap this module exists to survive:
//!
//! | situation | exit |
//! | --- | --- |
//! | clean run, no violations | `0` |
//! | violations found | `0` |
//! | **formula refused as not monitorable** | **`0`** |
//!
//! An adapter that trusts the exit code reports a *refusal* as a passing verdict — a fabricated
//! `holds` of exactly the kind REQ024 and REQ051 exist to prevent. The sibling trap runs the other
//! way and is already recorded in [`crate::engine`]: a blanket non-zero rule is equally wrong,
//! because Creusot and TLC exit 1 while perfectly healthy.
//!
//! **So the output is parsed, and a refusal is detected explicitly. Silence is a clean result only
//! once a refusal has been ruled out.**
//!
//! # What each answer becomes
//!
//! - silence → `holds` / [`crate::verdict::Basis::NotFalsified`], carrying the trace it saw (#229
//!   will not let the rung be claimed without it);
//! - a refusal → `inconclusive`, naming MonPoly's own words;
//! - a violation line → `fails`, and the line **is** the witness: `@100. (time point 0): (2)` is
//!   timestamp, time-point index, and the offending tuple.
//!
//! The subject is never written to. The signature, formula and converted log all go to a scratch
//! directory that is removed when the run ends, so there is no generated file to clobber an
//! operator's and none to clean up afterwards.
//!
//! Implements: #233 (MonPoly runs, and its silence is read honestly).

use super::declaration::{Monitor, TraceFormat};
use super::mfotl::MonitorClaim;
use super::trace::Extent;
use crate::verdict::{Basis, Evidence};

/// The engine's name wherever it is reported, so the CLI, the web surface and a divergence all
/// say the same word.
pub const ENGINE: &str = "MonPoly";

/// MonPoly's binary — `MONPOLY_BIN` if set, else `monpoly` on `PATH`. Mirrors
/// [`crate::tlc::jar_path`]'s `TLA2TOOLS_JAR`, and lets a vendored build be pointed at without
/// putting it on the operator's `PATH`.
pub fn monpoly_bin() -> String {
    std::env::var("MONPOLY_BIN").unwrap_or_else(|_| "monpoly".to_string())
}

/// What MonPoly established about the trace it read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It ran to completion and matched nothing. Since the formula is the **violation** pattern
    /// (#232), matching nothing means no violation appeared in the trace it read.
    NotFalsified,
    /// The violation pattern matched. Every matching line is kept: the first is the witness, and
    /// the count is what stops "one late message" from reading the same as "everything is late".
    Violated { lines: Vec<String> },
    /// It could not decide — the formula was refused, the log would not parse, or the binary
    /// would not run. D10's `inconclusive(…)`: the tool answered without answering the question.
    Inconclusive { reason: String },
}

/// MonPoly's own words for a formula it will not monitor. Matched as a substring rather than a
/// whole line because the message arrives with its own punctuation and framing.
const REFUSAL_MARKERS: [&str; 4] = [
    "not monitorable",
    "Parse error",
    "Failed to parse",
    "Fatal error",
];

impl Outcome {
    /// Read MonPoly's combined stdout+stderr.
    ///
    /// Order is load-bearing: **a refusal is checked first**, because a refused formula also
    /// produces no violation lines and would otherwise be indistinguishable from a clean run.
    pub fn read(output: &str) -> Outcome {
        if let Some(refusal) = output
            .lines()
            .find(|l| REFUSAL_MARKERS.iter().any(|m| l.contains(m)))
        {
            return Outcome::Inconclusive {
                reason: format!("MonPoly would not monitor this: {}", refusal.trim()),
            };
        }
        // A match is reported as `@<timestamp>. (time point N): <tuple>`. Anything else MonPoly
        // prints (banners, timing) does not start with `@`.
        let lines: Vec<String> = output
            .lines()
            .filter(|l| l.trim_start().starts_with('@'))
            .map(|l| l.trim().to_string())
            .collect();
        if lines.is_empty() {
            Outcome::NotFalsified
        } else {
            Outcome::Violated { lines }
        }
    }

    /// Map what MonPoly established into a piece of [`Evidence`] — the mapping lives here, in the
    /// engine, so [`crate::verdict`] never learns what MonPoly is (D2, running in this direction
    /// too).
    ///
    /// `extent` is what the trace actually held, and it is **required** rather than optional
    /// because [`Evidence::not_falsified`] will not accept the rung without it (#229): an
    /// empirical claim with no reach attached overclaims by omission.
    pub fn into_evidence(&self, extent: &Extent) -> Evidence {
        match self {
            Outcome::NotFalsified => Evidence::not_falsified(ENGINE, extent.describe()),
            Outcome::Violated { lines } => {
                let mut detail = vec![format!(
                    "the deadline was missed {} time(s) — checked over {}",
                    lines.len(),
                    extent.describe()
                )];
                detail.extend(lines.iter().skip(1).cloned());
                Evidence::fails(ENGINE, lines.first().cloned(), detail)
            }
            Outcome::Inconclusive { reason } => Evidence::inconclusive(
                ENGINE,
                vec![
                    reason.clone(),
                    format!("checked over {}", extent.describe()),
                ],
            ),
        }
    }

    /// A `holds` from this engine can only ever be the empirical rung. Stated as an assertion the
    /// type system can see, so a future edit cannot quietly hand MonPoly a stronger basis.
    pub const BASIS: Basis = Basis::NotFalsified;
}

/// Run MonPoly over the operator's trace with the lowered claim.
///
/// Everything generated — signature, formula, and the converted log — lands in a scratch directory
/// that goes away when this returns. The subject is only ever read.
pub fn run(monitor: &Monitor, claim: &MonitorClaim) -> Outcome {
    let log = match monpoly_log(monitor) {
        Ok(log) => log,
        Err(reason) => return Outcome::Inconclusive { reason },
    };
    let dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            return Outcome::Inconclusive {
                reason: format!("could not create a scratch directory to run MonPoly in: {e}"),
            };
        }
    };
    let sig = dir.path().join("subject.sig");
    let formula = dir.path().join("claim.mfotl");
    let trace = dir.path().join("trace.log");
    for (path, text) in [
        (&sig, claim.signature.clone()),
        (&formula, format!("{}\n", claim.formula)),
        (&trace, log),
    ] {
        if let Err(e) = std::fs::write(path, text) {
            return Outcome::Inconclusive {
                reason: format!("could not write MonPoly's input at {}: {e}", path.display()),
            };
        }
    }

    let output = std::process::Command::new(monpoly_bin())
        .args(["-sig", &sig.to_string_lossy()])
        .args(["-formula", &formula.to_string_lossy()])
        .args(["-log", &trace.to_string_lossy()])
        .output();
    match output {
        Ok(out) => Outcome::read(&format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )),
        Err(e) => Outcome::Inconclusive {
            reason: format!("MonPoly ({}) could not be run: {e}", monpoly_bin()),
        },
    }
}

/// The operator's trace as a MonPoly log: `@<timestamp> pred (args)`, one time point per line.
///
/// A `monpoly`-format trace is already that and is passed through untouched — provreq does not
/// rewrite a log the subject already produces in the engine's own syntax.
///
/// A `jsonl` trace is converted, and **its timestamps must be numeric**. MonPoly's log grammar
/// takes a number of time units; an ISO-8601 string is not one, and provreq does not carry a date
/// parser to turn it into one. That is refused with the value it found rather than guessed at — a
/// wrong epoch would silently move every deadline in the claim.
fn monpoly_log(monitor: &Monitor) -> Result<String, String> {
    let text = std::fs::read_to_string(monitor.trace()).map_err(|e| {
        format!(
            "the monitor trace `{}` could not be read at {}: {e}",
            monitor.declared(),
            monitor.trace().display()
        )
    })?;
    if monitor.format() == TraceFormat::Monpoly {
        return Ok(text);
    }
    let mut out = String::new();
    for (i, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let record: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| format!("the trace's line {} is not JSON: {e}", i + 1))?;
        let stamp = record.get(monitor.time_field()).ok_or_else(|| {
            format!(
                "the trace's line {} has no `{}` field, so it has no place in time",
                i + 1,
                monitor.time_field()
            )
        })?;
        let seconds = stamp.as_f64().ok_or_else(|| {
            format!(
                "the trace's line {} times the record at `{stamp}`, which is not a number of \
                 seconds — MonPoly's log takes numeric timestamps, so record epoch seconds (or \
                 supply the trace as `format: monpoly`). provreq will not guess an epoch: a wrong \
                 one moves every deadline in the claim",
                i + 1
            )
        })?;
        let Some(name) = record.get(monitor.event_field()).and_then(render) else {
            // A record naming no event is not an error: a log routinely carries lines the claim is
            // not about. It still occupies its time point, so the timestamp is kept.
            out.push_str(&format!("@{seconds}\n"));
            continue;
        };
        // Only the DECLARED events reach the log — MonPoly rejects a predicate its signature does
        // not declare, and the signature holds exactly what the formula names (#232). An
        // undeclared line still occupies its time point, so the timestamp is kept without it.
        let Some(event) = monitor.events().values().find(|e| e.name == name) else {
            out.push_str(&format!("@{seconds}\n"));
            continue;
        };
        let args: Vec<String> = event
            .args
            .iter()
            .map(|arg| record.get(arg).and_then(render).unwrap_or_default())
            .collect();
        out.push_str(&format!("@{seconds} {} ({})\n", event.name, args.join(",")));
    }
    Ok(out)
}

/// A JSON value as a MonPoly log token. Strings render bare — quotes would become part of the
/// value, which is the same defect the extent line avoids (#230).
fn render(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Null => None,
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::declaration::{Event, TraceFormat};
    use super::*;
    use crate::verdict::Status;
    use std::collections::BTreeMap;

    fn events() -> BTreeMap<String, Event> {
        BTreeMap::from([
            (
                "accepted".to_string(),
                Event {
                    name: "accepted".into(),
                    args: vec!["id".into()],
                },
            ),
            (
                "delivered".to_string(),
                Event {
                    name: "done".into(),
                    args: vec!["id".into()],
                },
            ),
        ])
    }

    fn monitor_over(text: &str, format: TraceFormat) -> (tempfile::TempDir, Monitor) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let trace = tmp.path().join("events.log");
        std::fs::write(&trace, text).expect("trace");
        let (t, e) = match format {
            TraceFormat::Jsonl => ("ts", "event"),
            TraceFormat::Monpoly => ("", ""),
        };
        let m = Monitor::new(trace, format, t, e, events());
        (tmp, m)
    }

    fn extent(m: &Monitor) -> Extent {
        Extent::read(m).expect("the fixture trace is readable and non-empty")
    }

    const CLEAN: &str = "{\"ts\":100,\"event\":\"accepted\",\"id\":\"a\"}\n\
                         {\"ts\":110,\"event\":\"done\",\"id\":\"a\"}\n";

    // Verifies: #233 — THE trap. MonPoly exits 0 for a clean run, for a violation, AND for a
    // formula it refuses outright, so the exit code carries no information. A refusal must be
    // detected in the OUTPUT, and it must be detected FIRST: a refused formula also produces no
    // violation lines, so checked in the other order it is indistinguishable from a clean run and
    // becomes a fabricated `holds`.
    #[test]
    fn a_refusal_is_never_read_as_a_clean_run() {
        let refusal = "The formula contains an unbounded future temporal operator. It is hence \
                       not monitorable.";
        assert!(matches!(
            Outcome::read(refusal),
            Outcome::Inconclusive { .. }
        ));
        // Every refusal shape MonPoly produces, all on exit 0 and all with no `@` lines.
        for text in [
            "[Main.main] Failed to parse formula file",
            "Fatal error: exception Stdlib.Parsing.Parse_error",
            "Parse error in the signature file",
        ] {
            assert!(
                matches!(Outcome::read(text), Outcome::Inconclusive { .. }),
                "{text}"
            );
        }
        // And the one that must NOT be an inconclusive: genuine silence.
        assert_eq!(Outcome::read(""), Outcome::NotFalsified);
        assert_eq!(Outcome::read("\n  \n"), Outcome::NotFalsified);
    }

    // Verifies: #233 — a violation line IS the witness. `@100. (time point 0): (2)` carries the
    // timestamp, the time-point index and the offending tuple, which is what makes a refutation
    // re-checkable (D9) rather than an assertion that something went wrong.
    #[test]
    fn a_violation_line_becomes_the_witness() {
        let out = Outcome::read("@100. (time point 0): (2)\n@400. (time point 7): (9)\n");
        let Outcome::Violated { lines } = &out else {
            panic!("two matching lines are a violation: {out:?}")
        };
        assert_eq!(lines.len(), 2);

        let (_tmp, m) = monitor_over(CLEAN, TraceFormat::Jsonl);
        let e = out.into_evidence(&extent(&m));
        assert_eq!(e.status, Status::Fails);
        assert_eq!(e.witness.as_deref(), Some("@100. (time point 0): (2)"));
        assert_eq!(e.engine, "MonPoly");
        // The count is on the verdict: "one late message" must not read like "everything is late".
        assert!(e.detail[0].contains("missed 2 time(s)"), "{:?}", e.detail);
    }

    // Verifies: #233 + #229 — silence earns the EMPIRICAL rung and nothing stronger, and it
    // carries the trace it saw. `Evidence::not_falsified` will not accept the rung without an
    // extent, so this cannot regress into a bare `not-falsified`.
    #[test]
    fn silence_earns_not_falsified_carrying_the_trace_it_saw() {
        let (_tmp, m) = monitor_over(CLEAN, TraceFormat::Jsonl);
        let e = Outcome::NotFalsified.into_evidence(&extent(&m));
        assert_eq!(e.status, Status::Holds);
        assert_eq!(e.basis, Some(Basis::NotFalsified));
        assert_eq!(e.basis, Some(Outcome::BASIS));
        assert!(e.detail[0].contains("2 events"), "{:?}", e.detail);
        assert!(e.detail[0].contains("100 … 110"), "{:?}", e.detail);
    }

    // Verifies: #233 — an inconclusive says what happened AND what it was looking at. A reason
    // with no extent leaves the operator unable to tell "the formula was refused" from "the log
    // was empty".
    #[test]
    fn an_inconclusive_names_the_reason_and_the_trace() {
        let (_tmp, m) = monitor_over(CLEAN, TraceFormat::Jsonl);
        let e = Outcome::Inconclusive {
            reason: "MonPoly would not monitor this: not monitorable".into(),
        }
        .into_evidence(&extent(&m));
        assert_eq!(e.status, Status::Unknown);
        assert_eq!(e.basis, None, "an inconclusive claims no rung at all");
        assert!(e.detail[0].contains("not monitorable"));
        assert!(e.detail[1].contains("events.log"), "{:?}", e.detail);
    }

    // Verifies: #233 — a jsonl trace becomes MonPoly's own log syntax, with the TRACE-SIDE names
    // (`done`), not the aliases the operator binds (`delivered`).
    #[test]
    fn a_jsonl_trace_converts_to_monpoly_syntax() {
        let (_tmp, m) = monitor_over(CLEAN, TraceFormat::Jsonl);
        assert_eq!(
            monpoly_log(&m).expect("converts"),
            "@100 accepted (a)\n@110 done (a)\n"
        );

        // A record naming no declared event still occupies its time point — dropping the line
        // would move every later event closer to its trigger, and MonPoly rejects a predicate the
        // signature does not declare.
        let (_tmp, m) = monitor_over(
            "{\"ts\":100,\"event\":\"accepted\",\"id\":\"a\"}\n{\"ts\":150,\"event\":\"other\"}\n",
            TraceFormat::Jsonl,
        );
        assert_eq!(
            monpoly_log(&m).expect("converts"),
            "@100 accepted (a)\n@150\n"
        );
    }

    // Verifies: #233 — a `monpoly` trace is passed through untouched. provreq does not rewrite a
    // log the subject already produces in the engine's own syntax.
    #[test]
    fn a_monpoly_trace_is_passed_through_untouched() {
        let raw = "@100 accepted (a)\n@110 done (a)\n";
        let (_tmp, m) = monitor_over(raw, TraceFormat::Monpoly);
        assert_eq!(monpoly_log(&m).expect("passes through"), raw);
    }

    // Verifies: #233 — a non-numeric timestamp is refused with the value it found. MonPoly's log
    // takes a number of time units; provreq carries no date parser, and inventing an epoch would
    // silently move every deadline in the claim.
    #[test]
    fn a_non_numeric_timestamp_is_refused_rather_than_guessed() {
        let (_tmp, m) = monitor_over(
            "{\"ts\":\"2026-08-01T00:00:00Z\",\"event\":\"accepted\",\"id\":\"a\"}\n",
            TraceFormat::Jsonl,
        );
        let err = monpoly_log(&m).expect_err("an ISO timestamp is not a number of seconds");
        assert!(err.contains("2026-08-01T00:00:00Z"), "{err}");
        assert!(err.contains("will not guess an epoch"), "{err}");
    }

    // Verifies: #233 against the REAL ENGINE — the direction that must hold. A satisfying trace
    // must reach `holds`/`not-falsified` and NEVER `fails`: #232 emits the negation, so an
    // inversion anywhere between the formula and this reading turns a satisfied policy into a
    // refuted one. That cannot be caught by a string assertion.
    #[test]
    #[ignore = "requires MonPoly (MONPOLY_BIN or `monpoly` on PATH)"]
    fn real_monpoly_holds_over_a_trace_that_meets_the_deadline() {
        let (_tmp, m) = monitor_over(CLEAN, TraceFormat::Jsonl);
        let claim = super::super::mfotl::MonitorClaim {
            signature: "accepted(id:string)\ndone(id:string)\n".into(),
            formula: "(accepted(m)) AND NOT EVENTUALLY[0,30] (done(m))".into(),
            within_seconds: 30,
        };
        let outcome = run(&m, &claim);
        assert_eq!(
            outcome,
            Outcome::NotFalsified,
            "a met deadline is not a violation"
        );
        let e = outcome.into_evidence(&extent(&m));
        assert_eq!(e.status, Status::Holds);
        assert_eq!(e.basis, Some(Basis::NotFalsified));
    }

    // Verifies: #233 against the REAL ENGINE — the direction that must refute, with a witness the
    // operator can replay. The other half of the inversion guard.
    #[test]
    #[ignore = "requires MonPoly (MONPOLY_BIN or `monpoly` on PATH)"]
    fn real_monpoly_refutes_and_witnesses_a_missed_deadline() {
        let late = "{\"ts\":100,\"event\":\"accepted\",\"id\":\"2\"}\n\
                    {\"ts\":200,\"event\":\"done\",\"id\":\"2\"}\n";
        let (_tmp, m) = monitor_over(late, TraceFormat::Jsonl);
        let claim = super::super::mfotl::MonitorClaim {
            signature: "accepted(id:string)\ndone(id:string)\n".into(),
            formula: "(accepted(m)) AND NOT EVENTUALLY[0,30] (done(m))".into(),
            within_seconds: 30,
        };
        let outcome = run(&m, &claim);
        let Outcome::Violated { lines } = &outcome else {
            panic!("100s is well past a 30s deadline: {outcome:?}")
        };
        assert!(
            lines[0].contains("(2)"),
            "the offending id is the witness: {lines:?}"
        );
        let e = outcome.into_evidence(&extent(&m));
        assert_eq!(e.status, Status::Fails);
        assert!(
            e.witness.is_some(),
            "a refutation without a witness is unactionable"
        );
    }

    // Verifies: #233 against the REAL ENGINE — a formula MonPoly refuses reaches `inconclusive`,
    // never `holds`. This is the trap driven end to end rather than asserted on a string: the
    // refusal really does arrive on exit code 0 with no violation lines.
    #[test]
    #[ignore = "requires MonPoly (MONPOLY_BIN or `monpoly` on PATH)"]
    fn real_monpoly_refusing_a_formula_is_inconclusive_never_a_pass() {
        let (_tmp, m) = monitor_over(CLEAN, TraceFormat::Jsonl);
        let claim = super::super::mfotl::MonitorClaim {
            signature: "accepted(id:string)\ndone(id:string)\n".into(),
            // The leading ALWAYS #232 exists to never emit — the shape MonPoly refuses.
            formula: "ALWAYS FORALL m. accepted(m) IMPLIES EVENTUALLY[0,30] done(m)".into(),
            within_seconds: 30,
        };
        let outcome = run(&m, &claim);
        let Outcome::Inconclusive { reason } = &outcome else {
            panic!("a refused formula is never a verdict: {outcome:?}")
        };
        assert!(reason.contains("not monitorable"), "{reason}");
        let e = outcome.into_evidence(&extent(&m));
        assert_eq!(e.status, Status::Unknown);
        assert_eq!(e.basis, None);
    }
}
