//! Reading the declared trace: what is in it, and whether there is anything in it at all.

use super::declaration::{Monitor, TraceFormat};

/// What a trace actually contained — the extent a `not-falsified` verdict is over
/// ([`crate::verdict::Evidence::not_falsified`] takes exactly this, as a required argument).
///
/// Constructing one means the trace was found, read, and had records in it. There is no
/// `Extent::empty`: an empty trace is refused at [`Extent::read`], so no code path can hold an
/// extent that corroborates nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extent {
    declared: String,
    records: usize,
    noun: &'static str,
    first: String,
    last: String,
    fingerprint: String,
}

impl Extent {
    /// Read the declared trace and measure it.
    ///
    /// Every failure here is loud and names the path, because the alternative is the one reading
    /// #230 exists to prevent: a monitor over a trace it could not read reports no violations, and
    /// "I saw nothing" is indistinguishable from "there was nothing to see" once it reaches a
    /// verdict. See [`crate::monitor`].
    pub fn read(monitor: &Monitor) -> Result<Extent, String> {
        let declared = monitor.declared();
        let path = monitor.trace();
        let text = std::fs::read_to_string(path).map_err(|e| {
            let resolved = path.display();
            match e.kind() {
                std::io::ErrorKind::NotFound => format!(
                    "the monitor trace `{declared}` does not exist (looked in {resolved}) — \
                     provreq will not read a missing log as `no violations`. Produce the trace, or \
                     correct `monitor.trace` in provreq.yml"
                ),
                _ => {
                    format!("the monitor trace `{declared}` could not be read (at {resolved}): {e}")
                }
            }
        })?;

        let format = monitor.format();
        let stamps = timestamps(&text, format, monitor.time_field())
            .map_err(|e| format!("the monitor trace `{declared}` {e}"))?;
        let (Some(first), Some(last)) = (stamps.first(), stamps.last()) else {
            return Err(format!(
                "the monitor trace `{declared}` has no {} in it (at {}) — an empty trace cannot \
                 falsify anything, so monitoring it would report `not-falsified` having observed \
                 nothing at all",
                format.record_noun(),
                path.display()
            ));
        };

        Ok(Extent {
            declared: declared.to_string(),
            records: stamps.len(),
            noun: format.record_noun(),
            first: first.clone(),
            last: last.clone(),
            fingerprint: fingerprint(declared, &text),
        })
    }

    /// The one line a verdict carries: which trace, how much of it, over what span. This is the
    /// `over` argument [`crate::verdict::Evidence::not_falsified`] demands, and the reason it
    /// demands one — `not-falsified` without this is a claim with no reach attached.
    ///
    /// `first`/`last` are the trace's own order, not a sorted minimum and maximum: provreq reports
    /// what it read rather than tidying it, and a log written out of order is the subject's to fix.
    pub fn describe(&self) -> String {
        format!(
            "{} — {} {}, {} … {}",
            self.declared, self.records, self.noun, self.first, self.last
        )
    }

    /// A fingerprint of the trace, for the drift anchor. A verdict proved against a log that keeps
    /// growing would read `fresh` forever while the evidence under it moved, which is what #120
    /// fingerprints external specs to prevent — a trace is the same problem with a faster clock.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn records(&self) -> usize {
        self.records
    }
}

/// The fingerprint of the declared trace as it is right now, for a caller that only needs to
/// compare (the staleness check) and not to read the trace.
///
/// `None` when no monitor is configured, when the manifest cannot be read, or when the trace is not
/// there — the same shape as [`crate::tla_adapter::current_external_fingerprint`], where "no such
/// axis" and "cannot answer" both mean this axis does not flag the verdict. A trace that has gone
/// missing is a loud failure at verification time, not a silent staleness flag now.
pub fn current_fingerprint(
    subject_root: &std::path::Path,
    companion_root: &std::path::Path,
) -> Option<String> {
    let monitor = Monitor::load(subject_root, companion_root).ok()??;
    let text = std::fs::read_to_string(monitor.trace()).ok()?;
    Some(fingerprint(monitor.declared(), &text))
}

/// How many times each **declared** event actually occurs in the trace right now, keyed by the
/// alias the operator declared it under.
///
/// `None` when the trace cannot be read at all — no monitor, no file, or a record provreq cannot
/// parse. That is deliberately quiet: this feeds the grounding dry-run, and a dry-run must not fail
/// on a log that happens not to exist yet. The loud version of the same question is
/// [`Extent::read`], which runs at verification time, where a missing trace is a refusal.
///
/// A declared event with **zero** occurrences is `Some(0)`, never absent — "never happened" and
/// "could not look" are different answers and the read-back says which.
pub fn occurrences(monitor: &Monitor) -> Option<std::collections::BTreeMap<String, usize>> {
    let text = std::fs::read_to_string(monitor.trace()).ok()?;
    let mut counts: std::collections::BTreeMap<String, usize> = monitor
        .events()
        .keys()
        .map(|alias| (alias.clone(), 0))
        .collect();
    let by_name: std::collections::BTreeMap<&str, &str> = monitor
        .events()
        .iter()
        .map(|(alias, e)| (e.name.as_str(), alias.as_str()))
        .collect();

    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let names = match monitor.format() {
            TraceFormat::Jsonl => jsonl_event_name(line, monitor.event_field())
                .into_iter()
                .collect::<Vec<_>>(),
            TraceFormat::Monpoly => monpoly_event_names(line),
        };
        for name in names {
            if let Some(alias) = by_name.get(name.as_str()) {
                *counts.entry((*alias).to_string()).or_default() += 1;
            }
        }
    }
    Some(counts)
}

/// A record's event name, or `None` for one provreq cannot read. Unreadable records are skipped
/// here rather than refused, because this is the quiet dry-run path — [`Extent::read`] is where a
/// malformed trace is an error, and it names the line.
fn jsonl_event_name(line: &str, event_field: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    match value.get(event_field)? {
        serde_json::Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// Every predicate named on one MonPoly line: `@100 accepted (1) done (2)` names two. A time point
/// carrying the same predicate twice is two occurrences of it, which is what a monitor sees.
///
// ponytail: a token scan, not a MonPoly parser. It answers one question — does this declared event
// ever appear — and the authority on reading the log is MonPoly itself, once #233 runs it. If this
// ever has to answer more than "how many times", replace it rather than extending it.
fn monpoly_event_names(line: &str) -> Vec<String> {
    let Some(rest) = line.trim_start().strip_prefix('@') else {
        return Vec::new();
    };
    // Drop the timestamp, then take every token that is not an argument list. `(` starts arguments,
    // so a name is the run of characters before one.
    rest.split_whitespace()
        .skip(1)
        .flat_map(|tok| tok.split('('))
        .map(str::trim)
        .filter(|t| !t.is_empty() && !t.ends_with(')') && !t.contains(','))
        .map(str::to_string)
        .collect()
}

fn fingerprint(declared: &str, text: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (declared, text).hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Every record's timestamp, in file order. Blank lines are skipped — a log ending in a newline is
/// not a malformed log — but a non-blank line that will not parse is an error naming its number,
/// never a record quietly dropped from the count.
fn timestamps(text: &str, format: TraceFormat, time_field: &str) -> Result<Vec<String>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            match format {
                TraceFormat::Jsonl => jsonl_timestamp(line, time_field),
                TraceFormat::Monpoly => monpoly_timestamp(line),
            }
            .map_err(|e| format!("{e} (line {})", i + 1))
        })
        .collect()
}

fn jsonl_timestamp(line: &str, time_field: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("is not one JSON object per line: {e}"))?;
    let stamp = value
        .get(time_field)
        .ok_or_else(|| format!("has a record with no `{time_field}` field"))?;
    // A string timestamp renders as itself; a numeric one as the number. `to_string()` alone would
    // put quotes around the string form, which then reads as part of the timestamp on the verdict.
    Ok(match stamp {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

/// MonPoly's log: `@<timestamp> pred(args) pred(args)`. The timestamp is everything between the
/// `@` and the first whitespace or `(`.
fn monpoly_timestamp(line: &str) -> Result<String, String> {
    let rest = line.trim_start().strip_prefix('@').ok_or(
        "has a line that does not start with `@<timestamp>`, which every MonPoly log line does",
    )?;
    let stamp: String = rest
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '(')
        .collect();
    if stamp.is_empty() {
        return Err("has an `@` with no timestamp after it".into());
    }
    Ok(stamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn monitor_over(contents: Option<&str>, format: TraceFormat) -> (tempfile::TempDir, Monitor) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let trace = tmp.path().join("events.log");
        if let Some(text) = contents {
            std::fs::write(&trace, text).expect("trace");
        }
        let m = Monitor::new(
            trace,
            format,
            ts_field(format),
            ev_field(format),
            BTreeMap::new(),
        );
        (tmp, m)
    }

    fn ts_field(format: TraceFormat) -> &'static str {
        match format {
            TraceFormat::Jsonl => "ts",
            TraceFormat::Monpoly => "",
        }
    }

    fn ev_field(format: TraceFormat) -> &'static str {
        match format {
            TraceFormat::Jsonl => "event",
            TraceFormat::Monpoly => "",
        }
    }

    const TWO_EVENTS: &str = "{\"ts\":\"2026-08-01T00:00:00Z\",\"event\":\"accepted\"}\n\
                              {\"ts\":\"2026-08-06T23:59:00Z\",\"event\":\"done\"}\n";

    // Verifies: #230 — a jsonl trace is measured, and the measurement is the line a `not-falsified`
    // verdict carries: which trace, how many records, over what span (#229).
    #[test]
    fn a_jsonl_trace_is_measured_into_the_line_a_verdict_carries() {
        let (_tmp, m) = monitor_over(Some(TWO_EVENTS), TraceFormat::Jsonl);
        let extent = Extent::read(&m).expect("reads");
        assert_eq!(extent.records(), 2);
        let line = extent.describe();
        assert!(line.contains("2 events"), "{line}");
        assert!(
            line.contains("2026-08-01T00:00:00Z … 2026-08-06T23:59:00Z"),
            "{line}"
        );
        // The span is not quoted: a JSON string timestamp must not reach the verdict wearing its
        // own quotation marks.
        assert!(!line.contains('"'), "{line}");
    }

    // Verifies: #230 — MonPoly's own log is accepted with no conversion, and its unit is named for
    // what it is. A line carrying three predicates is ONE time point, not three events.
    #[test]
    fn a_monpoly_trace_is_read_as_time_points() {
        let (_tmp, m) = monitor_over(
            Some("@100 accepted (1) accepted (2)\n@140 done (1)\n"),
            TraceFormat::Monpoly,
        );
        let extent = Extent::read(&m).expect("reads");
        assert_eq!(extent.records(), 2);
        let line = extent.describe();
        assert!(line.contains("2 time points"), "{line}");
        assert!(line.contains("100 … 140"), "{line}");
    }

    // Verifies: #230 — THE rule of this slice. A missing log must never read as "no violations":
    // the failure names the path the operator wrote AND where provreq looked, and says outright
    // that it will not treat the silence as a pass.
    #[test]
    fn a_missing_trace_is_a_loud_failure_naming_the_path() {
        let (_tmp, m) = monitor_over(None, TraceFormat::Jsonl);
        let err = Extent::read(&m).expect_err("a missing trace is never a clean result");
        assert!(err.contains("does not exist"), "{err}");
        assert!(err.contains("events.log"), "{err}");
        assert!(err.contains("no violations"), "{err}");
    }

    // Verifies: #230 — an EMPTY trace is the same defect wearing a file. Zero records cannot
    // falsify anything, so a monitor over one would report `not-falsified` having observed nothing.
    #[test]
    fn an_empty_trace_is_refused_rather_than_monitored_clean() {
        for (contents, format) in [
            ("", TraceFormat::Jsonl),
            ("\n\n   \n", TraceFormat::Jsonl),
            ("", TraceFormat::Monpoly),
        ] {
            let (_tmp, m) = monitor_over(Some(contents), format);
            let err = Extent::read(&m).expect_err("an empty trace corroborates nothing");
            assert!(err.contains("cannot falsify anything"), "{err}");
        }
    }

    // Verifies: #230 — a record provreq cannot read is an error naming the line, never a record
    // silently dropped. A dropped record shrinks the extent, and the extent is the whole claim.
    #[test]
    fn an_unreadable_record_names_its_line_instead_of_being_dropped() {
        let (_tmp, m) = monitor_over(
            Some("{\"ts\":\"1\"}\nnot json at all\n{\"ts\":\"3\"}\n"),
            TraceFormat::Jsonl,
        );
        let err = Extent::read(&m).expect_err("a bad line is not skipped");
        assert!(err.contains("line 2"), "{err}");

        // A record with no timestamp field is the same class: it is a record provreq cannot place
        // in time, and guessing would understate the span.
        let (_tmp, m) = monitor_over(
            Some("{\"ts\":\"1\"}\n{\"when\":\"2\"}\n"),
            TraceFormat::Jsonl,
        );
        let err = Extent::read(&m).expect_err("an untimed record is not skipped");
        assert!(err.contains("`ts`") && err.contains("line 2"), "{err}");

        let (_tmp, m) = monitor_over(Some("@100 a (1)\n200 b (2)\n"), TraceFormat::Monpoly);
        let err = Extent::read(&m).expect_err("a line with no @ is not skipped");
        assert!(err.contains("line 2"), "{err}");
    }

    // Verifies: #230 — a trailing newline is not a malformed record. The common shape of every log
    // file must not read as a parse error.
    #[test]
    fn a_trailing_newline_is_not_a_record() {
        let (_tmp, m) = monitor_over(Some("{\"ts\":1}\n"), TraceFormat::Jsonl);
        assert_eq!(Extent::read(&m).expect("reads").records(), 1);
    }

    // Verifies: #231 — how often each DECLARED event occurs, keyed by the alias the operator
    // declared it under, so the dry-run can warn that a policy is about to be vacuously satisfied.
    // An event that never fires is 0, not absent: "never happened" is an answer.
    #[test]
    fn occurrences_counts_declared_events_and_reports_zero_as_zero() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let trace = tmp.path().join("events.log");
        std::fs::write(
            &trace,
            "{\"ts\":\"1\",\"event\":\"msg_accepted\"}\n\
             {\"ts\":\"2\",\"event\":\"msg_accepted\"}\n\
             {\"ts\":\"3\",\"event\":\"something_else\"}\n",
        )
        .expect("trace");
        let events = BTreeMap::from([
            (
                "accepted".to_string(),
                super::super::declaration::Event {
                    name: "msg_accepted".into(),
                    args: vec!["id".into()],
                },
            ),
            (
                "swept".to_string(),
                super::super::declaration::Event {
                    name: "msg_swept".into(),
                    args: vec![],
                },
            ),
        ]);
        let m = Monitor::new(trace, TraceFormat::Jsonl, "ts", "event", events.clone());
        let counts = occurrences(&m).expect("readable");
        assert_eq!(counts["accepted"], 2);
        assert_eq!(counts["swept"], 0, "never fired is zero, not absent");

        // MonPoly's own log: one line can name several predicates, and each is an occurrence.
        let mono = tmp.path().join("trace.log");
        std::fs::write(
            &mono,
            "@100 msg_accepted (1) msg_swept (2)\n@140 msg_accepted(3)\n",
        )
        .expect("trace");
        let m = Monitor::new(mono, TraceFormat::Monpoly, "", "", events);
        let counts = occurrences(&m).expect("readable");
        assert_eq!(counts["accepted"], 2);
        assert_eq!(counts["swept"], 1);
    }

    // Verifies: #231 — a trace that cannot be read at all is `None`, never a map of zeroes. The
    // dry-run must be able to say "unknown" rather than assert that nothing ever happened.
    #[test]
    fn occurrences_over_an_unreadable_trace_is_unknown_not_zero() {
        let (_tmp, m) = monitor_over(None, TraceFormat::Jsonl);
        assert_eq!(occurrences(&m), None);
    }

    // Verifies: #230 — the trace is a drift axis. A log that grew since the verdict was recorded
    // fingerprints differently, so the verdict cannot read `fresh` while its evidence moves.
    #[test]
    fn a_trace_that_moved_fingerprints_differently() {
        let (tmp, m) = monitor_over(Some(TWO_EVENTS), TraceFormat::Jsonl);
        let before = Extent::read(&m).expect("reads").fingerprint().to_string();
        assert_eq!(
            before,
            Extent::read(&m).expect("reads").fingerprint(),
            "the same trace must fingerprint the same, or every verdict is instantly stale"
        );

        std::fs::write(
            tmp.path().join("events.log"),
            format!("{TWO_EVENTS}{{\"ts\":\"2026-08-07T00:00:00Z\"}}\n"),
        )
        .expect("append");
        assert_ne!(before, Extent::read(&m).expect("reads").fingerprint());
    }

    // Verifies: #230 — the staleness check can ask for the fingerprint without reading a trace it
    // may not have. No monitor, or no trace, is "this axis does not flag anything" — the loud
    // failure belongs at verification time, not to a background freshness read.
    #[test]
    fn the_drift_axis_is_absent_rather_than_loud_when_there_is_nothing_to_compare() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let companion = tmp.path().join("ProvableRequirements");
        std::fs::create_dir_all(&companion).expect("companion");
        assert_eq!(current_fingerprint(tmp.path(), &companion), None);

        std::fs::write(
            companion.join(crate::adopt::MANIFEST_FILE),
            "monitor:\n  trace: logs/events.jsonl\n  format: jsonl\n  time_field: ts\n  \
             event_field: event\n",
        )
        .expect("manifest");
        assert_eq!(
            current_fingerprint(tmp.path(), &companion),
            None,
            "a declared but absent trace has no fingerprint to compare"
        );

        std::fs::create_dir_all(tmp.path().join("logs")).expect("logs");
        std::fs::write(tmp.path().join("logs/events.jsonl"), TWO_EVENTS).expect("trace");
        assert!(current_fingerprint(tmp.path(), &companion).is_some());
    }
}
