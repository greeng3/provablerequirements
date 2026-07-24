//! R-eng-2 (install half) — consent-gated native provisioning of verification engines.
//!
//! Detection ([`crate::engine::detect`]) reports what is present; this module turns a `Missing`
//! into an `Available`, with the operator's consent, then re-detects to confirm. Per the Design-C
//! decision ([docs/design-c-decision.md]): native provisioning is **tiered**. Only the light tier
//! is a first-class native install — TLC first (a headless JRE plus a ~2 MB jar). The heavy tier
//! (Creusot/Prusti/MonPoly) is dev-container-first and reports an honest "no native recipe yet"
//! rather than a half-built install.
//!
//! Graceful degradation is load-bearing (R-eng-3): a JVM we do not install, an engine with no
//! native recipe, or a download that fails all *narrow the feature set* — they never fail the tool.
//!
//! Implements: REQ046 (consent-gated native install of a light-tier engine; honest degradation).

use crate::engine::{self, EngineStatus};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

/// The tla2tools release this build pins — the same version the devcontainer bakes in, so a
/// natively-provisioned TLC matches the one CI verifies against.
pub const TLA2TOOLS_VERSION: &str = "1.7.4";

/// The pinned tla2tools.jar download. A GitHub release asset — stable, checksum-pinned upstream.
pub const TLA2TOOLS_URL: &str =
    "https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar";

/// provreq's own data directory, where provisioned engine artifacts live. `PROVREQ_DATA_HOME`
/// overrides; else `XDG_DATA_HOME/provreq`; else `~/.local/share/provreq`.
///
/// `// ponytail: XDG/Linux resolution — the ADR's first target. Add the macOS/Windows dirs (or a
/// `dirs` crate) when those platforms get a light-tier install; not yet.`
pub fn data_dir() -> PathBuf {
    if let Ok(explicit) = std::env::var("PROVREQ_DATA_HOME") {
        return PathBuf::from(explicit);
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("provreq");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/provreq")
}

/// Where a natively-provisioned tla2tools.jar lives. [`crate::tlc::jar_path`] falls back to this
/// when `TLA2TOOLS_JAR` is unset, so the installer and the detector agree on one location.
pub fn tla2tools_jar_default() -> PathBuf {
    data_dir().join("tlaplus/tla2tools.jar")
}

/// The pure install decision for TLC: what to do given what is already detected, whether a JVM is
/// present, and whether the operator has consented. No IO, so it is exhaustively testable — the
/// `install_tlc` wrapper supplies the real detections and performs only the download for `Proceed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallDecision {
    /// TLC already detects as available — nothing to do.
    AlreadyPresent,
    /// TLC needs a Java runtime that is not present. provreq does not install system JVMs, so this
    /// is an honest degradation, not an attempt.
    BlockedNoJava,
    /// Everything is ready to install, but the operator has not consented yet — show the plan.
    NeedsConsent,
    /// Consent given and prerequisites met — perform the download.
    Proceed,
}

/// Decide what installing TLC should do. Pure over the three inputs.
pub fn decide_tlc(detected: &EngineStatus, java_present: bool, consent: bool) -> InstallDecision {
    if detected.is_ready() {
        return InstallDecision::AlreadyPresent;
    }
    if !java_present {
        return InstallDecision::BlockedNoJava;
    }
    if !consent {
        return InstallDecision::NeedsConsent;
    }
    InstallDecision::Proceed
}

/// The result of an install attempt, each variant a distinct operator-facing outcome.
#[derive(Debug)]
pub enum InstallOutcome {
    /// The engine was already available; no change.
    AlreadyPresent,
    /// The engine was installed and re-detected as available. Carries where it landed.
    Installed { path: PathBuf },
    /// Blocked on a missing prerequisite the tool will not install for the operator (a JVM).
    Blocked { reason: String },
    /// Consent required: the plan the operator must approve with `--yes`.
    NeedsConsent { plan: String },
    /// The install ran but the engine still does not detect — the download landed but something
    /// downstream (a broken jar, an unusable JVM) means it is not usable. Honest, not silent.
    Failed { reason: String },
    /// No native install recipe for this engine yet (heavy tier / not the light-tier target).
    Unsupported { reason: String },
}

impl InstallOutcome {
    /// A human line for the CLI, plus whether the outcome is a success (for the exit path).
    pub fn describe(&self) -> String {
        match self {
            InstallOutcome::AlreadyPresent => "already installed — nothing to do".to_string(),
            InstallOutcome::Installed { path } => {
                format!("installed and detected — {}", path.display())
            }
            InstallOutcome::Blocked { reason } => format!("not installed — {reason}"),
            InstallOutcome::NeedsConsent { plan } => plan.clone(),
            InstallOutcome::Failed { reason } => format!("install failed — {reason}"),
            InstallOutcome::Unsupported { reason } => reason.clone(),
        }
    }

    /// Whether provreq should exit non-zero: a genuine failure, not an honest degradation or a
    /// consent prompt (those are expected operator states, not errors).
    pub fn is_failure(&self) -> bool {
        matches!(self, InstallOutcome::Failed { .. })
    }
}

/// Whether a Java runtime is on PATH — TLC runs as `java -cp <jar> tlc2.TLC`, so no JVM means TLC
/// cannot run however the jar is provisioned. Best-effort: any successful `java -version` counts.
pub fn java_present() -> bool {
    Command::new("java")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Install TLC natively (REQ046): detect → java precheck → consent gate → download the pinned jar
/// into [`tla2tools_jar_default`] → re-detect. Every non-`Installed` outcome is an honest state the
/// operator can act on, never a panic and never a silent partial install.
pub async fn install_tlc(consent: bool) -> Result<InstallOutcome> {
    let tlc = tlc_engine();
    let before = engine::detect(&tlc);
    match decide_tlc(&before, java_present(), consent) {
        InstallDecision::AlreadyPresent => Ok(InstallOutcome::AlreadyPresent),
        InstallDecision::BlockedNoJava => Ok(InstallOutcome::Blocked {
            reason: "TLC needs a Java runtime, which is not on PATH. provreq does not install \
                     system JVMs — install a JRE (e.g. a headless OpenJDK) and retry."
                .to_string(),
        }),
        InstallDecision::NeedsConsent => Ok(InstallOutcome::NeedsConsent {
            plan: consent_plan(),
        }),
        InstallDecision::Proceed => download_and_verify().await,
    }
}

/// The consent prompt: exactly what `--yes` would do, so the operator approves a concrete action.
fn consent_plan() -> String {
    format!(
        "would download TLC {TLA2TOOLS_VERSION} (tla2tools.jar, ~2 MB)\n    from {TLA2TOOLS_URL}\n    \
         to   {}\n  re-run with --yes to install.",
        tla2tools_jar_default().display()
    )
}

/// Download the pinned jar to its data-dir home, then re-detect to confirm it is usable.
async fn download_and_verify() -> Result<InstallOutcome> {
    let target = tla2tools_jar_default();
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let bytes = reqwest::get(TLA2TOOLS_URL)
        .await
        .with_context(|| format!("downloading {TLA2TOOLS_URL}"))?
        .error_for_status()
        .with_context(|| format!("downloading {TLA2TOOLS_URL}"))?
        .bytes()
        .await
        .context("reading the tla2tools.jar download body")?;
    std::fs::write(&target, &bytes).with_context(|| format!("writing {}", target.display()))?;

    // Re-detect against the just-written jar so success means "provreq can actually run TLC", not
    // merely "a file landed". The detector reads TLA2TOOLS_JAR / the default path this wrote to.
    match engine::detect(&tlc_engine()) {
        s if s.is_ready() => Ok(InstallOutcome::Installed { path: target }),
        s => Ok(InstallOutcome::Failed {
            reason: format!(
                "the jar downloaded to {} but TLC still does not detect ({}). The JVM may be \
                 unusable, or the jar corrupt.",
                target.display(),
                s.describe()
            ),
        }),
    }
}

/// The registry's TLC engine (its probe drives both the before/after detection here).
fn tlc_engine() -> engine::Engine {
    engine::registry()
        .into_iter()
        .find(|e| e.name == "TLC (TLA+)")
        .expect("TLC is a registered engine")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: REQ046 — the install decision is exhaustive and pure over its three inputs.
    #[test]
    fn decide_covers_every_state() {
        let missing = EngineStatus::Missing;
        let available = EngineStatus::Available {
            version: "1.7.4".into(),
        };

        // Already present short-circuits regardless of java/consent.
        assert_eq!(
            decide_tlc(&available, false, false),
            InstallDecision::AlreadyPresent
        );
        // Missing + no java → blocked, before consent is even considered.
        assert_eq!(
            decide_tlc(&missing, false, true),
            InstallDecision::BlockedNoJava
        );
        // Missing + java + no consent → prompt.
        assert_eq!(
            decide_tlc(&missing, true, false),
            InstallDecision::NeedsConsent
        );
        // Missing + java + consent → go.
        assert_eq!(decide_tlc(&missing, true, true), InstallDecision::Proceed);
    }

    // Verifies: REQ046 — the provisioned jar is anchored under the data dir, so the installer's
    // write target and the detector's fallback (`tlc::jar_path`) resolve to the same location.
    #[test]
    fn jar_lives_under_the_data_dir() {
        assert!(tla2tools_jar_default().starts_with(data_dir()));
        assert!(tla2tools_jar_default().ends_with("tlaplus/tla2tools.jar"));
    }

    // Verifies: REQ046 — a failure is the only outcome that makes provreq exit non-zero; consent
    // prompts and honest degradations are expected states, not errors.
    #[test]
    fn only_failure_is_a_failure() {
        assert!(InstallOutcome::Failed { reason: "x".into() }.is_failure());
        assert!(!InstallOutcome::AlreadyPresent.is_failure());
        assert!(!InstallOutcome::Blocked { reason: "x".into() }.is_failure());
        assert!(!InstallOutcome::NeedsConsent { plan: "x".into() }.is_failure());
        assert!(!InstallOutcome::Unsupported { reason: "x".into() }.is_failure());
    }
}
