//! R-eng-2/3 — engine coverage. Reports which verification engines are installed and
//! version-compatible, and which formalized requirements are therefore checkable —
//! **without ever installing anything** (R-eng-2, detect presence *and* compatibility)
//! and without running any engine. Coverage is gated by installed+compatible engines and
//! reported first-class (R-eng-3), keeping *formalizable-but-no-engine* distinct from
//! *not formalized*.
//!
//! The R-eng-1 split: category 1 (code) is **toolchain-welded** — its engine needs the
//! subject's own compiler (R-eng-4), so it is deployed into the dev env rather than fed a
//! portable artifact. Categories 2a/2b/3 are **artifact-fed** portable engines (TLC,
//! MonPoly, a WebDriver grid). Every category is wired as of #245, and with the last one the
//! "detected the same way" part stopped being true: a grid is reached at an address, not
//! found on `PATH`, so [`Probe`] carries both kinds.
//!
//! "Toolchain-welded" classifies *how an engine is deployed*, never *whether it is
//! present*: R-eng-2 requires welded engines to be provisioned into the dev env and
//! detected like any other. Reading the class as readiness is what REQ024 fixed — see
//! [`EngineStatus::is_ready`].
//!
//! **`ready` now means what it says.** It previously meant only "the engine binary is
//! present", because no engine was wired and so no `Available` engine could actually back a
//! verdict. REQ027 closed that gap from both ends: category 1 gained a real engine (Kani),
//! and 2a/2b/3 lost the probes that would have reported a readiness nothing could honor. An
//! engine is probed only if provreq can run it, so `Available` ⇒ a verdict is really
//! obtainable.
//!
//! Implements: REQ022 (engine coverage — detect installed engines, report readiness),
//! REQ024 (a category-1 engine that is not wired never reports ready), REQ027 (category 1
//! is wired to Kani; only a runnable engine is probed), REQ030 (a category routes to an
//! ensemble via [`engines_for`]; it is ready as soon as any one engine is).

use crate::grounding::BindCategory;
use std::process::Command;

/// An engine's presence probe: the command to run (`bin` + `args`) that makes it print its
/// version, an optional marker the output must contain to count as present, and an optional
/// minimum version. Version thresholds are presence-only for now (`None`) — the compatibility
/// machinery is real and tested, but no minimums are shipped until a real engine is on hand to
/// calibrate against.
///
/// `version_marker` is what keeps a *host* being present from masquerading as the *engine*: TLC
/// runs as `java -cp <jar> tlc2.TLC`, so `java` spawning successfully is not evidence TLC is
/// there — only the marker (`TLC2 Version`) in the output is. `None` (Kani: `cargo-kani
/// --version` only runs at all if `cargo-kani` exists) means any successful run counts.
///
/// `// ponytail: probe args are best-effort (TLC has no clean --version — its banner is the
/// version); move bins/args/min-versions to provreq.yml config when a real subject needs it.`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineProbe {
    pub bin: String,
    pub args: Vec<String>,
    pub version_marker: Option<String>,
    pub min_version: Option<String>,
}

/// How an engine's presence is established. Every engine here was a `PATH` lookup until category 3
/// arrived, and a grid is not a file: the honest check is `GET <endpoint>/status`, which answers
/// "**this can seat a session right now**" rather than "a file with this name exists". That is a
/// stronger signal, not a weaker substitute for one (#245).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// Run a command and read its version output.
    Command(EngineProbe),
    /// Ask a W3C WebDriver grid whether it is ready. Where the grid *is* comes from the operator's
    /// environment (`WEBDRIVER_URL`) or the subject's `ui.endpoint`, so this carries no address.
    Grid,
}

impl Probe {
    /// The command behind a `PATH`-probed engine, for callers that reason about binaries.
    pub fn command(&self) -> Option<&EngineProbe> {
        match self {
            Probe::Command(probe) => Some(probe),
            Probe::Grid => None,
        }
    }
}

/// A verification engine a PRL category routes to. `probe` is `Some` exactly when provreq
/// has an integration that can run the engine — so `None` means "not wired", which is ours
/// to fix, and a failed probe means "not installed", which is the operator's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Engine {
    pub category: BindCategory,
    pub name: &'static str,
    pub probe: Option<Probe>,
}

/// The detected state of an engine (R-eng-2: presence *and* compatibility).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineStatus {
    /// No integration exists for this engine yet — provreq cannot run it whatever the
    /// operator installs. Distinct from [`EngineStatus::Missing`], which is the operator's
    /// to fix by installing a binary; this one is ours to fix by wiring the engine.
    NotWired,
    /// On `PATH` and (if a minimum is set) new enough. `version` is best-effort —
    /// `"unknown"` when the probe ran but printed nothing parseable.
    Available { version: String },
    /// Not on `PATH`.
    Missing,
    /// On `PATH` but the process never got as far as being the engine — the loader could not
    /// start it, or it is not executable. Deliberately **not** [`EngineStatus::Missing`]:
    /// installing it again is the wrong advice for a binary that is already there, and the
    /// operator has to repair the environment instead (REQ051).
    Unusable { reason: String },
    /// Present but older than the required minimum.
    Incompatible { found: String, required: String },
}

/// Make `EngineStatus` a sort Kani can quantify over. A cat-1 claim that ranges over an
/// `EngineState` variable lowers to `let d: EngineStatus = kani::any();`, and Kani needs a real
/// value per quantified variable — without this impl the harness does not compile and the verdict
/// is an `unknown` carrying a trait-bound error rather than an answer (#148).
///
/// Costs nothing in an ordinary build: the `kani` crate exists only under `cfg(kani)`, which only
/// `cargo kani` sets.
///
/// `// ponytail: the String payloads are FIXED, not symbolic — this quantifies over the five
/// discriminants only. Sound for every claim that reads the discriminant (`is_ready`, and so
/// `decide_install`, which is all of them today), and an UNDER-approximation for any future claim
/// that inspects a version or reason string: such a claim would be proved only for the empty
/// string. Make the payloads symbolic (bounded `[u8; N]` → `String`) if one ever does.`
#[cfg(kani)]
impl kani::Arbitrary for EngineStatus {
    fn any() -> Self {
        match kani::any::<u8>() % 5 {
            0 => EngineStatus::NotWired,
            1 => EngineStatus::Available {
                version: String::new(),
            },
            2 => EngineStatus::Missing,
            3 => EngineStatus::Unusable {
                reason: String::new(),
            },
            _ => EngineStatus::Incompatible {
                found: String::new(),
                required: String::new(),
            },
        }
    }
}

impl EngineStatus {
    /// Whether an engine in this state can back a verdict (R-eng-3 gate).
    ///
    /// `NotWired` is **not** ready. Before REQ024 the category-1 engine reported
    /// `ToolchainWelded` → ready unconditionally, on the reasoning that "the operator runs
    /// provreq in the subject's own build env". That conflated *having a build toolchain*
    /// with *having a verifier*: `cargo build` cannot discharge a pre/post obligation, and
    /// the category-1 engine is a deductive verifier (Viper lineage). The result was that
    /// `provreq engines` reported every category-1 requirement engine-ready when no
    /// verifier existed at all. R-eng-1's "toolchain-welded" is a statement about the
    /// engine's *class* (a Rust verifier needs the subject's `rustc`, so it cannot be a
    /// shared portable binary) — R-eng-2 still requires it to be provisioned and detected.
    pub fn is_ready(&self) -> bool {
        matches!(self, EngineStatus::Available { .. })
    }

    pub fn describe(&self) -> String {
        match self {
            EngineStatus::NotWired => "NOT WIRED (no integration yet)".to_string(),
            EngineStatus::Available { version } => format!("available ({version})"),
            EngineStatus::Missing => "MISSING".to_string(),
            EngineStatus::Unusable { reason } => {
                format!("PRESENT BUT UNUSABLE ({reason})")
            }
            EngineStatus::Incompatible { found, required } => {
                format!("INCOMPATIBLE (found {found}, needs >= {required})")
            }
        }
    }

    /// A stable kebab-case tag for the wire (`GET /api/engines`), so a UI can tone a status
    /// without parsing [`EngineStatus::describe`].
    pub fn tag(&self) -> &'static str {
        match self {
            EngineStatus::NotWired => "not-wired",
            EngineStatus::Available { .. } => "available",
            EngineStatus::Missing => "missing",
            EngineStatus::Unusable { .. } => "unusable",
            EngineStatus::Incompatible { .. } => "incompatible",
        }
    }
}

/// The category→engine registry. Most categories route to one engine, but category 1 is an
/// **ensemble** (D2b): Kani (bounded model-checking) and Creusot (deductive proof) both run,
/// their evidence aggregated — so this is a `Vec`, not a 1:1 map.
///
/// **A probe exists only for an engine provreq can actually run.** That is the whole
/// meaning of `probe: Option` — not "we know the binary's name", but "there is an
/// integration behind it". Detecting a binary we cannot drive would report a readiness we
/// cannot honor: the operator installs the tool, `engines` turns green, and `verify` still
/// answers `no-engine`. REQ024 fixed exactly that overclaim for category 1; REQ027 keeps
/// 2a/2b/3 honest by the same rule, and each gets its probe when its lowering is wired.
///
/// Routing follows the settled design (docs/requirement-language.md): 2a model checking is
/// the TLA+ lineage (TLC), 2b runtime monitoring is MonPoly (MFOTL), 3 UI is a
/// Selenium/Playwright driver.
pub fn registry() -> Vec<Engine> {
    vec![
        Engine {
            // Category 1 is the temporal-free fragment (pre/post/invariants), and its engine
            // is Kani — #1, first and not only (D2b wants a per-language ensemble). It is
            // toolchain-welded (R-eng-1/R-eng-4): it needs the subject's own compiler, so it
            // is not a portable artifact-fed binary. That classifies how it is DEPLOYED, not
            // whether it is present — R-eng-2 requires it to be provisioned into the dev env
            // and detected like any other, which is what this probe does. `cargo-kani` is the
            // binary `cargo kani` needs on PATH, so it is the one worth probing.
            category: BindCategory::Code,
            name: "Kani",
            probe: Some(Probe::Command(EngineProbe {
                bin: "cargo-kani".to_string(),
                args: vec!["--version".to_string()],
                version_marker: None,
                min_version: None,
            })),
        },
        Engine {
            // Category 1 is an ENSEMBLE (D2b), not a single engine: Creusot joins Kani as the
            // #2 member — REQ031. It is a **deductive** verifier, so it earns `proven` (∀
            // executions) where Kani earns bounded `model-checked`; `aggregate` reports the
            // stronger rung when both hold ("proven by Creusot, corroborated bounded by
            // Kani"). Toolchain-welded like Kani (R-eng-4). `cargo-creusot` is the binary
            // `cargo creusot` needs on PATH.
            //
            // The args are `creusot version`, and the leading `creusot` is load-bearing.
            // `cargo-creusot` parses with `parse_from(args().skip(1))` because cargo invokes it
            // as `cargo-creusot creusot …` — `skip(1)` drops the binary name and clap then
            // consumes the *next* word as argv[0]. Given `cargo-creusot --version` that leaves
            // NO arguments at all, which selects the default subcommand: a full compile-and-
            // prove of the subject. That is how a status probe came to build the operator's
            // crate and report the *subject's* version (`Checking provreq v0.0.1`) as Creusot's.
            // `creusot version` reaches the real subcommand, prints `cargo-creusot <version>`,
            // exits 0, and touches no subject.
            //
            // The marker is what keeps that honest: without it `parse_version` returns the first
            // version-shaped token from *any* line of whatever the command emitted, which is
            // exactly how an unrelated build log became an engine version.
            category: BindCategory::Code,
            name: "Creusot",
            probe: Some(Probe::Command(EngineProbe {
                bin: "cargo-creusot".to_string(),
                args: vec!["creusot".to_string(), "version".to_string()],
                version_marker: Some("cargo-creusot".to_string()),
                min_version: None,
            })),
        },
        Engine {
            // Category 1's THIRD ensemble member (D2b) — REQ032. Prusti is the second
            // **deductive** verifier (Viper backend, distinct from Creusot's Why3/SMT), so it too
            // earns `proven` (∀ executions); `aggregate` reports the stronger rung when it and a
            // bounded engine both hold. Toolchain-welded like Kani/Creusot (R-eng-4). The binary
            // `cargo prusti` needs on PATH is `cargo-prusti`; unlike Creusot it rejects
            // `--version`, but `--help` exits 0 anywhere — which, since the launcher is
            // `prefer-dynamic`, also confirms its runtime libraries load (the image's ldconfig
            // fix), making it the honest readiness signal.
            category: BindCategory::Code,
            name: "Prusti",
            probe: Some(Probe::Command(EngineProbe {
                bin: "cargo-prusti".to_string(),
                args: vec!["--help".to_string()],
                version_marker: None,
                min_version: None,
            })),
        },
        Engine {
            // Category 2a is the model world: the temporal properties (safety AND liveness)
            // checked against a TLA+ model. Its engine is TLC — REQ029, the model-world analog
            // of wiring Kani for category 1. TLC is not a PATH binary; it runs as
            // `java -cp <jar> tlc2.TLC`, so the probe is java with the jar on the classpath and
            // the marker guards against java-present-but-jar-absent.
            category: BindCategory::Model,
            name: "TLC (TLA+)",
            probe: Some(Probe::Command(EngineProbe {
                bin: "java".to_string(),
                args: vec![
                    "-cp".to_string(),
                    crate::tlc::jar_path(),
                    "tlc2.TLC".to_string(),
                ],
                version_marker: Some("TLC2 Version".to_string()),
                min_version: None,
            })),
        },
        Engine {
            category: BindCategory::Runtime,
            name: "MonPoly",
            // #233 — MonPoly is a portable binary, so it probes like Kani: a `PATH` lookup with a
            // version marker. `-version` prints `MonPoly (development build)` for a source build,
            // which carries no version number — the marker is what proves it is really MonPoly, and
            // an absent version is reported as such rather than invented.
            probe: Some(Probe::Command(EngineProbe {
                bin: crate::monitor::monpoly_bin(),
                args: vec!["-version".to_string()],
                version_marker: Some("MonPoly".to_string()),
                min_version: None,
            })),
        },
        Engine {
            // #245 — the last unwired category. The registry said `Selenium/Playwright driver`
            // while nothing was wired, because nothing had had to choose. Driving one settled it:
            // what provreq speaks is **W3C WebDriver**, and what answers here is **Selenium, as a
            // grid service** rather than a binary on `PATH` — which is why this probe cannot be an
            // `EngineProbe`. A name with a slash in it described an undecided question; this one
            // describes what ran.
            category: BindCategory::Ui,
            name: "Selenium (WebDriver)",
            probe: Some(Probe::Grid),
        },
    ]
}

/// The engines that run a given category — an **ensemble** (D2b), so this returns every
/// engine registered for it. One per category today (Kani, TLC); the deductive verifiers
/// join category 1 as further members without any caller here changing shape.
///
/// Implements: REQ030
pub fn engines_for(category: BindCategory) -> Vec<Engine> {
    registry()
        .into_iter()
        .filter(|e| e.category == category)
        .collect()
}

/// Detect an engine's status (R-eng-2). An engine with no probe has no integration yet and
/// reports [`EngineStatus::NotWired`]; portable engines are looked up on `PATH` and
/// version-checked. Never installs.
///
/// `companion` is where a subject's own `ui.endpoint` is read from, for the one engine that is
/// reached at an address rather than found on `PATH` (#245). `None` is honest — `provreq engines`
/// on an unadopted directory has no manifest to consult — and only narrows the grid lookup to
/// `WEBDRIVER_URL`. Every other engine ignores it.
pub fn detect(engine: &Engine, companion: Option<&std::path::Path>) -> EngineStatus {
    match &engine.probe {
        None => EngineStatus::NotWired,
        Some(Probe::Command(probe)) => detect_probe(probe),
        Some(Probe::Grid) => crate::ui::detect_grid(companion),
    }
}

fn detect_probe(probe: &EngineProbe) -> EngineStatus {
    // `Command::new(bare_name)` searches `PATH`; a not-found binary errors here, which is
    // exactly the honest "engine missing" signal.
    let output = match Command::new(&probe.bin).args(&probe.args).output() {
        Ok(o) => o,
        Err(_) => return EngineStatus::Missing,
    };
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // REQ051 — the exit status is evidence, but only this much of it. A *blanket* non-zero ⇒
    // absent rule would be a second wrong answer: measured here, `cargo-creusot --version`
    // exits 1 complaining about the current directory and TLC exits 1 asking for an input
    // module, and both engines are fine. What is unambiguous is a process that never got as
    // far as running: 126 (found, not executable) and 127 (the loader could not start it — a
    // missing shared library, which is how a broken `cargo-prusti` fails). Those carry no
    // information about the engine except that it cannot be one.
    if matches!(output.status.code(), Some(126 | 127)) {
        return EngineStatus::Unusable {
            reason: first_line(&combined),
        };
    }
    // A marker that is set but absent means the host ran (e.g. `java`) but the engine is not
    // actually reachable (e.g. the jar is missing) — that is Missing, not Available.
    //
    // A marker that IS present additionally scopes the version scan to the line carrying it
    // (#159). `parse_version` returns the first version-shaped token it meets, and probe output
    // is routinely prefixed with noise a build emitted, so scanning the whole text lets a
    // version belonging to something else be reported as the engine's. A version read off a line
    // that does not name the engine is a version of some other thing.
    let scan = match &probe.version_marker {
        Some(marker) => match combined.lines().find(|l| l.contains(marker.as_str())) {
            Some(line) => line,
            None => return EngineStatus::Missing,
        },
        None => combined.as_str(),
    };
    let found = parse_version(scan);
    match (probe.min_version.as_deref(), &found) {
        (Some(min), Some(v)) if !version_meets_min(v, min) => EngineStatus::Incompatible {
            found: v.clone(),
            required: min.to_string(),
        },
        _ => EngineStatus::Available {
            version: found.unwrap_or_else(|| "unknown".to_string()),
        },
    }
}

/// The first non-blank line of probe output, as the operator-facing reason an engine cannot
/// start. Truncated: a loader error is one line, and an engine that dumps a page of output is
/// not owed the whole page in a status listing.
fn first_line(output: &str) -> String {
    const MAX: usize = 200;
    let line = output
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no output");
    match line.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_string(),
    }
}

/// Extract the first `MAJOR.MINOR[.PATCH]` token from probe output (best-effort).
pub fn parse_version(text: &str) -> Option<String> {
    for token in text.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() >= 2
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        {
            return Some(token.to_string());
        }
    }
    None
}

/// Whether `found` is at least `min`, comparing dotted numeric components left to right.
pub fn version_meets_min(found: &str, min: &str) -> bool {
    let nums = |s: &str| -> Vec<u64> {
        s.split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (f, m) = (nums(found), nums(min));
    for i in 0..f.len().max(m.len()) {
        let (fi, mi) = (
            f.get(i).copied().unwrap_or(0),
            m.get(i).copied().unwrap_or(0),
        );
        if fi != mi {
            return fi > mi;
        }
    }
    true
}

/// One formalized requirement's engine readiness (R-eng-3). `categories` are the
/// requirement's declared PRL categories; `ready` is true only when **every** category's
/// engine is available — a multi-category requirement needs all its engines. `blockers`
/// names the missing/incompatible ones for the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readiness {
    pub id: String,
    pub categories: Vec<BindCategory>,
    pub ready: bool,
    pub blockers: Vec<String>,
}

/// Compute one requirement's readiness from its declared categories and the
/// **already-detected** per-category statuses. Pure — the caller probes once and passes
/// the map, so this stays testable without spawning processes. A requirement with no
/// declared category cannot be routed and is reported as blocked.
pub fn readiness(
    id: &str,
    categories: &[BindCategory],
    status_by_category: &std::collections::BTreeMap<BindCategory, Vec<EngineStatus>>,
) -> Readiness {
    let mut blockers = Vec::new();
    if categories.is_empty() {
        blockers.push("no declared category — cannot route to an engine".to_string());
    }
    for cat in categories {
        // A category is routable as soon as **any** of its ensemble engines is ready — the
        // others corroborate but are not required (D2b). None ready blocks it.
        let ready = status_by_category
            .get(cat)
            .map(|statuses| statuses.iter().any(EngineStatus::is_ready))
            .unwrap_or(false);
        if !ready {
            let names = engines_for(*cat)
                .iter()
                .map(|e| e.name)
                .collect::<Vec<_>>()
                .join(" / ");
            blockers.push(format!("category {} ({names}) not ready", cat.as_label()));
        }
    }
    Readiness {
        id: id.to_string(),
        categories: categories.to_vec(),
        ready: blockers.is_empty(),
        blockers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    // Verifies: REQ022/REQ027/REQ031/REQ032 — every PRL category is routed, and category 1 is an
    // ENSEMBLE of three wired engines (Kani + Creusot + Prusti, D2b), while 2a/2b/3 route to one
    // each.
    #[test]
    fn registry_routes_every_category() {
        for cat in [BindCategory::Model, BindCategory::Runtime, BindCategory::Ui] {
            assert_eq!(engines_for(cat).len(), 1, "{cat:?} routes to one engine");
        }
        // Category 1 is the ensemble — Kani first, Creusot second (REQ031), Prusti third (REQ032).
        let code = engines_for(BindCategory::Code);
        assert_eq!(code.len(), 3, "category 1 is a three-engine ensemble");
        let names: Vec<&str> = code.iter().map(|e| e.name).collect();
        assert!(names.contains(&"Kani"), "{names:?}");
        assert!(names.contains(&"Creusot"), "{names:?}");
        assert!(names.contains(&"Prusti"), "{names:?}");
        let kani = code.iter().find(|e| e.name == "Kani").expect("Kani wired");
        assert_eq!(
            kani.probe
                .as_ref()
                .and_then(Probe::command)
                .expect("cat-1 is wired")
                .bin,
            "cargo-kani"
        );
        let creusot = code
            .iter()
            .find(|e| e.name == "Creusot")
            .expect("Creusot wired");
        assert_eq!(
            creusot
                .probe
                .as_ref()
                .and_then(Probe::command)
                .expect("Creusot is wired")
                .bin,
            "cargo-creusot"
        );
        let prusti = code
            .iter()
            .find(|e| e.name == "Prusti")
            .expect("Prusti wired");
        assert_eq!(
            prusti
                .probe
                .as_ref()
                .and_then(Probe::command)
                .expect("Prusti is wired")
                .bin,
            "cargo-prusti"
        );
        // REQ029: category 2a is wired to TLC, probed via `java … tlc2.TLC`.
        let model = engines_for(BindCategory::Model);
        assert_eq!(model.len(), 1);
        assert_eq!(model[0].name, "TLC (TLA+)");
        assert_eq!(
            model[0]
                .probe
                .as_ref()
                .and_then(Probe::command)
                .expect("cat-2a is wired")
                .bin,
            "java"
        );
    }

    // Verifies: REQ024/REQ027/REQ029 (R-eng-2/3) — **a probe exists exactly when provreq can run
    // the engine.** Reporting `ready` for an engine nothing drives is the REQ024 overclaim wearing
    // a different hat: the operator installs the tool, `engines` turns green, and `verify` still
    // answers `no-engine`.
    //
    // This test used to name the categories that were unwired, and went out of date every time a
    // slice wired one — 2b left the list in #233, and #245 wired category 3, the last one. There is
    // no membership left to pin, which is the point: what has to survive is the RULE, in both
    // directions.
    #[test]
    fn a_probe_exists_exactly_when_provreq_can_run_the_engine() {
        // Forward: every registered engine has a lowering behind it, so every one is probed.
        for engine in registry() {
            assert!(
                engine.probe.is_some(),
                "{} is registered, so provreq claims it can run it — an engine with a lowering \
                 must be probed",
                engine.name
            );
        }
        // Back: an engine with no probe reports NotWired and can never be ready, whatever is
        // installed. Constructed rather than found, because the registry no longer holds one.
        let unwired = Engine {
            category: BindCategory::Ui,
            name: "a driver nobody wired",
            probe: None,
        };
        assert_eq!(detect(&unwired, None), EngineStatus::NotWired);
        assert!(!detect(&unwired, None).is_ready());
    }

    // Verifies: REQ024 — `NotWired` can never back a verdict, whoever reports it.
    #[test]
    fn an_unwired_engine_is_never_ready() {
        assert!(
            !EngineStatus::NotWired.is_ready(),
            "an unwired engine can never back a verdict"
        );
    }

    // Verifies: REQ024 — `NotWired` (ours to fix by wiring an engine) stays distinct from
    // `Missing` (the operator's to fix by installing a binary); both block readiness, but
    // they ask different people to act.
    #[test]
    fn not_wired_is_distinct_from_missing() {
        assert_ne!(EngineStatus::NotWired, EngineStatus::Missing);
        assert!(!EngineStatus::Missing.is_ready());
        assert!(EngineStatus::NotWired.describe().contains("NOT WIRED"));
    }

    // Verifies: REQ022 (R-eng-2) — a binary that is not on PATH is reported Missing, never
    // installed or faked as present.
    #[test]
    fn absent_binary_detects_as_missing() {
        let probe = EngineProbe {
            bin: "provreq_no_such_engine_xyz".to_string(),
            args: vec!["--version".to_string()],
            version_marker: None,
            min_version: None,
        };
        assert_eq!(detect_probe(&probe), EngineStatus::Missing);
    }

    // Verifies: REQ022 — a present binary detects as Available (uses `echo`, which exists
    // on the test/CI platform).
    #[test]
    fn present_binary_detects_as_available() {
        let probe = EngineProbe {
            bin: "echo".to_string(),
            args: vec!["9.9".to_string()],
            version_marker: None,
            min_version: None,
        };
        assert!(matches!(
            detect_probe(&probe),
            EngineStatus::Available { .. }
        ));
    }

    // Verifies: REQ029 — a host that runs but whose output lacks the engine's marker is
    // Missing, not falsely Available. This is the TLC-via-java case: `java` spawns fine but the
    // jar is absent, so the `TLC2 Version` banner never appears and the engine is not really
    // present. `echo` stands in for the host here.
    #[test]
    fn present_host_without_the_engine_marker_is_missing() {
        let probe = EngineProbe {
            bin: "echo".to_string(),
            args: vec!["some other output".to_string()],
            version_marker: Some("TLC2 Version".to_string()),
            min_version: None,
        };
        assert_eq!(detect_probe(&probe), EngineStatus::Missing);
    }

    // Verifies: REQ051 — a binary that is on PATH but cannot start (exit 127, the loader's code
    // for a missing shared library — how a broken `cargo-prusti` fails) is NOT available. Before
    // this, the probe looked only at "did a process spawn", so it reported `available (unknown)`
    // for an engine that had already died.
    #[test]
    fn a_binary_that_cannot_start_is_not_available() {
        let probe = EngineProbe {
            bin: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "echo 'error while loading shared libraries: libstd.so' >&2; exit 127".to_string(),
            ],
            version_marker: None,
            min_version: None,
        };
        let status = detect_probe(&probe);
        assert!(!status.is_ready(), "{status:?} must not back a verdict");
        let EngineStatus::Unusable { reason } = &status else {
            panic!("expected Unusable, got {status:?}");
        };
        assert!(reason.contains("shared libraries"), "{reason}");
        assert!(status.describe().contains("UNUSABLE"), "{status:?}");
    }

    // Verifies: REQ051 — "cannot start" stays distinct from "not installed", because the two ask
    // for different work: installing an engine that is already on disk fixes nothing.
    #[test]
    fn unusable_is_distinct_from_missing() {
        let unusable = EngineStatus::Unusable {
            reason: "cannot start".to_string(),
        };
        assert_ne!(unusable, EngineStatus::Missing);
        assert_eq!(unusable.tag(), "unusable");
        assert_ne!(unusable.tag(), EngineStatus::Missing.tag());
    }

    // Verifies: REQ051 — the rule is narrow on purpose. A non-zero exit is NOT itself evidence of
    // a broken engine: TLC exits 1 asking for an input module, and a deductive verifier exits 1
    // complaining about the subject it was pointed at. A blanket "non-zero ⇒ not present" would
    // trade one wrong answer for another and mark working engines unusable. (Creusot's own probe
    // now exits 0 — see #159 — but the rule it motivated still holds for the rest.)
    #[test]
    fn an_engine_that_ran_and_objected_to_its_input_is_still_present() {
        let creusot_like = EngineProbe {
            bin: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "echo 'Error: creusot-std not found in dependencies' >&2; exit 1".to_string(),
            ],
            version_marker: None,
            min_version: None,
        };
        assert!(
            detect_probe(&creusot_like).is_ready(),
            "an engine complaining about the SUBJECT is present"
        );

        let tlc_like = EngineProbe {
            bin: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "echo 'TLC2 Version 2.19 of 08 August 2024'; exit 1".to_string(),
            ],
            version_marker: Some("TLC2 Version".to_string()),
            min_version: None,
        };
        assert_eq!(
            detect_probe(&tlc_like),
            EngineStatus::Available {
                version: "2.19".to_string()
            }
        );
    }

    // Verifies: REQ049/#159 — an engine's reported version must come from a line that names the
    // engine. This is the exact shape that corrupted the proving-environment stamp: the probe
    // emitted a build log for the SUBJECT before anything about the engine, and the version scan
    // took the first version-shaped token it met — so `provreq engines` reported provreq's own
    // 0.0.1 as Creusot's. A wrong engine version is worse than none: REQ049 exists to detect
    // engine drift, and a stamp that tracks the subject never drifts when the engine changes and
    // always drifts when it does not.
    #[test]
    fn a_version_is_never_read_off_a_line_that_does_not_name_the_engine() {
        let noisy = EngineProbe {
            bin: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "echo '    Checking provreq v0.0.1 (/workspace)'; \
                 echo 'cargo-creusot 0.12.0'"
                    .to_string(),
            ],
            version_marker: Some("cargo-creusot".to_string()),
            min_version: None,
        };
        assert_eq!(
            detect_probe(&noisy),
            EngineStatus::Available {
                version: "0.12.0".to_string()
            },
            "the version must come from the engine's own line, not the subject's build log"
        );
    }

    // Verifies: REQ051/#159 — the marker still decides presence. Output that never names the
    // engine is Missing, even when it is full of version-shaped tokens: those belong to whatever
    // else ran, and inventing an engine version from them is the bug this guards.
    #[test]
    fn output_that_never_names_the_engine_is_missing_not_a_guessed_version() {
        let subject_build_only = EngineProbe {
            bin: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "echo '    Checking provreq v0.0.1 (/workspace)'; \
                 echo 'error: could not compile provreq'"
                    .to_string(),
            ],
            version_marker: Some("cargo-creusot".to_string()),
            min_version: None,
        };
        assert_eq!(detect_probe(&subject_build_only), EngineStatus::Missing);
    }

    // Verifies: REQ022 — version parsing and comparison (the compatibility machinery that
    // ships presence-only but is exercised here).
    #[test]
    fn version_parsing_and_comparison() {
        assert_eq!(
            parse_version("MonPoly 1.2.3 (build x)").as_deref(),
            Some("1.2.3")
        );
        assert_eq!(
            parse_version("echo (GNU coreutils) 9.4").as_deref(),
            Some("9.4")
        );
        assert_eq!(parse_version("no numbers here"), None);

        assert!(version_meets_min("1.2.3", "1.2.0"));
        assert!(version_meets_min("2.0", "1.9.9"));
        assert!(!version_meets_min("1.1", "1.2"));
        assert!(version_meets_min("1.2", "1.2"));
    }

    // Verifies: REQ022 (R-eng-3) — a requirement is ready only when every declared
    // category's engine is ready; missing engines are named as blockers.
    #[test]
    fn readiness_needs_every_category_engine() {
        let mut status = BTreeMap::new();
        status.insert(
            BindCategory::Runtime,
            vec![EngineStatus::Available {
                version: "1.0".into(),
            }],
        );
        status.insert(BindCategory::Model, vec![EngineStatus::Missing]);

        let ready_one = readiness("SR001", &[BindCategory::Runtime], &status);
        assert!(ready_one.ready);
        assert!(ready_one.blockers.is_empty());

        let blocked = readiness(
            "SR002",
            &[BindCategory::Runtime, BindCategory::Model],
            &status,
        );
        assert!(!blocked.ready);
        assert!(blocked.blockers.iter().any(|b| b.contains("2a")));
    }

    // Verifies: REQ024 (R-eng-3) — a category whose engine is not wired blocks readiness
    // and is named as a blocker, rather than being waved through as ready.
    #[test]
    fn unwired_engine_blocks_readiness() {
        let status = BTreeMap::from([(BindCategory::Code, vec![EngineStatus::NotWired])]);
        let r = readiness("SR004", &[BindCategory::Code], &status);
        assert!(!r.ready, "an unwired category-1 engine is not readiness");
        assert!(r.blockers.iter().any(|b| b.contains('1')));
    }

    // Verifies: REQ030 (D2b) — a category is routable as soon as ONE of its ensemble engines
    // is ready; a missing corroborating engine does not block it. This is the any-ready
    // semantics that replaces the one-status-per-category assumption (and the silent overwrite
    // it caused once a category has two engines).
    #[test]
    fn category_is_ready_when_any_ensemble_engine_is_ready() {
        let status = BTreeMap::from([(
            BindCategory::Code,
            vec![
                EngineStatus::Available {
                    version: "0.67".into(),
                },
                EngineStatus::Missing,
            ],
        )]);
        let r = readiness("SR005", &[BindCategory::Code], &status);
        assert!(r.ready, "one ready engine is enough to route the category");
        assert!(r.blockers.is_empty());
    }

    // Verifies: REQ022 — a requirement with no declared category is blocked (unroutable),
    // never silently treated as ready.
    #[test]
    fn uncategorized_requirement_is_blocked() {
        let r = readiness("SR003", &[], &BTreeMap::new());
        assert!(!r.ready);
        assert!(r
            .blockers
            .iter()
            .any(|b| b.contains("no declared category")));
    }
}
