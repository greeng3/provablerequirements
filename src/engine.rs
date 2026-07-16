//! R-eng-2/3 — engine coverage. Reports which verification engines are installed and
//! version-compatible, and which formalized requirements are therefore checkable —
//! **without ever installing anything** (R-eng-2, detect presence *and* compatibility)
//! and without running any engine. Coverage is gated by installed+compatible engines and
//! reported first-class (R-eng-3), keeping *formalizable-but-no-engine* distinct from
//! *not formalized*.
//!
//! The R-eng-1 split: category 1 (code) is **toolchain-welded** — its engine needs the
//! subject's own compiler (R-eng-4), so it is not a shared portable binary this module can
//! probe by name. Categories 2a/2b/3 are **artifact-fed** portable engines (TLC, MonPoly,
//! a UI driver) detected on `PATH`.
//!
//! "Toolchain-welded" classifies *how an engine is deployed*, never *whether it is
//! present*: R-eng-2 requires welded engines to be provisioned into the dev env and
//! detected like any other. Reading the class as readiness is what REQ024 fixed — see
//! [`EngineStatus::is_ready`].
//!
//! `// ponytail:` readiness here still means "the engine binary is present", not "provreq
//! can run it" — no engine is wired yet, so no `Available` engine can actually back a
//! verdict either. That gap closes when the first engine is wired and `ready` earns its
//! full meaning; category 1 is fixed now because it claimed readiness with no detection at
//! all, which is strictly worse.
//!
//! Implements: REQ022 (engine coverage — detect installed engines, report readiness),
//! REQ024 (a category-1 engine that is not wired never reports ready).

use crate::grounding::BindCategory;
use std::process::Command;

/// A portable engine's presence probe: the binary to look up on `PATH`, the argument
/// that makes it print its version, and an optional minimum version. Version thresholds
/// are presence-only for now (`None`) — the compatibility machinery is real and tested,
/// but no minimums are shipped until a real engine is on hand to calibrate against.
/// `// ponytail: probe args are best-effort (TLC has no clean --version); tune per engine
/// when one is actually installed, and move bins/min-versions to provreq.yml config.`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineProbe {
    pub bin: &'static str,
    pub version_arg: &'static str,
    pub min_version: Option<&'static str>,
}

/// A verification engine a PRL category routes to. `probe` is `None` when no integration
/// exists yet, which today is the toolchain-welded category-1 engine — it cannot be
/// probed as one portable binary, and nothing is wired to run it.
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

/// The category→engine registry: exactly one engine per PRL category. Engine names and
/// routing follow the settled design (docs/requirement-language.md): 2a model checking is
/// the TLA+ lineage (TLC), 2b runtime monitoring is MonPoly (MFOTL), 3 UI is a
/// Selenium/Playwright driver.
pub fn registry() -> Vec<Engine> {
    vec![
        Engine {
            // The category-1 engine is a deductive verifier over the temporal-free
            // fragment (docs/requirement-language.md, Core layer: "1 → the temporal-free
            // fragment (pre/post/invariants) → Viper/deductive"); for Rust the candidates
            // are Prusti/Verus/Creusot/Kani (D2b). It is toolchain-welded (R-eng-1/R-eng-4)
            // because it needs the subject's own compiler, so it is not a shared portable
            // binary this module can probe by name — `probe: None` says "no integration
            // yet", NOT "assume ready". Picking and wiring the verifier is the next slice.
            category: BindCategory::Code,
            name: "deductive verifier",
            probe: None,
        },
        Engine {
            category: BindCategory::Model,
            name: "TLC (TLA+)",
            probe: Some(EngineProbe {
                bin: "tlc",
                version_arg: "-h",
                min_version: None,
            }),
        },
        Engine {
            category: BindCategory::Runtime,
            name: "MonPoly",
            probe: Some(EngineProbe {
                bin: "monpoly",
                version_arg: "-version",
                min_version: None,
            }),
        },
        Engine {
            category: BindCategory::Ui,
            name: "Selenium/Playwright driver",
            probe: Some(EngineProbe {
                bin: "playwright",
                version_arg: "--version",
                min_version: None,
            }),
        },
    ]
}

/// The engine that runs a given category.
pub fn engine_for(category: BindCategory) -> Engine {
    registry()
        .into_iter()
        .find(|e| e.category == category)
        .expect("registry covers every BindCategory")
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
    let output = match Command::new(probe.bin).arg(probe.version_arg).output() {
        Ok(o) => o,
        Err(_) => return EngineStatus::Missing,
    };
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let found = parse_version(&combined);
    match (probe.min_version, &found) {
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
    status_by_category: &std::collections::BTreeMap<BindCategory, EngineStatus>,
) -> Readiness {
    let mut blockers = Vec::new();
    if categories.is_empty() {
        blockers.push("no declared category — cannot route to an engine".to_string());
    }
    for cat in categories {
        let ready = status_by_category
            .get(cat)
            .map(EngineStatus::is_ready)
            .unwrap_or(false);
        if !ready {
            let engine = engine_for(*cat);
            blockers.push(format!(
                "category {} ({}) not ready",
                cat.as_label(),
                engine.name
            ));
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

    // Verifies: REQ022 — the registry routes every PRL category to exactly one engine,
    // with category 1 toolchain-welded (no portable probe) and the rest artifact-fed.
    #[test]
    fn registry_covers_every_category_once() {
        let reg = registry();
        for cat in [
            BindCategory::Code,
            BindCategory::Model,
            BindCategory::Runtime,
            BindCategory::Ui,
        ] {
            assert_eq!(reg.iter().filter(|e| e.category == cat).count(), 1);
        }
        assert!(engine_for(BindCategory::Code).probe.is_none());
        assert!(engine_for(BindCategory::Model).probe.is_some());
    }

    // Verifies: REQ024 (R-eng-2) — the category-1 engine has no integration yet, so it
    // reports NotWired and is NOT ready. It previously claimed readiness unconditionally
    // from being toolchain-welded, which reported every category-1 requirement as
    // engine-ready when no verifier existed at all.
    #[test]
    fn code_engine_is_not_wired_and_not_ready() {
        assert_eq!(
            detect(&engine_for(BindCategory::Code)),
            EngineStatus::NotWired
        );
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
            bin: "provreq_no_such_engine_xyz",
            version_arg: "--version",
            min_version: None,
        };
        assert_eq!(detect_probe(&probe), EngineStatus::Missing);
    }

    // Verifies: REQ022 — a present binary detects as Available (uses `echo`, which exists
    // on the test/CI platform).
    #[test]
    fn present_binary_detects_as_available() {
        let probe = EngineProbe {
            bin: "echo",
            version_arg: "--version",
            min_version: None,
        };
        assert!(matches!(
            detect_probe(&probe),
            EngineStatus::Available { .. }
        ));
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
            EngineStatus::Available {
                version: "1.0".into(),
            },
        );
        status.insert(BindCategory::Model, EngineStatus::Missing);

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
        let status = BTreeMap::from([(BindCategory::Code, EngineStatus::NotWired)]);
        let r = readiness("SR004", &[BindCategory::Code], &status);
        assert!(!r.ready, "an unwired category-1 engine is not readiness");
        assert!(r.blockers.iter().any(|b| b.contains('1')));
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
