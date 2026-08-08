//! The environment a verdict was **produced in** — the fifth drift axis (REQ049).
//!
//! Deliberately *not* [`crate::buildenv`]. That module reports what the subject **offers**; this
//! one records where a verdict was actually **proved**. They diverge whenever provreq runs on the
//! host against a subject that ships a dev-container it never used, and recording the declared
//! environment as if it were the producing one would be exactly the sort of dishonest provenance
//! this tool exists to prevent.
//!
//! **What is honestly knowable, measured rather than assumed.** Probing the engines in this repo's
//! own devcontainer: Kani reports `0.67.0` and TLC `2.19`, but `cargo-creusot --version` fails
//! (its error is about the current directory, not about Creusot) and `cargo-prusti --version`
//! cannot load its shared libraries. So an engine-version fingerprint alone is **half-blind on
//! exactly the two heavy deductive verifiers whose environment matters most** — and silently so,
//! since `unknown` looks like a recorded value. Hence three parts, each honest about its source:
//!
//! 1. **Declared** — an `environment:` label in provreq.yml. The operator is the only one who
//!    knows whether a rebuilt container is "the same environment"; the container's own hostname is
//!    an instance id that changes on every rebuild from the same image tag.
//! 2. **Detected engine versions**, for the engines that genuinely report one.
//! 3. **A coarse container marker** — context only, never compared.
//!
//! The image digest a container was built from is not reliably readable from inside it, so
//! `proved in image X@sha…` is a claim this module will not make.

use crate::engine::{self, EngineStatus};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The version string [`crate::engine`] records when a probe ran but printed nothing parseable.
const UNKNOWN_VERSION: &str = "unknown";

/// The environment a verdict was produced in, as far as it can be honestly established.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvingEnv {
    /// The operator's own label for this environment, from provreq.yml. `None` when unconfigured —
    /// recorded as *declared*, never as detected.
    #[serde(default)]
    pub declared: Option<String>,
    /// `"<engine> <version>"` for every engine that reported a real version. The compared part.
    #[serde(default)]
    pub engines: Vec<String>,
    /// Engines that are present but report no parseable version. Recorded so the record's own
    /// blind spot is visible, and **excluded from the comparison**: Creusot's `--version` depends
    /// on the current directory, so comparing it would flap a verdict between fresh and stale for
    /// a reason that has nothing to do with the subject.
    #[serde(default)]
    pub unversioned: Vec<String>,
    /// Whether provreq ran inside a container. Context for a human reading the record, never an
    /// input to drift — it says nothing about *which* container.
    #[serde(default)]
    pub container: bool,
}

impl ProvingEnv {
    /// Probe the current environment: every registered engine, plus the declared label.
    pub fn current(companion_root: &Path) -> Self {
        let statuses: Vec<(&str, EngineStatus)> = engine::registry()
            .iter()
            .map(|e| (e.name, engine::detect(e, Some(companion_root))))
            .collect();
        Self::from_statuses(declared_label(companion_root), in_container(), &statuses)
    }

    /// Build the record from already-probed statuses, so a caller that has just detected every
    /// engine (`provreq engines`) does not spawn every probe a second time.
    pub fn from_statuses(
        declared: Option<String>,
        container: bool,
        statuses: &[(&str, EngineStatus)],
    ) -> Self {
        let mut engines = Vec::new();
        let mut unversioned = Vec::new();
        for (name, status) in statuses {
            if let EngineStatus::Available { version } = status {
                if version == UNKNOWN_VERSION {
                    unversioned.push((*name).to_string());
                } else {
                    engines.push(format!("{name} {version}"));
                }
            }
        }
        // Sorted so the record is a stable identity rather than an artifact of registry order.
        engines.sort();
        unversioned.sort();
        Self {
            declared,
            engines,
            unversioned,
            container,
        }
    }

    /// Why this environment differs from `other`, or `None` when the compared parts match.
    /// Only the declared label and the versioned engines are compared — see [`ProvingEnv`].
    pub fn drift_from(&self, other: &ProvingEnv) -> Option<String> {
        if self.declared != other.declared {
            return Some(format!(
                "the declared environment changed since this verdict ({} → {}) — re-verify",
                label_or_none(&self.declared),
                label_or_none(&other.declared)
            ));
        }
        if self.engines != other.engines {
            return Some(format!(
                "the verification environment changed since this verdict ({} → {}) — re-verify",
                engine_list(&self.engines),
                engine_list(&other.engines)
            ));
        }
        None
    }

    /// One operator-facing line, including what the record cannot see. The blind spot is stated
    /// rather than hidden: an engine with no detectable version cannot be caught upgrading.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(declared) = &self.declared {
            parts.push(format!("declared `{declared}`"));
        }
        parts.push(engine_list(&self.engines));
        if !self.unversioned.is_empty() {
            parts.push(format!(
                "no detectable version from {} (so an upgrade there cannot drift a verdict)",
                self.unversioned.join(", ")
            ));
        }
        if self.container {
            parts.push("in a container".to_string());
        }
        if self.declared.is_none() {
            parts.push(
                "no `environment:` label in provreq.yml — set one to make this record stable \
                 across container rebuilds"
                    .to_string(),
            );
        }
        parts.join("; ")
    }
}

/// The operator's declared label, read from the companion manifest. A manifest that is missing or
/// unparseable yields `None` rather than an error: an unlabelled environment is a weaker record,
/// never a reason to refuse a verdict.
pub fn declared_label(companion_root: &Path) -> Option<String> {
    #[derive(Deserialize)]
    struct ManifestEnv {
        #[serde(default)]
        environment: Option<String>,
    }
    let text = std::fs::read_to_string(companion_root.join(crate::adopt::MANIFEST_FILE)).ok()?;
    let manifest: ManifestEnv = serde_yaml::from_str(&text).ok()?;
    manifest
        .environment
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
}

/// Whether provreq is running inside a container. Best-effort and coarse **on purpose** — it is
/// context, not identity, so it never feeds the drift comparison.
pub fn in_container() -> bool {
    Path::new("/.dockerenv").exists() || std::env::var("REMOTE_CONTAINERS").is_ok()
}

fn label_or_none(declared: &Option<String>) -> String {
    match declared {
        Some(label) => format!("`{label}`"),
        None => "unlabelled".to_string(),
    }
}

fn engine_list(engines: &[String]) -> String {
    if engines.is_empty() {
        "no engine reported a version".to_string()
    } else {
        engines.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available(version: &str) -> EngineStatus {
        EngineStatus::Available {
            version: version.to_string(),
        }
    }

    fn env_with(declared: Option<&str>, statuses: &[(&str, EngineStatus)]) -> ProvingEnv {
        ProvingEnv::from_statuses(declared.map(str::to_string), false, statuses)
    }

    // Verifies: REQ049 — an engine that reports no version is recorded as a blind spot rather than
    // folded in as if `unknown` were a version. This is the whole reason the axis is not a naive
    // engine fingerprint: Creusot and Prusti report nothing parseable.
    #[test]
    fn unversioned_engines_are_recorded_separately_not_as_a_version() {
        let env = env_with(
            None,
            &[
                ("Kani", available("0.67.0")),
                ("Creusot", available(UNKNOWN_VERSION)),
                ("Prusti", available(UNKNOWN_VERSION)),
                ("TLC (TLA+)", available("2.19")),
                ("MonPoly", EngineStatus::NotWired),
            ],
        );

        assert_eq!(env.engines, vec!["Kani 0.67.0", "TLC (TLA+) 2.19"]);
        assert_eq!(env.unversioned, vec!["Creusot", "Prusti"]);
        assert!(
            !env.engines.iter().any(|e| e.contains(UNKNOWN_VERSION)),
            "`unknown` must never masquerade as a recorded version: {:?}",
            env.engines
        );
    }

    // Verifies: REQ051 — an engine that cannot start contributes nothing to the record. It is
    // neither a recorded version nor a "present but unversioned" blind spot: it did not prove
    // anything, so a verdict must not name it as part of where it was proved.
    #[test]
    fn an_engine_that_cannot_start_is_not_part_of_the_proving_environment() {
        let env = env_with(
            None,
            &[
                ("Kani", available("0.67.0")),
                (
                    "Prusti",
                    EngineStatus::Unusable {
                        reason: "error while loading shared libraries".to_string(),
                    },
                ),
            ],
        );
        assert_eq!(env.engines, vec!["Kani 0.67.0"]);
        assert!(env.unversioned.is_empty(), "{:?}", env.unversioned);
        assert!(!env.describe().contains("Prusti"), "{}", env.describe());
    }

    // Verifies: REQ049 — an unversioned engine cannot drift a verdict. Creusot's `--version`
    // depends on the current directory, so comparing it would flap a verdict between fresh and
    // stale for a reason that has nothing to do with the subject.
    #[test]
    fn an_engine_with_no_version_never_drifts_a_verdict() {
        let was = env_with(
            None,
            &[
                ("Kani", available("0.67.0")),
                ("Creusot", available(UNKNOWN_VERSION)),
            ],
        );
        // Creusot vanished from the record entirely; Kani is unchanged.
        let now = env_with(None, &[("Kani", available("0.67.0"))]);
        assert_eq!(was.drift_from(&now), None, "was {was:?} now {now:?}");
    }

    // Verifies: REQ049 — a real engine upgrade IS drift, and the reason names the change.
    #[test]
    fn an_engine_version_change_is_named_drift() {
        let was = env_with(None, &[("Kani", available("0.67.0"))]);
        let now = env_with(None, &[("Kani", available("0.68.0"))]);
        let reason = was.drift_from(&now).expect("an upgrade is drift");
        assert!(reason.contains("0.67.0"), "{reason}");
        assert!(reason.contains("0.68.0"), "{reason}");
    }

    // Verifies: REQ049 — the operator's own label is the stable identity, so changing it drifts
    // even when every detected engine is identical (a different machine, same toolchain).
    #[test]
    fn the_declared_label_drifts_independently_of_the_engines() {
        let engines = [("Kani", available("0.67.0"))];
        let lab = env_with(Some("lab-1"), &engines);
        let ci = env_with(Some("ci-runner"), &engines);

        let reason = lab
            .drift_from(&ci)
            .expect("a different environment is drift");
        assert!(reason.contains("lab-1"), "{reason}");
        assert!(reason.contains("ci-runner"), "{reason}");
        assert_eq!(lab.drift_from(&lab.clone()), None);

        // Gaining a label where there was none is also drift: the operator has just told us
        // something about this environment that the record did not carry before.
        let unlabelled = env_with(None, &engines);
        assert!(unlabelled.drift_from(&lab).is_some());
    }

    // Verifies: REQ049 — the container marker is context, never an axis. A verdict must not go
    // stale merely because the same tool ran on the host instead of in a container.
    #[test]
    fn the_container_marker_is_context_not_drift() {
        let statuses = [("Kani", available("0.67.0"))];
        let on_host = ProvingEnv::from_statuses(None, false, &statuses);
        let in_box = ProvingEnv::from_statuses(None, true, &statuses);
        assert_eq!(on_host.drift_from(&in_box), None);
    }

    // Verifies: REQ049 — the record states its own blind spot instead of hiding it, and nudges an
    // unlabelled environment toward the one thing that would make it stable.
    #[test]
    fn describe_states_the_blind_spot_and_the_missing_label() {
        let env = env_with(
            None,
            &[
                ("Kani", available("0.67.0")),
                ("Creusot", available(UNKNOWN_VERSION)),
            ],
        );
        let line = env.describe();
        assert!(line.contains("Kani 0.67.0"), "{line}");
        assert!(
            line.contains("no detectable version from Creusot"),
            "{line}"
        );
        assert!(line.contains("`environment:` label"), "{line}");

        let labelled = env_with(Some("lab-1"), &[("Kani", available("0.67.0"))]);
        let line = labelled.describe();
        assert!(line.contains("declared `lab-1`"), "{line}");
        assert!(
            !line.contains("`environment:` label"),
            "a labelled environment needs no nudge: {line}"
        );
    }

    // Verifies: REQ049 — the declared label comes from the manifest, and a missing or broken
    // manifest weakens the record rather than failing the verdict.
    #[test]
    fn the_declared_label_is_read_from_the_manifest_and_never_fails() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(declared_label(tmp.path()), None, "no manifest at all");

        std::fs::write(
            tmp.path().join(crate::adopt::MANIFEST_FILE),
            "schema: 1\nenvironment: lab-1\n",
        )
        .expect("write");
        assert_eq!(declared_label(tmp.path()), Some("lab-1".to_string()));

        std::fs::write(
            tmp.path().join(crate::adopt::MANIFEST_FILE),
            "schema: 1\nenvironment: '   '\n",
        )
        .expect("write");
        assert_eq!(declared_label(tmp.path()), None, "blank is not a label");

        std::fs::write(tmp.path().join(crate::adopt::MANIFEST_FILE), "{{{ not yaml")
            .expect("write");
        assert_eq!(
            declared_label(tmp.path()),
            None,
            "broken manifest, no panic"
        );
    }
}
