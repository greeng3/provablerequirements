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
//! was not established, and the crash report it drops in the subject is cleaned up — see #153),
//! REQ067 (a construct Creusot cannot translate is the prover's limit, is whole-crate in reach,
//! and carries the `#[trusted]` way out rather than a contract that cannot help), REQ070
//! (recognition of a checker's output covers every phrasing the checker is known to emit, matched
//! against real output; an unknown phrasing is unrecognised, never some other class of failure;
//! the first diagnostic decides).

use crate::grounding::Binding;
use crate::lowering::{self, LoweredClaim};
pub use crate::lowering::{harness_name, NotLowerable, HARNESS_PREFIX};
use crate::prl::ast::Requirement;
use crate::rust_adapter::{PredicateForm, Resolution, TypeResolution};
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

/// Redirect every predicate that has a **logic mirror** staged in the subject onto that mirror.
///
/// This is the Creusot-only half of [`crate::mirror_draft`], and it cannot be shared with Kani or
/// Prusti: Kani *executes* the subject's functions, so it must keep calling the real program items,
/// while Creusot's pearlite may only call `#[logic]` ones. Both engines lower the same claim from
/// the same grounding, so the difference has to live here rather than in [`crate::lowering`].
///
/// The seam is that a mirror is a *resolution* detail — which item a predicate resolves to — so
/// redirecting is a rewrite of the bindings and resolutions that `lower_property` already takes.
/// Nothing in the shared lowering core changes, and neither does the harness shape.
///
/// A mirror is used only when it is actually **present** in the subject's source (the operator
/// staged and kept it). An absent mirror leaves the predicate pointing at the program function,
/// which then fails as it does today — *called program function `f` in logic context* — rather than
/// producing a harness that names something that does not exist.
///
/// The mirror is a free function appended at module level in the *same file*, so the resolution's
/// `CodeMatch` (and hence its module path, REQ061) stays correct as-is.
pub fn with_mirrors(
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
    sources: &BTreeMap<String, String>,
) -> (Vec<Binding>, BTreeMap<String, Resolution>) {
    let mut out_bindings = bindings.to_vec();
    let mut out_resolutions = resolutions.clone();
    for (symbol, res) in resolutions {
        let Resolution::Resolved { at, params, form } = res else {
            continue;
        };
        let Some(binding) = bindings.iter().find(|b| b.symbol == *symbol) else {
            continue;
        };
        // The program item's own name, per form: the observable names it only for a free function.
        let program_name = match form {
            PredicateForm::Function => binding.observable.as_str(),
            PredicateForm::Method { name, .. } => name.as_str(),
            PredicateForm::VariantTest { name, .. } => name.as_str(),
        };
        let mirror = crate::mirror_draft::mirror_name(program_name);
        let staged = sources
            .get(&at.file)
            .is_some_and(|src| src.contains(&format!("fn {mirror}")));
        if !staged {
            continue;
        }
        // A method's mirror is a FREE function taking the receiver as its first parameter, so the
        // form changes as well as the name: `d.is_ready()` becomes `is_ready_logic(&d)`. A variant
        // test keeps its shape and only redirects the function it tests.
        let new_form = match form {
            PredicateForm::Function | PredicateForm::Method { .. } => PredicateForm::Function,
            PredicateForm::VariantTest {
                enum_name,
                variant,
                enum_module,
                ..
            } => PredicateForm::VariantTest {
                name: mirror.clone(),
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                enum_module: enum_module.clone(),
            },
        };
        out_resolutions.insert(
            symbol.clone(),
            Resolution::Resolved {
                at: at.clone(),
                params: params.clone(),
                form: new_form,
            },
        );
        if let Some(b) = out_bindings.iter_mut().find(|b| b.symbol == *symbol) {
            b.observable = mirror;
        }
    }
    (out_bindings, out_resolutions)
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

/// Why the installed Creusot cannot be run against this subject, if it cannot — `None` when it can.
///
/// `cargo-creusot` refuses **before running any subcommand** when the subject's `creusot-std` and
/// the installed tool disagree, in either direction (`creusot-std is out of date` /
/// `creusot-std is newer than Creusot`). The engine never translates anything, so a run started in
/// that state cannot produce evidence about the claim — and reporting it as `inconclusive` says the
/// prover tried and could not decide, which is false (#279).
///
/// Compared on major.minor, because that is the pair `cargo-creusot` accepts: our own manifest says
/// `creusot-std = "0.13"` against a 0.13.0 tool and is fine. Read from the manifest rather than
/// matched against the refusal text: the two phrasings are a REQ070 trap, and a version comparison
/// does not have to know either of them.
pub fn unusable_for_subject(cargo_toml: &str, tool_version: &str) -> Option<String> {
    let declared = declared_creusot_std(cargo_toml)?;
    let (want, got) = (minor_series(&declared)?, minor_series(tool_version)?);
    if want == got {
        return None;
    }
    Some(format!(
        "this subject declares creusot-std {declared} but the installed Creusot is {tool_version}; \
         cargo-creusot refuses that pair before it translates anything, so it cannot answer here \
         — align the dependency with the toolchain"
    ))
}

/// The version requirement the subject declares for `creusot-std`, e.g. `0.13` or `0.12.0`.
///
/// Crude on purpose, matching `contract_draft::marker_for_subject`: a dependency key at a line
/// start followed by a bare string. ponytail: no TOML parse — a `creusot-std = { version = "…" }`
/// table reads as absent, which fails OPEN (the engine stays ready and behaves as it does today),
/// so the worst case is the status quo rather than a wrong refusal.
fn declared_creusot_std(cargo_toml: &str) -> Option<String> {
    cargo_toml.lines().find_map(|l| {
        let rest = l.trim_start().strip_prefix("creusot-std")?;
        let rest = rest.trim_start().strip_prefix('=')?.trim();
        rest.strip_prefix('"')?
            .split('"')
            .next()
            .map(str::to_string)
    })
}

/// `major.minor` of a version or version requirement, ignoring any leading `^`/`=`/`~`.
fn minor_series(version: &str) -> Option<(u64, u64)> {
    let v = version.trim_start_matches(['^', '=', '~', 'v']);
    let mut parts = v.split('.');
    let major = parts.next()?.trim().parse().ok()?;
    let minor = parts.next()?.trim().parse().ok()?;
    Some((major, minor))
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
    // A construct Creusot cannot translate is the checker's own limit, not a defect in the subject
    // or the claim (REQ067) — and it is whole-crate, so it blocks claims that never go near it.
    // Above the build-error branch because it also prints `could not compile`, and the generic
    // branch would offer a `#[logic]` hint that cannot help: no contract makes an untranslatable
    // construct translatable. What CAN help is `#[trusted]`, so the message says so.
    if let Some((construct, lead)) = unsupported_construct(output) {
        return Outcome::Inconclusive {
            reason: format!(
                "Creusot cannot translate a construct this crate uses — {construct}. It is a \
                 limit of the prover, not something wrong with the claim or its bindings, and it \
                 blocks every claim about this crate because Creusot translates the whole crate \
                 rather than only what the claim mentions.{lead} Mark the items using that \
                 construct `#[trusted]` to declare them out of scope, and the rest of the crate \
                 verifies normally"
            ),
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

/// The construct Creusot refused to translate, read off its own diagnostic and trimmed to the part
/// that names it. `None` when the run failed some other way.
///
/// **Creusot says this two ways, and recognising one of them is recognising neither** (#250). The
/// rvalue form — `the rvalue Coroutine(…) is not currently supported` — was the only one matched,
/// so the constant form fell through to [`build_error`] and reached the operator as *the subject
/// did not compile*. Measured against this repo: `format!("…{x}")` at `src/adopt.rs:37` produced
/// `Unsupported constant value: Scalar(alloc412) of type &'{erased} [u8; 24_usize]`, and that
/// crate compiles perfectly under rustc. Reporting a prover limit as the subject's own build
/// failure sends the operator to fix code that was never broken — REQ067's case, answered with
/// REQ064's mistake.
///
/// The site travels with it because `Scalar(alloc412)` is the prover's internal name for the value
/// and there is nothing an operator can do with it. The line in their own tree is what they can
/// read.
///
/// **Only the first diagnostic is consulted**, which is the same rule [`build_error`] follows and
/// is load-bearing rather than tidy. Measured on a live `verify REQ014`: the staged harness failed
/// first with `called program function … in logic context`, whose branch names the mirror channel
/// and is the one thing the operator can act on. A scan of the whole output would let an
/// untranslatable construct appearing *later* in the same run preempt that advice — trading an
/// actionable message for an accurate one, which is the trade this whole area keeps getting wrong.
///
/// Returns the construct and the lead to offer about it, together, so the two cannot describe
/// different diagnostics.
fn unsupported_construct(output: &str) -> Option<(String, &'static str)> {
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    let idx = lines.iter().position(|l| l.starts_with("error"))?;
    let line = lines[idx];
    let lead = match () {
        _ if line.contains(UNSUPPORTED_CONSTANT) => CONSTANT_LEAD,
        _ if line.contains(UNSUPPORTED_RVALUE) => "",
        _ => return None,
    };
    let named = line
        .trim_start_matches("error: ")
        .trim_end_matches(UNSUPPORTED_RVALUE)
        .trim()
        .to_string();
    let named = match error_site(&lines, idx) {
        Some(site) => format!("{named}, at {site}"),
        None => named,
    };
    Some((named, lead))
}

/// The lead offered on an untranslatable *constant* — offered, not stated, which is REQ064's
/// distinction between a diagnosis that is one possibility among several and one the tool
/// determined.
///
/// Measured 2026-08-09: `format!`, `println!` and `write!` each produce this constant when they
/// interpolate an argument, and each is translated without complaint when they do not —
/// `format!("hello")` compiles, `format!("{s}")` does not. So a formatting macro is where this
/// almost always comes from and is worth saying, while *this* line having one is not something
/// provreq established. A byte-array constant is not exclusively a formatting artifact, and a flat
/// claim that it is would send an operator hunting a `format!` that may not be there.
const CONSTANT_LEAD: &str =
    " A constant of that shape usually comes from a formatting macro interpolating an argument — \
     `format!(\"{x}\")`, `println!(\"{x}\")`, `write!` — which is worth checking first, though the \
     same macro with nothing to interpolate translates fine.";

/// Creusot's two ways of declining to translate something. Matched as literals rather than
/// paraphrased, because a near-miss here reads to the operator as a broken subject.
const UNSUPPORTED_RVALUE: &str = "is not currently supported";
const UNSUPPORTED_CONSTANT: &str = "Unsupported constant value";

/// The first compiler error, reported with **where it is** and, where the error says so, what to do.
///
/// Two things this must get right, both learned from a live run (#171, #174).
///
/// **Where.** A compile failure under Creusot is not always in the generated harness. It is just as
/// often in the subject's own source — most sharply when `--draft-semantic` has just staged a mirror
/// there and asked the operator to review it. Measured: a drafted mirror wrote `failures == 0` where
/// a `&Session` match binds `&u32`, and the verdict said *the proof harness did not compile*,
/// sending the operator to a generated file that is deleted after the run and that they cannot edit,
/// when rustc had already named the line in their own tree. The location is in the diagnostic; this
/// reads it rather than discarding it.
///
/// **Whose fault.** This knows the file that failed and nothing about how it got that way, so it
/// says only that (#227). It used to add "if a draft was just staged there, it is the staged edit
/// that needs fixing" to *every* subject-source failure. Measured: the failing file was
/// `src/rust_adapter.rs`, which no draft had ever touched — the message named a cause it had not
/// established, at the one moment the operator is most likely to act on it. The case where a staged
/// mirror really is the cause has its own branch below, which recognises the error rather than
/// guessing from the location.
///
/// **What to do.** The old hint offered `#[logic]` on any compile error. Since #158 that is the one
/// action that cannot work: the attribute declares a *logical* function, so the item leaves the
/// program namespace and every call site stops compiling. Where the error is the call-in-logic-context
/// one, the answer is the mirror channel, which leaves the program function alone (REQ068); where it
/// is anything else, provreq has established no cause and says none.
fn build_error(output: &str) -> String {
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    let Some(idx) = lines
        .iter()
        .position(|l| l.starts_with("error[") || l.starts_with("error:"))
    else {
        return tail(output);
    };
    let err = lines[idx];
    let where_it_failed = match error_site(&lines, idx) {
        Some(site) if is_generated_harness(site) => {
            format!("the proof harness provreq generated did not compile — {err} ({site})")
        }
        Some(site) => format!(
            "the subject did not compile under Creusot — {err}, at {site}. That is the subject's \
             own source, not the generated harness, and Creusot compiles the whole crate — so the \
             file that failed need not be one the claim mentions, nor one a draft was staged in"
        ),
        None => format!("the proof harness did not compile — {err}"),
    };
    if err.contains("called program function") {
        return format!(
            "{where_it_failed}. Pearlite may only call `#[logic]` functions, and that is an \
             ordinary program function — marking it `#[logic]` is not the fix either, because that \
             removes the item from the program and breaks every call site. Reach it through a \
             `#[logic]` mirror instead, which leaves the function untouched: re-run `verify` with \
             `--draft-semantic`"
        );
    }
    where_it_failed
}

/// Where rustc said the error is: the `--> file:line:col` line that follows a diagnostic's header.
/// Bounded to the few lines after it, so a later diagnostic's location is never read as this one's.
fn error_site<'a>(lines: &[&'a str], header: usize) -> Option<&'a str> {
    lines
        .get(header + 1..)?
        .iter()
        .take(SITE_SEARCH_LINES)
        .find(|l| l.starts_with("-->"))
        .map(|l| l.trim_start_matches("-->").trim())
}

/// Whether a rustc location points at the file provreq generated, rather than the operator's own.
///
/// Every harness provreq writes is `src/<`[`HARNESS_PREFIX`]`>…rs` ([`harness_name`]), and a
/// name collision with a real subject file is refused before any of this runs — so the prefix is a
/// reliable discriminator and not a guess.
fn is_generated_harness(site: &str) -> bool {
    site.rsplit('/')
        .next()
        .is_some_and(|f| f.starts_with(HARNESS_PREFIX))
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

/// How far past a diagnostic's header to look for its `-->` location. rustc puts it on the very
/// next line; a small window keeps a *later* diagnostic's location from being read as this one's.
const SITE_SEARCH_LINES: usize = 3;

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

    // Verifies: REQ031 — a harness that does not compile is `inconclusive` and names the error and
    // WHERE it is. Also (#171): it no longer recommends `#[logic]`. That hint fired on every
    // compile error including this one, a plain type mismatch, and since #158 it is the one action
    // that cannot work — the attribute removes the item from the program and breaks every call
    // site. A cause provreq has not established is a cause it does not assert.
    #[test]
    fn a_compile_failure_is_inconclusive_and_names_the_error_and_its_site() {
        let output = "error[E0308]: mismatched types\n  --> src/provreq_check.rs:5:9\n\
                      error: could not compile `csmoke` (lib) due to 2 previous errors\n\
                      Error: Compilation failed\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a harness that does not compile decides nothing");
        };
        assert!(reason.contains("did not compile"), "{reason}");
        assert!(reason.contains("E0308"), "the error rides along: {reason}");
        assert!(
            reason.contains("src/provreq_check.rs:5:9"),
            "and where it is: {reason}"
        );
        assert!(
            reason.contains("harness provreq generated"),
            "this one really IS in the harness, and says so: {reason}"
        );
        assert!(
            !reason.contains("`#[logic]`"),
            "must not recommend the one thing that breaks the subject (#158/#171): {reason}"
        );
    }

    // Verifies (#174): an error in the SUBJECT's own source is not reported as the harness failing.
    // Measured on a live run — a drafted mirror wrote `failures == 0` where a `&Session` match
    // binds `&u32`, and the verdict said *the proof harness did not compile*, sending the operator
    // to a generated file that is deleted after the run and that they cannot edit, while rustc had
    // already named the line in their own tree.
    #[test]
    fn an_error_in_the_subjects_own_source_says_so_and_names_the_line() {
        let output = "error[E0308]: mismatched types\n  --> src/session.rs:27:59\n\
                      error: could not compile `gatekeeper` (lib) due to 1 previous error\n\
                      Error: Compilation failed\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a subject that does not compile decides nothing either");
        };
        assert!(
            reason.contains("src/session.rs:27:59"),
            "the operator needs the line: {reason}"
        );
        assert!(
            reason.contains("subject's \n             own source")
                || reason.contains("subject's own source"),
            "and needs to know it is theirs to fix: {reason}"
        );
        assert!(
            !reason.contains("harness provreq generated"),
            "it is NOT the harness: {reason}"
        );
    }

    // Verifies (#227): the subject-source branch knows WHERE the failure is and nothing about how
    // it got there, so it must not name a cause. Measured — `TypeResolution` in
    // `src/rust_adapter.rs` was refused as a recursive type on a run whose only staged edits were
    // mirrors in a different file, and the verdict told the operator to go fix the staged edit.
    #[test]
    fn a_subject_source_failure_does_not_blame_a_staged_draft() {
        let output = "error: Illegal recursive type\n  --> src/rust_adapter.rs:465:1\n\
                      error: could not compile `provreq` (lib) due to 1 previous error\n\
                      Error: Compilation failed\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a subject that does not compile decides nothing either");
        };
        assert!(
            !reason.contains("staged edit that needs fixing"),
            "provreq has not established that a staged edit is the cause: {reason}"
        );
        assert!(
            reason.contains("whole crate"),
            "and the operator needs the fact that explains an unrelated file failing: {reason}"
        );
    }

    // Verifies (#171): the call-in-logic-context error — the one the mirror channel exists to
    // answer — names the mirror channel. Before this it named `#[logic]`, at the exact moment the
    // operator was most likely to act on it, and doing so leaves the subject unable to compile in
    // any configuration.
    #[test]
    fn a_program_call_in_logic_context_points_at_the_mirror_channel() {
        let output = "error: called program function `access::decide` in logic context\n\
                      \x20 --> src/provreq_req001.rs:9:40\n\
                      error: could not compile `gatekeeper` (lib) due to 1 previous error\n\
                      Error: Compilation failed\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("no verdict from a harness that will not compile");
        };
        assert!(
            reason.contains("--draft-semantic"),
            "must name the channel that works: {reason}"
        );
        assert!(
            reason.contains("breaks every call site"),
            "and say why the obvious move is wrong: {reason}"
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

    // Verifies: REQ067 — a construct Creusot cannot translate is reported as the prover's limit,
    // with the escape hatch. Real output, measured 2026-07-30 against a patched driver (the same
    // crate ICEs without the patch, which is #153 / creusot-rs/creusot#2212).
    #[test]
    fn an_untranslatable_construct_is_the_provers_limit_not_the_subjects_fault() {
        let output = "error: the rvalue Coroutine(DefId(0:6 ~ cr_async[2fe3]::an_async_fn::\
                      {closure#0}), [(), std::future::ResumeTy, (), u32, (u32,)]) is not \
                      currently supported\n\
                      error: could not compile `cr_async` (lib) due to 1 previous error\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("an untranslatable construct decides nothing");
        };
        assert!(
            reason.contains("Coroutine"),
            "names the construct: {reason}"
        );
        assert!(
            reason.contains("limit of the prover"),
            "does not blame the subject: {reason}"
        );
        assert!(
            reason.contains("`#[trusted]`"),
            "offers the escape hatch: {reason}"
        );
        assert!(
            !reason.contains("`#[logic]`"),
            "no contract makes it translatable: {reason}"
        );
    }

    // Verifies: REQ067, REQ070 (#250) — Creusot has more than one way of saying "I cannot translate this",
    // and the second one was reaching the operator as *your crate does not compile*. Real output,
    // measured 2026-08-09 against this repo: `src/adopt.rs:37` is
    // `format!("ProvableRequirements-{requirements_dirname}")`, which rustc compiles perfectly.
    #[test]
    fn an_untranslatable_constant_is_the_provers_limit_too() {
        let output = "error: Unsupported constant value: Scalar(alloc412) of type \
                      &'{erased} [u8; 24_usize]\n\
                      --> src/adopt.rs:37:5\n\
                      error: could not compile `provreq` (lib) due to 1 previous error\n\
                      Error: Compilation failed\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("an untranslatable constant decides nothing");
        };
        assert!(
            !reason.contains("did not compile"),
            "the crate compiles under rustc — this is the prover's limit, not a broken subject: \
             {reason}"
        );
        assert!(
            reason.contains("limit of the prover"),
            "does not blame the subject: {reason}"
        );
        assert!(
            reason.contains("whole crate"),
            "states the reach (REQ067): {reason}"
        );
        assert!(
            reason.contains("`#[trusted]`"),
            "offers the escape hatch (REQ067): {reason}"
        );
        assert!(
            reason.contains("src/adopt.rs:37:5"),
            "`Scalar(alloc412)` is not a thing an operator can look up; the line in their own tree \
             is: {reason}"
        );
    }

    // Verifies: REQ064 (#250) — what that constant comes from is offered as a possibility, not
    // stated flatly. Measured: `format!`, `println!` and `write!` all produce this shape when they
    // interpolate an argument, and all three are translated fine when they do not — so the
    // formatting macro is a strong lead and not something provreq has established for a given line.
    #[test]
    fn the_constants_likely_source_is_offered_not_asserted() {
        let output = "error: Unsupported constant value: Scalar(alloc1) of type \
                      &'{erased} [u8; 10_usize]\n\
                      --> src/adopt.rs:37:5\n\
                      error: could not compile `provreq` (lib) due to 1 previous error\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("an untranslatable constant decides nothing");
        };
        assert!(
            reason.contains("interpolat"),
            "the lead is worth giving — it is what the operator would otherwise hunt for: {reason}"
        );
        assert!(
            reason.contains("usually") || reason.contains("typically"),
            "but it is a lead, not a determination (REQ064): {reason}"
        );
    }

    // Verifies: REQ070 (#250) — an untranslatable construct appearing *later* in a run must not preempt the
    // first diagnostic. Found by a live `verify REQ014`, not by reading the code: the staged
    // harness failed first with the call-in-logic-context error, whose branch names the mirror
    // channel and is the only message here the operator can act on. Recognising the whole output
    // would have swapped that for the prover-limit message — accurate, and useless.
    #[test]
    fn a_later_untranslatable_construct_does_not_preempt_the_first_error() {
        let output = "error: called program function `draft::is_stale` in logic context\n\
                      --> src/provreq_req014.rs:11:80\n\
                      error: Unsupported constant value: Scalar(alloc412) of type \
                      &'{erased} [u8; 24_usize]\n\
                      --> src/adopt.rs:37:5\n\
                      error: could not compile `provreq` (lib) due to 2 previous errors\n";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a failed compile decides nothing");
        };
        assert!(
            reason.contains("--draft-semantic"),
            "the actionable message wins: {reason}"
        );
        assert!(
            !reason.contains("limit of the prover"),
            "the later construct must not take over the verdict: {reason}"
        );
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

    // Verifies: REQ051 (#279) — a subject whose creusot-std cannot work with the installed tool must be
    // caught BEFORE the engine runs. cargo-creusot refuses at start-up, so a run in this state
    // produces no evidence about the claim; calling it `inconclusive` would say the prover tried.
    #[test]
    fn a_subject_whose_creusot_std_cannot_work_with_the_tool_is_unusable() {
        let older = "[dependencies]\ncreusot-std = \"0.12.0\"\n";
        let reason = unusable_for_subject(older, "0.13.0").expect("0.12 subject, 0.13 tool");
        assert!(
            reason.contains("0.12.0") && reason.contains("0.13.0"),
            "{reason}"
        );
        assert!(
            !reason.to_lowercase().contains("inconclusive"),
            "the engine never ran, so nothing about it is inconclusive: {reason}"
        );

        // The other direction is the one this container hit live: dep ahead of tool.
        let newer = "[dependencies]\ncreusot-std = \"0.13.0\"\n";
        assert!(
            unusable_for_subject(newer, "0.12.0").is_some(),
            "a dep newer than the tool is refused just as hard"
        );
    }

    // Verifies: REQ051 (#279) — the check must not cry wolf. A minor-level requirement covering the
    // installed patch release is exactly what this repo ships, and a subject that does not use
    // Creusot at all is not our business.
    #[test]
    fn a_usable_pair_and_an_uninvolved_subject_are_left_alone() {
        assert_eq!(
            unusable_for_subject("creusot-std = \"0.13\"\n", "0.13.0"),
            None,
            "a minor-level requirement covers the installed patch release"
        );
        assert_eq!(
            unusable_for_subject("[dependencies]\nserde = \"1\"\n", "0.13.0"),
            None,
            "a subject with no creusot-std has no coupling to violate"
        );
        assert_eq!(
            unusable_for_subject("creusot-std = { version = \"0.12\" }\n", "0.13.0"),
            None,
            "a table dependency is not parsed, and must fail OPEN rather than refuse wrongly"
        );
    }

    /// The `creusot-std` every fixture below depends on. Not free to differ from the installed
    /// tool: `cargo-creusot` refuses before running any subcommand when they disagree
    /// (`creusot-std is out of date. creusot-std 0.12.0 / creusot 0.13.0`).
    const CREUSOT_STD_VERSION: &str = "0.13.0";

    /// The manifest every real-engine fixture writes. One place, because the version in it is
    /// locked to the Dockerfile's tag.
    ///
    /// Also the choke point where a real-engine test is stopped from passing without the engine
    /// (#279): if Creusot is installed but would refuse this manifest at start-up, every test
    /// downstream of here is measuring a refusal, and one of them asserts `inconclusive` — which
    /// the refusal satisfies, in 0.62s, having verified nothing. A missing Creusot is NOT this
    /// case and must not panic: those tests are `#[ignore]`d and simply do not run.
    fn fixture_manifest(name: &str) -> String {
        let creusot = crate::engine::registry()
            .into_iter()
            .find(|e| e.name == "Creusot")
            .expect("Creusot is in the registry");
        if let crate::engine::EngineStatus::Available { version } =
            crate::engine::detect(&creusot, None)
        {
            let manifest = raw_fixture_manifest(name);
            assert!(
                unusable_for_subject(&manifest, &version).is_none(),
                "the installed Creusot ({version}) would refuse this fixture's creusot-std \
                 {CREUSOT_STD_VERSION} before translating anything — every real-engine test below \
                 would be measuring that refusal, not the engine. Reopen the dev container so the \
                 toolchain matches the tree."
            );
        }
        raw_fixture_manifest(name)
    }

    fn raw_fixture_manifest(name: &str) -> String {
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\ncreusot-std = \"{CREUSOT_STD_VERSION}\"\n\n\
             [lints.rust]\nunexpected_cfgs = {{ level = \"warn\", check-cfg = ['cfg(creusot)'] }}\n"
        )
    }

    // Verifies: the fixtures, this crate's own `creusot-std` dependency, and the Creusot the
    // image installs are ONE version. A mismatch is not a subtle degradation — `cargo-creusot`
    // refuses outright — and nothing else catches it until the CI `creusot` job runs against a
    // freshly built image, a 1-2h loop for a one-character slip.
    #[test]
    fn creusot_std_moves_in_lockstep_with_the_installed_tag() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let dockerfile = std::fs::read_to_string(root.join(".devcontainer/Dockerfile"))
            .expect("the Dockerfile that installs Creusot");
        let tag = dockerfile
            .lines()
            .find_map(|l| l.trim().strip_prefix("ARG CREUSOT_TAG=v"))
            .expect("ARG CREUSOT_TAG=v… in the Dockerfile");
        assert_eq!(
            tag, CREUSOT_STD_VERSION,
            "the image installs Creusot {tag} but the fixtures ask for creusot-std \
             {CREUSOT_STD_VERSION}; cargo-creusot refuses the pair"
        );

        let manifest =
            std::fs::read_to_string(root.join("Cargo.toml")).expect("this crate's manifest");
        let dep = manifest
            .lines()
            .find_map(|l| l.strip_prefix("creusot-std = \""))
            .map(|rest| rest.trim_end_matches('"'))
            .expect("a creusot-std dependency in Cargo.toml");
        assert!(
            CREUSOT_STD_VERSION.starts_with(&format!("{dep}.")),
            "this crate depends on creusot-std {dep}, which does not cover \
             {CREUSOT_STD_VERSION} — provreq is its own Creusot subject, so it would be refused"
        );
    }

    /// A real cargo subject: a sort and two `#[logic]` predicates over it, `has_session`'s
    /// body supplied so a test can make the invariant true or false.
    fn cargo_subject(has_session_body: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("Cargo.toml"), fixture_manifest("csmoke"))
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
        std::fs::write(tmp.path().join("Cargo.toml"), fixture_manifest("csmoke"))
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

    /// A subject shaped like a real one, unlike [`cargo_subject`]: its predicates are **ordinary
    /// program functions** the crate calls, not `#[logic]` items written for a prover. That is the
    /// case [`real_creusot_is_inconclusive_on_opaque_predicates`] shows Creusot cannot reach — and
    /// the case [`with_mirrors`] exists to bridge.
    ///
    /// Two modules on purpose. A single-module subject cannot exercise a mirror calling a sibling
    /// mirror across files, and that gap hid a real defect until a live run found it
    /// (`error[E0425]: cannot find function 'is_ready_logic' in this scope`).
    ///
    /// `ready_body` is the `is_ready` mirror's body, so one fixture serves both the honest case and
    /// the wrong-mirror case. Nothing here uses `format!`, `panic!` or `async`: those are what stop
    /// Creusot translating provreq itself (#153), and a fixture that tripped them would measure the
    /// translator rather than the mirror channel.
    fn mirror_subject(
        ready_body: &str,
        extra_clause_on_is_ready: &str,
        decide_body: &str,
        extra_clause_on_decide: &str,
    ) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            fixture_manifest("mirrorsmoke"),
        )
        .expect("manifest");
        std::fs::create_dir_all(tmp.path().join("src")).expect("src");
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            "pub mod decide;\npub mod status;\n",
        )
        .expect("lib.rs");
        // The mirror and its linking `#[ensures]` are exactly what `--draft-semantic` stages:
        // `use creusot_std::macros::*`, a `pub` mirror named `<fn>_logic`, and the link applying it
        // to the function's own parameters.
        std::fs::write(
            tmp.path().join("src/status.rs"),
            format!(
                "use creusot_std::macros::*;\n\n\
                 pub enum Status {{ Ready, Missing }}\n\n\
                 impl Status {{\n\
                 \x20   #[ensures(result == crate::status::is_ready_logic(self))]\n\
                 {extra_clause_on_is_ready}\
                 \x20   pub fn is_ready(&self) -> bool {{ matches!(self, Status::Ready) }}\n\
                 }}\n\n\
                 #[logic(open)]\n\
                 pub fn is_ready_logic(s: &Status) -> bool {{ pearlite! {{ {ready_body} }} }}\n"
            ),
        )
        .expect("status.rs");
        std::fs::write(
            tmp.path().join("src/decide.rs"),
            format!(
                "use creusot_std::macros::*;\n\
                 use crate::status::Status;\n\n\
                 pub enum Outcome {{ Proceed, AlreadyThere, Blocked }}\n\n\
                 #[ensures(result == crate::decide::decide_logic(s, allowed))]\n\
                 {extra_clause_on_decide}\
                 pub fn decide(s: &Status, allowed: bool) -> Outcome {{\n\
                 \x20   if s.is_ready() {{ Outcome::AlreadyThere }}\n\
                 \x20   else if !allowed {{ Outcome::Blocked }}\n\
                 \x20   else {{ Outcome::Proceed }}\n\
                 }}\n\n\
                 #[logic(open)]\n\
                 pub fn decide_logic(s: &Status, allowed: bool) -> Outcome {{\n\
                 \x20   pearlite! {{ {decide_body} }}\n\
                 }}\n"
            ),
        )
        .expect("decide.rs");
        tmp
    }

    /// The claim over [`mirror_subject`], in REQ047's shape: a decision that reaches `Proceed` only
    /// from a state that is not already ready. Binders come from the vocabulary's parameter types
    /// and are closed automatically.
    const MIRROR_REQ: &str = "requirement proceed_only_when_not_ready {
        category: 1
        vocabulary { state ready(s: Stat)
                     state proceeds(s: Stat, a: Flag) }
        require { always (not proceeds(s, a) or not ready(s)) }
    }";

    /// Bindings and resolutions for [`mirror_subject`] as the adapter would report them: `ready` is
    /// a **method**, `proceeds` is a **variant test** on a free function in another module, and
    /// `Flag` is the language's own `bool` (REQ058), which carries no `CodeMatch`.
    fn mirror_bindings() -> (
        Vec<Binding>,
        BTreeMap<String, Resolution>,
        BTreeMap<String, TypeResolution>,
    ) {
        let at = |file: &str, module: &str, text: &str| CodeMatch {
            file: file.into(),
            line: 1,
            text: text.into(),
            module: Some(vec![module.to_string()]),
        };
        let bindings = vec![
            binding("ready", "Status::is_ready"),
            binding("proceeds", "decide::Proceed"),
            binding("Stat", "Status"),
            binding("Flag", "bool"),
        ];
        let resolutions = BTreeMap::from([
            (
                "ready".to_string(),
                Resolution::Resolved {
                    at: at("src/status.rs", "status", "pub fn is_ready(&self) -> bool"),
                    params: vec![ParamMode::ByRef],
                    form: PredicateForm::Method {
                        name: "is_ready".into(),
                        via_trait: None,
                    },
                },
            ),
            (
                "proceeds".to_string(),
                Resolution::Resolved {
                    at: at(
                        "src/decide.rs",
                        "decide",
                        "pub fn decide(s: &Status, allowed: bool) -> Outcome",
                    ),
                    params: vec![ParamMode::ByRef, ParamMode::ByValue],
                    form: PredicateForm::VariantTest {
                        name: "decide".into(),
                        enum_name: "Outcome".into(),
                        variant: "Proceed".into(),
                        enum_module: Some(vec!["decide".into()]),
                    },
                },
            ),
        ]);
        let sorts = BTreeMap::from([
            (
                "Stat".to_string(),
                TypeResolution::Resolved(at("src/status.rs", "status", "pub enum Status")),
            ),
            ("Flag".to_string(), TypeResolution::Primitive("bool".into())),
        ]);
        (bindings, resolutions, sorts)
    }

    /// Lower [`MIRROR_REQ`] against the subject **through the mirror seam**, exactly as
    /// `verify` does: the bindings still name the program functions, and [`with_mirrors`]
    /// redirects them onto the staged mirrors before anything is lowered.
    fn mirror_harness(tmp: &std::path::Path) -> Harness {
        let (bindings, resolutions, sorts) = mirror_bindings();
        let read = |rel: &str| {
            (
                rel.to_string(),
                std::fs::read_to_string(tmp.join(rel)).expect("subject source"),
            )
        };
        let sources = BTreeMap::from([read("src/status.rs"), read("src/decide.rs")]);
        let (bindings, resolutions) = with_mirrors(&bindings, &resolutions, &sources);
        lower(
            &req(MIRROR_REQ),
            &bindings,
            &resolutions,
            &sorts,
            "provreq_mirror",
        )
        .expect("the mirrored fixture must lower")
    }

    // Verifies: REQ068 — THE POINT OF THE MIRROR CHANNEL, against the real prover. The same
    // ordinary program predicates that `real_creusot_is_inconclusive_on_opaque_predicates` shows
    // Creusot cannot reach become **provable** once each carries a checked `#[logic]` mirror, with
    // the existing `proof_assert!` harness unchanged (probe E).
    //
    // This is the first `proven` the tool produces end to end. It could not be measured on provreq
    // itself: Creusot cannot translate that crate at all (#153).
    #[test]
    #[ignore = "needs Creusot installed — run via `cargo test -- --ignored` (the CI `creusot` job)"]
    fn real_creusot_proves_a_claim_over_ordinary_functions_through_their_mirrors() {
        // The honest mirror: `is_ready` is true exactly on `Ready`.
        let tmp = mirror_subject(
            "match s { Status::Ready => true, _ => false }",
            "",
            "if crate::status::is_ready_logic(s) { Outcome::AlreadyThere } else if !allowed { Outcome::Blocked } else { Outcome::Proceed }",
            "",
        );
        let outcome = run(tmp.path(), &mirror_harness(tmp.path()));
        assert_eq!(
            outcome,
            Outcome::Holds,
            "a claim over ordinary program functions must be PROVED once they carry mirrors"
        );
    }

    // Verifies: REQ068 (the honesty crux) — a WRONG mirror does not prove something false. The
    // linking `#[ensures]` makes the prover discharge the mirror against the real body, so a mirror
    // that misstates its function fails at its own link. Probe D, reproduced through provreq's own
    // seam rather than by hand.
    //
    // provreq emits only the mirror's NAME; the meaning behind it is the operator's to review and
    // the prover's to check. This test is what makes that claim more than a slogan.
    #[test]
    #[ignore = "needs Creusot installed — run via `cargo test -- --ignored` (the CI `creusot` job)"]
    fn real_creusot_will_not_prove_a_claim_through_a_mirror_that_lies() {
        // Deliberately inverted: this mirror says `is_ready` means `Missing`.
        let tmp = mirror_subject(
            "match s { Status::Missing => true, _ => false }",
            "",
            "if crate::status::is_ready_logic(s) { Outcome::AlreadyThere } else if !allowed { Outcome::Blocked } else { Outcome::Proceed }",
            "",
        );
        let outcome = run(tmp.path(), &mirror_harness(tmp.path()));
        assert!(
            matches!(outcome, Outcome::Inconclusive { .. }),
            "a lying mirror must fail at its link, never yield a proof, got {outcome:?}"
        );
    }

    // Verifies (#164) — WHY provreq must never draft a contract clause onto a mirrored function.
    // This test asserts a **false** `proven`, deliberately, because the configuration is one a
    // prover will happily accept and only provreq can decline to produce.
    //
    // The linking `#[ensures(result == mirror(..))]` is discharged *assuming the function's
    // preconditions*. A precondition therefore narrows the domain on which the mirror was ever
    // checked, while the harness's `forall` ranges over all of it. Nothing below is contrived:
    //
    //   * `#[requires(!allowed)]` is an ordinary precondition, of exactly the kind the contract
    //     channel was prompted to propose.
    //   * The mirror is GENUINELY CORRECT under `!allowed`, so its link discharges honestly. There
    //     is no vacuity and no trick — the mirror really was checked against the real body.
    //   * But the harness also quantifies over `allowed = true`, where this mirror says `Blocked`
    //     and so never `Proceed`, making the invariant vacuously true.
    //
    // Creusot returns `Holds`. The requirement is false of the program — `decide` does return
    // `Proceed` when `allowed` and not ready — so a verdict built on this would be a false `proven`,
    // the one outcome the whole channel exists to make impossible.
    //
    // Nothing in the prover is wrong here; it discharged what it was asked. The defence has to be
    // that provreq never writes a precondition onto a function carrying a mirror link, which is why
    // contracts are now a Prusti-only channel. If this test ever stops returning `Holds`, the
    // reasoning behind that rule has changed and the rule should be re-derived, not assumed.
    #[test]
    #[ignore = "needs Creusot installed — run via `cargo test -- --ignored` (the CI `creusot` job)"]
    fn a_precondition_on_a_mirrored_function_can_prove_something_false() {
        let tmp = mirror_subject(
            "match s { Status::Ready => true, _ => false }",
            "",
            "if crate::status::is_ready_logic(s) { Outcome::AlreadyThere } else { Outcome::Blocked }",
            "#[requires(!allowed)]\n",
        );
        let outcome = run(tmp.path(), &mirror_harness(tmp.path()));
        assert_eq!(
            outcome,
            Outcome::Holds,
            "the prover accepts this — the defence is that provreq must not generate it"
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
