//! Verus — a category-1 engine, **#4: a second SMT-backed deductive verifier that earns `proven`**.
//!
//! D2 gives the core one meaning and lowers it to each engine; Verus is **lowering #4** (Kani #1,
//! Creusot #2, Prusti #3). The binding stays core-owned ([`crate::grounding`]), the claim stays the
//! shared lowering's ([`crate::lowering`]), and this module owns exactly one thing: how a gated
//! category-1 requirement becomes something Verus can prove, and what its answer means.
//!
//! **Additive, and in-crate like Prusti/Creusot.** Verus processes a crate's `verus! { }` code, so
//! the harness is a new **module** in the crate root (`#[cfg(provreq_verus)] mod provreq_…;`) plus
//! its source file. The subject's own code is never edited — only a `mod` line is appended and then
//! removed. The `#[cfg(provreq_verus)]` flag is passed to the `verus` binary as `--cfg
//! provreq_verus`; it keeps an interrupted run's leaked harness out of an ordinary `cargo build`,
//! exactly as Prusti's `#[cfg(prusti)]` does.
//!
//! **The soundness precondition.** Verus reasons only about code in its supported subset. The
//! harness calls the subject's predicates inside a `proof`-mode `assert(forall …)`, which requires
//! those predicates to be **specification functions** (`spec fn`). An ordinary `fn` is *ignored* by
//! Verus as external, and the harness that references it cannot be checked — reported as an honest
//! `inconclusive` naming the cause ("not authored in the Verus subset"), never a guess. So the
//! engine is sound in place only on a Verus-authored subject; otherwise it reports a ceiling, the
//! same honest shape the other deductive engines report theirs in.
//!
//! **Honest by construction (D8/D9) — the same two-valued shape as Prusti/Creusot.** Verus is a
//! *deductive* verifier: a discharged obligation holds for **every** execution (spec-relative), so a
//! pass is [`Basis::Proven`], the strongest rung. A *failed* obligation is NOT a counterexample —
//! there is no re-checkable witness — so Verus yields [`Outcome::Holds`] or
//! [`Outcome::Inconclusive`] and **never a `fails`**. A definitive refutation needs a witness-
//! producing engine (Kani).
//!
//! **A pass requires a check to have run.** Verus reports `N verified, M errors`. A `Holds` demands
//! **`N ≥ 1` and `M == 0`**: a run in which the `#[cfg(provreq_verus)]` module was excluded reports
//! `0 verified, 0 errors`, and reading that as a proof would be claiming a proof nothing performed.
//!
//! Implements: REQ093 (wire Verus as cat-1 engine #4 — a grounded invariant authored in the Verus
//! subset earns a real `proven` verdict; everything else is an honest `inconclusive`).

use crate::grounding::Binding;
use crate::lowering::{self, LoweredClaim};
pub use crate::lowering::{NotLowerable, harness_name};
use crate::prl::ast::Requirement;
use crate::rust_adapter::{Resolution, TypeResolution};
use crate::verdict::{Basis, Evidence};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A generated Verus proof harness. `name` is both the harness `fn` name and the module (file stem)
/// it is written to, so it cannot collide with the subject's own items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Harness {
    pub name: String,
    pub source: String,
}

/// What running Verus established. Two-valued (contrast Kani's three): a deductive prover confirms
/// or fails-to-decide, and its failure-to-decide is not a refutation — there is no counterexample to
/// carry, so there is no `Fails`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Proved — at least one obligation was discharged and none failed, so the claim holds for all
    /// executions (spec-relative). This is `proven`, the strongest D8 rung.
    Holds,
    /// Verus ran but did not prove the claim: an undischarged obligation, a predicate Verus cannot
    /// see (not authored in its subset), a harness that did not compile, or an absent toolchain.
    /// D10's `inconclusive(…)`. NOT a refutation.
    Inconclusive { reason: String },
}

impl Outcome {
    /// Map what Verus established into a piece of [`Evidence`]. The mapping lives here, in the
    /// engine, so [`crate::verdict`] never learns what Verus is. The load-bearing line is `Holds` →
    /// [`Basis::Proven`]: a deductive proof is `∀`-executions, never bounded.
    pub fn into_evidence(&self) -> Evidence {
        match self {
            Outcome::Holds => Evidence::holds("Verus", Basis::Proven),
            Outcome::Inconclusive { reason } => {
                Evidence::inconclusive("Verus", vec![reason.clone()])
            }
        }
    }
}

/// Lower a gated category-1 requirement to a Verus proof harness.
///
/// The claim itself is lowered by the shared [`crate::lowering`] core (prefix `crate`, since this
/// harness is a module *inside* the subject crate); this function owns only the Verus wrapper — a
/// `proof fn` whose body is one `assert(forall … )` per property, inside a `verus! { }` block.
pub fn lower(
    req: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
    sort_resolutions: &BTreeMap<String, TypeResolution>,
    name: &str,
) -> Result<Harness, NotLowerable> {
    if req.require.is_empty() {
        return Err(NotLowerable::new(
            "the requirement claims nothing — there is no property to check",
        ));
    }
    let mut body = String::new();
    for prop in &req.require {
        let claim =
            lowering::lower_property(req, prop, "crate", bindings, resolutions, sort_resolutions)?;
        body.push_str(&assertion(&claim));
    }
    let source = format!(
        "{marker} it is rewritten on every `verify` and removed afterwards.\n\
         //\n\
         // An ADDITIVE Verus proof harness: a module inside the subject crate that asserts the\n\
         // invariant as a universally-quantified proposition in a `verus! {{ }}` proof fn. The\n\
         // subject's own code is untouched. The `mod` line in the crate root is\n\
         // `#[cfg(provreq_verus)]`, so an\n\
         // ordinary `cargo build` never sees this file.\n\
         #![allow(unused)]\n\
         use vstd::prelude::*;\n\
         \n\
         verus! {{\n\
         \n\
         proof fn {name}() {{\n\
         {body}}}\n\
         \n\
         }} // verus!\n",
        marker = lowering::GENERATED_HARNESS_MARKER,
    );
    Ok(Harness {
        name: name.to_string(),
        source,
    })
}

/// Wrap one lowered claim as a Verus `assert`. A quantified claim becomes `assert(forall|v: ty| …)`
/// (what makes it a ∀ proof rather than a spot check); an unquantified one asserts the ground fact
/// directly.
fn assertion(claim: &LoweredClaim) -> String {
    let body = if claim.quantified.is_empty() {
        claim.claim.clone()
    } else {
        let binders = claim
            .quantified
            .iter()
            .map(|q| format!("{}: {}", q.var, q.ty))
            .collect::<Vec<_>>()
            .join(", ");
        format!("forall|{binders}| {}", claim.claim)
    };
    format!("    assert({body});\n")
}

/// The subject's crate-root source file (`src/lib.rs`, else `src/main.rs`) and whether it is a
/// library (which decides the `--crate-type=lib` flag Verus needs). `None` when the subject is not
/// a cargo crate — an honest `inconclusive`, there being no crate to attach the harness to.
fn crate_root(subject_root: &Path) -> Option<(PathBuf, bool)> {
    let lib = subject_root.join("src").join("lib.rs");
    if lib.exists() {
        return Some((lib, true));
    }
    let main = subject_root.join("src").join("main.rs");
    if main.exists() {
        return Some((main, false));
    }
    None
}

/// Write the harness into the subject as a `#[cfg(provreq_verus)]` module, run `verus`, and restore
/// the subject exactly as it was.
pub fn run(subject_root: &Path, harness: &Harness) -> Outcome {
    let Some((root_file, is_lib)) = crate_root(subject_root) else {
        return Outcome::Inconclusive {
            reason: "the subject has no src/lib.rs or src/main.rs, so Verus has no crate to \
                     attach the proof harness to"
                .to_string(),
        };
    };
    let harness_path = subject_root
        .join("src")
        .join(format!("{}.rs", harness.name));
    // Never clobber an operator's file. A provreq-generated harness left behind by an interrupted
    // run carries the marker and is reclaimable; anything else stops the run.
    if harness_path.exists() {
        let existing = std::fs::read_to_string(&harness_path).unwrap_or_default();
        if !lowering::reclaimable_harness(&existing) {
            return Outcome::Inconclusive {
                reason: format!(
                    "{} already exists — refusing to overwrite a file provreq did not write",
                    harness_path.display()
                ),
            };
        }
    }
    let original_root = match std::fs::read_to_string(&root_file) {
        Ok(s) => s,
        Err(e) => {
            return Outcome::Inconclusive {
                reason: format!("could not read {}: {e}", root_file.display()),
            };
        }
    };

    // Mutate: harness file, then the `mod` line.
    if let Err(e) = std::fs::write(&harness_path, &harness.source) {
        return Outcome::Inconclusive {
            reason: format!(
                "could not write the harness to {}: {e}",
                harness_path.display()
            ),
        };
    }
    let with_mod = format!(
        "{original_root}\n#[cfg(provreq_verus)]\nmod {};\n",
        harness.name
    );
    if let Err(e) = std::fs::write(&root_file, &with_mod) {
        let _ = std::fs::remove_file(&harness_path);
        return Outcome::Inconclusive {
            reason: format!(
                "could not attach the harness module to {}: {e}",
                root_file.display()
            ),
        };
    }

    // The `verus` binary is a rustc wrapper: point it at the crate root, pass `--crate-type=lib`
    // for a library (so the entry file is read as one), and `--cfg provreq_verus` so the gated
    // harness module is included. Without that cfg the module is excluded and Verus reports
    // `0 verified` — which `classify` refuses to read as a proof.
    let mut cmd = std::process::Command::new("verus");
    cmd.arg(&root_file);
    if is_lib {
        cmd.arg("--crate-type=lib");
    }
    cmd.arg("--cfg").arg("provreq_verus");
    cmd.current_dir(subject_root);
    let output = cmd.output();

    // Restore before interpreting anything, so an early return cannot leak the harness.
    let _ = std::fs::write(&root_file, &original_root);
    let _ = std::fs::remove_file(&harness_path);

    match output {
        Ok(o) => classify(&format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )),
        Err(e) => Outcome::Inconclusive {
            reason: format!(
                "could not run `verus` ({e}) — the Verus toolchain is not installed or not on \
                 PATH. This is a toolchain ceiling, not a missing annotation."
            ),
        },
    }
}

/// Map Verus's output to an outcome. Pure and separately tested — the mapping is where a verdict
/// could silently become dishonest, so it must be checkable without running Verus.
pub fn classify(output: &str) -> Outcome {
    // The honest ceiling first, and the most specific signal: a predicate or type Verus *ignores*
    // because it is not authored in its subset. This is not a build error the operator could
    // annotate their way out of inside the harness — the fix is on the subject's side.
    if output.contains("ignored because it is either declared outside the verus! macro") {
        return Outcome::Inconclusive {
            reason: "Verus could not see the subject's predicate — it is ignored as external \
                     because it is not authored in the Verus subset. Author the predicate as a \
                     `spec fn` inside a `verus! { }` block for Verus to reason about it."
                .to_string(),
        };
    }
    // The toolchain ceiling, named as itself: Verus is welded to a specific rust toolchain and
    // cannot run without it. No annotation fixes this.
    if (output.contains("required rust toolchain") && output.contains("not found"))
        || output.contains("rustup not found")
    {
        return Outcome::Inconclusive {
            reason: "Verus's pinned rust toolchain is not installed — a toolchain ceiling, not a \
                     missing annotation. Install the toolchain Verus names (`rustup install …`)."
                .to_string(),
        };
    }
    // The verification tally. A proof requires BOTH that something was checked (≥1 verified) and
    // that nothing failed (0 errors). `0 verified, 0 errors` is the excluded-module shape and is
    // NOT a proof; `M ≥ 1 errors` is an undischarged goal, which is inconclusive, never a `fails`.
    if let Some((verified, errors)) = parse_tally(output) {
        if errors == 0 && verified >= 1 {
            return Outcome::Holds;
        }
        if errors == 0 {
            return Outcome::Inconclusive {
                reason: "Verus verified nothing — nothing was checked (the proof harness did not \
                         run). This is not a proof."
                    .to_string(),
            };
        }
        return Outcome::Inconclusive {
            reason: "Verus could not discharge the proof obligation — the invariant may be false, \
                     or its predicates may be opaque to the prover. A deductive prover yields no \
                     counterexample, so this is inconclusive, not a refutation."
                .to_string(),
        };
    }
    // A compile or other error with no tally: name the first error line, the actionable cause.
    if let Some(line) = first_error(output) {
        return Outcome::Inconclusive {
            reason: format!("the Verus harness did not compile — {line}"),
        };
    }
    Outcome::Inconclusive {
        reason: tail(output),
    }
}

/// Parse Verus's `N verified, M errors` tally. `None` when no tally line is present (a compile
/// error or toolchain failure prints none).
fn parse_tally(output: &str) -> Option<(u32, u32)> {
    let idx = output.find(" verified, ")?;
    let verified = output[..idx].split_whitespace().last()?.parse().ok()?;
    let rest = &output[idx + " verified, ".len()..];
    let errors = rest.split_whitespace().next()?.parse().ok()?;
    Some((verified, errors))
}

/// The first compiler error line, which names the actionable cause — the top of a rustc diagnostic,
/// not the boilerplate tail.
fn first_error(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("error[") || l.starts_with("error:"))
        .map(String::from)
}

/// The last few non-empty lines of engine output — enough for the operator to see why Verus could
/// not decide, without pasting a whole log into the verdict.
fn tail(output: &str) -> String {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(TAIL_LINES);
    let tail = lines[start..].join("\n");
    if tail.trim().is_empty() {
        "`verus` produced no recognisable verdict".to_string()
    } else {
        tail
    }
}

/// How many lines of engine output an `inconclusive` carries. Enough to name a cause; short enough
/// to stay a verdict rather than a log.
const TAIL_LINES: usize = 12;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grounding::{BindCategory, Fidelity};
    use crate::prl::gate;
    use crate::rust_adapter::{CodeMatch, ParamMode, PredicateForm};
    use crate::verdict::Provenance;

    const CODE_REQ: &str = "requirement r {
        category: 1
        vocabulary { state logged_in(u), has_session(u) }
        require { each u: User . always (not logged_in(u) or has_session(u)) }
    }";

    fn req(src: &str) -> Requirement {
        gate(src)
            .expect("test candidate should clear the gate")
            .requirement
    }

    fn binding(symbol: &str, observable: &str) -> Binding {
        Binding {
            symbol: symbol.into(),
            category: BindCategory::Code,
            observable: observable.into(),
            fidelity: Fidelity::Definitional,
        }
    }

    fn sorts() -> BTreeMap<String, TypeResolution> {
        BTreeMap::from([(
            "User".to_string(),
            TypeResolution::Resolved(CodeMatch {
                file: "src/lib.rs".into(),
                line: 1,
                text: "pub struct User;".into(),
                module: Some(vec![]),
            }),
        )])
    }

    fn resolved(params: Vec<ParamMode>) -> Resolution {
        Resolution::Resolved {
            at: CodeMatch {
                file: "src/lib.rs".into(),
                line: 1,
                text: "spec fn f() -> bool { true }".into(),
                module: Some(vec![]),
            },
            params,
            form: PredicateForm::Function,
        }
    }

    fn by_value_resolutions() -> BTreeMap<String, Resolution> {
        BTreeMap::from([
            ("logged_in".to_string(), resolved(vec![ParamMode::ByValue])),
            (
                "has_session".to_string(),
                resolved(vec![ParamMode::ByValue]),
            ),
        ])
    }

    fn standard_bindings() -> Vec<Binding> {
        vec![
            binding("logged_in", "logged_in"),
            binding("has_session", "has_session"),
            binding("User", "User"),
        ]
    }

    fn lower_standard() -> Result<Harness, NotLowerable> {
        lower(
            &req(CODE_REQ),
            &standard_bindings(),
            &by_value_resolutions(),
            &sorts(),
            "provreq_req001",
        )
    }

    // Verifies: REQ093 — a quantified cat-1 invariant lowers to a `verus! { }` block containing a
    // `proof fn` whose body is `assert(forall|u: crate::User| …)`, calling the subject's predicates
    // via `crate::`.
    #[test]
    fn quantified_invariant_lowers_to_a_verus_proof_fn() {
        let h = lower_standard().expect("should lower");
        assert_eq!(h.name, "provreq_req001");
        assert!(h.source.contains("verus! {"), "{}", h.source);
        assert!(h.source.contains("use vstd::prelude::*;"), "{}", h.source);
        assert!(
            h.source.contains("proof fn provreq_req001()"),
            "{}",
            h.source
        );
        assert!(
            h.source.contains(
                "assert(forall|u: crate::User| \
                 (!(crate::logged_in(u)) || crate::has_session(u)));"
            ),
            "the claim must lower to a forall over the subject's real predicates: {}",
            h.source
        );
    }

    // Verifies: REQ093 — the harness carries the generated-file marker, so an interrupted run's
    // leaked harness can be reclaimed rather than mistaken for the operator's file.
    #[test]
    fn the_harness_carries_the_generated_marker() {
        let h = lower_standard().expect("should lower");
        assert!(lowering::reclaimable_harness(&h.source), "{}", h.source);
    }

    // Verifies: REQ093 — calls reach the subject through `crate::`, not a crate name; the harness is
    // a module inside the subject crate.
    #[test]
    fn calls_go_through_crate_not_a_crate_name() {
        let h = lower_standard().expect("should lower");
        assert!(h.source.contains("crate::logged_in"), "{}", h.source);
        assert!(!h.source.contains("subject::"), "{}", h.source);
    }

    // Verifies: REQ093 — a by-ref predicate is called with `&u`, matching the subject's signature.
    #[test]
    fn calls_follow_the_subjects_parameter_modes() {
        let by_ref = BTreeMap::from([
            ("logged_in".to_string(), resolved(vec![ParamMode::ByRef])),
            ("has_session".to_string(), resolved(vec![ParamMode::ByRef])),
        ]);
        let h = lower(&req(CODE_REQ), &standard_bindings(), &by_ref, &sorts(), "h")
            .expect("should lower");
        assert!(h.source.contains("crate::logged_in(&u)"), "{}", h.source);
    }

    // Verifies: REQ093 — `never P` is `always not P`, and an unquantified claim asserts the ground
    // fact directly with no `forall`.
    #[test]
    fn never_lowers_to_a_negated_unquantified_assertion() {
        let r = req("requirement r {
            category: 1
            vocabulary { state overdrawn }
            require { never overdrawn }
        }");
        let h = lower(
            &r,
            &[binding("overdrawn", "is_overdrawn")],
            &BTreeMap::from([("overdrawn".to_string(), resolved(vec![]))]),
            &sorts(),
            "h",
        )
        .expect("should lower");
        assert!(
            h.source.contains("assert(!(crate::is_overdrawn()));"),
            "{}",
            h.source
        );
        assert!(
            !h.source.contains("assert(forall"),
            "no quantifier in the assertion: {}",
            h.source
        );
    }

    // Verifies: REQ093 — an unbound sort cannot be quantified over, so the requirement does not
    // lower rather than silently becoming an unquantified spot check.
    #[test]
    fn unbound_sort_does_not_lower() {
        let e = lower(
            &req(CODE_REQ),
            &[
                binding("logged_in", "logged_in"),
                binding("has_session", "has_session"),
            ],
            &by_value_resolutions(),
            &sorts(),
            "h",
        )
        .expect_err("an unbound sort has no domain");
        assert!(e.reason.contains("User"), "{}", e.reason);
    }

    // Verifies: REQ093 — a temporal pattern does not lower (the gate rejects these at cat 1, but
    // `lower` is public and must not assume it was called).
    #[test]
    fn temporal_patterns_do_not_lower() {
        let r = req("requirement r {
            category: 2b
            vocabulary { state p, q }
            require { p leads_to q }
        }");
        let e = lower(&r, &[], &BTreeMap::new(), &sorts(), "h")
            .expect_err("liveness is not an invariant");
        assert!(e.reason.contains("leads_to"), "{}", e.reason);
    }

    // Verifies: REQ093 — Verus's success marker is read ONLY as `N verified, 0 errors` with N ≥ 1.
    #[test]
    fn one_verified_zero_errors_is_holds() {
        assert_eq!(
            classify("note: verifying root module\nverification results:: 1 verified, 0 errors\n"),
            Outcome::Holds
        );
    }

    // Verifies: REQ093 (the soundness crux) — `0 verified, 0 errors` is the excluded-module shape
    // (the `#[cfg(provreq_verus)]` flag did not take), and MUST NOT be read as a proof. Nothing was
    // checked, so there is nothing proved.
    #[test]
    fn zero_verified_is_never_holds() {
        let Outcome::Inconclusive { reason } =
            classify("verification results:: 0 verified, 0 errors\n")
        else {
            panic!("a run that verified nothing is not a proof");
        };
        assert!(
            reason.contains("nothing was checked") || reason.contains("no obligation"),
            "{reason}"
        );
    }

    // Verifies: REQ093 (the honesty crux) — an error count is `inconclusive`, NEVER a `fails`. A
    // deductive prover's failure to discharge is not a counterexample.
    #[test]
    fn a_verification_error_is_inconclusive_never_fails() {
        let Outcome::Inconclusive { reason } =
            classify("error: assertion might fail\nverification results:: 0 verified, 1 errors\n")
        else {
            panic!("an undischarged goal must be inconclusive, not a refutation");
        };
        assert!(reason.contains("could not"), "{reason}");
    }

    // Verifies: REQ093 — a predicate that is an ordinary `fn` (ignored by Verus as external) is the
    // honest ceiling: `inconclusive` naming the cause — the subject is not authored in the Verus
    // subset — never a guess and never a proof.
    #[test]
    fn a_predicate_not_in_the_verus_subset_is_an_honest_ceiling() {
        let output = "error: cannot use function `lib::has_session` which is ignored because it \
                      is either declared outside the verus! macro or it is marked as `external`\n\
                      error: aborting due to 1 previous error\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("an opaque predicate decides nothing");
        };
        assert!(reason.contains("Verus subset"), "{reason}");
        assert!(reason.contains("spec fn"), "names the fix: {reason}");
    }

    // Verifies: REQ093 — the toolchain ceiling (Verus's pinned rust toolchain absent) is named as a
    // ceiling, not handed to the operator as a build error they could annotate their way out of.
    #[test]
    fn a_toolchain_ceiling_is_named_as_one() {
        let Outcome::Inconclusive { reason } =
            classify("verus: required rust toolchain 1.99.0-x86_64-unknown-linux-gnu not found\n")
        else {
            panic!("a toolchain ceiling decides nothing");
        };
        assert!(reason.contains("toolchain"), "{reason}");
    }

    // Verifies: REQ093 — unrecognised output (e.g. an empty run) is inconclusive with a readable
    // reason, never an optimistic pass.
    #[test]
    fn unrecognised_output_is_inconclusive() {
        let Outcome::Inconclusive { .. } = classify("") else {
            panic!("no output decides nothing");
        };
    }

    fn prov() -> Provenance {
        Provenance {
            requirement_revision: "rev-1".into(),
            subject_commit: Some("abc123".into()),
            tool_version: "0.0.1".into(),
            verified_scope: None,
        }
    }

    // Verifies: REQ093 (D8) — a Verus pass is `proven`, the strongest rung, and the read-back does
    // NOT wear the bounded caveat.
    #[test]
    fn a_verus_pass_is_proven_and_not_bounded() {
        let v = crate::verdict::aggregate("SR001", vec![Outcome::Holds.into_evidence()], prov());
        assert_eq!(v.status, crate::verdict::Status::Holds);
        assert_eq!(v.basis, Some(Basis::Proven));
        let text = crate::verdict::render(&v);
        assert!(text.contains("proven: established deductively"), "{text}");
        assert!(!text.contains("NOT proven for all executions"), "{text}");
    }

    // Verifies: REQ093 (D10) — an inconclusive run yields unknown/inconclusive, never a verdict; the
    // engine's own message rides along.
    #[test]
    fn an_inconclusive_run_is_unknown_never_a_verdict() {
        let outcome = Outcome::Inconclusive {
            reason: "Verus could not discharge the proof obligation".into(),
        };
        let v = crate::verdict::aggregate("SR002", vec![outcome.into_evidence()], prov());
        assert_eq!(v.status, crate::verdict::Status::Unknown);
        assert_eq!(v.reason, Some(crate::verdict::UnknownReason::Inconclusive));
        assert!(crate::verdict::render(&v).contains("could not discharge"));
    }

    // Verifies: REQ093 — the harness name is a valid, prefixed identifier from the req id.
    #[test]
    fn harness_name_is_a_valid_prefixed_identifier() {
        assert_eq!(harness_name("REQ001"), "provreq_req001");
        assert_eq!(harness_name("REQ-1.2"), "provreq_req_1_2");
    }

    // Verifies: REQ093 — an existing operator file at the harness path is NEVER clobbered: a name
    // collision stops the run, it does not overwrite their work.
    #[test]
    fn an_existing_operator_file_is_never_overwritten() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src");
        std::fs::write(tmp.path().join("src/lib.rs"), "// the operator's crate\n").expect("root");
        let victim = tmp.path().join("src/provreq_smoke.rs");
        std::fs::write(&victim, "// the operator's own file\n").expect("write");

        let harness = Harness {
            name: "provreq_smoke".into(),
            source: "// generated\n".into(),
        };
        let Outcome::Inconclusive { reason } = run(tmp.path(), &harness) else {
            panic!("a collision must not be treated as a verdict");
        };
        assert!(reason.contains("refus"), "{reason}");
        assert_eq!(
            std::fs::read_to_string(&victim).expect("read"),
            "// the operator's own file\n",
            "the operator's file must be untouched"
        );
    }

    // Verifies: REQ093 — a subject that is not a cargo crate (no src root) is honest `inconclusive`:
    // there is no crate to attach the harness to.
    #[test]
    fn a_subject_with_no_crate_root_is_inconclusive() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let harness = Harness {
            name: "provreq_smoke".into(),
            source: "// generated\n".into(),
        };
        let Outcome::Inconclusive { reason } = run(tmp.path(), &harness) else {
            panic!("no crate root, no verdict");
        };
        assert!(reason.contains("no src/lib.rs or src/main.rs"), "{reason}");
    }

    /// A real verus-authored subject: a sort and two `spec fn` predicates in a `verus! { }` block.
    /// `has_session`'s body is supplied so a test can make the invariant `logged_in ==> has_session`
    /// a tautology or false.
    fn verus_subject(has_session_body: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src");
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            format!(
                "use vstd::prelude::*;\nverus! {{\n\
                 pub struct User {{ pub logged_in: bool, pub extra: bool }}\n\
                 pub open spec fn logged_in(u: User) -> bool {{ u.logged_in }}\n\
                 pub open spec fn has_session(u: User) -> bool {{ {has_session_body} }}\n\
                 }}\n"
            ),
        )
        .expect("lib.rs");
        tmp
    }

    /// The harness the real-engine tests share: the CODE_REQ invariant lowered over the by-value
    /// `spec fn` predicates.
    fn smoke_harness() -> Harness {
        lower(
            &req(CODE_REQ),
            &standard_bindings(),
            &by_value_resolutions(),
            &sorts(),
            "provreq_smoke",
        )
        .expect("the fixture must lower")
    }

    // Verifies: REQ093 — THE REAL ENGINE, end to end: a true invariant over a real Verus-authored
    // subject is proved and earns `proven`.
    //
    // `#[ignore]` is deliberate: the common user state is engine-ABSENT, so CI's main `test` job
    // stays Verus-free and a separate job runs `-- --ignored`.
    #[test]
    #[ignore = "needs Verus installed — run via `cargo test -- --ignored`"]
    fn real_verus_proves_a_true_invariant() {
        // has_session = logged_in → `logged_in ==> has_session` is a tautology.
        let tmp = verus_subject("u.logged_in");
        assert_eq!(
            run(tmp.path(), &smoke_harness()),
            Outcome::Holds,
            "a true invariant must be proved"
        );
    }

    // Verifies: REQ093 (the honesty crux) — THE REAL ENGINE on a FALSE invariant is `inconclusive`,
    // NEVER a proof and NEVER a `fails`. Verus cannot discharge the goal; that is not a witness.
    #[test]
    #[ignore = "needs Verus installed — run via `cargo test -- --ignored`"]
    fn real_verus_cannot_prove_a_false_invariant() {
        // has_session = extra, independent of logged_in → false at logged_in=true, extra=false.
        let tmp = verus_subject("u.extra");
        assert!(
            matches!(
                run(tmp.path(), &smoke_harness()),
                Outcome::Inconclusive { .. }
            ),
            "a false invariant must NOT be proved, and Verus yields no witness so it is inconclusive"
        );
    }

    // Verifies: REQ093 — THE REAL ENGINE on a non-subset subject (ordinary `fn` predicates, not
    // `spec fn`): Verus ignores them as external, so the harness cannot be checked — an honest
    // ceiling, never a wrong verdict.
    #[test]
    #[ignore = "needs Verus installed — run via `cargo test -- --ignored`"]
    fn real_verus_is_a_ceiling_on_non_subset_predicates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src");
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            "pub struct User { pub logged_in: bool, pub extra: bool }\n\
             pub fn logged_in(u: User) -> bool { u.logged_in }\n\
             pub fn has_session(u: User) -> bool { u.logged_in }\n",
        )
        .expect("lib.rs");
        let Outcome::Inconclusive { reason } = run(tmp.path(), &smoke_harness()) else {
            panic!("an opaque predicate decides nothing");
        };
        assert!(reason.contains("Verus subset"), "{reason}");
    }

    // Verifies: REQ093 — provreq leaves no litter: the harness file and the appended `mod` line are
    // gone afterward, and the crate root is byte-for-byte what it was.
    #[test]
    #[ignore = "needs Verus installed — run via `cargo test -- --ignored`"]
    fn real_verus_run_leaves_no_trace_in_the_subject() {
        let tmp = verus_subject("u.logged_in");
        let root_before = std::fs::read_to_string(tmp.path().join("src/lib.rs")).expect("read");
        let _ = run(tmp.path(), &smoke_harness());
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("src/lib.rs")).expect("read"),
            root_before,
            "the crate root must be restored exactly"
        );
        assert!(
            !tmp.path().join("src/provreq_smoke.rs").exists(),
            "harness file must be gone"
        );
    }
}
