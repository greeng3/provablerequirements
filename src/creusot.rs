//! Creusot — a category-1 engine, **#2: the first that earns `proven`**.
//!
//! D2 gives the core one meaning and lowers it to each engine; Creusot is **lowering #2**
//! (Kani was #1). The binding stays core-owned ([`crate::grounding`]), the language stays the
//! adapter's ([`crate::rust_adapter`]), and this module owns exactly one thing: how a gated
//! category-1 requirement becomes something Creusot can prove, and what its answer means.
//!
//! **Additive, like Kani — but in-crate.** Kani's harness is a separate `tests/` binary that
//! imports the subject; Creusot verifies the crate itself, so the harness is a new **module**
//! in the crate root (`#[cfg(creusot)] mod provreq_…;`) plus its source file. The subject's
//! own code is never edited — only a `mod` line is appended and then removed, and if the
//! subject has no `why3find.json` (the prover config) the installed Creusot's own canonical
//! one is copied in and removed again. This keeps Creusot on the additive side of the "does
//! provreq annotate the subject?"
//! question: it **consumes** the subject's existing `#[logic]` predicates, it does not write
//! contracts into them (the A6 contract-*draft* channel is a later slice).
//!
//! **Honest by construction (D8/D9) — and differently from Kani.** Creusot is a *deductive*
//! verifier: a discharged proof obligation holds for **every** execution (spec-relative), so
//! a pass is [`Basis::Proven`], the strongest rung. But an *un*discharged obligation is NOT a
//! counterexample — an SMT solver returning "unproved" means it could not prove the claim,
//! which may be because the claim is false **or** merely because the predicates lack the
//! logic contracts the prover needs to see inside them. There is no witness. So Creusot
//! yields [`Outcome::Holds`] or [`Outcome::Inconclusive`] and **never a `fails`**: mapping an
//! unproved goal to a refutation would be the overclaim D8 guards against, pointed the other
//! way. A definitive refutation needs an engine that produces a re-checkable witness (Kani).
//!
//! **What cannot be lowered is said, not approximated.** The gate guarantees a category-1
//! requirement is temporal-free (REQ024), so the target is small: `always`/`never` over
//! boolean combinations, optionally quantified. Anything this module cannot faithfully
//! express — a scope, a guard, an argument that is not a variable the claim ranges over — is a
//! [`NotLowerable`], which becomes an honest `unknown`.
//!
//! Implements: REQ031 (wire Creusot as cat-1 engine #2 — a grounded invariant earns a real
//! `proven` verdict), REQ064 (a crashed prover is reported as a crash, no cause is asserted that
//! was not established, and the crash report it drops in the subject is cleaned up — see #153).

use crate::grounding::Binding;
use crate::lowering::{self, LoweredClaim};
pub use crate::lowering::{harness_name, NotLowerable};
use crate::prl::ast::Requirement;
use crate::rust_adapter::{Resolution, TypeResolution};
use crate::verdict::{Basis, Evidence};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A generated Creusot proof harness. `name` is both the `proof` function name and the module
/// (file stem) it is written to, so it cannot collide with the subject's own items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Harness {
    pub name: String,
    pub source: String,
}

/// What running Creusot established. Note the **two**-valued shape (contrast Kani's three):
/// a deductive prover confirms or fails-to-decide, but its failure-to-decide is not a
/// refutation — there is no counterexample to carry, so there is no `Fails`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Proved — the obligation was discharged, so the claim holds for all executions
    /// (spec-relative). This is `proven`, the strongest D8 rung.
    Holds,
    /// The prover ran but did not discharge the obligation, or the harness did not compile.
    /// D10's `inconclusive(…)`. NOT a refutation: an unproved deductive goal may be false or
    /// merely under-annotated, and either way it is not evidence the claim is wrong.
    Inconclusive { reason: String },
}

impl Outcome {
    /// Map what Creusot established into a piece of [`Evidence`]. The mapping lives here, in
    /// the engine, so [`crate::verdict`] never learns what Creusot is (D2's "one meaning,
    /// lowering to each engine" runs in this direction too). The load-bearing line is `Holds`
    /// → [`Basis::Proven`]: a deductive proof is `∀`-executions, never bounded.
    pub fn into_evidence(&self) -> Evidence {
        match self {
            Outcome::Holds => Evidence::holds("Creusot", Basis::Proven),
            Outcome::Inconclusive { reason } => {
                Evidence::inconclusive("Creusot", vec![reason.clone()])
            }
        }
    }
}

/// Lower a gated category-1 requirement to a Creusot proof harness.
///
/// The claim itself is lowered by the shared [`crate::lowering`] core (prefix `crate`, since this
/// harness is a module *inside* the subject crate, reaching the subject through `crate::…`); this
/// function owns only the Creusot wrapper — a `proof_assert! { forall<> }` per property.
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
        "// Generated by provreq — do not edit; it is rewritten on every `verify` and \
         removed afterwards.\n\
         //\n\
         // An ADDITIVE Creusot proof harness: a module inside the subject crate that asserts \
         the\n\
         // invariant as a pearlite `forall`. The subject's own code is untouched. The \
         `mod` line\n\
         // in the crate root is `#[cfg(creusot)]`, so an ordinary `cargo build`/`cargo test` \
         never\n\
         // sees this file.\n\
         #![allow(unused)]\n\
         use creusot_std::prelude::*;\n\
         \n\
         pub fn {name}() {{\n\
         {body}}}\n"
    );
    Ok(Harness {
        name: name.to_string(),
        source,
    })
}

/// Wrap one lowered claim as a Creusot `proof_assert!`. A quantified claim becomes a pearlite
/// `forall` over the sort's type (what makes it a ∀ proof rather than a spot check); an
/// unquantified one (e.g. `never overdrawn`) asserts the ground fact directly.
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
        format!("forall<{binders}> {}", claim.claim)
    };
    format!("    proof_assert! {{ {body} }};\n")
}

/// The installed Creusot's own canonical prover configuration — the very file `cargo creusot
/// init` copies into a project (`creusot-install` places it in the data dir). When a subject
/// has no `why3find.json`, provreq copies THIS in rather than embedding a hand-rolled prover
/// list: the operator's actual prover set is honored, and provreq never second-guesses the
/// toolchain (which is where the cvc4→cvc5 prover migration belongs — see the vendored
/// `creusot-linux-aarch64-provers.patch`, not here). Resolved from `CREUSOT_DATA` (what the
/// install sets), falling back to the XDG data dir, then `~/.local/share`. `None` when Creusot
/// is not installed/configured — which makes an honest `inconclusive`, never a guessed config.
fn install_why3find_config() -> Option<PathBuf> {
    let data_dir = std::env::var_os("CREUSOT_DATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(|x| PathBuf::from(x).join("creusot")))
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/creusot"))
        })?;
    let cfg = data_dir.join("why3find.json");
    cfg.exists().then_some(cfg)
}

/// The subject's crate-root source file (`src/lib.rs`, else `src/main.rs`). The harness `mod`
/// declaration is appended here. `None` when the subject has neither — then Creusot has no
/// crate to attach the harness to, which is an honest `inconclusive`.
fn crate_root(subject_root: &Path) -> Option<PathBuf> {
    for rel in ["src/lib.rs", "src/main.rs"] {
        let p = subject_root.join(rel);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Write the harness into the subject as a `#[cfg(creusot)]` module, run `cargo creusot`, and
/// restore the subject exactly as it was.
///
/// Additive discipline, mirroring the Kani slice: the subject's own code is never edited (only
/// a `mod` line is appended to the crate root and then removed); an existing file is never
/// clobbered; and every artifact provreq *created* — the harness file, a supplied
/// `why3find.json`, and Creusot's `verif/` and `.why3find/` output — is removed afterward,
/// while anything that was already there is left untouched. Cleanup runs on every path.
///
/// `// ponytail: `.why3find/` is the prover-calibration cache; removing it means each verify
/// recalibrates (a few seconds). Correct-and-clean over fast for a first slice — cache it in
/// provreq's own dir if verify latency ever matters.`
pub fn run(subject_root: &Path, harness: &Harness) -> Outcome {
    let Some(root_file) = crate_root(subject_root) else {
        return Outcome::Inconclusive {
            reason: "the subject has no src/lib.rs or src/main.rs, so Creusot has no crate to \
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

    // Creusot needs a why3find.json (prover config). If the subject already has one, respect it
    // (the operator's config wins). If not, copy the INSTALL's own canonical config — never a
    // hand-rolled prover list. If neither exists, that is an honest inconclusive, resolved BEFORE
    // any mutation so there is nothing to clean up on that path.
    let why3find = subject_root.join("why3find.json");
    let why3find_created = !why3find.exists();
    let config_source = if why3find_created {
        match install_why3find_config() {
            Some(src) => Some(src),
            None => {
                return Outcome::Inconclusive {
                    reason: "the subject has no why3find.json and Creusot's own prover \
                             configuration could not be found (set CREUSOT_DATA, or install \
                             Creusot) — provreq will not guess a prover set"
                        .to_string(),
                }
            }
        }
    } else {
        None
    };
    // Remember what already existed, so cleanup removes only what provreq creates.
    let verif_dir = subject_root.join("verif");
    let verif_created = !verif_dir.exists();
    let cache_dir = subject_root.join(".why3find");
    let cache_created = !cache_dir.exists();
    // A crashing prover drops a `rustc-ice-*.txt` report in the subject root, named by timestamp
    // and pid — an artifact provreq's run caused and so must remove. Snapshotted rather than
    // glob-deleted afterwards, because an operator's own earlier crash report is theirs to keep.
    let ice_before = ice_reports(subject_root);

    // Mutate: harness file, then the `mod` line, then the prover config.
    if let Err(e) = std::fs::write(&harness_path, &harness.source) {
        return Outcome::Inconclusive {
            reason: format!(
                "could not write the harness to {}: {e}",
                harness_path.display()
            ),
        };
    }
    let with_mod = format!("{original_root}\n#[cfg(creusot)]\nmod {};\n", harness.name);
    if let Err(e) = std::fs::write(&root_file, &with_mod) {
        let _ = std::fs::remove_file(&harness_path);
        return Outcome::Inconclusive {
            reason: format!(
                "could not attach the harness module to {}: {e}",
                root_file.display()
            ),
        };
    }
    if let Some(src) = &config_source {
        let _ = std::fs::copy(src, &why3find);
    }

    let output = std::process::Command::new("cargo")
        .arg("creusot")
        .current_dir(subject_root)
        .output();

    // Restore before interpreting anything, so an early return cannot leak the harness. Only
    // artifacts provreq created are removed; an existing verif/why3find is the operator's.
    let _ = std::fs::write(&root_file, &original_root);
    let _ = std::fs::remove_file(&harness_path);
    if why3find_created {
        let _ = std::fs::remove_file(&why3find);
    }
    if verif_created {
        let _ = std::fs::remove_dir_all(&verif_dir);
    }
    if cache_created {
        let _ = std::fs::remove_dir_all(&cache_dir);
    }
    for ice in ice_reports(subject_root).difference(&ice_before) {
        let _ = std::fs::remove_file(ice);
    }

    match output {
        Ok(o) => classify(&format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )),
        Err(e) => Outcome::Inconclusive {
            reason: format!("could not run `cargo creusot`: {e}"),
        },
    }
}

/// The prover-crash reports (`rustc-ice-*.txt`) sitting in the subject root right now. Compared
/// before and after a run so cleanup removes the ones this run caused and none of the operator's.
fn ice_reports(root: &Path) -> BTreeSet<PathBuf> {
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("rustc-ice-") && n.ends_with(".txt"))
        })
        .collect()
}

/// Map Creusot's output to an outcome. Pure and separately tested — the mapping is where a
/// verdict could silently become dishonest, so it must be checkable without running Creusot.
///
/// The order matters: a build failure and an unproved goal are both checked BEFORE the success
/// marker, because a run can print `Proved` for one goal and `✘` for another — a partial proof
/// is not a proof. And an unproved goal is `inconclusive`, never `fails`: a deductive prover's
/// "could not prove" is not a counterexample.
pub fn classify(output: &str) -> Outcome {
    // A CRASHED prover is its own outcome, checked first: an ICE also prints `could not compile`,
    // so the build-error branch below would otherwise claim the harness is at fault and volunteer
    // a cause (`#[logic]`) that has nothing to do with it. Asserting a cause the tool has not
    // established is the D8 overclaim pointed at diagnosis instead of verdicts (REQ064).
    if output.contains("the compiler unexpectedly panicked")
        || output.contains("internal error: entered unreachable code")
    {
        return Outcome::Inconclusive {
            reason: "Creusot's compiler crashed while translating the subject (internal error) — \
                     a defect in the prover, not a missing annotation in the subject. Measured \
                     trigger as of Creusot 0.12/0.13: any `async fn` in the crate is enough, \
                     because an async body reports as a closure but is a coroutine. No contract \
                     the operator writes changes this"
                .to_string(),
        };
    }
    if output.contains("Compilation failed") || output.contains("could not compile") {
        return Outcome::Inconclusive {
            reason: build_error(output),
        };
    }
    if output.contains("unproved") || output.contains('✘') {
        return Outcome::Inconclusive {
            reason: "Creusot could not discharge the proof obligation — the invariant may be \
                     false, or its predicates may need stronger logic contracts for the prover \
                     to see inside them"
                .to_string(),
        };
    }
    if output.contains("Proved") && output.contains('✔') {
        return Outcome::Holds;
    }
    Outcome::Inconclusive {
        reason: tail(output),
    }
}

/// The first compiler error line, which names the actionable cause (a predicate that is not
/// `#[logic]`, a type mismatch) — the top of a rustc diagnostic, not the boilerplate tail.
fn build_error(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("error[") || l.starts_with("error:"))
        .map(|l| {
            // The hint is offered as a possibility, not asserted as the cause: this branch sees
            // every compile error, and a type mismatch is not an opaque predicate (REQ064).
            format!(
                "the proof harness did not compile — {l} (if that error names a call the \
                     prover cannot see into, the predicate may need `#[logic]`)"
            )
        })
        .unwrap_or_else(|| tail(output))
}

/// The last few non-empty lines of engine output — enough for the operator to see why Creusot
/// could not decide, without pasting a whole log into the verdict.
fn tail(output: &str) -> String {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(TAIL_LINES);
    let tail = lines[start..].join("\n");
    if tail.trim().is_empty() {
        "`cargo creusot` produced no recognisable verdict".to_string()
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

    /// Sort resolutions for the fixtures: `User` is a type at the crate root, so the harness names
    /// it `<prefix>::User` (REQ061 — the module comes from where the adapter found it).
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
                text: "fn f() -> bool { true }".into(),
                module: Some(vec![]),
            },
            params,
            form: PredicateForm::Function,
        }
    }

    /// Both predicates take the sort by value — the `#[logic]` idiom a Creusot subject uses.
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

    // Verifies: REQ031 — a quantified cat-1 invariant lowers to a `proof_assert!` with a
    // pearlite `forall` over the sort's real type, calling the subject's predicates via
    // `crate::` (the harness is in-crate, unlike Kani's tests/ harness).
    #[test]
    fn quantified_invariant_lowers_to_a_forall_proof_assert() {
        let h = lower_standard().expect("should lower");
        assert_eq!(h.name, "provreq_req001");
        assert!(
            h.source.contains("use creusot_std::prelude::*;"),
            "{}",
            h.source
        );
        assert!(
            h.source.contains(
                "proof_assert! { forall<u: crate::User> \
                 (!(crate::logged_in(u)) || crate::has_session(u)) };"
            ),
            "the claim must lower to a forall over the subject's real predicates: {}",
            h.source
        );
    }

    // Verifies: REQ031 — the harness reaches the subject through `crate::`, NOT through a
    // crate name; it lives inside the subject crate as a module.
    #[test]
    fn calls_go_through_crate_not_a_crate_name() {
        let h = lower_standard().expect("should lower");
        assert!(h.source.contains("crate::logged_in"), "{}", h.source);
        assert!(!h.source.contains("subject::"), "{}", h.source);
    }

    // Verifies: REQ031 — a by-ref `#[logic]` predicate is called with `&u`, matching the
    // subject's real signature (the adapter's ParamMode).
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

    // Verifies: REQ031 — `never P` is `always not P`, and an unquantified claim asserts the
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
            &sorts(),
            "h",
        )
        .expect("should lower");
        assert!(
            h.source
                .contains("proof_assert! { !(crate::is_overdrawn()) };"),
            "{}",
            h.source
        );
        assert!(
            !h.source.contains("forall<"),
            "no quantifier, no forall binder: {}",
            h.source
        );
    }

    // Verifies: REQ031 — an unbound sort cannot be quantified over, so the requirement does
    // not lower rather than silently becoming an unquantified spot check.
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
        assert!(e.reason.contains("no domain"), "{}", e.reason);
    }

    // Verifies: REQ031 — an unresolved predicate does not lower. Absence of a resolution is
    // not evidence a call would compile, let alone be the right one.
    #[test]
    fn unresolved_predicate_does_not_lower() {
        let e = lower(
            &req(CODE_REQ),
            &standard_bindings(),
            &BTreeMap::from([("logged_in".to_string(), resolved(vec![ParamMode::ByValue]))]),
            &sorts(),
            "h",
        )
        .expect_err("has_session never resolved");
        assert!(e.reason.contains("has_session"), "{}", e.reason);
    }

    // Verifies: REQ031 — a temporal pattern does not lower. The gate rejects these at
    // category 1 (REQ024), but `lower` is public and must not assume it was called.
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
        assert!(e.reason.contains("temporal-free"), "{}", e.reason);
    }

    // Verifies: REQ031 — Creusot's explicit success marker is the ONLY thing read as a proof.
    #[test]
    fn proved_output_is_holds() {
        assert_eq!(
            classify("Proved (verif/csmoke_rlib/provreq_check/provreq_check.coma) ✔\n"),
            Outcome::Holds
        );
    }

    // Verifies: REQ031 (the honesty crux) — an UNPROVED goal is `inconclusive`, NEVER a
    // `fails`. A deductive prover's failure to discharge is not a counterexample.
    #[test]
    fn unproved_goal_is_inconclusive_never_fails() {
        let Outcome::Inconclusive { reason } =
            classify("Goal Coma.vc_provreq_check: ✘\nError: 1 unproved file\n")
        else {
            panic!("an unproved deductive goal must be inconclusive, not a refutation");
        };
        assert!(
            reason.contains("could not discharge"),
            "the reason must not read as a refutation: {reason}"
        );
    }

    // Verifies: REQ031 — a partial run (one goal proved, another unproved) is NOT a proof.
    // The order of checks in `classify` guarantees the `✘` wins over the `Proved` line.
    #[test]
    fn a_partial_proof_is_not_holds() {
        let output = "Proved (verif/x/a.coma) ✔\nGoal Coma.vc_b: ✘\nError: 1 unproved file\n";
        assert!(matches!(classify(output), Outcome::Inconclusive { .. }));
    }

    // Verifies: REQ031 — an opaque predicate (an ordinary `fn`, not `#[logic]`) makes the
    // harness fail to compile, which is `inconclusive` and names the actionable cause.
    #[test]
    fn a_compile_failure_is_inconclusive_and_names_the_cause() {
        let output = "error[E0308]: mismatched types\n  --> src/provreq_check.rs:5:9\n\
                      error: could not compile `csmoke` (lib) due to 2 previous errors\n\
                      Error: Compilation failed\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a harness that does not compile decides nothing");
        };
        assert!(reason.contains("did not compile"), "{reason}");
        assert!(
            reason.contains("`#[logic]`"),
            "must point at the fix: {reason}"
        );
    }

    // Verifies: REQ064 — a crashed prover is reported as a crashed prover. Real output, measured
    // on this repo 2026-07-30 (#153): an ICE also prints `could not compile`, so before this
    // branch existed the operator was told the harness was at fault and handed a `#[logic]` cause
    // the tool had not established. The claim under check was never even reached.
    #[test]
    fn a_prover_crash_is_not_reported_as_the_subjects_fault() {
        let output = "thread 'rustc' (1223) panicked at \
                      creusot/src/translation/specification.rs:423:61:\n\
                      internal error: entered unreachable code\n\
                      error: the compiler unexpectedly panicked. This is a bug\n\
                      error: could not compile `provreq` (lib); 1 warning emitted\n\
                      Error: Compilation failed\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a crashed prover decides nothing");
        };
        assert!(reason.contains("crashed"), "{reason}");
        assert!(
            reason.contains("not a missing annotation"),
            "must not blame the subject: {reason}"
        );
        assert!(
            !reason.contains("did not compile"),
            "must not be read as a harness build failure: {reason}"
        );
    }

    // Verifies: REQ064 — the crash report a run causes is removed, and one the operator already
    // had is not. `run` is what cleans up, but the discrimination is `ice_reports`' snapshot, so
    // that is what this checks — no prover needed.
    #[test]
    fn crash_reports_are_told_apart_from_the_operators_own() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let theirs = tmp.path().join("rustc-ice-2026-01-01T00_00_00-1.txt");
        std::fs::write(&theirs, "an earlier crash, the operator's").expect("write");
        let before = ice_reports(tmp.path());
        let ours = tmp.path().join("rustc-ice-2026-07-30T02_50_26-1222.txt");
        std::fs::write(&ours, "caused by this run").expect("write");
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]").expect("write");

        let new: Vec<_> = ice_reports(tmp.path())
            .difference(&before)
            .cloned()
            .collect();
        assert_eq!(new, vec![ours], "only the run's own report is removable");
    }

    // Verifies: REQ031 — unrecognised output (e.g. a prover error) is inconclusive with a
    // readable reason, never an optimistic pass.
    #[test]
    fn unrecognised_output_is_inconclusive() {
        let Outcome::Inconclusive { reason } = classify("") else {
            panic!("no output decides nothing");
        };
        assert!(reason.contains("no recognisable verdict"), "{reason}");
    }

    // Verifies: REQ031 — the harness name is a valid, prefixed identifier from the req id.
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

    // Verifies: REQ031 (D8) — a Creusot pass is `proven`, the strongest rung, and the
    // read-back does NOT wear the bounded caveat.
    #[test]
    fn a_creusot_pass_is_proven_and_not_bounded() {
        let v = crate::verdict::aggregate("SR001", vec![Outcome::Holds.into_evidence()], prov());
        assert_eq!(v.status, crate::verdict::Status::Holds);
        assert_eq!(v.basis, Some(Basis::Proven));
        let text = crate::verdict::render(&v);
        assert!(text.contains("proven: established deductively"), "{text}");
        assert!(!text.contains("NOT proven for all executions"), "{text}");
    }

    // Verifies: REQ031 (D10) — an inconclusive run yields unknown/inconclusive, never a
    // verdict; the engine's own message rides along.
    #[test]
    fn an_inconclusive_run_is_unknown_never_a_verdict() {
        let outcome = Outcome::Inconclusive {
            reason: "Creusot could not discharge the proof obligation".into(),
        };
        let v = crate::verdict::aggregate("SR002", vec![outcome.into_evidence()], prov());
        assert_eq!(v.status, crate::verdict::Status::Unknown);
        assert_eq!(v.reason, Some(crate::verdict::UnknownReason::Inconclusive));
        assert!(crate::verdict::render(&v).contains("could not discharge"));
    }

    /// A real cargo subject: a sort and two `#[logic]` predicates over it, `has_session`'s
    /// body supplied so a test can make the invariant true or false.
    fn cargo_subject(has_session_body: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"csmoke\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\ncreusot-std = \"0.12.0\"\n\n\
             [lints.rust]\nunexpected_cfgs = { level = \"warn\", check-cfg = ['cfg(creusot)'] }\n",
        )
        .expect("manifest");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src");
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            format!(
                "use creusot_std::prelude::*;\n\
                 pub struct User {{ pub logged_in: bool, pub id: u64 }}\n\
                 #[logic]\n\
                 pub fn logged_in(u: User) -> bool {{ pearlite! {{ u.logged_in }} }}\n\
                 #[logic]\n\
                 pub fn has_session(u: User) -> bool {{ pearlite! {{ {has_session_body} }} }}\n"
            ),
        )
        .expect("lib.rs");
        tmp
    }

    /// The harness for the `cargo_subject` fixture, which every real-engine test shares.
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

    // Verifies: REQ031 — THE REAL ENGINE, end to end: a true invariant over a real Creusot
    // subject is PROVED and earns `proven`.
    //
    // `#[ignore]` is deliberate, not neglect (R-eng-2): the common user state is
    // engine-ABSENT, and that path is the one most worth proving continuously — so CI's main
    // `test` job stays Creusot-free and a separate `creusot` job runs `-- --ignored`.
    #[test]
    #[ignore = "needs Creusot installed — run via `cargo test -- --ignored` (the CI `creusot` job)"]
    fn real_creusot_proves_a_true_invariant() {
        // has_session = logged_in || id==0 → the invariant !logged_in||has_session is a tautology.
        let tmp = cargo_subject("u.logged_in || u.id == 0u64");
        let outcome = run(tmp.path(), &smoke_harness());
        assert_eq!(outcome, Outcome::Holds, "a true invariant must be proved");
    }

    // Verifies: REQ031 (the honesty crux) — THE REAL ENGINE on a FALSE invariant is
    // `inconclusive`, NEVER a proof and NEVER a `fails`. Creusot cannot discharge the goal;
    // that is not a counterexample.
    #[test]
    #[ignore = "needs Creusot installed — run via `cargo test -- --ignored` (the CI `creusot` job)"]
    fn real_creusot_cannot_prove_a_false_invariant() {
        // has_session = logged_in && id!=7 → false at logged_in=true, id=7.
        let tmp = cargo_subject("u.logged_in && u.id != 7u64");
        let outcome = run(tmp.path(), &smoke_harness());
        assert!(
            matches!(outcome, Outcome::Inconclusive { .. }),
            "a false invariant must NOT be proved, and Creusot yields no witness so it is \
             inconclusive, got {outcome:?}"
        );
    }

    // Verifies: REQ031 — THE REAL ENGINE on opaque predicates (ordinary `fn`, not `#[logic]`):
    // the harness cannot compile, so the verdict is `inconclusive`, never wrong.
    #[test]
    #[ignore = "needs Creusot installed — run via `cargo test -- --ignored` (the CI `creusot` job)"]
    fn real_creusot_is_inconclusive_on_opaque_predicates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"csmoke\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\ncreusot-std = \"0.12.0\"\n\n\
             [lints.rust]\nunexpected_cfgs = { level = \"warn\", check-cfg = ['cfg(creusot)'] }\n",
        )
        .expect("manifest");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src");
        // Ordinary program fns — pearlite cannot call them.
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            "pub struct User { pub logged_in: bool, pub id: u64 }\n\
             pub fn logged_in(u: User) -> bool { u.logged_in }\n\
             pub fn has_session(u: User) -> bool { u.logged_in }\n",
        )
        .expect("lib.rs");
        let outcome = run(tmp.path(), &smoke_harness());
        assert!(
            matches!(outcome, Outcome::Inconclusive { .. }),
            "got {outcome:?}"
        );
    }

    // Verifies: REQ031 — provreq leaves no litter in someone else's repo. The harness file,
    // the appended `mod` line, and Creusot's verif//.why3find outputs are gone afterward, and
    // the crate root is byte-for-byte what it was.
    #[test]
    #[ignore = "needs Creusot installed — run via `cargo test -- --ignored` (the CI `creusot` job)"]
    fn real_creusot_run_leaves_no_trace_in_the_subject() {
        let tmp = cargo_subject("u.logged_in || u.id == 0u64");
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
        assert!(
            !tmp.path().join("why3find.json").exists(),
            "supplied why3find must be gone"
        );
        assert!(
            !tmp.path().join("verif").exists(),
            "verif/ provreq created must be gone"
        );
        assert!(
            !tmp.path().join(".why3find").exists(),
            ".why3find/ provreq created must be gone"
        );
    }

    // Verifies: REQ031 — an existing file is NEVER clobbered. provreq writes into someone
    // else's repo, so a name collision must stop the run, not overwrite their work.
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

    // Verifies: REQ031 — a subject that is not a cargo crate (no src root) is honest
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
