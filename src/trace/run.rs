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
use std::path::Path;
use std::process::Command;

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
pub fn run_test(subject: &Path, test_name: &str) -> TestOutcome {
    let output = Command::new("cargo")
        .arg("test")
        .arg(test_name)
        .current_dir(subject)
        .output();
    match output {
        Ok(out) => rate(
            out.status.success(),
            &String::from_utf8_lossy(&out.stdout),
            &String::from_utf8_lossy(&out.stderr),
            test_name,
        ),
        Err(err) => TestOutcome::Inconclusive {
            detail: vec![format!("could not run `cargo test {test_name}`: {err}")],
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
pub fn evidence_for(subject: &Path, tag: &super::Tag) -> Evidence {
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
    run_test(subject, symbol).into_evidence(location)
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
