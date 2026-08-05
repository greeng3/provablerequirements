//! TLC — the category-2a engine, the **model** world's #1.
//!
//! The REQ027 analog of [`crate::kani`] for models. D2 gives the core one meaning and lowers
//! it to each engine; TLC is the lowering for category 2a, exactly as Kani is for category 1.
//! The binding stays core-owned ([`crate::grounding`]), the language stays the adapter's
//! ([`crate::tla_adapter`]), and this module owns one thing: how a gated category-2a
//! requirement becomes something TLC can run, and what its answer means.
//!
//! **Additive, subject untouched** (the whole-engine-family discipline, settled for Kani). The
//! subject writes its own TLA+ spec — the behaviour (`Init`/`Next`/`Spec`), the state
//! operators, the sets. provreq generates a *new* module that `EXTENDS` that spec and adds a
//! single temporal property, plus a `.cfg` naming the subject's `Spec` and provreq's property.
//! Nothing in the subject's spec is edited.
//!
//! Since #120 the generated files are not written **anywhere near** the spec: they go into
//! provreq's own scratch directory, and TLC is pointed at the spec through its module search path.
//! The subject's tree is only ever read. That began as a necessity — a spec may live in a
//! configured root outside the subject, which could be a repository provreq has no business
//! writing into — and it retires a guard and a sweeper along with it: there is no file to clobber
//! and no trace spec to remove.
//!
//! **Honest by construction (D8).** TLC is a *bounded* model checker — it explores the states
//! of the model the operator configured, not every execution — so a pass is
//! [`crate::verdict::Basis::ModelCheckedBounded`] and **never** `proven`, the same rung Kani
//! earns. A violation is the robust half: TLC prints a concrete behaviour, which is D9's
//! re-checkable witness for `fails`. Everything else (a spec that will not parse, an
//! unassigned `CONSTANT`, a missing `Spec`) is `unknown` with a reason, never a verdict.
//!
//! **What cannot be lowered is said, not approximated.** The linear-temporal core lowers:
//! `always`→`[]`, `never`→`[]~`, `eventually`→`<>`, `leads_to`→`~>`, over a `\A x \in Sort`
//! quantifier. A scope, a `with` guard, a metric `within`, a non-variable argument, or a
//! pattern outside that core (`precedes`, `occurs at most`, `can_reach`) is a [`NotLowerable`],
//! which becomes an honest `unknown` — D2's rule that an out-of-fragment operator is a typed
//! error surfaced to the author, never a silent approximation.
//!
//! Implements: REQ029 (wire TLC as cat-2a engine — a grounded model requirement earns a real
//! verdict).

use crate::grounding::Binding;
use crate::prl::ast::{Atom, Expr, Pattern, Property, Quantifier, Requirement, Scope};
use crate::tla_adapter::{self, ModelResolution};
use crate::verdict::{Basis, Evidence};
use std::path::{Path, PathBuf};

/// The behaviour-spec operator provreq checks against. TLA+ convention names the full
/// behaviour `Spec == Init /\ [][Next]_vars`; provreq requires that name rather than guessing
/// `Init`/`Next`, so a resolved `Spec` is the operator's explicit "this is the system".
const SPEC_OPERATOR: &str = "Spec";

/// The property name provreq's generated module defines and the `.cfg` checks. Prefixed so it
/// cannot collide with a definition already in the subject's spec.
const PROPERTY_NAME: &str = "ProvreqProp";

/// A generated TLC model check: an additive TLA+ `module` and its `.cfg`, both written under
/// `name` (`<name>.tla` / `<name>.cfg`), so `<name>` is the generated module's name too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub module: String,
    pub cfg: String,
}

/// Why a gated category-2a requirement could not be lowered. Never an approximation — the
/// reason is the operator's to read and act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotLowerable {
    pub reason: String,
}

impl NotLowerable {
    fn new(reason: impl Into<String>) -> Self {
        NotLowerable {
            reason: reason.into(),
        }
    }
}

/// What running TLC established (D7's three-valued polarity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Verified over the states TLC explored. Bounded — `model-checked`, never `proven`.
    Holds,
    /// Refuted. `violated` is TLC's own description of the broken property/invariant;
    /// `witness` is the concrete counter-example behaviour (D9), when TLC printed one.
    Fails {
        violated: Option<String>,
        witness: Option<String>,
    },
    /// TLC ran but could not decide — the spec would not parse, a `CONSTANT` was unassigned,
    /// `Spec` was missing, or the run errored. D10's `inconclusive(…)`.
    Inconclusive { reason: String },
}

impl Outcome {
    /// Map what TLC established into a piece of [`Evidence`]. The mapping lives here, in the
    /// engine, so [`crate::verdict`] never learns what TLC is (D2's "one meaning, lowering to
    /// each engine", running in this direction too). The core aggregates it into the
    /// requirement's verdict (D2b).
    ///
    /// The load-bearing line is `Holds` → [`Basis::ModelCheckedBounded`]: TLC is bounded, so a
    /// pass is `model-checked (bounded)` and never `proven`.
    pub fn into_evidence(&self) -> Evidence {
        match self {
            Outcome::Holds => Evidence::holds("TLC (TLA+)", Basis::ModelCheckedBounded),
            Outcome::Fails { violated, witness } => Evidence::fails(
                "TLC (TLA+)",
                witness.clone(),
                violated.iter().cloned().collect(),
            ),
            Outcome::Inconclusive { reason } => {
                Evidence::inconclusive("TLC (TLA+)", vec![reason.clone()])
            }
        }
    }
}

/// The generated module name for a requirement id — a valid TLA+ identifier (letter, then
/// letters/digits/underscores) prefixed so it cannot collide with the subject's own modules.
/// The file stem must equal the module name, so this is both.
pub fn module_name(id: &str) -> String {
    let sanitized: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("provreq_{}", sanitized.to_ascii_lowercase())
}

/// Lower a gated category-2a requirement to a TLC model check.
///
/// Pure — the caller locates the subject's spec and passes its module name in, so the whole
/// lowering is testable without TLC installed, which is what lets CI prove the engine-absent
/// path continuously (R-eng-2). `extends_module` is the subject's TLA+ module the generated
/// module extends (so its `Spec` and definitions are in scope); a symbol the subject's spec
/// does not define makes TLC fail to parse, and the verdict is honestly `unknown`.
pub fn lower(
    req: &Requirement,
    extends_module: &str,
    bindings: &[Binding],
    name: &str,
) -> Result<Check, NotLowerable> {
    if req.require.is_empty() {
        return Err(NotLowerable::new(
            "the requirement claims nothing — there is no property to check",
        ));
    }
    let mut formulas = Vec::new();
    for prop in &req.require {
        formulas.push(lower_property(prop, bindings)?);
    }
    // Several `require` claims are one conjoined temporal property, so the `.cfg` names one
    // `PROPERTY` (each claim already parenthesised, so `/\` binds correctly).
    let body = formulas.join("\n    /\\ ");
    let module = format!(
        "\\* Generated by provreq — do not edit; rewritten on every `verify` and removed \
         afterwards.\n\
         \\* An ADDITIVE module: it EXTENDS `{extends_module}` and changes nothing in the \
         subject's spec.\n\
         ---- MODULE {name} ----\n\
         EXTENDS {extends_module}\n\
         {PROPERTY_NAME} ==\n    {body}\n\
         ====\n"
    );
    let cfg = format!("SPECIFICATION {SPEC_OPERATOR}\nPROPERTY {PROPERTY_NAME}\n");
    Ok(Check {
        name: name.to_string(),
        module,
        cfg,
    })
}

/// Lower one `require` claim into a TLA+ temporal formula.
fn lower_property(prop: &Property, bindings: &[Binding]) -> Result<String, NotLowerable> {
    if prop.scope != Scope::Globally {
        return Err(NotLowerable::new(
            "the claim is limited to a scope (`before`/`after`/`between`) — the Dwyer-scope \
             encoding into linear temporal logic is deferred, so it is not lowered rather than \
             lowered wrongly",
        ));
    }
    let claim = lower_pattern(&prop.pattern, prop.quantifier.as_ref(), bindings)?;
    match &prop.quantifier {
        Some(q) => {
            let domain = sort_target(q, bindings)?;
            // `\A x \in Domain` is what makes this range over the sort rather than a single
            // element — TLC enumerates the (bounded) domain the operator's model configures.
            Ok(format!("(\\A {} \\in {domain} : ({claim}))", q.var))
        }
        None => Ok(format!("({claim})")),
    }
}

/// The TLA+ set a quantifier's sort resolves to. Unbound → not lowerable (REQ026 made sorts
/// bindable exactly so a quantifier has a domain).
fn sort_target(q: &Quantifier, bindings: &[Binding]) -> Result<String, NotLowerable> {
    bindings
        .iter()
        .find(|b| b.symbol == q.sort)
        .map(|b| b.observable.clone())
        .ok_or_else(|| {
            NotLowerable::new(format!(
                "the sort `{}` is not bound to a model set, so `{}` has no domain to range over",
                q.sort, q.var
            ))
        })
}

fn lower_pattern(
    pattern: &Pattern,
    quantifier: Option<&Quantifier>,
    bindings: &[Binding],
) -> Result<String, NotLowerable> {
    match pattern {
        Pattern::Always(e) => Ok(format!("[]({})", lower_expr(e, quantifier, bindings)?)),
        // `never P` is `always not P`.
        Pattern::Never(e) => Ok(format!("[](~({}))", lower_expr(e, quantifier, bindings)?)),
        Pattern::Eventually(e) => Ok(format!("<>({})", lower_expr(e, quantifier, bindings)?)),
        Pattern::LeadsTo { from, to, within } => {
            if within.is_some() {
                return Err(NotLowerable::new(
                    "`leads_to … within` is a metric (real-time) bound — plain TLC checks the \
                     qualitative `~>`, so the deadline is not expressible here (it belongs to a \
                     2b runtime monitor)",
                ));
            }
            Ok(format!(
                "(({}) ~> ({}))",
                lower_expr(from, quantifier, bindings)?,
                lower_expr(to, quantifier, bindings)?
            ))
        }
        other => Err(NotLowerable::new(format!(
            "`{}` is not in the linear-temporal core provreq lowers to TLC \
             (`always`/`never`/`eventually`/`leads_to`); its encoding is deferred rather than \
             approximated",
            pattern_verb(other)
        ))),
    }
}

fn lower_expr(
    e: &Expr,
    quantifier: Option<&Quantifier>,
    bindings: &[Binding],
) -> Result<String, NotLowerable> {
    match e {
        Expr::Atom(a) => lower_atom(a, quantifier, bindings),
        Expr::Not(inner) => Ok(format!("~({})", lower_expr(inner, quantifier, bindings)?)),
        Expr::And(l, r) => Ok(format!(
            "({} /\\ {})",
            lower_expr(l, quantifier, bindings)?,
            lower_expr(r, quantifier, bindings)?
        )),
        Expr::Or(l, r) => Ok(format!(
            "({} \\/ {})",
            lower_expr(l, quantifier, bindings)?,
            lower_expr(r, quantifier, bindings)?
        )),
    }
}

/// Lower one predicate application to a reference to the subject's model definition.
///
/// The name is the binding's observable — the definition [`crate::tla_adapter`] resolved
/// against the real spec. Arity is not re-checked here (existence-only grounding, REQ028): a
/// mismatch surfaces as a spec TLC cannot parse → `unknown`, never a wrong verdict.
fn lower_atom(
    a: &Atom,
    quantifier: Option<&Quantifier>,
    bindings: &[Binding],
) -> Result<String, NotLowerable> {
    if let Some(guard) = &a.guard {
        return Err(NotLowerable::new(format!(
            "`{}` carries a `with` guard ({guard}), which the parser keeps as raw text — \
             lowering it would mean emitting TLA+ this tool never understood",
            a.name
        )));
    }
    let binding = bindings
        .iter()
        .find(|b| b.symbol == a.name)
        .ok_or_else(|| {
            NotLowerable::new(format!(
                "`{}` is not bound to a model definition, so there is nothing to reference",
                a.name
            ))
        })?;

    let mut args = Vec::new();
    for arg in &a.args {
        let arg = arg.trim();
        // Only the quantified variable can be referenced. Any other term would emit a name
        // that exists in the requirement's world but not in the spec's.
        match quantifier {
            Some(q) if q.var == arg => args.push(arg.to_string()),
            _ => {
                return Err(NotLowerable::new(format!(
                    "`{}` is applied to `{arg}`, which is not the quantified variable — there \
                     is no value to give it",
                    a.name
                )))
            }
        }
    }
    if args.is_empty() {
        Ok(binding.observable.clone())
    } else {
        Ok(format!("{}({})", binding.observable, args.join(", ")))
    }
}

fn pattern_verb(pattern: &Pattern) -> &'static str {
    match pattern {
        Pattern::Never(_) => "never",
        Pattern::Always(_) => "always",
        Pattern::Eventually(_) => "eventually",
        Pattern::LeadsTo { .. } => "leads_to",
        Pattern::Precedes { .. } => "precedes",
        Pattern::OccursAtMost { .. } => "occurs at most",
        Pattern::CanReach(_) => "can_reach",
    }
}

/// Where the subject's behaviour spec lives, so provreq can generate a module beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecSite {
    /// The directory holding the spec module. **Not** where the generated module is written —
    /// provreq writes into its own scratch dir and names this on TLC's module search path
    /// instead, because a configured spec root may be a repository provreq has no business
    /// writing into (#120). Kept because it is where the operator's spec actually is.
    pub dir: PathBuf,
    /// The spec's TLA+ module name (from its `---- MODULE X ----` header), which the generated
    /// module extends.
    pub module: String,
    /// Every directory SANY may resolve an `EXTENDS` from: the spec's own directory, plus each
    /// configured root, so a spec that extends a sibling module still parses.
    pub library: Vec<PathBuf>,
}

/// The tla2tools.jar path — `TLA2TOOLS_JAR` if set, else the location the native provisioner
/// installs to ([`crate::provision::tla2tools_jar_default`]). TLC is invoked as
/// `java -cp <jar> tlc2.TLC`, so there is no PATH binary to probe. The devcontainer image sets
/// `TLA2TOOLS_JAR` explicitly, so its baked-in jar still wins there; a natively-provisioned TLC
/// (REQ046) lands at the default path this returns, so installer and detector agree.
///
/// `// ponytail: env var + provisioned default is enough until a real subject needs a per-project
/// jar; move to provreq.yml config then.`
pub fn jar_path() -> String {
    std::env::var("TLA2TOOLS_JAR").unwrap_or_else(|_| {
        crate::provision::tla2tools_jar_default()
            .to_string_lossy()
            .into_owned()
    })
}

/// Locate the subject's behaviour spec (the module defining `Spec`) so a check can be generated
/// against it. `Err` when there is no single `Spec` to check against — an honest `inconclusive`,
/// never a guess at `Init`/`Next`.
pub fn locate_spec(
    subject_root: &Path,
    companion_root: &Path,
    extra: &crate::spec_paths::SpecPaths,
) -> Result<SpecSite, String> {
    // One lookup, so this loads the specs for itself rather than taking a shared read (#144) — the
    // sharing that matters is a binding set's many symbols, which is `grounding::resolve_bindings`.
    let specs = tla_adapter::SubjectSpecs::load(subject_root, companion_root, extra);
    // A behaviour definition takes no arguments: the `.cfg` names `SPECIFICATION Spec`, which
    // TLC reads as a formula, not as something to apply.
    let at = match tla_adapter::resolve(&specs, SPEC_OPERATOR, 0) {
        ModelResolution::Resolved(at) => at,
        ModelResolution::WrongArity { at, declared, .. } => {
            return Err(format!(
                "`{SPEC_OPERATOR}` at {}:{} takes {declared} argument(s) — provreq checks a \
                 behaviour formula `{SPEC_OPERATOR} == Init /\\ [][Next]_vars`, and a \
                 `SPECIFICATION` TLC can use takes none",
                at.file, at.line
            ))
        }
        ModelResolution::NotFound => {
            return Err(format!(
                "no `{SPEC_OPERATOR}` behaviour definition in the subject's TLA+ — provreq \
                 checks a named `{SPEC_OPERATOR} == Init /\\ [][Next]_vars`; define one so the \
                 model has a behaviour to check the property against"
            ))
        }
        ModelResolution::Ambiguous(ats) => {
            let places = ats
                .iter()
                .map(|a| format!("{}:{}", a.file, a.line))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "`{SPEC_OPERATOR}` is defined in several specs ({places}) — provreq cannot tell \
                 which behaviour to check; keep one `{SPEC_OPERATOR}` in the subject"
            ));
        }
    };
    let spec_path = subject_root.join(&at.file);
    let dir = spec_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| subject_root.to_path_buf());
    let text = std::fs::read_to_string(&spec_path)
        .map_err(|e| format!("could not read the spec {}: {e}", at.file))?;
    let module = module_header(&text).ok_or_else(|| {
        format!(
            "the spec {} has no `---- MODULE X ----` header, so there is no module to extend",
            at.file
        )
    })?;
    // The spec's own directory first, so a module beside it wins over a same-named one in a
    // configured root — nearest-to-the-spec is the reading least likely to surprise.
    let mut library = vec![dir.clone()];
    for root in extra.roots() {
        if !library.contains(root) {
            library.push(root.clone());
        }
    }
    Ok(SpecSite {
        dir,
        module,
        library,
    })
}

/// The module name from a spec's `---- MODULE X ----` header (the first such line).
fn module_header(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("----") {
            let rest = rest.trim_start_matches('-').trim();
            if let Some(after) = rest.strip_prefix("MODULE") {
                let name = after.trim().trim_end_matches('-').trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

/// The full `java` argument list for one TLC run, pure so the isolation below is testable
/// without spawning a JVM.
///
/// **`-Djava.io.tmpdir` is load-bearing, not tidiness.** SANY extracts the standard modules
/// (`Naturals`, …) out of `tla2tools.jar` into the JVM's temp dir, which is process-shared by
/// default. Two TLC runs at once race on that path: one reads a half-written module and the whole
/// spec fails semantic analysis with `Module-Table lookup failure`, which provreq would report as
/// an `inconclusive` **about our own scratch space rather than the subject** — a dishonest verdict.
/// Measured at 13 failures / 240 runs at concurrency 8, and 0 / 240 once each run gets its own
/// temp dir. Concurrent runs are reachable in production: `verify::verify` calls [`run`], and the
/// server's verify handler serves concurrent requests.
///
/// The metadir doubles as the temp dir — it is already unique per run and swept with it.
///
/// **`-DTLA-Library` is what lets the generated module live away from the spec.** SANY resolves
/// `EXTENDS` from the main module's own directory and from the directories this names, so provreq
/// generates into its own scratch dir and points here at the spec instead of writing beside it
/// (#120). That keeps provreq out of the subject's tree entirely, and out of a configured spec root
/// it may have no business writing into. Verified against real TLC before being relied on.
fn tlc_args(jar: &str, metadir: &Path, library: &[PathBuf], name: &str) -> Vec<String> {
    let mut args = vec![format!("-Djava.io.tmpdir={}", metadir.display())];
    // `join_paths` uses the platform's own separator, so a spec directory is never split on a
    // character that is legal inside its name.
    if let Ok(joined) = std::env::join_paths(library) {
        args.push(format!("-DTLA-Library={}", joined.to_string_lossy()));
    }
    args.extend([
        "-cp".to_string(),
        jar.to_string(),
        "tlc2.TLC".to_string(),
        // Scratch (`states/`) goes outside the subject, so the run leaves no litter.
        "-metadir".to_string(),
        metadir.display().to_string(),
        "-config".to_string(),
        format!("{name}.cfg"),
        format!("{name}.tla"),
    ]);
    args
}

/// Write the generated module + cfg into provreq's own scratch directory, run TLC there against
/// the subject's spec, and take the whole directory away again.
///
/// Additive and non-destructive, the Kani discipline — and since #120, more strictly so: the
/// generated files no longer go beside the spec at all. TLC finds the spec through
/// `-DTLA-Library` instead ([`tlc_args`]). The subject's tree is never written to, so there is no
/// file to clobber, no litter to sweep, and nothing to clean up on the failing path; and a spec
/// root the operator configured — which may be a repository provreq has no business writing into,
/// or one it cannot write to at all — is only ever read.
///
/// The scratch directory is the metadir, which already had to exist for TLC's own `states/`, so
/// this costs nothing beyond the paths.
///
/// `// ponytail: TLC's default worker/heap settings and no timeout — its own defaults until a
/// real subject shows they are wrong; workers/timeout belong in provreq.yml config.`
pub fn run(site: &SpecSite, check: &Check) -> Outcome {
    let metadir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            return Outcome::Inconclusive {
                reason: format!("could not create a scratch metadir for TLC: {e}"),
            }
        }
    };
    let tla_path = metadir.path().join(format!("{}.tla", check.name));
    let cfg_path = metadir.path().join(format!("{}.cfg", check.name));
    if let Err(e) = std::fs::write(&tla_path, &check.module) {
        return Outcome::Inconclusive {
            reason: format!(
                "could not write the generated module to {}: {e}",
                tla_path.display()
            ),
        };
    }
    if let Err(e) = std::fs::write(&cfg_path, &check.cfg) {
        return Outcome::Inconclusive {
            reason: format!("could not write the config to {}: {e}", cfg_path.display()),
        };
    }

    let output = std::process::Command::new("java")
        .args(tlc_args(
            &jar_path(),
            metadir.path(),
            &site.library,
            &check.name,
        ))
        .current_dir(metadir.path())
        .output();

    match output {
        Ok(o) => classify(&format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )),
        Err(e) => Outcome::Inconclusive {
            reason: format!(
                "could not run TLC (`java -cp {} tlc2.TLC`): {e}",
                jar_path()
            ),
        },
    }
}

// The trace-spec sweeper that used to live here is gone with #120. It existed because a violation
// makes TLC drop `<name>_TTrace_*.tla` beside the module it checked, which used to be the subject's
// own directory. The module is now generated inside the scratch metadir, so the trace lands there
// too and goes with it — the sweep has nothing left to sweep.

/// Map TLC's output to an outcome. Pure and separately tested — the mapping is where a verdict
/// could silently become dishonest, so it must be checkable without running TLC.
///
/// The default is [`Outcome::Inconclusive`]: only TLC's own explicit success line is read as a
/// pass. Unrecognised output is never optimistically treated as `holds`.
pub fn classify(output: &str) -> Outcome {
    if output.contains("Model checking completed. No error has been found.") {
        return Outcome::Holds;
    }
    // Both a temporal violation ("Temporal properties were violated.") and a safety violation
    // ("Invariant X is violated.") print an `Error: … violated` line — catch either.
    if let Some(violated) = violated(output) {
        return Outcome::Fails {
            violated: Some(violated),
            witness: witness(output),
        };
    }
    Outcome::Inconclusive {
        reason: diagnostic(output),
    }
}

/// TLC's own one-line description of the broken property or invariant.
fn violated(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("Error:") && l.contains("violated"))
        .map(str::to_string)
}

/// The concrete counter-example behaviour TLC printed — D9's re-checkable witness. `None` when
/// TLC refuted the property without printing a trace. Captured line-wise from the trace header
/// up to (not including) the run summary, so a stray `Finished`/`states generated` summary line
/// never leaks into the witness.
fn witness(output: &str) -> Option<String> {
    const HEADERS: [&str; 2] = [
        "Error: The following behavior constitutes a counter-example:",
        "Error: The behavior up to this point is:",
    ];
    let mut trace = Vec::new();
    let mut started = false;
    for line in output.lines() {
        if !started {
            if HEADERS.iter().any(|h| line.contains(h)) {
                started = true;
                trace.push(line);
            }
            continue;
        }
        let t = line.trim_start();
        if t.starts_with("Finished") || t.contains("states generated") || t.starts_with("The depth")
        {
            break;
        }
        trace.push(line);
    }
    if trace.is_empty() {
        return None;
    }
    Some(trace.join("\n").trim_end().to_string())
}

/// Why TLC could not decide, in the operator's terms — the first `Error:`/`***` line, plus the
/// cause that follows when that line is one of SANY's banners, else the tail of the log.
///
/// **A `***` line is a banner, never a diagnosis.** SANY's semantic family prints
/// `*** Errors: N` and its syntactic family `***Parse Error***`; in both, the location and the
/// message follow on later lines. Reporting the banner alone handed the operator a *count*
/// where they needed a *cause* — a wrong-arity model binding read as `*** Errors: 1` while TLC
/// had plainly said `The operator accepted requires 0 arguments.` (#206). That silence covered
/// every SANY parse/semantic failure reachable from cat-2a, not just arity.
///
/// An `Error:` line is carried unchanged, because TLC's own runtime errors do state their cause
/// there (`Error: The constant parameter MaxLen is not assigned a value …`) — which is why the
/// banner case went unnoticed: the case this was first written against genuinely works.
fn diagnostic(output: &str) -> String {
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    let Some(at) = lines
        .iter()
        .position(|l| l.starts_with("Error:") || l.starts_with("***"))
    else {
        return tail(output);
    };
    if !lines[at].starts_with("***") {
        return lines[at].to_string();
    }
    // The banner is kept ahead of the cause: `*** Errors: 3` tells the operator that what
    // follows is the first of several, so a single reason never reads as the whole story.
    std::iter::once(lines[at])
        .chain(
            lines[at + 1..]
                .iter()
                .filter(|l| !l.is_empty())
                .take(BANNER_CAUSE_LINES)
                .copied(),
        )
        .collect::<Vec<_>>()
        .join(" — ")
}

/// How many lines after a SANY banner carry the cause. Two is what each banner family needs —
/// location then message for a semantic error, `Was expecting` then `Encountered` for a parse
/// error — and it holds a multi-error run to its *first* cause instead of pasting the list into
/// a verdict.
const BANNER_CAUSE_LINES: usize = 2;

/// The last few non-empty lines of TLC output — enough to see why it could not decide without
/// pasting a whole log into the verdict.
fn tail(output: &str) -> String {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(TAIL_LINES);
    let tail = lines[start..].join("\n");
    if tail.trim().is_empty() {
        "TLC produced no recognisable verdict".to_string()
    } else {
        tail
    }
}

/// How many lines of TLC output an `inconclusive` carries.
const TAIL_LINES: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grounding::{BindCategory, Fidelity};
    use crate::prl::gate;
    use crate::verdict::Provenance;

    const MODEL_REQ: &str = "requirement r {
        category: 2a
        vocabulary { state accepted(m), succeeded(m) }
        require { each m: Message . accepted(m) leads_to succeeded(m) }
    }";

    // Verifies: every TLC run gets its OWN JVM temp dir. Without this, concurrent runs race on
    // SANY's shared extraction of the jar's standard modules and one of them reports a
    // `Module-Table lookup failure` — an `inconclusive` about provreq's scratch space rather than
    // the subject. The race is only ~5% per run, so a real-engine test cannot be trusted to catch
    // a regression here; this asserts the isolation directly.
    #[test]
    fn every_run_gets_its_own_jvm_temp_dir() {
        let metadir = Path::new("/scratch/meta-xyz");
        let args = tlc_args(
            "/opt/tla2tools.jar",
            metadir,
            &[PathBuf::from("/subject/specs")],
            "provreq_smoke",
        );

        assert!(
            args.contains(&format!("-Djava.io.tmpdir={}", metadir.display())),
            "the JVM temp dir must be redirected to the per-run metadir: {args:?}"
        );
        // A JVM `-D` property is only read as such before the class name; after it, it is just an
        // argument to TLC.
        let tmpdir_at = args.iter().position(|a| a.starts_with("-Djava.io.tmpdir"));
        let main_class_at = args.iter().position(|a| a == "tlc2.TLC");
        assert!(
            tmpdir_at < main_class_at,
            "-D must precede the main class: {args:?}"
        );
    }

    fn req(src: &str) -> Requirement {
        gate(src)
            .expect("test candidate should clear the gate")
            .requirement
    }

    fn binding(symbol: &str, observable: &str) -> Binding {
        Binding {
            symbol: symbol.into(),
            category: BindCategory::Model,
            observable: observable.into(),
            fidelity: Fidelity::Definitional,
        }
    }

    fn standard_bindings() -> Vec<Binding> {
        vec![
            binding("accepted", "Accepted"),
            binding("succeeded", "Succeeded"),
            binding("Message", "Message"),
        ]
    }

    fn lower_standard() -> Result<Check, NotLowerable> {
        lower(
            &req(MODEL_REQ),
            "Msg",
            &standard_bindings(),
            "provreq_req001",
        )
    }

    // Verifies: REQ029 — a quantified 2a `leads_to` lowers to an additive module that EXTENDS
    // the subject's spec and defines a `~>` property over the sort's model set.
    #[test]
    fn quantified_leads_to_lowers_to_a_temporal_property() {
        let c = lower_standard().expect("should lower");
        assert_eq!(c.name, "provreq_req001");
        assert!(
            c.module.contains("---- MODULE provreq_req001 ----"),
            "{}",
            c.module
        );
        assert!(c.module.contains("EXTENDS Msg"), "{}", c.module);
        assert!(
            c.module
                .contains("(\\A m \\in Message : (((Accepted(m)) ~> (Succeeded(m)))))"),
            "the claim must lower to a quantified leads-to over the model definitions: {}",
            c.module
        );
    }

    // Verifies: REQ029 — the generated `.cfg` names the subject's `Spec` behaviour and
    // provreq's property, so TLC checks the property against the subject's real model.
    #[test]
    fn cfg_names_the_subject_spec_and_the_property() {
        let c = lower_standard().expect("should lower");
        assert!(c.cfg.contains("SPECIFICATION Spec"), "{}", c.cfg);
        assert!(c.cfg.contains("PROPERTY ProvreqProp"), "{}", c.cfg);
    }

    // Verifies: REQ029 — `always`/`never`/`eventually` each lower to their TLA+ operator.
    #[test]
    fn safety_and_eventually_patterns_lower_to_tla_operators() {
        let always = lower(
            &req(
                "requirement r { category: 2a vocabulary { state safe } require { always safe } }",
            ),
            "M",
            &[binding("safe", "Safe")],
            "h",
        )
        .expect("always");
        assert!(always.module.contains("[](Safe)"), "{}", always.module);

        let never = lower(
            &req("requirement r { category: 2a vocabulary { state bad } require { never bad } }"),
            "M",
            &[binding("bad", "Bad")],
            "h",
        )
        .expect("never");
        assert!(never.module.contains("[](~(Bad))"), "{}", never.module);

        let eventually = lower(
            &req("requirement r { category: 2a vocabulary { state done } require { eventually done } }"),
            "M",
            &[binding("done", "Done")],
            "h",
        )
        .expect("eventually");
        assert!(
            eventually.module.contains("<>(Done)"),
            "{}",
            eventually.module
        );
    }

    // Verifies: REQ029 — an unbound sort has no model set to range over, so the requirement
    // does not lower rather than silently dropping the quantifier.
    #[test]
    fn unbound_sort_does_not_lower() {
        let e = lower(
            &req(MODEL_REQ),
            "Msg",
            &[
                binding("accepted", "Accepted"),
                binding("succeeded", "Succeeded"),
            ],
            "h",
        )
        .expect_err("an unbound sort has no domain");
        assert!(e.reason.contains("Message"), "{}", e.reason);
        assert!(e.reason.contains("no domain"), "{}", e.reason);
    }

    // Verifies: REQ029 — an unbound predicate does not lower; there is no model definition to
    // reference.
    #[test]
    fn unbound_predicate_does_not_lower() {
        let e = lower(
            &req(MODEL_REQ),
            "Msg",
            &[
                binding("accepted", "Accepted"),
                binding("Message", "Message"),
            ],
            "h",
        )
        .expect_err("succeeded is unbound");
        assert!(e.reason.contains("succeeded"), "{}", e.reason);
    }

    // Verifies: REQ029 — a metric `leads_to … within` is not lowered to the qualitative `~>`;
    // the deadline would be silently dropped, which is a 2b concern, not a 2a one.
    #[test]
    fn metric_leads_to_does_not_lower() {
        let e = lower(
            &req("requirement r {
                category: 2a
                vocabulary { state p, q }
                require { p leads_to q within 30s }
            }"),
            "M",
            &[binding("p", "P"), binding("q", "Q")],
            "h",
        )
        .expect_err("a real-time bound is not expressible in plain TLC");
        assert!(e.reason.contains("within"), "{}", e.reason);
    }

    // Verifies: REQ029 — a pattern outside the linear-temporal core (`can_reach`) is not
    // lowered rather than approximated.
    #[test]
    fn out_of_core_pattern_does_not_lower() {
        let e = lower(
            &req("requirement r {
                category: 2a
                vocabulary { state deadlock }
                require { can_reach deadlock }
            }"),
            "M",
            &[binding("deadlock", "Deadlock")],
            "h",
        )
        .expect_err("can_reach is CTL EF, not in the lowered core");
        assert!(e.reason.contains("can_reach"), "{}", e.reason);
    }

    // Verifies: REQ029 — TLC's explicit success line is the ONLY thing read as a pass.
    #[test]
    fn successful_check_is_holds() {
        assert_eq!(
            classify("Checking temporal properties...\nModel checking completed. No error has been found.\n"),
            Outcome::Holds
        );
    }

    // Verifies: REQ029 (D9) — a temporal violation is `fails`, carrying the property line and
    // the counter-example behaviour as the re-checkable witness.
    #[test]
    fn temporal_violation_is_fails_with_a_witness() {
        let output = "\
Error: Temporal properties were violated.
Error: The following behavior constitutes a counter-example:
State 1: <Initial predicate>
pc = 0

State 2: <Next line 5>
pc = 1

2 states generated, 2 distinct states found, 0 states left on queue.
Finished in 00s
";
        let Outcome::Fails { violated, witness } = classify(output) else {
            panic!("a violated property must refute");
        };
        assert!(violated
            .expect("names the property")
            .contains("Temporal properties were violated"));
        let w = witness.expect("must carry the counter-example");
        assert!(w.contains("State 1"), "{w}");
        assert!(w.contains("pc = 1"), "{w}");
        assert!(
            !w.contains("states generated"),
            "the summary is not part of the witness: {w}"
        );
    }

    // Verifies: REQ029 — a safety (invariant) violation is also `fails`; TLC reports it with a
    // different phrase but the same `Error: … violated` shape.
    #[test]
    fn invariant_violation_is_fails() {
        let output = "\
Error: Invariant Accepted is violated.
Error: The behavior up to this point is:
State 1: <Initial predicate>
pc = 0

2 states generated
";
        let Outcome::Fails { violated, .. } = classify(output) else {
            panic!("a violated invariant must refute");
        };
        assert!(violated
            .expect("names the invariant")
            .contains("Invariant Accepted"));
    }

    // Verifies: REQ029 — an unassigned CONSTANT is INCONCLUSIVE, never an optimistic pass, and
    // names the actionable cause (the constant the operator must give a model value).
    #[test]
    fn unassigned_constant_is_inconclusive_and_names_the_cause() {
        let output = "\
Starting...
Error: The constant parameter MaxLen is not assigned a value by the configuration file.
";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("an unassigned constant decides nothing");
        };
        assert!(reason.contains("MaxLen"), "{reason}");
        assert!(reason.contains("not assigned"), "{reason}");
    }

    // Verifies: REQ029 (#206) — a SANY *semantic* failure reports the CAUSE, not the count.
    // Captured verbatim from a real TLC run over a wrong-arity model binding (`accepted(m)`
    // bound to a 0-ary VARIABLE): the actionable sentence sits three lines below the banner,
    // and reporting `*** Errors: 1` alone told the operator only that something was wrong.
    #[test]
    fn a_sany_semantic_banner_reports_the_cause_not_the_count() {
        let output = "\
Semantic processing of module probe
Semantic errors:

*** Errors: 1

line 4, col 55 to line 4, col 57 of module probe

The operator accepted requires 0 arguments.

Starting... (2026-08-04 23:07:37)
Error: Parsing or semantic analysis failed.
";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a spec SANY rejects decides nothing");
        };
        assert!(
            reason.contains("requires 0 arguments"),
            "the cause must survive: {reason}"
        );
        assert!(
            reason.contains("line 4, col 55"),
            "the location must survive: {reason}"
        );
    }

    // Verifies: REQ029 (#206) — the *syntactic* banner family is carried the same way. Also
    // captured from a real run. `***Parse Error***` is as empty a reason as `*** Errors: 1`.
    #[test]
    fn a_sany_parse_banner_reports_the_cause_not_the_count() {
        let output = "\
Parsing file probe2.tla
***Parse Error***
Was expecting \"==== or more Module body\"
Encountered \"<EOF>\" at line 8, column 15 and token \"0\"

Residual stack trace follows:
";
        let Outcome::Inconclusive { reason } = classify(output) else {
            panic!("a spec SANY cannot parse decides nothing");
        };
        assert!(reason.contains("Was expecting"), "{reason}");
        assert!(reason.contains("at line 8, column 15"), "{reason}");
        assert!(
            !reason.contains("Residual stack trace"),
            "the cause is bounded — a stack-trace header is not a diagnosis: {reason}"
        );
    }

    // Verifies: REQ029 (#206) — a banner with nothing after it still reports the banner rather
    // than falling through to the log tail; the operator loses nothing that was ever there.
    #[test]
    fn a_banner_with_no_cause_after_it_is_still_the_reason() {
        let Outcome::Inconclusive { reason } = classify("Starting...\n*** Errors: 1\n") else {
            panic!("a banner decides nothing");
        };
        assert_eq!(reason, "*** Errors: 1");
    }

    // Verifies: REQ029 — output with no verdict line is inconclusive and says so, never a
    // silent pass.
    #[test]
    fn empty_output_is_inconclusive() {
        let Outcome::Inconclusive { reason } = classify("") else {
            panic!("no output decides nothing");
        };
        assert!(reason.contains("no recognisable verdict"), "{reason}");
    }

    // Verifies: REQ029 — the module name is a valid TLA+ identifier derived from the id,
    // prefixed so it cannot collide with the subject's own modules.
    #[test]
    fn module_name_is_a_valid_prefixed_identifier() {
        assert_eq!(module_name("REQ001"), "provreq_req001");
        assert_eq!(module_name("REQ-1.2"), "provreq_req_1_2");
    }

    // Verifies: REQ029 — the `---- MODULE X ----` header is read so the generated module can
    // EXTEND the subject's real module name.
    #[test]
    fn module_header_is_read_from_the_spec() {
        assert_eq!(
            module_header("---- MODULE Msg ----\nVARIABLES x\n").as_deref(),
            Some("Msg")
        );
        assert_eq!(
            module_header("------------- MODULE Foo -------------\n").as_deref(),
            Some("Foo")
        );
        assert_eq!(module_header("VARIABLES x\nInit == x = 0\n"), None);
    }

    fn prov() -> Provenance {
        Provenance {
            requirement_revision: "rev-1".into(),
            subject_commit: Some("abc123".into()),
            tool_version: "0.0.1".into(),
        }
    }

    // Verifies: REQ029 (D8) — a TLC pass is `model-checked (bounded)` and NEVER `proven`. TLC
    // explores a bounded model, so claiming ∀-executions would be the overclaim REQ024 guards.
    #[test]
    fn a_tlc_pass_is_bounded_model_checked_never_proven() {
        let v = crate::verdict::aggregate("SR001", vec![Outcome::Holds.into_evidence()], prov());
        assert_eq!(v.status, crate::verdict::Status::Holds);
        assert_eq!(v.basis, Some(Basis::ModelCheckedBounded));
        let text = crate::verdict::render(&v);
        assert!(text.contains("model-checked (bounded)"), "{text}");
        assert!(text.contains("NOT proven"), "{text}");
    }

    // Verifies: REQ029 (D9) — a violation becomes a `fails` carrying its counter-example as a
    // re-checkable witness.
    #[test]
    fn a_tlc_violation_becomes_a_fails_carrying_its_witness() {
        let outcome = Outcome::Fails {
            violated: Some("Error: Temporal properties were violated.".into()),
            witness: Some("State 1: <Initial predicate>\npc = 0".into()),
        };
        let v = crate::verdict::aggregate("SR002", vec![outcome.into_evidence()], prov());
        assert_eq!(v.status, crate::verdict::Status::Fails);
        assert_eq!(v.basis, None, "a fails has a witness, not a basis");
        let text = crate::verdict::render(&v);
        assert!(text.contains("SR002: fails"), "{text}");
        assert!(text.contains("witness"), "{text}");
        assert!(text.contains("State 1"), "{text}");
    }

    // Verifies: REQ029 (D10) — an engine that could not decide yields unknown/inconclusive,
    // never a verdict.
    #[test]
    fn an_undecided_run_is_unknown_inconclusive_never_a_verdict() {
        let outcome = Outcome::Inconclusive {
            reason: "Error: The constant parameter MaxLen is not assigned a value.".into(),
        };
        let v = crate::verdict::aggregate("SR003", vec![outcome.into_evidence()], prov());
        assert_eq!(v.status, crate::verdict::Status::Unknown);
        assert_eq!(v.reason, Some(crate::verdict::UnknownReason::Inconclusive));
        let text = crate::verdict::render(&v);
        assert!(text.contains("not evidence either way"), "{text}");
        assert!(text.contains("MaxLen"), "{text}");
    }

    // ----- Real-engine tests (need TLC installed): `cargo test -- --ignored`, the CI `tlc`
    // job. `#[ignore]` is deliberate (R-eng-2): the common state is engine-ABSENT, so the main
    // job stays TLC-free and proves that path continuously.

    /// A real TLA+ subject: a two-state machine, `Spec` with weak fairness so `Accepted ~>
    /// Succeeded` holds, or without it (via `fair`) so it fails. Constant-free, so TLC needs no
    /// model beyond what the spec defines.
    fn tla_subject(fair: bool) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("Msg.tla"), spec_text(fair)).expect("Msg.tla");
        tmp
    }

    /// The same spec text, so a test can place it somewhere other than the subject root.
    fn spec_text(fair: bool) -> String {
        let fairness = if fair { " /\\ WF_pc(Next)" } else { "" };
        {
            format!(
                "---- MODULE Msg ----\n\
                 EXTENDS Naturals\n\
                 VARIABLES pc\n\
                 Init == pc = 0\n\
                 Next == (pc = 0 /\\ pc' = 1) \\/ (pc = 1 /\\ pc' = 1)\n\
                 Spec == Init /\\ [][Next]_pc{fairness}\n\
                 Accepted(m) == pc = 0\n\
                 Succeeded(m) == pc = 1\n\
                 Message == {{0}}\n\
                 ====\n"
            )
        }
    }

    fn site_for(tmp: &tempfile::TempDir) -> SpecSite {
        locate_spec(
            tmp.path(),
            &tmp.path().join("ProvableRequirements"),
            &crate::spec_paths::SpecPaths::default(),
        )
        .expect("Spec located")
    }

    // Verifies: REQ029 — THE REAL ENGINE, end to end: with fairness, `Accepted ~> Succeeded`
    // holds over the real model and earns a bounded `holds`.
    #[test]
    #[ignore = "needs TLC installed — run via `cargo test -- --ignored` (the CI `tlc` job)"]
    fn real_tlc_verifies_a_true_leads_to() {
        let tmp = tla_subject(true);
        let check = lower(
            &req(MODEL_REQ),
            "Msg",
            &standard_bindings(),
            "provreq_smoke",
        )
        .expect("should lower");
        let outcome = run(&site_for(&tmp), &check);
        assert_eq!(
            outcome,
            Outcome::Holds,
            "a fair leads-to must verify: {outcome:?}"
        );
    }

    // Verifies: REQ029 (#206) — THE REAL ENGINE, on a spec SANY rejects: the inconclusive
    // carries TLC's own cause, not its count. `accepted` is bound to the 0-ary VARIABLE `pc`
    // instead of the 1-ary `Accepted`, which grounds today (existence-only, REQ028) and reaches
    // TLC — the exact operator slip #119 is about. The pure `classify` tests pin the parsing,
    // but only the real engine proves SANY still frames a semantic error this way; a banner
    // whose cause moved would leave the operator with `*** Errors: 1` again.
    #[test]
    #[ignore = "needs TLC installed — run via `cargo test -- --ignored` (the CI `tlc` job)"]
    fn real_tlc_reports_the_cause_of_a_wrong_arity_binding() {
        let tmp = tla_subject(true);
        let check = lower(
            &req(MODEL_REQ),
            "Msg",
            &[
                binding("accepted", "pc"),
                binding("succeeded", "Succeeded"),
                binding("Message", "Message"),
            ],
            "provreq_smoke",
        )
        .expect("a wrong-arity binding still lowers — arity is not checked here");
        let Outcome::Inconclusive { reason } = run(&site_for(&tmp), &check) else {
            panic!("a spec SANY rejects decides nothing");
        };
        assert!(
            reason.contains("requires 0 arguments"),
            "the operator must get TLC's cause, not its count: {reason}"
        );
    }

    // Verifies: REQ028/REQ029 (#120) — THE REAL ENGINE against a spec that lives OUTSIDE the
    // subject. The subject directory is empty; the spec sits in a sibling directory named only by
    // a configured root. Before this, such a layout could not be checked at all.
    //
    // This is the test that matters for #120, because the mechanism it depends on is TLC's, not
    // provreq's: SANY has to resolve `EXTENDS Msg` through `-DTLA-Library` from a module generated
    // somewhere else entirely. Nothing pure can establish that.
    #[test]
    #[ignore = "needs TLC installed — run via `cargo test -- --ignored` (the CI `tlc` job)"]
    fn real_tlc_checks_a_spec_outside_the_subject() {
        let home = tempfile::tempdir().expect("tempdir");
        let subject = home.path().join("subject");
        let models = home.path().join("models");
        std::fs::create_dir_all(&subject).expect("subject");
        std::fs::create_dir_all(&models).expect("models");
        std::fs::write(models.join("Msg.tla"), spec_text(true)).expect("Msg.tla");

        let paths = crate::spec_paths::SpecPaths::from_roots(vec![models.clone()]);
        let site = locate_spec(&subject, &subject.join("ProvableRequirements"), &paths)
            .expect("the external Spec must be located");
        let check = lower(
            &req(MODEL_REQ),
            &site.module,
            &standard_bindings(),
            "provreq_smoke",
        )
        .expect("should lower");

        let outcome = run(&site, &check);
        assert_eq!(
            outcome,
            Outcome::Holds,
            "a spec outside the subject must still verify: {outcome:?}"
        );
        // And provreq wrote nothing into a directory it does not own.
        let left: Vec<_> = std::fs::read_dir(&models)
            .expect("readdir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["Msg.tla".to_string()], "{left:?}");
    }

    // Verifies: REQ029 (D9) — THE REAL ENGINE refutes a false leads-to (no fairness: pc can
    // stall at 0 forever) and hands back a concrete counter-example behaviour.
    #[test]
    #[ignore = "needs TLC installed — run via `cargo test -- --ignored` (the CI `tlc` job)"]
    fn real_tlc_refutes_an_unfair_leads_to_with_a_witness() {
        let tmp = tla_subject(false);
        let check = lower(
            &req(MODEL_REQ),
            "Msg",
            &standard_bindings(),
            "provreq_smoke",
        )
        .expect("should lower");
        let outcome = run(&site_for(&tmp), &check);
        let Outcome::Fails { witness, .. } = outcome else {
            panic!("an unfair leads-to must be refuted, got {outcome:?}");
        };
        assert!(witness
            .expect("TLC must print a behaviour")
            .contains("pc = 0"));
    }

    // Verifies: REQ029 — THE REAL ENGINE leaves no litter: the generated module, cfg, and any
    // trace spec are gone afterwards, on the failing path too.
    #[test]
    #[ignore = "needs TLC installed — run via `cargo test -- --ignored` (the CI `tlc` job)"]
    fn real_tlc_run_leaves_no_trace_in_the_subject() {
        let tmp = tla_subject(false);
        let check = lower(
            &req(MODEL_REQ),
            "Msg",
            &standard_bindings(),
            "provreq_smoke",
        )
        .expect("should lower");
        let _ = run(&site_for(&tmp), &check);
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
            .expect("readdir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("provreq_smoke"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "provreq left files behind: {leftovers:?}"
        );
    }

    // Verifies: REQ029 — a subject with no `Spec` behaviour is honestly located as an error,
    // so `verify` can report a clear inconclusive rather than guessing Init/Next.
    #[test]
    fn a_subject_without_spec_is_an_honest_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Msg.tla"),
            "---- MODULE Msg ----\nVARIABLES pc\nInit == pc = 0\n====\n",
        )
        .expect("Msg.tla");
        let err = locate_spec(
            tmp.path(),
            &tmp.path().join("ProvableRequirements"),
            &crate::spec_paths::SpecPaths::default(),
        )
        .expect_err("no Spec to check against");
        assert!(err.contains("Spec"), "{err}");
    }

    // Verifies: REQ029 (#120) — a run writes NOTHING into the spec's directory, so a file already
    // named like the generated module is not at risk in the first place.
    //
    // This replaces a test that asserted the run *refused* on a name collision. That guard was
    // right while the module was generated beside the spec; a spec directory may now be a
    // configured root in someone else's repository, so provreq generates into its own scratch dir
    // instead and the collision cannot arise. Asserting the stronger property directly — the
    // directory is untouched — is what keeps that from quietly regressing.
    #[test]
    fn a_run_writes_nothing_into_the_spec_directory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let neighbour = tmp.path().join("provreq_smoke.tla");
        std::fs::write(&neighbour, "\\* the operator's own module\n").expect("write");
        let site = SpecSite {
            dir: tmp.path().to_path_buf(),
            module: "Msg".into(),
            library: vec![tmp.path().to_path_buf()],
        };
        let check = Check {
            name: "provreq_smoke".into(),
            module: "\\* generated\n".into(),
            cfg: "SPECIFICATION Spec\n".into(),
        };
        let _ = run(&site, &check);

        assert_eq!(
            std::fs::read_to_string(&neighbour).expect("read"),
            "\\* the operator's own module\n",
            "the operator's file must be untouched"
        );
        let entries: Vec<String> = std::fs::read_dir(tmp.path())
            .expect("readdir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            entries,
            vec!["provreq_smoke.tla".to_string()],
            "the run must add nothing to the spec's directory"
        );
    }
}
