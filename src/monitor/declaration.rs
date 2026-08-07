//! Reading the `monitor:` block out of the companion manifest.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The trace formats provreq will read (#230). Two, because both are honest for a subject that
/// already exists: JSONL is what most services log, and MonPoly's own log syntax is zero-conversion
/// for a subject already producing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceFormat {
    /// One JSON object per line. `time_field` names the key carrying the timestamp.
    Jsonl,
    /// MonPoly's own log: `@<timestamp> pred(args) pred(args)`, one time point per line. The
    /// timestamp is the `@` prefix, so no `time_field` applies.
    Monpoly,
}

impl TraceFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            TraceFormat::Jsonl => "jsonl",
            TraceFormat::Monpoly => "monpoly",
        }
    }

    /// What one line of this format is. Named per format rather than flattened to a common word:
    /// a MonPoly line is a *time point* that may carry several predicates, and calling that "an
    /// event" would overstate what was counted.
    pub fn record_noun(&self) -> &'static str {
        match self {
            TraceFormat::Jsonl => "events",
            TraceFormat::Monpoly => "time points",
        }
    }

    fn parse(raw: &str) -> Option<TraceFormat> {
        match raw.trim() {
            "jsonl" => Some(TraceFormat::Jsonl),
            "monpoly" => Some(TraceFormat::Monpoly),
            _ => None,
        }
    }
}

/// One event the requirement's vocabulary can refer to, as the operator declared it: the name it
/// goes by in the trace and the arguments it carries. Resolving a requirement's terms against these
/// is #231's job; this module only reads the declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub name: String,
    pub args: Vec<String>,
}

/// A subject's declared monitor input. Absent for every subject that has not configured one, which
/// is every subject that worked before this existed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    /// The trace as an absolute path — where provreq will actually look.
    trace: PathBuf,
    /// The path exactly as the operator wrote it, kept for messages: an error about
    /// `logs/events.jsonl` is one they can find in their manifest.
    declared: String,
    format: TraceFormat,
    /// The JSON key carrying the timestamp. Empty for [`TraceFormat::Monpoly`], where the
    /// timestamp is the line's `@` prefix.
    time_field: String,
    /// The JSON key carrying the event's name. Empty for [`TraceFormat::Monpoly`], where the name
    /// is the predicate token after the timestamp.
    event_field: String,
    events: BTreeMap<String, Event>,
}

#[derive(serde::Deserialize)]
struct ManifestMonitor {
    #[serde(default)]
    monitor: Option<MonitorBlock>,
}

#[derive(serde::Deserialize)]
struct MonitorBlock {
    #[serde(default)]
    trace: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    time_field: Option<String>,
    #[serde(default)]
    event_field: Option<String>,
    #[serde(default)]
    events: BTreeMap<String, EventBlock>,
}

#[derive(serde::Deserialize)]
struct EventBlock {
    #[serde(default)]
    name: String,
    #[serde(default)]
    args: Vec<String>,
}

impl Monitor {
    /// Read `monitor:` from the companion `provreq.yml`.
    ///
    /// `Ok(None)` — no monitor configured — for a missing file, a missing block, or a manifest that
    /// will not parse, the same forgiving read as [`crate::spec_paths::SpecPaths::load`] and
    /// [`crate::tlc::Constants::load`]: a subject that never configured this must not be broken by
    /// the field existing.
    ///
    /// `Err` once the block **is** there and says something provreq cannot act on. That is the same
    /// split [`crate::tlc::Constants::load`] draws: silence is a choice not to configure, but a
    /// half-written declaration is a mistake the operator is looking straight at, and dropping it
    /// would strand them debugging a monitor that quietly never ran.
    pub fn load(subject_root: &Path, companion_root: &Path) -> Result<Option<Monitor>, String> {
        let Ok(text) = std::fs::read_to_string(companion_root.join(crate::adopt::MANIFEST_FILE))
        else {
            return Ok(None);
        };
        let Ok(manifest) = serde_yaml::from_str::<ManifestMonitor>(&text) else {
            return Ok(None);
        };
        let Some(block) = manifest.monitor else {
            return Ok(None);
        };
        Monitor::from_block(subject_root, block).map(Some)
    }

    fn from_block(subject_root: &Path, block: MonitorBlock) -> Result<Monitor, String> {
        let declared = block.trace.trim().to_string();
        if declared.is_empty() {
            return Err(
                "`monitor.trace` in provreq.yml is empty — name the log the subject \
                        already produces, as a path relative to the subject (for example \
                        `logs/events.jsonl`)"
                    .into(),
            );
        }
        let format = TraceFormat::parse(&block.format).ok_or_else(|| {
            format!(
                "`monitor.format` in provreq.yml is `{}`, which provreq cannot read — write \
                 `jsonl` (one JSON object per line) or `monpoly` (MonPoly's own \
                 `@<timestamp> pred(args)` log)",
                block.format.trim()
            )
        })?;
        let time_field = jsonl_key(&TIME_FIELD, format, block.time_field.as_deref())?;
        let event_field = jsonl_key(&EVENT_FIELD, format, block.event_field.as_deref())?;
        let events = block
            .events
            .into_iter()
            .map(|(alias, e)| declared_event(&alias, e).map(|event| (alias, event)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;

        Ok(Monitor {
            trace: resolve(subject_root, &declared),
            declared,
            format,
            time_field,
            event_field,
            events,
        })
    }

    /// Build from already-resolved parts — for tests, and any future caller that is not reading a
    /// manifest. Mirrors [`crate::spec_paths::SpecPaths::from_roots`].
    pub fn new(
        trace: PathBuf,
        format: TraceFormat,
        time_field: impl Into<String>,
        event_field: impl Into<String>,
        events: BTreeMap<String, Event>,
    ) -> Monitor {
        Monitor {
            declared: trace.to_string_lossy().into_owned(),
            trace,
            format,
            time_field: time_field.into(),
            event_field: event_field.into(),
            events,
        }
    }

    pub fn trace(&self) -> &Path {
        &self.trace
    }

    /// The trace path as the operator wrote it — what an error message should say, because it is
    /// what they will search their manifest for.
    pub fn declared(&self) -> &str {
        &self.declared
    }

    pub fn format(&self) -> TraceFormat {
        self.format
    }

    pub fn time_field(&self) -> &str {
        &self.time_field
    }

    pub fn event_field(&self) -> &str {
        &self.event_field
    }

    /// The event an operator's binding names, looked up by the **alias** they declared it under.
    ///
    /// The alias is provreq's handle for the event; the `name` inside is the trace's own spelling,
    /// which the adapter needs when it writes MonPoly's signature (#232/#233) and the operator
    /// should not have to repeat. This mirrors category 2a, where a binding names the TLA+
    /// definition it resolves to.
    pub fn event(&self, alias: &str) -> Option<&Event> {
        self.events.get(alias)
    }

    /// Every declared alias, in declaration order — what an unresolvable binding is told it could
    /// have named. A "not declared" that does not say what *is* declared makes the operator go
    /// read the manifest to find out what they typo'd.
    pub fn aliases(&self) -> Vec<String> {
        self.events.keys().cloned().collect()
    }

    pub fn events(&self) -> &BTreeMap<String, Event> {
        &self.events
    }
}

/// One JSON key a `jsonl` trace needs the operator to name, and where a `monpoly` trace carries
/// the same thing in its own syntax instead.
struct JsonlKey {
    key: &'static str,
    needs: &'static str,
    example: &'static str,
    monpoly_carries: &'static str,
}

const TIME_FIELD: JsonlKey = JsonlKey {
    key: "time_field",
    needs: "each record's timestamp",
    example: "time_field: ts",
    monpoly_carries: "each line's `@` prefix",
};

const EVENT_FIELD: JsonlKey = JsonlKey {
    key: "event_field",
    needs: "each record's event name",
    example: "event_field: event",
    monpoly_carries: "the predicate name after each line's timestamp",
};

/// A jsonl key, checked against the format rather than defaulted.
///
/// A guessed key would not fail loudly — every record would parse as untimed or unnamed, and the
/// trace would read as empty or as containing none of the declared events. Both are the reading
/// #230 exists to prevent. And a key set under `monpoly` is a line of manifest the operator
/// believes is doing something; saying so beats ignoring it.
fn jsonl_key(
    spec: &JsonlKey,
    format: TraceFormat,
    configured: Option<&str>,
) -> Result<String, String> {
    let JsonlKey {
        key,
        needs,
        example,
        monpoly_carries,
    } = spec;
    let configured = configured.map(str::trim).filter(|f| !f.is_empty());
    match (format, configured) {
        (TraceFormat::Jsonl, Some(field)) => Ok(field.to_string()),
        (TraceFormat::Jsonl, None) => Err(format!(
            "`monitor.{key}` in provreq.yml is missing — a jsonl trace needs the JSON key carrying \
             {needs} (for example `{example}`)"
        )),
        (TraceFormat::Monpoly, Some(field)) => Err(format!(
            "`monitor.{key}: {field}` in provreq.yml does nothing for a `monpoly` trace — MonPoly's \
             log carries that in {monpoly_carries}. Remove it, or switch `monitor.format` to \
             `jsonl` if the trace really is JSON"
        )),
        (TraceFormat::Monpoly, None) => Ok(String::new()),
    }
}

fn declared_event(alias: &str, block: EventBlock) -> Result<Event, String> {
    let name = block.name.trim().to_string();
    if name.is_empty() {
        return Err(format!(
            "`monitor.events.{alias}` in provreq.yml has no `name` — name the event as it appears \
             in the trace (for example `{alias}: {{ name: msg_{alias}, args: [id] }}`)"
        ));
    }
    Ok(Event {
        name,
        args: block.args.iter().map(|a| a.trim().to_string()).collect(),
    })
}

/// The declared trace as an absolute path. Relative resolves against the **subject** root, not the
/// companion tree, for the reason [`crate::spec_paths`] already documents: the operator is
/// describing where their artifact lives relative to the thing being verified.
///
/// A trace that does not exist yet cannot be canonicalized — the joined path is kept as-is, so the
/// operator sees the path they configured rather than nothing at all. Whether it exists is
/// [`crate::monitor::Extent::read`]'s question, and it answers it out loud.
fn resolve(subject_root: &Path, declared: &str) -> PathBuf {
    let joined = subject_root.join(declared);
    std::fs::canonicalize(&joined).unwrap_or(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject_with_manifest(manifest: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        let companion = tmp.path().join("ProvableRequirements");
        std::fs::create_dir_all(&companion).expect("companion");
        std::fs::write(companion.join(crate::adopt::MANIFEST_FILE), manifest).expect("manifest");
        tmp
    }

    fn load(tmp: &tempfile::TempDir) -> Result<Option<Monitor>, String> {
        Monitor::load(tmp.path(), &tmp.path().join("ProvableRequirements"))
    }

    const JSONL: &str =
        "monitor:\n  trace: logs/events.jsonl\n  format: jsonl\n  time_field: ts\n  event_field: event\n";

    // Verifies: #230 — the declaration loads, and the trace resolves against the SUBJECT, so the
    // operator describes where their log lives relative to the thing being verified.
    #[test]
    fn a_declared_trace_resolves_against_the_subject() {
        let tmp = subject_with_manifest(JSONL);
        std::fs::create_dir_all(tmp.path().join("logs")).expect("logs");
        std::fs::write(tmp.path().join("logs/events.jsonl"), "").expect("trace");

        let m = load(&tmp).expect("loads").expect("configured");
        assert_eq!(
            m.trace(),
            std::fs::canonicalize(tmp.path().join("logs/events.jsonl")).expect("canonical")
        );
        assert_eq!(m.declared(), "logs/events.jsonl");
        assert_eq!(m.format(), TraceFormat::Jsonl);
        assert_eq!(m.time_field(), "ts");
    }

    // Verifies: #230 — a subject that never configured a monitor is not broken by the field
    // existing. The same forgiving read as spec_paths and tla.constants.
    #[test]
    fn an_unconfigured_subject_has_no_monitor() {
        for manifest in ["kani:\n  default_unwind: 3\n", "", "monitor:\n"] {
            assert_eq!(
                load(&subject_with_manifest(manifest)).expect("forgiving"),
                None,
                "manifest: {manifest:?}"
            );
        }
        // No manifest at all — an unadopted or half-set-up subject reads as unconfigured, never as
        // an error about a file it has no reason to have.
        let bare = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            Monitor::load(bare.path(), bare.path()).expect("forgiving"),
            None
        );
    }

    // Verifies: #230 — a manifest that will not parse means no monitor, never a panic: a subject
    // whose provreq.yml has an unrelated problem must still work everywhere else.
    #[test]
    fn an_unparseable_manifest_means_no_monitor() {
        assert_eq!(
            load(&subject_with_manifest("monitor: [this is not a map\n")).expect("forgiving"),
            None
        );
    }

    // Verifies: #230 — the block being PRESENT changes the rules. Silence is a choice not to
    // configure; a half-written declaration is a mistake in front of the operator, and each error
    // says what to write rather than that something is invalid.
    #[test]
    fn a_half_written_declaration_says_what_to_write() {
        let cases = [
            (
                "monitor:\n  format: jsonl\n  time_field: ts\n",
                "monitor.trace",
            ),
            (
                "monitor:\n  trace: logs/e.jsonl\n  format: csv\n",
                "monitor.format",
            ),
            (
                "monitor:\n  trace: logs/e.jsonl\n  format: jsonl\n",
                "monitor.time_field",
            ),
        ];
        for (manifest, names) in cases {
            let err = load(&subject_with_manifest(manifest)).expect_err("must not be forgiven");
            assert!(err.contains(names), "{err}");
        }
    }

    // Verifies: #230 — an unreadable format names both formats provreq accepts, so the fix is in
    // the message rather than in the source.
    #[test]
    fn an_unknown_format_names_the_ones_that_work() {
        let err = load(&subject_with_manifest(
            "monitor:\n  trace: logs/e.log\n  format: csv\n",
        ))
        .expect_err("csv is not a trace format");
        assert!(err.contains("jsonl") && err.contains("monpoly"), "{err}");
    }

    // Verifies: #230 — MonPoly's log carries its timestamps in the `@` prefix, so no time_field
    // applies. Accepting one silently would leave the operator believing a line of their manifest
    // was doing something.
    #[test]
    fn monpoly_needs_no_time_field_and_refuses_one() {
        let m = load(&subject_with_manifest(
            "monitor:\n  trace: logs/trace.log\n  format: monpoly\n",
        ))
        .expect("loads")
        .expect("configured");
        assert_eq!(m.format(), TraceFormat::Monpoly);
        assert_eq!(m.time_field(), "");

        let err = load(&subject_with_manifest(
            "monitor:\n  trace: logs/trace.log\n  format: monpoly\n  time_field: ts\n",
        ))
        .expect_err("a time_field does nothing here");
        assert!(err.contains("`@` prefix"), "{err}");

        // Same rule for the event key: MonPoly names the predicate itself, so declaring a JSON key
        // for it is a line of manifest that would quietly do nothing.
        let err = load(&subject_with_manifest(
            "monitor:\n  trace: logs/trace.log\n  format: monpoly\n  event_field: event\n",
        ))
        .expect_err("an event_field does nothing here either");
        assert!(err.contains("predicate name"), "{err}");
    }

    // Verifies: #230 — the declared events load with their names and arguments, which is what a
    // runtime binding will resolve against (#231). An event with no name is refused: it would
    // resolve to nothing while looking configured.
    #[test]
    fn declared_events_carry_their_name_and_arguments() {
        let tmp = subject_with_manifest(
            "monitor:\n  trace: logs/e.jsonl\n  format: jsonl\n  time_field: ts\n  event_field: event\n  events:\n    \
             accepted: { name: msg_accepted, args: [id] }\n    succeeded: { name: msg_done, args: \
             [id, worker] }\n",
        );
        let m = load(&tmp).expect("loads").expect("configured");
        assert_eq!(
            m.events()["accepted"],
            Event {
                name: "msg_accepted".into(),
                args: vec!["id".into()]
            }
        );
        assert_eq!(m.events()["succeeded"].args, vec!["id", "worker"]);

        let err = load(&subject_with_manifest(
            "monitor:\n  trace: logs/e.jsonl\n  format: jsonl\n  time_field: ts\n  event_field: event\n  events:\n    \
             accepted: { args: [id] }\n",
        ))
        .expect_err("an unnamed event resolves to nothing");
        assert!(err.contains("monitor.events.accepted"), "{err}");
    }

    // Verifies: #230 — a trace outside the subject tree is a legitimate layout (a log shipped to a
    // sibling directory), and it resolves without the `..` still in it.
    #[test]
    fn a_sibling_trace_resolves_to_a_clean_absolute_path() {
        let parent = tempfile::tempdir().expect("tempdir");
        let subject = parent.path().join("subject");
        let companion = subject.join("ProvableRequirements");
        std::fs::create_dir_all(&companion).expect("companion");
        let logs = parent.path().join("logs");
        std::fs::create_dir_all(&logs).expect("logs");
        std::fs::write(logs.join("events.jsonl"), "").expect("trace");
        std::fs::write(
            companion.join(crate::adopt::MANIFEST_FILE),
            "monitor:\n  trace: ../logs/events.jsonl\n  format: jsonl\n  time_field: ts\n  event_field: event\n",
        )
        .expect("manifest");

        let m = Monitor::load(&subject, &companion)
            .expect("loads")
            .expect("configured");
        assert!(!m.trace().to_string_lossy().contains(".."), "{m:?}");
        assert!(m.trace().ends_with("logs/events.jsonl"), "{m:?}");
        // The message still says what the operator wrote, not the resolved path.
        assert_eq!(m.declared(), "../logs/events.jsonl");
    }

    // Verifies: #230 — a trace that does not exist yet keeps the configured path rather than
    // vanishing, so the failure can name it. Whether it exists is the reader's question.
    #[test]
    fn a_missing_trace_keeps_the_configured_path() {
        let m = load(&subject_with_manifest(JSONL))
            .expect("loads")
            .expect("configured");
        assert!(m.trace().ends_with("logs/events.jsonl"), "{m:?}");
    }
}
