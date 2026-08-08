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
//! Implements: REQ046 (consent-gated native install of a light-tier engine; honest degradation),
//! REQ047 (platform support is a first-class gate; the decision ladder is one shared rule).

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

/// The pure install decision: what an install attempt should do, given what is already detected,
/// whether this platform is supported at all, whether the engine's prerequisite is present, and
/// whether the operator has consented. No IO, so it is exhaustively testable — each engine's
/// `install_*` wrapper supplies the real detections and performs the install only for `Proceed`.
///
/// One ladder for every light-tier engine (REQ047), so they all degrade the same way; only the
/// *inputs* and the *messages* differ per engine (TLC needs a JVM everywhere; Kani needs cargo and
/// has no Windows upstream).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallDecision {
    /// The engine already detects as available — nothing to do.
    AlreadyPresent,
    /// The engine has no upstream support on this platform. Installing cannot succeed, so saying
    /// so is the honest outcome — not a half-install to discover the same thing slowly.
    UnsupportedPlatform,
    /// The engine needs a prerequisite provreq does not install on the operator's behalf (a system
    /// JVM for TLC, a Rust toolchain for Kani). An honest degradation, not an attempt.
    BlockedPrereq,
    /// Everything is ready to install, but the operator has not consented yet — show the plan.
    NeedsConsent,
    /// Consent given, platform supported, prerequisites met — perform the install.
    Proceed,
}

/// Decide what an install attempt should do. Pure over the four inputs; order matters — a platform
/// that cannot host the engine outranks a missing prerequisite, which outranks the consent gate.
pub fn decide_install(
    detected: &EngineStatus,
    platform_supported: bool,
    prereq_present: bool,
    consent: bool,
) -> InstallDecision {
    if detected.is_ready() {
        return InstallDecision::AlreadyPresent;
    }
    if !platform_supported {
        return InstallDecision::UnsupportedPlatform;
    }
    if !prereq_present {
        return InstallDecision::BlockedPrereq;
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

/// The `provreq install` argument for a registry engine, or `None` when provreq has no native
/// recipe for it. This is the light/heavy tier line from the Design-C decision, in one place: the
/// heavy tier is dev-container-first **by decision**, so a missing heavy engine can only be
/// explained in terms of the subject's build environment ([`crate::buildenv`], REQ048).
pub fn native_install_arg(engine_name: &str) -> Option<&'static str> {
    match engine_name {
        "TLC (TLA+)" => Some("tlc"),
        "Kani" => Some("kani"),
        _ => None,
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
    let before = engine::detect(&tlc, None);
    // TLC runs anywhere a JVM does, so the platform gate is always open for it.
    match decide_install(&before, true, java_present(), consent) {
        InstallDecision::AlreadyPresent => Ok(InstallOutcome::AlreadyPresent),
        InstallDecision::UnsupportedPlatform => Ok(InstallOutcome::Unsupported {
            reason: "TLC is supported wherever a JVM runs".to_string(),
        }),
        InstallDecision::BlockedPrereq => Ok(InstallOutcome::Blocked {
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
    match engine::detect(&tlc_engine(), None) {
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

/// The two commands that provision Kani, in order. `cargo install` puts `cargo-kani` on PATH (the
/// binary [`engine::registry`] probes); `cargo kani setup` then fetches the solver backend (CBMC
/// and friends) that Kani cannot run without.
pub const KANI_COMMANDS: [&[&str]; 2] = [
    &["cargo", "install", "--locked", "kani-verifier"],
    &["cargo", "kani", "setup"],
];

/// Whether Kani's upstream supports this platform. Kani ships Linux and macOS only — there is no
/// upstream Windows build, so provisioning there is an honest "unavailable", not a failed attempt
/// (REQ047). Keep this in step with the Design-C ADR's platform-reach table.
pub fn kani_platform_supported() -> bool {
    kani_supports(std::env::consts::OS)
}

/// The platform rule itself, over an OS name as [`std::env::consts::OS`] spells it.
pub fn kani_supports(os: &str) -> bool {
    matches!(os, "linux" | "macos")
}

/// Whether a Rust toolchain is on PATH. Kani installs *through* cargo, so no cargo means no Kani
/// however the install is driven — and provreq does not install rustup on the operator's behalf.
pub fn cargo_present() -> bool {
    Command::new("cargo")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Where `cargo install` puts a binary: `CARGO_HOME/bin`, else `~/.cargo/bin`. Reported so the
/// operator knows what has to be on PATH for the engine to keep detecting.
fn cargo_bin(name: &str) -> PathBuf {
    let cargo_home = std::env::var("CARGO_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.cargo")
    });
    PathBuf::from(cargo_home).join("bin").join(name)
}

/// Install Kani natively (REQ047): detect → platform gate → cargo precheck → consent gate → run
/// [`KANI_COMMANDS`] → re-detect. Light tier #2 per the Design-C decision; the heavy tier stays
/// dev-container-first.
pub async fn install_kani(consent: bool) -> Result<InstallOutcome> {
    let before = engine::detect(&kani_engine(), None);
    match decide_install(&before, kani_platform_supported(), cargo_present(), consent) {
        InstallDecision::AlreadyPresent => Ok(InstallOutcome::AlreadyPresent),
        InstallDecision::UnsupportedPlatform => Ok(InstallOutcome::Unsupported {
            reason: format!(
                "Kani has no upstream support on {} — it ships for Linux and macOS only. Use a \
                 devcontainer for category-1 verification here (docs/design-c-decision.md).",
                std::env::consts::OS
            ),
        }),
        InstallDecision::BlockedPrereq => Ok(InstallOutcome::Blocked {
            reason: "Kani installs through cargo, which is not on PATH. provreq does not install \
                     Rust toolchains — install rustup (https://rustup.rs) and retry."
                .to_string(),
        }),
        InstallDecision::NeedsConsent => Ok(InstallOutcome::NeedsConsent {
            plan: kani_consent_plan(),
        }),
        InstallDecision::Proceed => run_kani_commands(),
    }
}

/// The consent prompt: the exact commands `--yes` would run, so the operator approves what will
/// actually execute — including that the second one downloads a solver backend.
fn kani_consent_plan() -> String {
    let commands: Vec<String> = KANI_COMMANDS.iter().map(|c| c.join(" ")).collect();
    format!(
        "would install Kani by running, in order:\n    {}\n  the second command downloads Kani's \
         solver backend (CBMC + SMT, a few hundred MB) into ~/.kani.\n  re-run with --yes to \
         install.",
        commands.join("\n    ")
    )
}

/// Run the install commands, then re-detect. Output is inherited so the operator watches cargo's
/// own progress — these take minutes, and a silent multi-minute install reads as a hang.
///
/// `// ponytail: blocking Command in an async fn — `install` is a one-shot CLI path with nothing
/// else on the runtime. Switch to tokio's process feature if this ever runs inside the server.`
fn run_kani_commands() -> Result<InstallOutcome> {
    for command in KANI_COMMANDS {
        let (bin, args) = command.split_first().expect("each command has a binary");
        let status = Command::new(bin)
            .args(args)
            .status()
            .with_context(|| format!("running `{}`", command.join(" ")))?;
        if !status.success() {
            return Ok(InstallOutcome::Failed {
                reason: format!("`{}` exited with {status}", command.join(" ")),
            });
        }
    }

    // Success means "provreq can actually run Kani", not "cargo exited 0": re-detect the probe.
    match engine::detect(&kani_engine(), None) {
        s if s.is_ready() => Ok(InstallOutcome::Installed {
            path: cargo_bin("cargo-kani"),
        }),
        s => Ok(InstallOutcome::Failed {
            reason: format!(
                "the install commands succeeded but Kani still does not detect ({}). \
                 `cargo-kani` may not be on PATH — check that ~/.cargo/bin is in it.",
                s.describe()
            ),
        }),
    }
}

/// The registry's Kani engine (its probe drives both the before/after detection here).
fn kani_engine() -> engine::Engine {
    engine::registry()
        .into_iter()
        .find(|e| e.name == "Kani")
        .expect("Kani is a registered engine")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: REQ046/REQ047 — the shared install decision is exhaustive and pure over its four
    // inputs, and the gates rank in the right order (platform > prerequisite > consent).
    #[test]
    fn decide_covers_every_state() {
        let missing = EngineStatus::Missing;
        let available = EngineStatus::Available {
            version: "1.7.4".into(),
        };

        // Already present short-circuits regardless of platform/prereq/consent.
        assert_eq!(
            decide_install(&available, false, false, false),
            InstallDecision::AlreadyPresent
        );
        // Missing + unsupported platform → unsupported, even with the prereq and consent in hand.
        assert_eq!(
            decide_install(&missing, false, true, true),
            InstallDecision::UnsupportedPlatform
        );
        // Missing + no prerequisite → blocked, before consent is even considered.
        assert_eq!(
            decide_install(&missing, true, false, true),
            InstallDecision::BlockedPrereq
        );
        // Missing + prerequisite + no consent → prompt.
        assert_eq!(
            decide_install(&missing, true, true, false),
            InstallDecision::NeedsConsent
        );
        // Missing + prerequisite + consent → go.
        assert_eq!(
            decide_install(&missing, true, true, true),
            InstallDecision::Proceed
        );
    }

    // Verifies: REQ047 — the consent plan discloses the exact commands that will run, so approving
    // it approves the real actions (including the backend download).
    #[test]
    fn kani_plan_names_the_commands_it_would_run() {
        let plan = kani_consent_plan();
        assert!(
            plan.contains("cargo install --locked kani-verifier"),
            "{plan}"
        );
        assert!(plan.contains("cargo kani setup"), "{plan}");
        assert!(plan.contains("--yes"), "{plan}");
    }

    // Verifies: REQ047 — Kani's platform gate matches its upstream reach (Linux/macOS, no Windows).
    #[test]
    fn kani_platform_gate_matches_upstream_reach() {
        for (os, supported) in [
            ("linux", true),
            ("macos", true),
            ("windows", false),
            ("freebsd", false),
        ] {
            assert_eq!(kani_supports(os), supported, "{os}");
        }
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
