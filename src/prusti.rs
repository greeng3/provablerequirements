//! Prusti — a category-1 engine, **#3: the second deductive verifier that earns `proven`**.
//!
//! D2 gives the core one meaning and lowers it to each engine; Prusti is **lowering #3**
//! (Kani #1, Creusot #2). The binding stays core-owned ([`crate::grounding`]), the language
//! stays the adapter's ([`crate::rust_adapter`]), and this module owns exactly one thing: how a
//! gated category-1 requirement becomes something Prusti can prove, and what its answer means.
//!
//! **Additive, and in-crate like Creusot.** Prusti verifies the crate itself (it is a rustc
//! driver), so the harness is a new **module** in the crate root (`#[cfg(prusti)] mod
//! provreq_…;`) plus its source file. The subject's own code is never edited — only a `mod`
//! line is appended and then removed. Unlike Creusot there is no prover-config file to supply;
//! what Prusti needs instead is the subject's `prusti-contracts` dependency (the crate the
//! `prusti_assert!`/`forall` macros come from). This module **consumes** that dependency and the
//! subject's existing `#[pure]` predicates — it does not add a dependency or write contracts into
//! the subject (that is the A6 contract-*draft* channel, a later slice). A subject that does not
//! already depend on `prusti-contracts` yields an honest `inconclusive`, never a guess.
//!
//! **Honest by construction (D8/D9) — the same two-valued shape as Creusot.** Prusti is a
//! *deductive* verifier: a discharged proof obligation holds for **every** execution
//! (spec-relative), so a pass is [`Basis::Proven`], the strongest rung. But a *failed*
//! obligation is NOT a counterexample — Prusti reports "the asserted expression might not hold",
//! which may be because the claim is false **or** merely because a predicate is not `#[pure]` and
//! the verifier cannot see inside it. There is no re-checkable witness. So Prusti yields
//! [`Outcome::Holds`] or [`Outcome::Inconclusive`] and **never a `fails`**: mapping a "might not
//! hold" to a refutation would be the overclaim D8 guards against. A definitive refutation needs
//! an engine that produces a witness (Kani).
//!
//! **What cannot be lowered is said, not approximated.** The gate guarantees a category-1
//! requirement is temporal-free (REQ024), so the target is small: `always`/`never` over boolean
//! combinations, optionally quantified. Anything this module cannot faithfully express — a scope,
//! a guard, an argument that is not the quantified variable — is a [`NotLowerable`], which becomes
//! an honest `unknown`.
//!
//! Implements: REQ032 (wire Prusti as cat-1 engine #3 — a grounded invariant earns a real
//! `proven` verdict).

use crate::grounding::Binding;
use crate::lowering::{self, LoweredClaim};
pub use crate::lowering::{harness_name, NotLowerable};
use crate::prl::ast::Requirement;
use crate::rust_adapter::Resolution;
use crate::verdict::{Basis, Evidence};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A generated Prusti proof harness. `name` is both the harness `fn` name and the module (file
/// stem) it is written to, so it cannot collide with the subject's own items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Harness {
    pub name: String,
    pub source: String,
}

/// What running Prusti established. Note the **two**-valued shape (contrast Kani's three): a
/// deductive prover confirms or fails-to-decide, but its failure-to-decide is not a refutation —
/// there is no counterexample to carry, so there is no `Fails`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Proved — the obligation was discharged, so the claim holds for all executions
    /// (spec-relative). This is `proven`, the strongest D8 rung.
    Holds,
    /// Prusti ran but did not discharge the obligation, or the harness did not compile. D10's
    /// `inconclusive(…)`. NOT a refutation: an undischarged deductive goal may be false or merely
    /// under-annotated, and either way it is not evidence the claim is wrong.
    Inconclusive { reason: String },
}

impl Outcome {
    /// Map what Prusti established into a piece of [`Evidence`]. The mapping lives here, in the
    /// engine, so [`crate::verdict`] never learns what Prusti is (D2's "one meaning, lowering to
    /// each engine" runs in this direction too). The load-bearing line is `Holds` →
    /// [`Basis::Proven`]: a deductive proof is `∀`-executions, never bounded.
    pub fn into_evidence(&self) -> Evidence {
        match self {
            Outcome::Holds => Evidence::holds("Prusti", Basis::Proven),
            Outcome::Inconclusive { reason } => {
                Evidence::inconclusive("Prusti", vec![reason.clone()])
            }
        }
    }
}

/// Lower a gated category-1 requirement to a Prusti proof harness.
///
/// The claim itself is lowered by the shared [`crate::lowering`] core (prefix `crate`, since this
/// harness is a module *inside* the subject crate, reaching the subject through `crate::…`); this
/// function owns only the Prusti wrapper — a `prusti_assert!(forall(||))` per property.
pub fn lower(
    req: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
    name: &str,
) -> Result<Harness, NotLowerable> {
    if req.require.is_empty() {
        return Err(NotLowerable::new(
            "the requirement claims nothing — there is no property to check",
        ));
    }
    let mut body = String::new();
    for prop in &req.require {
        let claim = lowering::lower_property(prop, "crate", bindings, resolutions)?;
        body.push_str(&assertion(&claim));
    }
    let source = format!(
        "// Generated by provreq — do not edit; it is rewritten on every `verify` and \
         removed afterwards.\n\
         //\n\
         // An ADDITIVE Prusti proof harness: a module inside the subject crate that asserts the\n\
         // invariant as a `prusti_assert!` over a `forall`. The subject's own code is untouched. \
         The `mod`\n\
         // line in the crate root is `#[cfg(prusti)]`, so an ordinary `cargo build`/`cargo test` \
         never\n\
         // sees this file.\n\
         #![allow(unused)]\n\
         use prusti_contracts::*;\n\
         \n\
         pub fn {name}() {{\n\
         {body}}}\n"
    );
    Ok(Harness {
        name: name.to_string(),
        source,
    })
}

/// Wrap one lowered claim as a Prusti `prusti_assert!`. A quantified claim becomes a `forall`
/// closure over the sort's type (what makes it a ∀ proof rather than a spot check); an
/// unquantified one (e.g. `never overdrawn`) asserts the ground fact directly.
fn assertion(claim: &LoweredClaim) -> String {
    let body = match &claim.quantified {
        Some(q) => format!("forall(|{}: {}| {})", q.var, q.ty, claim.claim),
        None => claim.claim.clone(),
    };
    format!("    prusti_assert!({body});\n")
}

/// The subject's crate-root source file (`src/lib.rs`, else `src/main.rs`). The harness `mod`
/// declaration is appended here. `None` when the subject has neither — then Prusti has no crate
/// to attach the harness to, which is an honest `inconclusive`.
fn crate_root(subject_root: &Path) -> Option<PathBuf> {
    for rel in ["src/lib.rs", "src/main.rs"] {
        let p = subject_root.join(rel);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Write the harness into the subject as a `#[cfg(prusti)]` module, run `cargo prusti`, and
/// restore the subject exactly as it was.
///
/// Additive discipline, mirroring the Kani/Creusot slices: the subject's own code is never edited
/// (only a `mod` line is appended to the crate root and then removed); an existing file is never
/// clobbered; and every artifact provreq *created* — the harness file and Prusti's
/// `target/verify/` output — is removed afterward, while anything that was already there is left
/// untouched. Cleanup runs on every path.
pub fn run(subject_root: &Path, harness: &Harness) -> Outcome {
    let Some(root_file) = crate_root(subject_root) else {
        return Outcome::Inconclusive {
            reason: "the subject has no src/lib.rs or src/main.rs, so Prusti has no crate to \
                     attach the proof harness to"
                .to_string(),
        };
    };
    let harness_path = subject_root
        .join("src")
        .join(format!("{}.rs", harness.name));
    if harness_path.exists() {
        return Outcome::Inconclusive {
            reason: format!(
                "{} already exists — refusing to overwrite a file provreq did not write",
                harness_path.display()
            ),
        };
    }
    let original_root = match std::fs::read_to_string(&root_file) {
        Ok(s) => s,
        Err(e) => {
            return Outcome::Inconclusive {
                reason: format!("could not read {}: {e}", root_file.display()),
            }
        }
    };

    // Remember what already existed, so cleanup removes only what provreq creates. `cargo prusti`
    // writes all of its output under `target/verify/`.
    let verify_dir = subject_root.join("target").join("verify");
    let verify_created = !verify_dir.exists();

    // Mutate: harness file, then the `mod` line.
    if let Err(e) = std::fs::write(&harness_path, &harness.source) {
        return Outcome::Inconclusive {
            reason: format!(
                "could not write the harness to {}: {e}",
                harness_path.display()
            ),
        };
    }
    let with_mod = format!("{original_root}\n#[cfg(prusti)]\nmod {};\n", harness.name);
    if let Err(e) = std::fs::write(&root_file, &with_mod) {
        let _ = std::fs::remove_file(&harness_path);
        return Outcome::Inconclusive {
            reason: format!(
                "could not attach the harness module to {}: {e}",
                root_file.display()
            ),
        };
    }

    // Clear `RUSTUP_TOOLCHAIN` so the subject's own `rust-toolchain.toml` governs. If the caller
    // runs under a newer toolchain (e.g. `cargo test`, or provreq built on stable), a leaked
    // `RUSTUP_TOOLCHAIN` makes `cargo-prusti` resolve an absolute newer-cargo path that writes a
    // lock-file version the pinned 2023 driver cannot parse ("lock file version 4 requires
    // -Znext-lockfile-bump") — a spurious `inconclusive`. Prusti is toolchain-welded to its own
    // nightly, which the subject pins, so the caller's toolchain must not leak in.
    let output = std::process::Command::new("cargo")
        .arg("prusti")
        .current_dir(subject_root)
        .env_remove("RUSTUP_TOOLCHAIN")
        .output();

    // Restore before interpreting anything, so an early return cannot leak the harness. Only
    // artifacts provreq created are removed; an existing target/verify is the operator's.
    let _ = std::fs::write(&root_file, &original_root);
    let _ = std::fs::remove_file(&harness_path);
    if verify_created {
        let _ = std::fs::remove_dir_all(&verify_dir);
    }

    match output {
        Ok(o) => classify(&format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )),
        Err(e) => Outcome::Inconclusive {
            reason: format!("could not run `cargo prusti`: {e}"),
        },
    }
}

/// Map Prusti's output to an outcome. Pure and separately tested — the mapping is where a verdict
/// could silently become dishonest, so it must be checkable without running Prusti.
///
/// The order matters. A **verification** error ("might not hold") and a **compile** error are
/// both checked BEFORE the success marker, because Prusti prints its verification error and then
/// `could not compile`, so a naive success check on `Finished` would still be false there. A
/// verification error is `inconclusive`, never `fails`: a deductive prover's "might not hold" is
/// not a counterexample. A missing `prusti-contracts` dependency surfaces as a cargo/feature or
/// unresolved-import error, also `inconclusive`, naming the actionable cause.
pub fn classify(output: &str) -> Outcome {
    // A verification error is the most specific signal and the honesty crux — check it first. It
    // is `inconclusive`, never `fails`: "might not hold" is not a re-checkable counterexample.
    if output.contains("[Prusti: verification error]") || output.contains("might not hold") {
        return Outcome::Inconclusive {
            reason: "Prusti could not discharge the proof obligation — the invariant may be \
                     false, or its predicates may need to be `#[pure]` for the prover to see \
                     inside them"
                .to_string(),
        };
    }
    // Success next: a run that reached `Finished` with no verification error verified cleanly.
    // This is checked BEFORE the error branches because benign cargo chatter (e.g. the 2023
    // toolchain's "to enable this feature" note about the `[lints]` key) must never be mistaken
    // for a failure.
    if output.contains("Finished") {
        return Outcome::Holds;
    }
    // A missing `prusti-contracts` dependency shows up as cargo's specific feature error (the
    // `--features prusti-contracts/prusti` cargo-prusti injects has no such dependency) or an
    // unresolved import of the crate — matched precisely so it cannot fire on a healthy run.
    if output.contains("these features: prusti-contracts")
        || output.contains("unresolved import `prusti_contracts`")
    {
        return Outcome::Inconclusive {
            reason: "the subject does not depend on `prusti-contracts`, so the `prusti_assert!` \
                     harness has nothing to compile against — add it (with a `uuid = \"=1.10.0\"` \
                     cap) to verify with Prusti"
                .to_string(),
        };
    }
    if output.contains("could not compile") || output.contains("error[") {
        return Outcome::Inconclusive {
            reason: build_error(output),
        };
    }
    Outcome::Inconclusive {
        reason: tail(output),
    }
}

/// The first compiler error line, which names the actionable cause (a predicate that is not
/// `#[pure]`, a type mismatch) — the top of a rustc diagnostic, not the boilerplate tail.
fn build_error(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("error[") || l.starts_with("error:"))
        .map(|l| {
            format!("the proof harness did not compile — {l} (a predicate that is not `#[pure]` is opaque to the prover)")
        })
        .unwrap_or_else(|| tail(output))
}

/// The last few non-empty lines of engine output — enough for the operator to see why Prusti
/// could not decide, without pasting a whole log into the verdict.
fn tail(output: &str) -> String {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(TAIL_LINES);
    let tail = lines[start..].join("\n");
    if tail.trim().is_empty() {
        "`cargo prusti` produced no recognisable verdict".to_string()
    } else {
        tail
    }
}

/// How many lines of engine output an `inconclusive` carries. Enough to name a cause; short
/// enough to stay a verdict rather than a log.
const TAIL_LINES: usize = 12;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grounding::{BindCategory, Fidelity};
    use crate::prl::gate;
    use crate::rust_adapter::{CodeMatch, ParamMode};
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

    fn resolved(params: Vec<ParamMode>) -> Resolution {
        Resolution::Resolved {
            at: CodeMatch {
                file: "src/lib.rs".into(),
                line: 1,
                text: "fn f() -> bool { true }".into(),
            },
            params,
        }
    }

    /// Both predicates take the sort by value — the `#[pure]` idiom a Prusti subject uses.
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
            "provreq_req001",
        )
    }

    // Verifies: REQ032 — a quantified cat-1 invariant lowers to a `prusti_assert!` with a
    // `forall` closure over the sort's real type, calling the subject's predicates via `crate::`
    // (the harness is in-crate, unlike Kani's tests/ harness).
    #[test]
    fn quantified_invariant_lowers_to_a_forall_prusti_assert() {
        let h = lower_standard().expect("should lower");
        assert_eq!(h.name, "provreq_req001");
        assert!(
            h.source.contains("use prusti_contracts::*;"),
            "{}",
            h.source
        );
        assert!(
            h.source.contains(
                "prusti_assert!(forall(|u: crate::User| \
                 (!(crate::logged_in(u)) || crate::has_session(u))));"
            ),
            "the claim must lower to a forall over the subject's real predicates: {}",
            h.source
        );
    }

    // Verifies: REQ032 — the harness reaches the subject through `crate::`, NOT through a crate
    // name; it lives inside the subject crate as a module.
    #[test]
    fn calls_go_through_crate_not_a_crate_name() {
        let h = lower_standard().expect("should lower");
        assert!(h.source.contains("crate::logged_in"), "{}", h.source);
        assert!(!h.source.contains("subject::"), "{}", h.source);
    }

    // Verifies: REQ032 — a by-ref `#[pure]` predicate is called with `&u`, matching the subject's
    // real signature (the adapter's ParamMode).
    #[test]
    fn calls_follow_the_subjects_parameter_modes() {
        let by_ref = BTreeMap::from([
            ("logged_in".to_string(), resolved(vec![ParamMode::ByRef])),
            ("has_session".to_string(), resolved(vec![ParamMode::ByRef])),
        ]);
        let h = lower(&req(CODE_REQ), &standard_bindings(), &by_ref, "h").expect("should lower");
        assert!(h.source.contains("crate::logged_in(&u)"), "{}", h.source);
    }

    // Verifies: REQ032 — `never P` is `always not P`, and an unquantified claim asserts the
    // ground fact directly with no `forall`.
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
            "h",
        )
        .expect("should lower");
        assert!(
            h.source
                .contains("prusti_assert!(!(crate::is_overdrawn()));"),
            "{}",
            h.source
        );
        assert!(
            !h.source.contains("forall("),
            "no quantifier, no forall binder: {}",
            h.source
        );
    }

    // Verifies: REQ032 — an unbound sort cannot be quantified over, so the requirement does not
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
            "h",
        )
        .expect_err("an unbound sort has no domain");
        assert!(e.reason.contains("User"), "{}", e.reason);
        assert!(e.reason.contains("no domain"), "{}", e.reason);
    }

    // Verifies: REQ032 — an unresolved predicate does not lower. Absence of a resolution is not
    // evidence a call would compile, let alone be the right one.
    #[test]
    fn unresolved_predicate_does_not_lower() {
        let e = lower(
            &req(CODE_REQ),
            &standard_bindings(),
            &BTreeMap::from([("logged_in".to_string(), resolved(vec![ParamMode::ByValue]))]),
            "h",
        )
        .expect_err("has_session never resolved");
        assert!(e.reason.contains("has_session"), "{}", e.reason);
    }

    // Verifies: REQ032 — a temporal pattern does not lower. The gate rejects these at category 1
    // (REQ024), but `lower` is public and must not assume it was called.
    #[test]
    fn temporal_patterns_do_not_lower() {
        let r = req("requirement r {
            category: 2b
            vocabulary { state p, q }
            require { p leads_to q }
        }");
        let e = lower(&r, &[], &BTreeMap::new(), "h").expect_err("liveness is not an invariant");
        assert!(e.reason.contains("leads_to"), "{}", e.reason);
        assert!(e.reason.contains("temporal-free"), "{}", e.reason);
    }

    // Verifies: REQ032 — Prusti's success marker (`Finished`) with no verification/compile error
    // is the ONLY thing read as a proof.
    #[test]
    fn finished_output_is_holds() {
        assert_eq!(
            classify("   Checking psubj v0.1.0\n    Finished dev [unoptimized] target(s) in 4s\n"),
            Outcome::Holds
        );
    }

    // Verifies: REQ032 (the honesty crux) — a "might not hold" is `inconclusive`, NEVER a
    // `fails`. A deductive prover's failure to discharge is not a counterexample.
    #[test]
    fn verification_error_is_inconclusive_never_fails() {
        let Outcome::Inconclusive { reason } = classify(
            "error: [Prusti: verification error] the asserted expression might not hold\n\
             error: could not compile `psubj`\n",
        ) else {
            panic!("an undischarged deductive goal must be inconclusive, not a refutation");
        };
        assert!(
            reason.contains("could not discharge"),
            "the reason must not read as a refutation: {reason}"
        );
    }

    // Verifies: REQ032 — an opaque predicate (an ordinary `fn`, not `#[pure]`) makes the harness
    // fail to compile/verify, which is `inconclusive` and names the actionable cause.
    #[test]
    fn a_compile_failure_is_inconclusive_and_names_the_cause() {
        let output = "error[E0308]: mismatched types\n  --> src/provreq_check.rs:5:9\n\
                      error: could not compile `psubj` (lib) due to 2 previous errors\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a harness that does not compile decides nothing");
        };
        assert!(reason.contains("did not compile"), "{reason}");
        assert!(
            reason.contains("`#[pure]`"),
            "must point at the fix: {reason}"
        );
    }

    // Verifies: REQ032 — a subject that does not depend on prusti-contracts is `inconclusive`
    // with a message naming the missing dependency (and the uuid cap it needs), never a guess.
    #[test]
    fn a_missing_prusti_contracts_dependency_is_inconclusive() {
        let output = "error: none of the selected packages contains these features: \
                      prusti-contracts/prusti\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a missing dependency decides nothing");
        };
        assert!(reason.contains("prusti-contracts"), "{reason}");
        assert!(reason.contains("uuid"), "names the cap it needs: {reason}");
    }

    // Verifies: REQ032 — unrecognised output (e.g. an empty run) is inconclusive with a readable
    // reason, never an optimistic pass.
    #[test]
    fn unrecognised_output_is_inconclusive() {
        let Outcome::Inconclusive { reason } = classify("") else {
            panic!("no output decides nothing");
        };
        assert!(reason.contains("no recognisable verdict"), "{reason}");
    }

    // Verifies: REQ032 — the harness name is a valid, prefixed identifier from the req id.
    #[test]
    fn harness_name_is_a_valid_prefixed_identifier() {
        assert_eq!(harness_name("REQ001"), "provreq_req001");
        assert_eq!(harness_name("REQ-1.2"), "provreq_req_1_2");
    }

    fn prov() -> Provenance {
        Provenance {
            requirement_revision: "rev-1".into(),
            subject_commit: Some("abc123".into()),
            tool_version: "0.0.1".into(),
        }
    }

    // Verifies: REQ032 (D8) — a Prusti pass is `proven`, the strongest rung, and the read-back
    // does NOT wear the bounded caveat.
    #[test]
    fn a_prusti_pass_is_proven_and_not_bounded() {
        let v = crate::verdict::aggregate("SR001", vec![Outcome::Holds.into_evidence()], prov());
        assert_eq!(v.status, crate::verdict::Status::Holds);
        assert_eq!(v.basis, Some(Basis::Proven));
        let text = crate::verdict::render(&v);
        assert!(text.contains("proven: established deductively"), "{text}");
        assert!(!text.contains("NOT proven for all executions"), "{text}");
    }

    // Verifies: REQ032 (D10) — an inconclusive run yields unknown/inconclusive, never a verdict;
    // the engine's own message rides along.
    #[test]
    fn an_inconclusive_run_is_unknown_never_a_verdict() {
        let outcome = Outcome::Inconclusive {
            reason: "Prusti could not discharge the proof obligation".into(),
        };
        let v = crate::verdict::aggregate("SR002", vec![outcome.into_evidence()], prov());
        assert_eq!(v.status, crate::verdict::Status::Unknown);
        assert_eq!(v.reason, Some(crate::verdict::UnknownReason::Inconclusive));
        assert!(crate::verdict::render(&v).contains("could not discharge"));
    }

    /// A real cargo subject: a sort and two `#[pure]` predicates over it, `has_session`'s body
    /// supplied so a test can make the invariant true or false. It depends on `prusti-contracts`
    /// with the `uuid` cap the 2023 toolchain needs (see the Dockerfile B0 notes).
    fn cargo_subject(has_session_body: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"psmoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [dependencies]\n\
             prusti-contracts = { path = \"/opt/prusti-src/prusti-contracts/prusti-contracts\" }\n\
             uuid = \"=1.10.0\"\n\n\
             [lints.rust]\nunexpected_cfgs = { level = \"warn\", check-cfg = ['cfg(prusti)'] }\n",
        )
        .expect("manifest");
        std::fs::write(
            tmp.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"nightly-2023-08-15\"\n",
        )
        .expect("toolchain");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src");
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            format!(
                "use prusti_contracts::*;\n\
                 pub struct User {{ pub logged_in: bool, pub id: u64 }}\n\
                 #[pure]\n\
                 pub fn logged_in(u: &User) -> bool {{ u.logged_in }}\n\
                 #[pure]\n\
                 pub fn has_session(u: &User) -> bool {{ {has_session_body} }}\n"
            ),
        )
        .expect("lib.rs");
        tmp
    }

    /// Both predicates take the sort BY REFERENCE — the idiomatic `#[pure]` signature over a
    /// struct, and the one the real-engine fixture uses: a `forall`-bound `User` is not `Copy`,
    /// so two by-value calls would move it twice; two `&u` borrows do not.
    fn by_ref_resolutions() -> BTreeMap<String, Resolution> {
        BTreeMap::from([
            ("logged_in".to_string(), resolved(vec![ParamMode::ByRef])),
            ("has_session".to_string(), resolved(vec![ParamMode::ByRef])),
        ])
    }

    /// The harness for the `cargo_subject` fixture, which every real-engine test shares. It uses
    /// by-reference predicate calls (`crate::logged_in(&u)`) so the non-`Copy` `User` bound by the
    /// `forall` is borrowed, not moved, across the two calls.
    fn smoke_harness() -> Harness {
        lower(
            &req(CODE_REQ),
            &standard_bindings(),
            &by_ref_resolutions(),
            "provreq_smoke",
        )
        .expect("the fixture must lower")
    }

    // Verifies: REQ032 — THE REAL ENGINE, end to end: a true invariant over a real Prusti subject
    // is proved and earns `proven`.
    //
    // `#[ignore]` is deliberate, not neglect (R-eng-2): the common user state is engine-ABSENT,
    // and that path is the one most worth proving continuously — so CI's main `test` job stays
    // Prusti-free and a separate `prusti` job runs `-- --ignored`.
    #[test]
    #[ignore = "needs Prusti installed — run via `cargo test -- --ignored` (the CI `prusti` job)"]
    fn real_prusti_proves_a_true_invariant() {
        // has_session = logged_in || id==0 → the invariant !logged_in||has_session is a tautology.
        let tmp = cargo_subject("u.logged_in || u.id == 0");
        let outcome = run(tmp.path(), &smoke_harness());
        assert_eq!(outcome, Outcome::Holds, "a true invariant must be proved");
    }

    // Verifies: REQ032 (the honesty crux) — THE REAL ENGINE on a FALSE invariant is
    // `inconclusive`, NEVER a proof and NEVER a `fails`. Prusti cannot discharge the goal; that
    // is not a counterexample.
    #[test]
    #[ignore = "needs Prusti installed — run via `cargo test -- --ignored` (the CI `prusti` job)"]
    fn real_prusti_cannot_prove_a_false_invariant() {
        // has_session = id==5 → false at logged_in=true, id!=5.
        let tmp = cargo_subject("u.id == 5");
        let outcome = run(tmp.path(), &smoke_harness());
        assert!(
            matches!(outcome, Outcome::Inconclusive { .. }),
            "a false invariant must NOT be proved, and Prusti yields no witness so it is \
             inconclusive, got {outcome:?}"
        );
    }

    // Verifies: REQ032 — THE REAL ENGINE on opaque predicates (ordinary `fn`, not `#[pure]`): the
    // harness cannot verify, so the verdict is `inconclusive`, never wrong.
    #[test]
    #[ignore = "needs Prusti installed — run via `cargo test -- --ignored` (the CI `prusti` job)"]
    fn real_prusti_is_inconclusive_on_opaque_predicates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"psmoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [dependencies]\n\
             prusti-contracts = { path = \"/opt/prusti-src/prusti-contracts/prusti-contracts\" }\n\
             uuid = \"=1.10.0\"\n\n\
             [lints.rust]\nunexpected_cfgs = { level = \"warn\", check-cfg = ['cfg(prusti)'] }\n",
        )
        .expect("manifest");
        std::fs::write(
            tmp.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"nightly-2023-08-15\"\n",
        )
        .expect("toolchain");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src");
        // Ordinary program fns (by-ref, matching the harness's `&u` calls, so the ONLY reason
        // this cannot verify is that they are not `#[pure]` — Prusti cannot unfold them).
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            "pub struct User { pub logged_in: bool, pub id: u64 }\n\
             pub fn logged_in(u: &User) -> bool { u.logged_in }\n\
             pub fn has_session(u: &User) -> bool { u.logged_in }\n",
        )
        .expect("lib.rs");
        let outcome = run(tmp.path(), &smoke_harness());
        assert!(
            matches!(outcome, Outcome::Inconclusive { .. }),
            "got {outcome:?}"
        );
    }

    // Verifies: REQ032 — provreq leaves no litter in someone else's repo. The harness file, the
    // appended `mod` line, and Prusti's target/verify output are gone afterward, and the crate
    // root is byte-for-byte what it was.
    #[test]
    #[ignore = "needs Prusti installed — run via `cargo test -- --ignored` (the CI `prusti` job)"]
    fn real_prusti_run_leaves_no_trace_in_the_subject() {
        let tmp = cargo_subject("u.logged_in || u.id == 0");
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

    // Verifies: REQ032 — an existing file is NEVER clobbered. provreq writes into someone else's
    // repo, so a name collision must stop the run, not overwrite their work.
    #[test]
    fn an_existing_harness_file_is_never_overwritten() {
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
        assert!(reason.contains("refusing to overwrite"), "{reason}");
        assert_eq!(
            std::fs::read_to_string(&victim).expect("read"),
            "// the operator's own file\n",
            "the operator's file must be untouched"
        );
    }

    // Verifies: REQ032 — a subject that is not a cargo crate (no src root) is honest
    // `inconclusive`: there is no crate to attach the harness to.
    #[test]
    fn a_subject_with_no_crate_root_is_inconclusive() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let Outcome::Inconclusive { reason } = run(tmp.path(), &smoke_harness()) else {
            panic!("no crate root, no verdict");
        };
        assert!(reason.contains("no src/lib.rs or src/main.rs"), "{reason}");
    }
}
