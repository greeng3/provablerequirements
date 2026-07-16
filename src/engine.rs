//! R-eng-2/3 — engine coverage. Reports which verification engines are installed and
//! version-compatible, and which formalized requirements are therefore checkable —
//! **without ever installing anything** (R-eng-2, detect presence *and* compatibility)
//! and without running any engine. Coverage is gated by installed+compatible engines and
//! reported first-class (R-eng-3), keeping *formalizable-but-no-engine* distinct from
//! *not formalized*.
//!
//! The R-eng-1 split: category 1 (code) is **toolchain-welded** — its engine is the
//! subject's own per-language build toolchain (R-eng-4), not a portable binary this
//! module probes. Categories 2a/2b/3 are **artifact-fed** portable engines (TLC, MonPoly,
//! a UI driver) detected on `PATH`.
//!
//! Implements: REQ022 (engine coverage — detect installed engines, report readiness).

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

/// A verification engine a PRL category routes to. `probe` is `None` for the
/// toolchain-welded category-1 engine (checked per-subject at verify time, not a single
/// portable binary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Engine {
    pub category: BindCategory,
    pub name: &'static str,
    pub probe: Option<EngineProbe>,
}

/// The detected state of an engine (R-eng-2: presence *and* compatibility).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineStatus {
    /// Category 1 — welded to the subject's build toolchain; readiness is confirmed
    /// per-subject when a verdict is actually produced, not by a portable probe here.
    ToolchainWelded,
    /// On `PATH` and (if a minimum is set) new enough. `version` is best-effort —
    /// `"unknown"` when the probe ran but printed nothing parseable.
    Available { version: String },
    /// Not on `PATH`, or present but unrunnable.
    Missing,
    /// Present but older than the required minimum.
    Incompatible { found: String, required: String },
}

impl EngineStatus {
    /// Whether an engine in this state can back a verdict (R-eng-3 gate). Toolchain-welded
    /// counts as ready — the operator runs provreq in the subject's own build env.
    pub fn is_ready(&self) -> bool {
        matches!(
            self,
            EngineStatus::ToolchainWelded | EngineStatus::Available { .. }
        )
    }

    pub fn describe(&self) -> String {
        match self {
            EngineStatus::ToolchainWelded => "welded, checked per subject".to_string(),
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
            category: BindCategory::Code,
            name: "build toolchain (per-language)",
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

/// Detect an engine's status (R-eng-2). Toolchain-welded engines report as such without
/// a probe; portable engines are looked up on `PATH` and version-checked. Never installs.
pub fn detect(engine: &Engine) -> EngineStatus {
    match &engine.probe {
        None => EngineStatus::ToolchainWelded,
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

    // Verifies: REQ022 (R-eng-2) — the toolchain-welded engine reports without a probe.
    #[test]
    fn code_engine_is_toolchain_welded() {
        assert_eq!(
            detect(&engine_for(BindCategory::Code)),
            EngineStatus::ToolchainWelded
        );
        assert!(EngineStatus::ToolchainWelded.is_ready());
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
        status.insert(BindCategory::Code, EngineStatus::ToolchainWelded);
        status.insert(BindCategory::Model, EngineStatus::Missing);

        let ready_one = readiness("SR001", &[BindCategory::Code], &status);
        assert!(ready_one.ready);
        assert!(ready_one.blockers.is_empty());

        let blocked = readiness("SR002", &[BindCategory::Code, BindCategory::Model], &status);
        assert!(!blocked.ready);
        assert!(blocked.blockers.iter().any(|b| b.contains("2a")));
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
