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
//! MonPoly, a UI driver). Both are detected the same way when wired — a `PATH` probe — and
//! only category 1 (Kani) is wired today, so 2a/2b/3 report NotWired.
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

/// A verification engine a PRL category routes to. `probe` is `Some` exactly when provreq
/// has an integration that can run the engine — so `None` means "not wired", which is ours
/// to fix, and a failed probe means "not installed", which is the operator's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Engine {
    pub category: BindCategory,
    pub name: &'static str,
    pub probe: Option<EngineProbe>,
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
    /// Not on `PATH`, or present but unrunnable.
    Missing,
    /// Present but older than the required minimum.
    Incompatible { found: String, required: String },
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
            EngineStatus::Incompatible { found, required } => {
                format!("INCOMPATIBLE (found {found}, needs >= {required})")
            }
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
            probe: Some(EngineProbe {
                bin: "cargo-kani".to_string(),
                args: vec!["--version".to_string()],
                version_marker: None,
                min_version: None,
            }),
        },
        Engine {
            // Category 1 is an ENSEMBLE (D2b), not a single engine: Creusot joins Kani as the
            // #2 member — REQ031. It is a **deductive** verifier, so it earns `proven` (∀
            // executions) where Kani earns bounded `model-checked`; `aggregate` reports the
            // stronger rung when both hold ("proven by Creusot, corroborated bounded by
            // Kani"). Toolchain-welded like Kani (R-eng-4). `cargo-creusot` is the binary
            // `cargo creusot` needs on PATH; it runs (exit 0) even outside a subject, so its
            // presence is the honest readiness signal (there is no clean --version to parse).
            category: BindCategory::Code,
            name: "Creusot",
            probe: Some(EngineProbe {
                bin: "cargo-creusot".to_string(),
                args: vec!["--version".to_string()],
                version_marker: None,
                min_version: None,
            }),
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
            probe: Some(EngineProbe {
                bin: "cargo-prusti".to_string(),
                args: vec!["--help".to_string()],
                version_marker: None,
                min_version: None,
            }),
        },
        Engine {
            // Category 2a is the model world: the temporal properties (safety AND liveness)
            // checked against a TLA+ model. Its engine is TLC — REQ029, the model-world analog
            // of wiring Kani for category 1. TLC is not a PATH binary; it runs as
            // `java -cp <jar> tlc2.TLC`, so the probe is java with the jar on the classpath and
            // the marker guards against java-present-but-jar-absent.
            category: BindCategory::Model,
            name: "TLC (TLA+)",
            probe: Some(EngineProbe {
                bin: "java".to_string(),
                args: vec![
                    "-cp".to_string(),
                    crate::tlc::jar_path(),
                    "tlc2.TLC".to_string(),
                ],
                version_marker: Some("TLC2 Version".to_string()),
                min_version: None,
            }),
        },
        Engine {
            category: BindCategory::Runtime,
            name: "MonPoly",
            probe: None,
        },
        Engine {
            category: BindCategory::Ui,
            name: "Selenium/Playwright driver",
            probe: None,
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
pub fn detect(engine: &Engine) -> EngineStatus {
    match &engine.probe {
        None => EngineStatus::NotWired,
        Some(probe) => detect_probe(probe),
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
    // A marker that is set but absent means the host ran (e.g. `java`) but the engine is not
    // actually reachable (e.g. the jar is missing) — that is Missing, not Available.
    if let Some(marker) = &probe.version_marker {
        if !combined.contains(marker) {
            return EngineStatus::Missing;
        }
    }
    let found = parse_version(&combined);
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
            kani.probe.as_ref().expect("cat-1 is wired").bin,
            "cargo-kani"
        );
        let creusot = code
            .iter()
            .find(|e| e.name == "Creusot")
            .expect("Creusot wired");
        assert_eq!(
            creusot.probe.as_ref().expect("Creusot is wired").bin,
            "cargo-creusot"
        );
        let prusti = code
            .iter()
            .find(|e| e.name == "Prusti")
            .expect("Prusti wired");
        assert_eq!(
            prusti.probe.as_ref().expect("Prusti is wired").bin,
            "cargo-prusti"
        );
        // REQ029: category 2a is wired to TLC, probed via `java … tlc2.TLC`.
        let model = engines_for(BindCategory::Model);
        assert_eq!(model.len(), 1);
        assert_eq!(model[0].name, "TLC (TLA+)");
        assert_eq!(
            model[0].probe.as_ref().expect("cat-2a is wired").bin,
            "java"
        );
    }

    // Verifies: REQ027/REQ029 (R-eng-2/3) — an engine is probed ONLY if provreq can run it. A
    // category with no lowering reports NotWired even when its binary is installed, because
    // reporting `ready` for an engine nothing drives is the REQ024 overclaim wearing a
    // different hat: the operator installs the tool, `engines` turns green, and `verify`
    // still answers `no-engine`. Categories 1 (Kani) and 2a (TLC) are wired; 2b/3 are not.
    #[test]
    fn unwired_categories_are_not_probed_and_never_report_ready() {
        for cat in [BindCategory::Runtime, BindCategory::Ui] {
            for engine in engines_for(cat) {
                assert!(
                    engine.probe.is_none(),
                    "{} has no lowering, so probing its binary would promise a verdict provreq \
                     cannot produce",
                    engine.name
                );
                assert_eq!(detect(&engine), EngineStatus::NotWired);
                assert!(!detect(&engine).is_ready());
            }
        }
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
