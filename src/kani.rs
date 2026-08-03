//! Kani — the category-1 engine, **#1: first, not only**.
//!
//! D2 gives the core one meaning and lowers it to each engine; Kani is **lowering #1**, not
//! the definition. The binding stays core-owned ([`crate::grounding`]), the language stays
//! the adapter's ([`crate::rust_adapter`]), and this module owns exactly one thing: how a
//! gated category-1 requirement becomes something Kani can run, and what its answer means.
//!
//! **Why Kani first** (settled 2026-07-17): it takes **additive proof harnesses** — a
//! generated file that imports the subject's public API — so it never forces the "does
//! provreq write annotations into the subject's own code?" decision. Prusti/Creusot force
//! it immediately; Verus needs the subject *written in* its Rust subset, a rewrite that
//! contradicts the adopt-existing-repos premise. That decision stays open, to be made
//! deliberately when `proven` is worth it.
//!
//! **Honest by construction (D8).** Kani is a *bounded* model checker, so a pass is
//! `model-checked (bounded)` and **never** `proven`: it establishes the claim over the
//! states it explored, not over all executions. A failure is the robust half — a real
//! counterexample, which is D9's re-checkable witness. Everything else (a harness that will
//! not compile, a timeout) is `unknown` with a reason, never a verdict.
//!
//! **What cannot be lowered is said, not approximated.** The gate already guarantees a
//! category-1 requirement is temporal-free (REQ024), so the target is small: `always`/`never`
//! over boolean combinations, optionally quantified. Anything this module cannot faithfully
//! express — a scope, a guard, an argument that is not a variable the claim ranges over — is a
//! [`NotLowerable`], which becomes an honest `unknown`. D2's rule is that an out-of-fragment
//! operator is "a typed error surfaced to the author, never a silent approximation".
//!
//! Implements: REQ027 (wire Kani as cat-1 engine #1 — a grounded invariant earns a real
//! verdict), REQ065 (a sort Kani cannot instantiate is reported as the subject-side precondition
//! it is, along with what it does not block — see #148).

use crate::grounding::Binding;
use crate::lowering::{self, LoweredClaim};
pub use crate::lowering::{harness_name, NotLowerable};
use crate::prl::ast::Requirement;
use crate::rust_adapter::{Resolution, TypeResolution};
use crate::verdict::{Basis, Evidence};
use std::collections::BTreeMap;
use std::path::Path;

/// A generated Kani proof harness. `name` is both the `#[kani::proof]` function name and
/// the file stem it is written to, so `--harness <name>` selects exactly this claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Harness {
    pub name: String,
    pub source: String,
}

/// What running Kani established. Mirrors D7's three-valued polarity: the engine may
/// confirm, refute (with a witness), or fail to decide — and failing to decide is a first
/// class answer, not an error to swallow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Verified over the states Kani explored. Bounded — this is `model-checked`, never
    /// `proven`.
    Holds,
    /// Refuted. `failed_check` is Kani's own description of the violated assertion;
    /// `witness` is the concrete counterexample as a runnable replay test (D9), when Kani
    /// produced one.
    Fails {
        failed_check: Option<String>,
        witness: Option<String>,
    },
    /// The engine ran but could not decide — the harness did not compile, or Kani errored.
    /// D10's `inconclusive(…)`.
    Inconclusive { reason: String },
}

/// The two knobs that decide what a bounded `holds` is worth: how deep loops are unwound, and how
/// long Kani may take before it is cut off. Read per subject from `provreq.yml` (#117).
///
/// Both default to `None`, which means *Kani's own default* — the shipped behaviour, unchanged.
/// That is a real fallback, not a placeholder: provreq has no basis for choosing a bound that suits
/// someone else's loops. But it is also the case the operator most needs told about, which is why
/// [`Bounds::describe`] never renders silence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bounds {
    /// `--default-unwind`: the loop-unwinding depth CBMC uses for every loop without its own bound.
    pub default_unwind: Option<u32>,
    /// `--harness-timeout`, in seconds. Kani's flag is experimental, so setting this also passes
    /// `-Z unstable-options`; leaving it unset keeps provreq off the unstable surface entirely.
    pub timeout_seconds: Option<u64>,
}

impl Bounds {
    /// Read the `kani:` block from the companion manifest. A manifest that is missing, unparseable,
    /// or silent on Kani yields the defaults — a subject that never configured a bound is the
    /// normal case, not an error.
    pub fn load(companion_root: &Path) -> Bounds {
        #[derive(serde::Deserialize)]
        struct ManifestKani {
            #[serde(default)]
            kani: Option<Bounds_>,
        }
        #[derive(serde::Deserialize)]
        struct Bounds_ {
            #[serde(default)]
            default_unwind: Option<u32>,
            #[serde(default)]
            timeout_seconds: Option<u64>,
        }
        let Ok(text) = std::fs::read_to_string(companion_root.join(crate::adopt::MANIFEST_FILE))
        else {
            return Bounds::default();
        };
        let Ok(manifest) = serde_yaml::from_str::<ManifestKani>(&text) else {
            return Bounds::default();
        };
        manifest
            .kani
            .map(|k| Bounds {
                default_unwind: k.default_unwind,
                timeout_seconds: k.timeout_seconds,
            })
            .unwrap_or_default()
    }

    /// The bounds this run used, in one line, for the verdict that depends on them.
    ///
    /// A bounded `holds` means "no counterexample **within these bounds**", so the bounds are part
    /// of what the verdict says — and an unset bound is the case where that matters most, because
    /// the depth the claim was checked to is then Kani's choice and the operator never made it.
    /// Reporting only configured values would hide exactly the ones nobody chose, so both are
    /// always named.
    pub fn describe(&self) -> String {
        let unwind = match self.default_unwind {
            Some(n) => format!("unwind {n} (`kani.default_unwind` in provreq.yml)"),
            None => "unwind: Kani's own default — no `kani.default_unwind` is set, so the depth \
                     this was checked to is Kani's choice, not yours"
                .to_string(),
        };
        let timeout = match self.timeout_seconds {
            Some(s) => format!("timeout {s}s (`kani.timeout_seconds` in provreq.yml)"),
            None => "no timeout".to_string(),
        };
        format!("bounded by — {unwind}; {timeout}")
    }

    /// The `cargo kani` arguments these bounds add. Empty when nothing is configured, which keeps
    /// an unconfigured subject on exactly the command line provreq shipped with.
    fn args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(n) = self.default_unwind {
            args.push("--default-unwind".to_string());
            args.push(n.to_string());
        }
        if let Some(s) = self.timeout_seconds {
            // Kani gates `--harness-timeout` behind its unstable surface; ask for it only when the
            // operator did, so an unconfigured subject never runs an experimental code path.
            args.push("-Z".to_string());
            args.push("unstable-options".to_string());
            args.push("--harness-timeout".to_string());
            args.push(s.to_string());
        }
        args
    }
}

impl Outcome {
    /// Map what Kani established into a piece of [`Evidence`]. The mapping lives here, in the
    /// engine, so [`crate::verdict`] never learns what Kani is — D2's "one meaning, lowering
    /// to each engine" runs in this direction too. The core then aggregates this evidence
    /// (alongside any other engine's) into the requirement's verdict (D2b).
    ///
    /// The load-bearing line is `Holds` → [`Basis::ModelCheckedBounded`]: Kani is bounded,
    /// so a pass is `model-checked (bounded)` and never `proven`.
    ///
    /// `bounds` rides along because a bounded pass is only meaningful against the bounds it ran
    /// under (#117) — so they are attached to the `holds`, never left for the operator to assume.
    pub fn into_evidence(&self, bounds: &Bounds) -> Evidence {
        match self {
            Outcome::Holds => {
                let mut evidence = Evidence::holds("Kani", Basis::ModelCheckedBounded);
                evidence.detail.push(bounds.describe());
                evidence
            }
            Outcome::Fails {
                failed_check,
                witness,
            } => Evidence::fails(
                "Kani",
                witness.clone(),
                failed_check.iter().cloned().collect(),
            ),
            Outcome::Inconclusive { reason } => {
                Evidence::inconclusive("Kani", vec![reason.clone()])
            }
        }
    }
}

/// Lower a gated category-1 requirement to a Kani proof harness.
///
/// Pure — the caller resolves the bindings and passes them in, so the whole lowering is
/// testable without Kani installed, which is what lets CI prove the engine-absent path
/// continuously (R-eng-2).
///
/// `crate_name` is the subject's own crate, since the harness lives in `tests/` and reaches
/// the subject through its **public** API. A predicate that is not public is not reachable
/// from there; the harness then fails to compile and the verdict is honestly `unknown`
/// rather than wrong.
pub fn lower(
    req: &Requirement,
    crate_name: &str,
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
        let claim = lowering::lower_property(
            req,
            prop,
            crate_name,
            bindings,
            resolutions,
            sort_resolutions,
        )?;
        body.push_str(&assertion(&claim));
    }
    let source = format!(
        "// Generated by provreq — do not edit; it is rewritten on every `verify` and \
         removed afterwards.\n\
         //\n\
         // An ADDITIVE proof harness: it imports `{crate_name}`'s public API and changes \
         nothing in\n\
         // the subject's own code. `#[cfg(kani)]` keeps it out of the subject's ordinary \
         `cargo test`.\n\
         #![allow(unused)]\n\
         \n\
         #[cfg(kani)]\n\
         #[kani::proof]\n\
         fn {name}() {{\n\
         {body}}}\n"
    );
    Ok(Harness {
        name: name.to_string(),
        source,
    })
}

/// Wrap one lowered claim as a Kani assertion. Each property gets its own scope so its quantified
/// variable cannot leak into another's. `kani::any()` is what makes a quantified claim a ∀ over the
/// sort rather than a spot check: the variable is unconstrained, so Kani explores the whole
/// (bounded) domain.
fn assertion(claim: &LoweredClaim) -> String {
    let mut out = String::from("    {\n");
    for q in &claim.quantified {
        out.push_str(&format!("        let {}: {} = kani::any();\n", q.var, q.ty));
    }
    out.push_str(&format!("        assert!({});\n", claim.claim));
    out.push_str("    }\n");
    out
}

/// The subject's crate name, as the harness must spell it (`-` becomes `_`). Read from
/// `cargo metadata` rather than parsed by hand, so a workspace or an unusual manifest is the
/// cargo team's problem, not ours. `None` when the subject is not a cargo crate at all.
pub fn subject_crate_name(subject_root: &Path) -> Option<String> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(subject_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let name = meta.get("packages")?.get(0)?.get("name")?.as_str()?;
    Some(name.replace('-', "_"))
}

/// Write the harness into the subject, run Kani against it, and remove it again.
///
/// The harness is **additive**: a new file under `tests/`, nothing in the subject's own code
/// is touched, and an existing file is never clobbered. It is removed on every path,
/// including failure — provreq must not leave litter in someone else's repo.
///
/// `bounds` are the subject's own (#117), defaulting to Kani's when the manifest says nothing.
pub fn run(subject_root: &Path, harness: &Harness, bounds: &Bounds) -> Outcome {
    let tests_dir = subject_root.join("tests");
    // Remembered so cleanup can put the subject back exactly as it was: a `tests/` provreq
    // created is provreq's to remove, and one that was already there is not.
    let tests_dir_existed = tests_dir.exists();
    if let Err(e) = std::fs::create_dir_all(&tests_dir) {
        return Outcome::Inconclusive {
            reason: format!("could not create {}: {e}", tests_dir.display()),
        };
    }
    let path = tests_dir.join(format!("{}.rs", harness.name));
    if path.exists() {
        return Outcome::Inconclusive {
            reason: format!(
                "{} already exists — refusing to overwrite a file provreq did not write",
                path.display()
            ),
        };
    }
    if let Err(e) = std::fs::write(&path, &harness.source) {
        return Outcome::Inconclusive {
            reason: format!("could not write the harness to {}: {e}", path.display()),
        };
    }

    let output = std::process::Command::new("cargo")
        .args(["kani", "--tests", "--harness", &harness.name])
        // The concrete counterexample is D9's re-checkable witness, and it is the whole
        // value of a `fails`. Unstable, hence -Z.
        .args(["-Z", "concrete-playback", "--concrete-playback=print"])
        .args(bounds.args())
        .current_dir(subject_root)
        .output();

    // Remove the harness before interpreting anything, so an early return cannot leak it.
    let _ = std::fs::remove_file(&path);
    if !tests_dir_existed {
        // Only if provreq created it, and `remove_dir` refuses a non-empty directory anyway.
        let _ = std::fs::remove_dir(&tests_dir);
    }

    match output {
        Ok(o) => classify(&format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )),
        Err(e) => Outcome::Inconclusive {
            reason: format!("could not run `cargo kani`: {e}"),
        },
    }
}

/// Map Kani's output to an outcome. Pure and separately tested — the mapping is where a
/// verdict could silently become dishonest, so it must be checkable without running Kani.
///
/// Note the default is [`Outcome::Inconclusive`]: only Kani's own explicit verdict line is
/// read as an answer. Unrecognised output is never optimistically treated as a pass.
pub fn classify(output: &str) -> Outcome {
    if output.contains("VERIFICATION:- SUCCESSFUL") {
        return Outcome::Holds;
    }
    if output.contains("VERIFICATION:- FAILED") {
        return Outcome::Fails {
            failed_check: failed_check(output),
            witness: witness(output),
        };
    }
    Outcome::Inconclusive {
        reason: diagnostic(output),
    }
}

/// Why the engine could not decide, in the operator's terms.
///
/// The compiler's own `error…` lines are preferred over the tail of the log, because the
/// cause is stated at the TOP of a rustc diagnostic and the bottom is boilerplate: a
/// trailing slice of a failed Kani run shows a list of candidate trait impls and a `rustc
/// --explain` footer, while the line that tells the operator what to do — "the trait bound
/// `User: kani::Arbitrary` is not satisfied" — has already scrolled past.
fn diagnostic(output: &str) -> String {
    // A sort Kani cannot instantiate is a PRECONDITION on the subject, not a defect in the
    // binding, the sort, or the claim — and the tool knows that, so it must not hand over the
    // raw trait-bound error and let the operator infer it (REQ065). It is also a precondition of
    // *this* engine only: a deductive `forall` ranges over a type without ever constructing a
    // value, so saying nothing about that invites the conclusion that the claim is unreachable
    // when only this route to it is.
    if let Some(sort) = uninstantiable_sort(output) {
        return format!(
            "the sort `{sort}` cannot be instantiated by Kani — a bounded model checker needs a \
             concrete value for every quantified variable, so a type it cannot enumerate stops \
             the harness before the claim is reached. Nothing is wrong with the binding, the \
             sort, or the claim. Either implement `kani::Arbitrary` for `{sort}` in the subject \
             (an adoption step the subject owns, like a contracts dependency — provreq will not \
             write it for you), or reach this claim through a deductive engine, whose `forall` is \
             logical and needs no value at all"
        );
    }
    let errors: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("error[") || l.starts_with("error:"))
        .take(ERROR_LINES)
        .collect();
    if errors.is_empty() {
        tail(output)
    } else {
        errors.join("\n")
    }
}

/// How many compiler error lines an `inconclusive` carries. The first names the cause; the
/// rest are usually "could not compile" noise.
const ERROR_LINES: usize = 2;

/// The type Kani demanded an [`Arbitrary`](https://model-checking.github.io/kani/) impl for, read
/// off its own `E0277`. Matched on the trait rather than on the message shape, so an unrelated
/// unsatisfied bound (the far more common `E0277`) is left to the generic diagnostic.
fn uninstantiable_sort(output: &str) -> Option<&str> {
    output
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("error[E0277]"))
        .find_map(|l| {
            let (bound, _) = l.split_once("the trait bound `")?.1.split_once('`')?;
            let (ty, bounded_by) = bound.rsplit_once(": ")?;
            (bounded_by == "kani::Arbitrary").then_some(ty)
        })
}

/// Kani's own description of the violated assertion.
fn failed_check(output: &str) -> Option<String> {
    output
        .lines()
        .find(|l| l.trim_start().starts_with("Failed Checks:"))
        .map(|l| l.trim().to_string())
}

/// The concrete counterexample Kani printed, as a runnable replay test (D9's re-checkable
/// witness). `None` when Kani refuted the claim without producing one.
fn witness(output: &str) -> Option<String> {
    let start = output.find("fn kani_concrete_playback")?;
    let rest = &output[start..];
    let end = rest.find("\n}")?;
    Some(rest[..end + 2].to_string())
}

/// The last few lines of engine output — enough for the operator to see why Kani could not
/// decide (a compile error names the type or the missing trait) without pasting a whole log
/// into the verdict.
fn tail(output: &str) -> String {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(TAIL_LINES);
    let tail = lines[start..].join("\n");
    if tail.trim().is_empty() {
        "`cargo kani` produced no recognisable verdict".to_string()
    } else {
        tail
    }
}

/// How many lines of engine output an `inconclusive` carries. Enough to name a compile
/// error's cause; short enough to stay a verdict rather than a log.
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
                file: "src/auth.rs".into(),
                line: 1,
                text: "fn f() -> bool { true }".into(),
                module: Some(vec![]),
            },
            params,
            form: PredicateForm::Function,
        }
    }

    /// The standard fixture: both predicates take the sort by reference.
    fn by_ref_resolutions() -> BTreeMap<String, Resolution> {
        BTreeMap::from([
            ("logged_in".to_string(), resolved(vec![ParamMode::ByRef])),
            ("has_session".to_string(), resolved(vec![ParamMode::ByRef])),
        ])
    }

    fn standard_bindings() -> Vec<Binding> {
        vec![
            binding("logged_in", "login"),
            binding("has_session", "has_session"),
            binding("User", "User"),
        ]
    }

    fn lower_standard() -> Result<Harness, NotLowerable> {
        lower(
            &req(CODE_REQ),
            "subject",
            &standard_bindings(),
            &by_ref_resolutions(),
            &sorts(),
            "provreq_req001",
        )
    }

    // Verifies: REQ027 — a quantified cat-1 invariant lowers to a proof harness that
    // instantiates the sort with `kani::any()` and asserts the claim over it. This is the
    // whole point of REQ026 binding sorts: `let u: User = kani::any()` needs a real type.
    #[test]
    fn quantified_invariant_lowers_to_a_proof_harness() {
        let h = lower_standard().expect("should lower");
        assert_eq!(h.name, "provreq_req001");
        assert!(h.source.contains("#[kani::proof]"), "{}", h.source);
        assert!(
            h.source.contains("let u: subject::User = kani::any();"),
            "the sort must be instantiated over its real type: {}",
            h.source
        );
        assert!(
            h.source
                .contains("assert!((!(subject::login(&u)) || subject::has_session(&u)));"),
            "the claim must lower to calls on the subject's real functions: {}",
            h.source
        );
    }

    // Verifies: REQ027 — the generated harness is ADDITIVE and inert in the subject's own
    // test run: `#[cfg(kani)]` keeps `cargo test` from ever seeing `kani::any()`.
    #[test]
    fn harness_is_inert_under_the_subjects_own_cargo_test() {
        let h = lower_standard().expect("should lower");
        assert!(h.source.contains("#[cfg(kani)]"), "{}", h.source);
    }

    // Verifies: REQ027 — the call matches the subject's real signature. A by-value predicate
    // must NOT be handed `&u`; that is what the adapter's ParamMode is for.
    #[test]
    fn calls_follow_the_subjects_parameter_modes() {
        let by_value = BTreeMap::from([
            ("logged_in".to_string(), resolved(vec![ParamMode::ByValue])),
            (
                "has_session".to_string(),
                resolved(vec![ParamMode::ByValue]),
            ),
        ]);
        let h = lower(
            &req(CODE_REQ),
            "subject",
            &standard_bindings(),
            &by_value,
            &sorts(),
            "h",
        )
        .expect("should lower");
        assert!(h.source.contains("subject::login(u)"), "{}", h.source);
        assert!(!h.source.contains("login(&u)"), "{}", h.source);
    }

    // Verifies: REQ027 — `never P` is `always not P`, the code fragment's other invariant.
    #[test]
    fn never_lowers_to_the_negated_invariant() {
        let r = req("requirement r {
            category: 1
            vocabulary { state overdrawn }
            require { never overdrawn }
        }");
        let h = lower(
            &r,
            "subject",
            &[binding("overdrawn", "is_overdrawn")],
            &BTreeMap::from([("overdrawn".to_string(), resolved(vec![]))]),
            &sorts(),
            "h",
        )
        .expect("should lower");
        assert!(
            h.source.contains("assert!(!(subject::is_overdrawn()));"),
            "{}",
            h.source
        );
    }

    // Verifies: REQ027 — an unbound sort cannot be instantiated, so the requirement does not
    // lower. It must never silently become an unquantified spot check, which would claim far
    // more than was checked.
    #[test]
    fn unbound_sort_does_not_lower() {
        let e = lower(
            &req(CODE_REQ),
            "subject",
            &[
                binding("logged_in", "login"),
                binding("has_session", "has_session"),
            ],
            &by_ref_resolutions(),
            &sorts(),
            "h",
        )
        .expect_err("an unbound sort has no domain");
        assert!(e.reason.contains("User"), "{}", e.reason);
        assert!(e.reason.contains("no domain"), "{}", e.reason);
    }

    // Verifies: REQ027 — an unresolved predicate does not lower. Absence of a resolution is
    // not evidence that a call would compile, let alone be the right one.
    #[test]
    fn unresolved_predicate_does_not_lower() {
        let e = lower(
            &req(CODE_REQ),
            "subject",
            &standard_bindings(),
            &BTreeMap::from([("logged_in".to_string(), resolved(vec![ParamMode::ByRef]))]),
            &sorts(),
            "h",
        )
        .expect_err("has_session never resolved");
        assert!(e.reason.contains("has_session"), "{}", e.reason);
    }

    // Verifies: REQ027/REQ059 — an argument with no domain has no value to give it, so the claim
    // does not lower rather than emitting a free name. Since REQ059 a free variable IS quantified
    // — but only over the sort the vocabulary declares for it, and this one declares none.
    #[test]
    fn argument_with_no_domain_does_not_lower() {
        let r = req("requirement r {
            category: 1
            vocabulary { state logged_in(u) }
            require { each u: User . always logged_in(other) }
        }");
        let e = lower(
            &r,
            "subject",
            &[binding("logged_in", "login"), binding("User", "User")],
            &BTreeMap::from([("logged_in".to_string(), resolved(vec![ParamMode::ByRef]))]),
            &sorts(),
            "h",
        )
        .expect_err("`other` has no declared sort to range over");
        assert!(e.reason.contains("other"), "{}", e.reason);
    }

    // Verifies: REQ027 — a temporal pattern does not lower. The gate rejects these at
    // category 1 (REQ024), but `lower` is public and must not assume it was called.
    #[test]
    fn temporal_patterns_do_not_lower() {
        let r = req("requirement r {
            category: 2b
            vocabulary { state p, q }
            require { p leads_to q }
        }");
        let e = lower(&r, "subject", &[], &BTreeMap::new(), &sorts(), "h")
            .expect_err("liveness is not an invariant");
        assert!(e.reason.contains("leads_to"), "{}", e.reason);
        assert!(e.reason.contains("temporal-free"), "{}", e.reason);
    }

    // Verifies: REQ027 — Kani's explicit success line is the ONLY thing read as a pass.
    #[test]
    fn successful_verification_is_holds() {
        assert_eq!(
            classify("Checking harness provreq_r...\nVERIFICATION:- SUCCESSFUL\n"),
            Outcome::Holds
        );
    }

    // Verifies: REQ027 (D9) — a refutation carries the violated check AND the concrete
    // counterexample as a runnable replay test, which is what makes `fails` re-checkable.
    #[test]
    fn failed_verification_is_fails_with_a_witness() {
        let output = "Failed Checks: assertion failed: !(login(&u)) || (has_session(&u))\n\
             fn kani_concrete_playback_provreq_r_123() {\n\
             \x20   let concrete_vals: Vec<Vec<u8>> = vec![\n\
             \x20       // 7\n\
             \x20       vec![7, 0, 0, 0],\n\
             \x20   ];\n\
             }\n\
             VERIFICATION:- FAILED\n";
        let Outcome::Fails {
            failed_check,
            witness,
        } = classify(output)
        else {
            panic!("must refute");
        };
        assert!(failed_check
            .expect("names the check")
            .contains("assertion failed"));
        let w = witness.expect("must carry the counterexample");
        assert!(w.contains("concrete_vals"), "{w}");
        assert!(w.contains("vec![7, 0, 0, 0]"), "{w}");
    }

    // Verifies: REQ027 — a refutation without a counterexample is still a refutation; the
    // witness is optional evidence, not a precondition for the polarity.
    #[test]
    fn failed_verification_without_playback_still_fails() {
        assert_eq!(
            classify("VERIFICATION:- FAILED\n"),
            Outcome::Fails {
                failed_check: None,
                witness: None,
            }
        );
    }

    // Verifies: REQ027 — output with no verdict line is INCONCLUSIVE, never an optimistic
    // pass. This is the harness-does-not-compile path (a sort without `kani::Arbitrary`, a
    // parameter type that does not match the sort), and it must stay `unknown`.
    //
    // The fixture is deliberately the REAL shape of a failed Kani run (observed against Kani
    // 0.67.0): the actionable cause is the FIRST line and the tail is candidate-impl noise.
    // Reporting the tail is what the operator cannot act on.
    #[test]
    fn unrecognised_output_is_inconclusive_and_names_the_actionable_cause() {
        let compile_error = "\
error[E0277]: the trait bound `User: kani::Arbitrary` is not satisfied
help: the trait `kani::Arbitrary` is not implemented for `User`
   = help: the following other types implement trait `kani::Arbitrary`:
             (A, B, C, D, E, F, G)
           and 50 others
note: required by a bound in `kani::any`
   = note: this error originates in the macro `kani_core::kani_intrinsics`
For more information about this error, try `rustc --explain E0277`.
";
        let Outcome::Inconclusive { reason } = classify(compile_error) else {
            panic!("a harness that does not compile decides nothing");
        };
        // REQ065 moved this one case past "quote the first error line": an unsatisfied
        // `kani::Arbitrary` bound is now explained as a precondition. What the test has always
        // been about is unchanged — the reason names what the operator has to act on, and the
        // tail of the log is not it.
        assert!(
            reason.contains("`User`") && reason.contains("kani::Arbitrary"),
            "must name what the operator has to fix, not the tail of the log: {reason}"
        );
        assert!(
            !reason.contains("and 50 others"),
            "the candidate-impl noise is not a reason: {reason}"
        );
    }

    // Verifies: REQ027 — empty engine output is inconclusive and says so, rather than
    // rendering a blank reason the operator cannot act on.
    #[test]
    fn empty_output_is_inconclusive_with_a_readable_reason() {
        let Outcome::Inconclusive { reason } = classify("") else {
            panic!("no output decides nothing");
        };
        assert!(reason.contains("no recognisable verdict"), "{reason}");
    }

    // Verifies: REQ065 — an uninstantiable sort is explained as the subject-side precondition it
    // is. The error line is the real one measured against this repo (#148), back when
    // `EngineStatus` had no `Arbitrary` impl.
    #[test]
    fn an_uninstantiable_sort_is_explained_as_a_precondition_not_a_compiler_error() {
        let output = "error[E0277]: the trait bound `provreq::engine::EngineStatus: \
                      kani::Arbitrary` is not satisfied\n\
                      error: Failed to execute cargo (exit status: 101). Found 1 compilation \
                      errors.\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a harness that cannot compile decides nothing");
        };
        assert!(
            reason.contains("`provreq::engine::EngineStatus`"),
            "names the sort: {reason}"
        );
        assert!(
            reason.contains("Nothing is wrong with the binding, the sort, or the claim"),
            "does not blame the formalization: {reason}"
        );
        assert!(
            reason.contains("deductive engine"),
            "says what is NOT blocked: {reason}"
        );
    }

    // Verifies: REQ065 — the branch keys on the trait, not on the error code. `E0277` is the
    // ordinary unsatisfied-bound error, so a different one must fall through to the generic
    // diagnostic rather than be dressed up as an instantiability precondition.
    #[test]
    fn an_unrelated_unsatisfied_bound_is_not_read_as_an_instantiability_problem() {
        let output = "error[E0277]: the trait bound `Session: std::fmt::Display` is not \
                      satisfied\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a harness that cannot compile decides nothing");
        };
        assert!(!reason.contains("cannot be instantiated"), "{reason}");
        assert!(reason.contains("std::fmt::Display"), "{reason}");
    }

    // Verifies: REQ027 — the harness name is a valid Rust identifier derived from the
    // requirement id, prefixed so it cannot collide with the subject's own tests.
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

    /// A real cargo subject: a sort and two predicates over it, with `has_session`'s body
    /// supplied so a test can make the invariant true or false. `cfg_attr` keeps the subject
    /// buildable without Kani, which is how a real adopted repo would be written.
    fn cargo_subject(has_session_body: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"smoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [lib]\nname = \"smoke\"\npath = \"src/lib.rs\"\n",
        )
        .expect("manifest");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src");
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            format!(
                "#[cfg_attr(kani, derive(kani::Arbitrary))]\n\
                 pub struct User {{ pub id: u32, pub logged_in: bool }}\n\
                 pub fn login(u: &User) -> bool {{ u.logged_in }}\n\
                 pub fn has_session(u: &User) -> bool {{ {has_session_body} }}\n"
            ),
        )
        .expect("lib.rs");
        tmp
    }

    /// The harness for the `cargo_subject` fixture, which every real-engine test shares.
    fn smoke_harness() -> Harness {
        lower(
            &req(CODE_REQ),
            "smoke",
            &standard_bindings(),
            &by_ref_resolutions(),
            &sorts(),
            "provreq_smoke",
        )
        .expect("the fixture must lower")
    }

    // Verifies: REQ027 — THE REAL ENGINE, end to end: a true invariant over a real cargo
    // subject is verified by Kani and earns a bounded `holds`.
    //
    // `#[ignore]` is deliberate, not neglect (R-eng-2): the common user state is
    // engine-ABSENT, and that path is the one most worth proving continuously — so CI's main
    // `test` job stays Kani-free and the separate `kani` job runs `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs Kani installed — run via `cargo test -- --ignored` (the CI `kani` job)"]
    fn real_kani_verifies_a_true_invariant() {
        let tmp = cargo_subject("u.logged_in || u.id == 0");
        let outcome = run(tmp.path(), &smoke_harness(), &Bounds::default());
        assert_eq!(outcome, Outcome::Holds, "a true invariant must verify");
    }

    // Verifies: #117 — THE REAL ENGINE accepts the bounds provreq builds from a subject's config.
    // The unconfigured path adds no arguments and is covered by the test above; this is the one
    // that cannot be checked without Kani, because `--harness-timeout` is experimental and only
    // Kani decides whether the `-Z unstable-options` provreq pairs with it is the right key. A
    // wrong flag here would not degrade — it would turn every configured subject's run into an
    // `Inconclusive`, and the unit tests would still pass.
    #[test]
    #[ignore = "needs Kani installed — run via `cargo test -- --ignored` (the CI `kani` job)"]
    fn real_kani_accepts_the_configured_bounds() {
        let tmp = cargo_subject("u.logged_in || u.id == 0");
        let outcome = run(
            tmp.path(),
            &smoke_harness(),
            &Bounds {
                default_unwind: Some(4),
                timeout_seconds: Some(600),
            },
        );
        assert_eq!(
            outcome,
            Outcome::Holds,
            "the same true invariant must still verify with both bounds set"
        );
    }

    // Verifies: REQ027 (D9) — THE REAL ENGINE refutes a false invariant and hands back a
    // concrete counterexample. The planted bug is `u.id != 7`, so the witness must contain
    // the discriminating value 7 — this asserts the witness is REAL evidence about this
    // subject, not merely a well-formed block of text.
    #[test]
    #[ignore = "needs Kani installed — run via `cargo test -- --ignored` (the CI `kani` job)"]
    fn real_kani_refutes_a_false_invariant_with_a_concrete_witness() {
        let tmp = cargo_subject("u.logged_in && u.id != 7");
        let outcome = run(tmp.path(), &smoke_harness(), &Bounds::default());
        let Outcome::Fails { witness, .. } = outcome else {
            panic!("a violated invariant must be refuted, got {outcome:?}");
        };
        let w = witness.expect("Kani must produce a counterexample");
        assert!(
            w.contains("vec![7, 0, 0, 0]"),
            "the witness must carry the value that breaks the claim: {w}"
        );
    }

    // Verifies: REQ027 — THE REAL ENGINE on an uninstantiable sort: no `Arbitrary`, so the
    // harness cannot compile. That is `unknown`, never a verdict — and the reason must name
    // the trait the operator has to implement.
    #[test]
    #[ignore = "needs Kani installed — run via `cargo test -- --ignored` (the CI `kani` job)"]
    fn real_kani_cannot_decide_when_the_sort_is_not_instantiable() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"smoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [lib]\nname = \"smoke\"\npath = \"src/lib.rs\"\n",
        )
        .expect("manifest");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src");
        // No `Arbitrary` derive — `kani::any::<User>()` cannot be written.
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            "pub struct User { pub id: u32, pub logged_in: bool }\n\
             pub fn login(u: &User) -> bool { u.logged_in }\n\
             pub fn has_session(u: &User) -> bool { u.logged_in }\n",
        )
        .expect("lib.rs");

        let outcome = run(tmp.path(), &smoke_harness(), &Bounds::default());
        let Outcome::Inconclusive { reason } = outcome else {
            panic!("an uncompilable harness decides nothing, got {outcome:?}");
        };
        // Against the REAL engine, so it is Kani's actual `E0277` wording being recognised and
        // not just the unit fixture's (REQ065). If Kani ever restates that error, this is what
        // catches it — and the reason degrades to the raw diagnostic rather than becoming wrong.
        assert!(reason.contains("kani::Arbitrary"), "{reason}");
        // `smoke::User`, not `User`: the sort is named as the harness reaches it, which is the
        // form the operator has to write the impl against.
        assert!(
            reason.contains("`smoke::User` cannot be instantiated by Kani"),
            "the real error must reach the precondition branch, naming the sort: {reason}"
        );
        assert!(
            reason.contains("deductive engine"),
            "and must say what is not blocked: {reason}"
        );
    }

    // Verifies: REQ027 — provreq leaves no litter in someone else's repo. The harness file
    // AND the `tests/` directory provreq created are gone afterwards, on the failing path
    // too (the run is refuted here, and cleanup still happens).
    #[test]
    #[ignore = "needs Kani installed — run via `cargo test -- --ignored` (the CI `kani` job)"]
    fn real_kani_run_leaves_no_trace_in_the_subject() {
        let tmp = cargo_subject("u.logged_in && u.id != 7");
        let _ = run(tmp.path(), &smoke_harness(), &Bounds::default());
        assert!(
            !tmp.path().join("tests").exists(),
            "a tests/ directory provreq created must not survive the run"
        );
    }

    // Verifies: REQ027 — an existing file is NEVER clobbered. provreq writes into someone
    // else's repo, so a name collision must stop the run, not overwrite their work.
    #[test]
    fn an_existing_file_is_never_overwritten() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("tests")).expect("tests");
        let victim = tmp.path().join("tests/provreq_smoke.rs");
        std::fs::write(&victim, "// the operator's own file\n").expect("write");

        let harness = Harness {
            name: "provreq_smoke".into(),
            source: "// generated\n".into(),
        };
        let Outcome::Inconclusive { reason } = run(tmp.path(), &harness, &Bounds::default()) else {
            panic!("a collision must not be treated as a verdict");
        };
        assert!(reason.contains("refusing to overwrite"), "{reason}");
        assert_eq!(
            std::fs::read_to_string(&victim).expect("read"),
            "// the operator's own file\n",
            "the operator's file must be untouched"
        );
    }

    // Verifies: REQ027 (D8) — a Kani pass is `model-checked (bounded)` and NEVER `proven`.
    // Kani explores a bounded state space, so claiming ∀-executions would be an overclaim
    // of exactly the kind REQ024 fixed for engine readiness.
    #[test]
    fn a_kani_pass_is_bounded_model_checked_never_proven() {
        let v = crate::verdict::aggregate(
            "SR001",
            vec![Outcome::Holds.into_evidence(&Bounds::default())],
            prov(),
        );
        assert_eq!(v.status, crate::verdict::Status::Holds);
        assert_eq!(v.basis, Some(Basis::ModelCheckedBounded));
        let text = crate::verdict::render(&v);
        assert!(text.contains("model-checked (bounded)"), "{text}");
        assert!(
            text.contains("NOT proven"),
            "the read-back must forestall reading a bounded pass as a proof: {text}"
        );
    }

    // Verifies: #117 — a bounded `holds` says what bounds it was bounded by, and says it loudest
    // when nobody chose them. A `holds` means "no counterexample within these bounds"; reporting
    // only configured values would hide exactly the bound the operator never made, which is the
    // one they most need to know is Kani's and not theirs.
    #[test]
    fn a_bounded_hold_names_the_bounds_it_was_checked_under() {
        let unset = crate::verdict::render(&crate::verdict::aggregate(
            "SR001",
            vec![Outcome::Holds.into_evidence(&Bounds::default())],
            prov(),
        ));
        assert!(
            unset.contains("Kani's own default") && unset.contains("not yours"),
            "an unchosen bound must not be invisible in a verdict that depends on it: {unset}"
        );

        let chosen = crate::verdict::render(&crate::verdict::aggregate(
            "SR001",
            vec![Outcome::Holds.into_evidence(&Bounds {
                default_unwind: Some(12),
                timeout_seconds: Some(300),
            })],
            prov(),
        ));
        assert!(chosen.contains("unwind 12"), "{chosen}");
        assert!(chosen.contains("timeout 300s"), "{chosen}");
        assert!(
            !chosen.contains("not yours"),
            "a bound the operator chose needs no warning: {chosen}"
        );
    }

    // Verifies: #117 — the bounds reach `cargo kani` as its own flags, and an unconfigured subject
    // adds none at all: the shipped default is Kani's default, so a subject that configured nothing
    // must run the exact command line provreq shipped with, off the unstable surface.
    #[test]
    fn only_a_configured_bound_reaches_the_command_line() {
        assert!(Bounds::default().args().is_empty());

        let args = Bounds {
            default_unwind: Some(12),
            timeout_seconds: Some(300),
        }
        .args();
        assert_eq!(
            args,
            [
                "--default-unwind",
                "12",
                "-Z",
                "unstable-options",
                "--harness-timeout",
                "300"
            ],
            "`--harness-timeout` is experimental, so it must carry its own `-Z`"
        );

        let unwind_only = Bounds {
            default_unwind: Some(4),
            timeout_seconds: None,
        }
        .args();
        assert!(
            !unwind_only.contains(&"unstable-options".to_string()),
            "an unwind bound must not drag in the unstable surface: {unwind_only:?}"
        );
    }

    // Verifies: #117 — the `kani:` block is read per subject, and every way of not having one
    // lands on Kani's defaults rather than an error. A subject that never configured a bound is
    // the normal case.
    #[test]
    fn bounds_come_from_the_manifest_and_default_when_it_says_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest = tmp.path().join(crate::adopt::MANIFEST_FILE);

        assert_eq!(
            Bounds::load(tmp.path()),
            Bounds::default(),
            "no manifest at all is the pre-adoption case, not a failure"
        );

        std::fs::write(&manifest, "schema: 1\n").unwrap();
        assert_eq!(Bounds::load(tmp.path()), Bounds::default());

        std::fs::write(&manifest, "schema: 1\nkani:\n  default_unwind: 12\n").unwrap();
        assert_eq!(
            Bounds::load(tmp.path()),
            Bounds {
                default_unwind: Some(12),
                timeout_seconds: None
            },
            "one knob is configurable without the other"
        );

        std::fs::write(
            &manifest,
            "schema: 1\nkani:\n  default_unwind: 12\n  timeout_seconds: 300\n",
        )
        .unwrap();
        assert_eq!(
            Bounds::load(tmp.path()),
            Bounds {
                default_unwind: Some(12),
                timeout_seconds: Some(300)
            }
        );

        std::fs::write(&manifest, "schema: 1\nkani: [not, a, map]\n").unwrap();
        assert_eq!(
            Bounds::load(tmp.path()),
            Bounds::default(),
            "a malformed block falls back to Kani's defaults — a bound provreq cannot read is \
             not a reason to refuse to verify, and the verdict still says the bound is Kani's"
        );
    }

    // Verifies: REQ027 (D9) — a refutation becomes a `fails` carrying the counterexample as
    // a re-checkable witness, which is what makes falsification the robust half.
    #[test]
    fn a_kani_refutation_becomes_a_fails_carrying_its_witness() {
        let outcome = Outcome::Fails {
            failed_check: Some("Failed Checks: assertion failed: has_session(&u)".into()),
            witness: Some("fn kani_concrete_playback_provreq_r_1() {\n    // 7\n}".into()),
        };
        let v = crate::verdict::aggregate(
            "SR002",
            vec![outcome.into_evidence(&Bounds::default())],
            prov(),
        );
        assert_eq!(v.status, crate::verdict::Status::Fails);
        assert_eq!(v.basis, None, "a fails has a witness, not a basis");
        let text = crate::verdict::render(&v);
        assert!(text.contains("SR002: fails"), "{text}");
        assert!(text.contains("assertion failed"), "{text}");
        assert!(text.contains("witness"), "{text}");
        assert!(text.contains("kani_concrete_playback"), "{text}");
    }

    // Verifies: REQ027 (D10) — an engine that could not decide yields unknown/inconclusive,
    // never a verdict. A harness that will not compile is not evidence of anything.
    #[test]
    fn an_undecided_run_is_unknown_inconclusive_never_a_verdict() {
        let outcome = Outcome::Inconclusive {
            reason: "error[E0277]: the trait bound `User: kani::Arbitrary` is not satisfied".into(),
        };
        let v = crate::verdict::aggregate(
            "SR003",
            vec![outcome.into_evidence(&Bounds::default())],
            prov(),
        );
        assert_eq!(v.status, crate::verdict::Status::Unknown);
        assert_eq!(v.reason, Some(crate::verdict::UnknownReason::Inconclusive));
        let text = crate::verdict::render(&v);
        assert!(text.contains("not evidence either way"), "{text}");
        assert!(text.contains("Arbitrary"), "{text}");
    }
}
