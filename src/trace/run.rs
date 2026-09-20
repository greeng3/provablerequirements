//! Run a human-tagged test and rate the result as asserted [`Evidence`] (Phase 4b, REQ076).
//!
//! A `Verifies:` tag resolved by [`super::resolve`] names a test the operator vouches for.
//! provreq runs it and records what it saw — the test ran and passed (`not-falsified`,
//! **asserted**), ran and failed (a refutation), or did not run / did not compile / errored
//! (`inconclusive`). A pass is never inferred from an exit code: `cargo test <name>` exits 0
//! even when the name matches nothing, so the rating parses the run's own summary — the cat-2b
//! lesson (parse the output, check what actually ran).
//!
//! The rating ([`rate`]) is the pure, unit-tested core; [`run_test`] is the thin `Command`
//! wrapper around it. The strength ceiling is `not-falsified`: a tagged test that passes is a
//! statement about what ran, and the asserted marker keeps it from ever reading as a mechanical
//! proof (epic decision 1). Stronger asserted rungs (a tagged Kani proof → model-checked, a
//! tagged contract → proven) are a later slice — nothing in the tree tags them yet.
//!
//! Implements: REQ076

use crate::verdict::{Evidence, SourceLocation};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// Per-subject configuration for the asserted `cargo test` route, read from the `verify.cargo`
/// block of the companion `provreq.yml` (#475). It exists because a subject's tests can need
/// build inputs provreq cannot guess — a cargo feature that gates test-only helpers, a release
/// profile, an env var. Without it a bare `cargo test <name>` fails to compile and the honest
/// verdict is a permanent `unknown`, even though the test passes under the subject's own command.
///
/// Every field defaults to empty/false, which is a real fallback and not a placeholder: an
/// unconfigured subject runs exactly the bare `cargo test <name>` provreq shipped with, with no
/// features, no extra args, and no env — the behaviour before this block existed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CargoTestConfig {
    /// `--features a,b` — the cargo features to enable (compile and run both, since `cargo test`
    /// takes features once for the whole build).
    pub features: Vec<String>,
    /// `--all-features`.
    pub all_features: bool,
    /// `--no-default-features`.
    pub no_default_features: bool,
    /// Arbitrary passthrough before the test name, e.g. `["--release"]`.
    pub extra_args: Vec<String>,
    /// Args after `--`, handed to the test harness rather than to cargo.
    pub harness_args: Vec<String>,
    /// Environment variables set on the `cargo test` process.
    pub env: BTreeMap<String, String>,
}

impl CargoTestConfig {
    /// Read the `verify.cargo` block from the companion manifest. A manifest that is missing,
    /// unparseable, or silent on it yields the defaults — a subject that never configured it is
    /// the normal case, not an error (the same tolerance [`crate::kani::Bounds::load`] takes).
    pub fn load(companion_root: &Path) -> CargoTestConfig {
        #[derive(serde::Deserialize)]
        struct Manifest {
            #[serde(default)]
            verify: Option<VerifyBlock>,
        }
        #[derive(serde::Deserialize)]
        struct VerifyBlock {
            #[serde(default)]
            cargo: Option<Cargo_>,
        }
        #[derive(serde::Deserialize)]
        struct Cargo_ {
            #[serde(default)]
            features: Vec<String>,
            #[serde(default)]
            all_features: bool,
            #[serde(default)]
            no_default_features: bool,
            #[serde(default)]
            extra_args: Vec<String>,
            #[serde(default)]
            harness_args: Vec<String>,
            #[serde(default)]
            env: BTreeMap<String, String>,
        }
        let Ok(text) = std::fs::read_to_string(companion_root.join(crate::adopt::MANIFEST_FILE))
        else {
            return CargoTestConfig::default();
        };
        let Ok(manifest) = serde_yaml::from_str::<Manifest>(&text) else {
            return CargoTestConfig::default();
        };
        manifest
            .verify
            .and_then(|v| v.cargo)
            .map(|c| CargoTestConfig {
                features: c.features,
                all_features: c.all_features,
                no_default_features: c.no_default_features,
                extra_args: c.extra_args,
                harness_args: c.harness_args,
                env: c.env,
            })
            .unwrap_or_default()
    }

    /// Whether the subject configured anything — an unconfigured subject stays on the exact bare
    /// command, and its provenance stays byte-for-byte what it was before this block existed.
    fn is_configured(&self) -> bool {
        *self != CargoTestConfig::default()
    }

    /// The cargo-test arguments that go *before* the test name. Empty when nothing is configured.
    fn cargo_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if self.all_features {
            args.push("--all-features".to_string());
        }
        if self.no_default_features {
            args.push("--no-default-features".to_string());
        }
        if !self.features.is_empty() {
            args.push("--features".to_string());
            args.push(self.features.join(","));
        }
        args.extend(self.extra_args.iter().cloned());
        args
    }

    /// The exact command this config runs for `test_name`, recorded in provenance so an asserted
    /// verdict is reproducible. Shell-ish, for reading — not meant to be re-parsed.
    fn invocation(&self, test_name: &str) -> String {
        let mut parts = vec!["cargo".to_string(), "test".to_string()];
        parts.extend(self.cargo_args());
        parts.push(test_name.to_string());
        if !self.harness_args.is_empty() {
            parts.push("--".to_string());
            parts.extend(self.harness_args.iter().cloned());
        }
        let env_prefix = if self.env.is_empty() {
            String::new()
        } else {
            let kv: Vec<String> = self
                .env
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            format!("{} ", kv.join(" "))
        };
        format!("{env_prefix}{}", parts.join(" "))
    }
}

/// What running a tagged test showed. Deliberately three-valued like the verdict it becomes: a
/// pass is `not-falsified` (asserted), a failure is a refutation, and anything that did not
/// actually run and pass is `inconclusive` — never a pass by default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestOutcome {
    Passed,
    Failed { detail: Vec<String> },
    Inconclusive { detail: Vec<String> },
}

/// Rate a `cargo test` run from its output. Pure — [`run_test`] feeds it the real process
/// result, and the tests feed it captured samples.
///
/// The exit code alone is not trusted: `cargo test <name>` exits 0 when the name matched nothing
/// (`0 passed`), which is not a pass. So the rating reads the `test result:` summary lines and
/// sums what ran across every test binary:
/// - any failures → `Failed` (the requirement's tagged check refuted it);
/// - else at least one passed → `Passed`;
/// - else nothing ran (0 passed, 0 failed) → `Inconclusive` — the tag names no test that ran;
/// - no summary line at all → `Inconclusive`, carrying the error output (a compile failure, a
///   cargo error): the harness did not run, which is not the answer being no.
pub fn rate(exit_success: bool, stdout: &str, stderr: &str, test_name: &str) -> TestOutcome {
    let summaries: Vec<(u64, u64)> = stdout
        .lines()
        .filter(|l| l.contains("test result:"))
        .map(summary_counts)
        .collect();

    if summaries.is_empty() {
        // No test binary reported a result — a compile error or a cargo error, not an answer.
        let mut detail = vec![format!(
            "`cargo test {test_name}` produced no test result (the harness did not run)"
        )];
        detail.extend(significant_lines(stderr));
        return TestOutcome::Inconclusive { detail };
    }

    let passed: u64 = summaries.iter().map(|(p, _)| p).sum();
    let failed: u64 = summaries.iter().map(|(_, f)| f).sum();

    if failed > 0 {
        return TestOutcome::Failed {
            detail: failure_lines(stdout),
        };
    }
    if passed >= 1 {
        return TestOutcome::Passed;
    }
    // Every binary reported `0 passed; 0 failed`: the name matched no test that ran. `cargo test`
    // reports this with exit success, which is exactly why the count, not the code, decides.
    let _ = exit_success;
    TestOutcome::Inconclusive {
        detail: vec![format!(
            "no test named `{test_name}` ran (the tag names no runnable test)"
        )],
    }
}

/// The `(passed, failed)` counts on a `test result:` line — `test result: ok. 3 passed; 0
/// failed; …`. The count is the whitespace token immediately before `passed`/`failed`.
fn summary_counts(line: &str) -> (u64, u64) {
    let toks: Vec<&str> = line.split_whitespace().collect();
    let count_before = |label: &str| -> u64 {
        toks.iter()
            .position(|t| t.starts_with(label))
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| toks.get(i))
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0)
    };
    (count_before("passed"), count_before("failed"))
}

/// The lines of a failing run worth showing — the per-test `… FAILED` markers and any panic
/// message — capped so a large run does not flood the verdict.
fn failure_lines(stdout: &str) -> Vec<String> {
    let mut out: Vec<String> = stdout
        .lines()
        .filter(|l| l.contains("FAILED") || l.contains("panicked"))
        .map(|l| l.trim().to_string())
        .take(10)
        .collect();
    if out.is_empty() {
        out.push("the tagged test failed".to_string());
    }
    out
}

/// The first few non-empty lines of stderr — enough to name a compile or cargo error without
/// pasting the whole build log.
fn significant_lines(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(5)
        .map(str::to_string)
        .collect()
}

/// Run the tagged test `test_name` in `subject` and rate it. `cargo test <name>` substring-matches
/// the test path; a resolved leaf name is normally unique, and the rating tolerates extra passing
/// matches (they include the target). `// ponytail: substring match; exact path if a collision
/// ever mis-attributes a run.`
pub fn run_test(subject: &Path, test_name: &str, cfg: &CargoTestConfig) -> TestOutcome {
    let mut command = Command::new("cargo");
    command.arg("test").args(cfg.cargo_args()).arg(test_name);
    if !cfg.harness_args.is_empty() {
        command.arg("--").args(&cfg.harness_args);
    }
    for (key, value) in &cfg.env {
        command.env(key, value);
    }
    command.current_dir(subject);
    match command.output() {
        Ok(out) => rate(
            out.status.success(),
            &String::from_utf8_lossy(&out.stdout),
            &String::from_utf8_lossy(&out.stderr),
            test_name,
        ),
        Err(err) => TestOutcome::Inconclusive {
            detail: vec![format!("could not run `{}`: {err}", cfg.invocation(test_name))],
        },
    }
}

impl TestOutcome {
    /// Map the outcome onto an asserted [`Evidence`] stamped with the tagged source. The polarity
    /// runs through the ordinary ladder constructor — a passing tagged test earns `not-falsified`
    /// and no stronger rung — then [`Evidence::asserted_at`] marks it asserted and attaches the
    /// location, so the honest marker travels with it wherever the verdict is shown.
    pub fn into_evidence(self, location: SourceLocation) -> Evidence {
        let over = location
            .symbol
            .clone()
            .unwrap_or_else(|| "a tagged test".to_string());
        match self {
            TestOutcome::Passed => {
                Evidence::not_falsified("cargo test", format!("the tagged test `{over}` passed"))
                    .asserted_at(location)
            }
            TestOutcome::Failed { detail } => {
                Evidence::fails("cargo test", None, detail).asserted_at(location)
            }
            TestOutcome::Inconclusive { detail } => {
                Evidence::inconclusive("cargo test", detail).asserted_at(location)
            }
        }
    }
}

/// Run the test a resolved `Verifies:` tag names and rate it as asserted [`Evidence`]. A tag that
/// resolved to no symbol, or one in a language provreq has no runner for yet, is honestly
/// `inconclusive` — never a pass. Rust-only for now (the run command is a per-language concern,
/// like the resolver's declaration table).
pub fn evidence_for(subject: &Path, tag: &super::Tag, cfg: &CargoTestConfig) -> Evidence {
    let location = SourceLocation {
        file: tag.file.clone(),
        line: tag.line,
        symbol: tag.symbol.clone(),
    };
    let file_name = tag
        .file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let is_rust = super::languages::language_for(file_name).is_some_and(|l| l.name == "Rust");
    if !is_rust {
        return TestOutcome::Inconclusive {
            detail: vec!["no test runner for this language yet".to_string()],
        }
        .into_evidence(location);
    }
    let Some(symbol) = &tag.symbol else {
        return TestOutcome::Inconclusive {
            detail: vec!["the tag resolved to no runnable symbol".to_string()],
        }
        .into_evidence(location);
    };
    let mut evidence = run_test(subject, symbol, cfg).into_evidence(location);
    // Record the exact command a configured run used, so the asserted verdict is reproducible.
    // An unconfigured subject adds nothing, keeping its provenance identical to before #475.
    if cfg.is_configured() {
        evidence.detail.push(format!("ran: {}", cfg.invocation(symbol)));
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::{Basis, Correspondence, Status};
    use std::path::PathBuf;

    fn loc() -> SourceLocation {
        SourceLocation {
            file: PathBuf::from("src/a.rs"),
            line: 10,
            symbol: Some("the_test".to_string()),
        }
    }

    // The whole point of #475: features must reach the cargo invocation so a test gated behind a
    // feature can compile and run, instead of parking on a permanent unknown.
    #[test]
    fn features_and_flags_become_cargo_args() {
        let cfg = CargoTestConfig {
            features: vec!["test-helpers".into(), "extra".into()],
            no_default_features: true,
            extra_args: vec!["--release".into()],
            ..Default::default()
        };
        assert_eq!(
            cfg.cargo_args(),
            vec![
                "--no-default-features",
                "--features",
                "test-helpers,extra",
                "--release",
            ]
        );
    }

    #[test]
    fn unconfigured_config_adds_nothing() {
        let cfg = CargoTestConfig::default();
        assert!(cfg.cargo_args().is_empty());
        assert!(!cfg.is_configured());
        // The bare invocation is exactly what provreq ran before this block existed.
        assert_eq!(cfg.invocation("t_conflict"), "cargo test t_conflict");
    }

    #[test]
    fn invocation_records_env_features_and_harness_args() {
        let mut env = BTreeMap::new();
        env.insert("RUST_LOG".to_string(), "debug".to_string());
        let cfg = CargoTestConfig {
            features: vec!["test-helpers".into()],
            harness_args: vec!["--nocapture".into()],
            env,
            ..Default::default()
        };
        assert!(cfg.is_configured());
        assert_eq!(
            cfg.invocation("t_conflict"),
            "RUST_LOG=debug cargo test --features test-helpers t_conflict -- --nocapture"
        );
    }

    #[test]
    fn loads_verify_cargo_block_from_manifest() {
        let dir = std::env::temp_dir().join(format!("provreq-cargocfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(crate::adopt::MANIFEST_FILE),
            "verify:\n  cargo:\n    features: [test-helpers]\n    extra_args: [--release]\n",
        )
        .unwrap();
        let cfg = CargoTestConfig::load(&dir);
        assert_eq!(cfg.features, vec!["test-helpers".to_string()]);
        assert_eq!(cfg.extra_args, vec!["--release".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn absent_manifest_or_block_is_default() {
        let dir = std::env::temp_dir().join(format!("provreq-nocfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // No manifest at all.
        assert_eq!(CargoTestConfig::load(&dir), CargoTestConfig::default());
        // A manifest with no verify block.
        std::fs::write(dir.join(crate::adopt::MANIFEST_FILE), "documents: {}\n").unwrap();
        assert_eq!(CargoTestConfig::load(&dir), CargoTestConfig::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    // A named test that ran and passed → Passed. The count decides, not the exit code.
    #[test]
    fn a_passing_run_rates_passed() {
        let stdout = "running 1 test\ntest a::the_test ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out\n";
        assert_eq!(rate(true, stdout, "", "the_test"), TestOutcome::Passed);
    }

    // A named test that ran and failed → Failed, carrying the failure lines.
    #[test]
    fn a_failing_run_rates_failed() {
        let stdout = "running 1 test\ntest a::the_test ... FAILED\n\nfailures:\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 42 filtered out\n";
        match rate(false, stdout, "", "the_test") {
            TestOutcome::Failed { detail } => {
                assert!(detail.iter().any(|l| l.contains("FAILED")), "{detail:?}")
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    // The cat-2b trap: `cargo test <name>` exits 0 when the name matched nothing. `0 passed`
    // must rate Inconclusive, never Passed.
    #[test]
    fn a_name_that_matched_nothing_rates_inconclusive_despite_exit_zero() {
        let stdout =
            "\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 51 filtered out\n";
        match rate(true, stdout, "", "nonexistent") {
            TestOutcome::Inconclusive { detail } => {
                assert!(
                    detail.iter().any(|l| l.contains("nonexistent")),
                    "{detail:?}"
                )
            }
            other => panic!("expected Inconclusive, got {other:?}"),
        }
    }

    // A compile failure produces no `test result:` line → Inconclusive, carrying the error.
    #[test]
    fn a_compile_failure_rates_inconclusive_with_the_error() {
        let stderr = "error[E0425]: cannot find value `x` in this scope\n  --> src/a.rs:3:5\n";
        match rate(false, "", stderr, "the_test") {
            TestOutcome::Inconclusive { detail } => {
                assert!(detail.iter().any(|l| l.contains("E0425")), "{detail:?}")
            }
            other => panic!("expected Inconclusive, got {other:?}"),
        }
    }

    // Counts sum across multiple test binaries; one binary's failure fails the whole rating.
    #[test]
    fn counts_sum_across_binaries() {
        let stdout = "test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n";
        assert!(matches!(
            rate(false, stdout, "", "t"),
            TestOutcome::Failed { .. }
        ));
    }

    // A passing tagged test becomes not-falsified evidence that is ASSERTED and located — never
    // a mechanical `holds`, and never a rung above not-falsified.
    #[test]
    fn passed_maps_to_asserted_not_falsified_evidence_with_location() {
        let e = TestOutcome::Passed.into_evidence(loc());
        assert_eq!(e.status, Status::Holds);
        assert_eq!(e.basis, Some(Basis::NotFalsified));
        assert_eq!(e.correspondence, Correspondence::Asserted);
        assert_eq!(e.source_location.as_ref().map(|l| l.line), Some(10));
        assert_eq!(
            e.source_location.and_then(|l| l.symbol).as_deref(),
            Some("the_test")
        );
    }

    // A failing tagged test refutes, and the refutation is still marked asserted + located.
    #[test]
    fn failed_maps_to_asserted_fails() {
        let e = TestOutcome::Failed {
            detail: vec!["boom".into()],
        }
        .into_evidence(loc());
        assert_eq!(e.status, Status::Fails);
        assert_eq!(e.correspondence, Correspondence::Asserted);
    }

    // The honest core: a tagged pass is asserted, and an engine's own evidence stays mechanical
    // by default — the two can never be confused.
    #[test]
    fn engine_evidence_defaults_to_mechanical() {
        assert_eq!(
            Evidence::holds("Creusot", Basis::Proven).correspondence,
            Correspondence::Mechanical
        );
    }
}
