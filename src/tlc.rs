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
//! Nothing in the subject's spec is edited; the generated files are removed after the run and
//! an existing file is never clobbered.
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
use crate::verdict::{self, Basis, Provenance, Verdict};
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
    /// Map what TLC established into a core verdict. The mapping lives here, in the engine, so
    /// [`crate::verdict`] never learns what TLC is (D2's "one meaning, lowering to each
    /// engine", running in this direction too).
    ///
    /// The load-bearing line is `Holds` → [`Basis::ModelCheckedBounded`]: TLC is bounded, so a
    /// pass is `model-checked (bounded)` and never `proven`.
    pub fn into_verdict(&self, id: &str, provenance: Provenance) -> Verdict {
        match self {
            Outcome::Holds => verdict::holds(id, Basis::ModelCheckedBounded, provenance),
            Outcome::Fails { violated, witness } => verdict::fails(
                id,
                witness.clone(),
                violated.iter().cloned().collect(),
                provenance,
            ),
            Outcome::Inconclusive { reason } => {
                verdict::inconclusive(id, vec![reason.clone()], provenance)
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
    /// The directory holding the spec module — TLC resolves `EXTENDS` from the main module's
    /// own directory, so the generated module is written here.
    pub dir: PathBuf,
    /// The spec's TLA+ module name (from its `---- MODULE X ----` header), which the generated
    /// module extends.
    pub module: String,
}

/// The tla2tools.jar path — `TLA2TOOLS_JAR` if set, else the image's install location. TLC is
/// invoked as `java -cp <jar> tlc2.TLC`, so there is no PATH binary to probe.
///
/// `// ponytail: env var + baked-in default is enough until a real subject needs a per-project
/// jar; move to provreq.yml config then.`
pub fn jar_path() -> String {
    std::env::var("TLA2TOOLS_JAR").unwrap_or_else(|_| "/opt/tlaplus/tla2tools.jar".to_string())
}

/// Locate the subject's behaviour spec (the module defining `Spec`) so a check can be
/// generated beside it. `Err` when there is no single `Spec` to check against — an honest
/// `inconclusive`, never a guess at `Init`/`Next`.
pub fn locate_spec(subject_root: &Path, companion_root: &Path) -> Result<SpecSite, String> {
    let at = match tla_adapter::resolve(subject_root, companion_root, SPEC_OPERATOR) {
        ModelResolution::Resolved(at) => at,
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
    Ok(SpecSite { dir, module })
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

/// Write the generated module + cfg beside the subject's spec, run TLC, and remove them again.
///
/// Additive and non-destructive, the Kani discipline: nothing in the subject's spec is
/// touched, an existing file is never clobbered, and the generated files are removed on every
/// path including failure. TLC's own scratch (`states/`) is redirected to a throwaway metadir
/// outside the subject, so the run leaves no trace.
///
/// `// ponytail: TLC's default worker/heap settings and no timeout — its own defaults until a
/// real subject shows they are wrong; workers/timeout belong in provreq.yml config.`
pub fn run(site: &SpecSite, check: &Check) -> Outcome {
    let tla_path = site.dir.join(format!("{}.tla", check.name));
    let cfg_path = site.dir.join(format!("{}.cfg", check.name));
    for path in [&tla_path, &cfg_path] {
        if path.exists() {
            return Outcome::Inconclusive {
                reason: format!(
                    "{} already exists — refusing to overwrite a file provreq did not write",
                    path.display()
                ),
            };
        }
    }
    let metadir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            return Outcome::Inconclusive {
                reason: format!("could not create a scratch metadir for TLC: {e}"),
            }
        }
    };
    if let Err(e) = std::fs::write(&tla_path, &check.module) {
        return Outcome::Inconclusive {
            reason: format!(
                "could not write the generated module to {}: {e}",
                tla_path.display()
            ),
        };
    }
    if let Err(e) = std::fs::write(&cfg_path, &check.cfg) {
        let _ = std::fs::remove_file(&tla_path);
        return Outcome::Inconclusive {
            reason: format!("could not write the config to {}: {e}", cfg_path.display()),
        };
    }

    let output = std::process::Command::new("java")
        .args(["-cp", &jar_path(), "tlc2.TLC"])
        // Scratch (`states/`) goes outside the subject, so the run leaves no litter.
        .arg("-metadir")
        .arg(metadir.path())
        .args(["-config", &format!("{}.cfg", check.name)])
        .arg(format!("{}.tla", check.name))
        .current_dir(&site.dir)
        .output();

    // Remove the generated files before interpreting anything, so an early return cannot leak
    // them; a violation trace spec (`<name>_TTrace_*.tla`) is swept too.
    let _ = std::fs::remove_file(&tla_path);
    let _ = std::fs::remove_file(&cfg_path);
    remove_trace_specs(&site.dir, &check.name);

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

/// Sweep any `<name>_TTrace_*.tla` trace-spec TLC may generate on a violation, so the subject
/// is left exactly as provreq found it.
fn remove_trace_specs(dir: &Path, name: &str) {
    let prefix = format!("{name}_TTrace");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with(&prefix))
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

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

/// Why TLC could not decide, in the operator's terms — the first `Error:`/`***` line (a parse
/// error or an unassigned constant states its cause on that line), else the tail of the log.
fn diagnostic(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("Error:") || l.starts_with("***"))
        .map(str::to_string)
        .unwrap_or_else(|| tail(output))
}

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

    const MODEL_REQ: &str = "requirement r {
        category: 2a
        vocabulary { state accepted(m), succeeded(m) }
        require { each m: Message . accepted(m) leads_to succeeded(m) }
    }";

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
        let v = Outcome::Holds.into_verdict("SR001", prov());
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
        let v = outcome.into_verdict("SR002", prov());
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
        let v = outcome.into_verdict("SR003", prov());
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
        let fairness = if fair { " /\\ WF_pc(Next)" } else { "" };
        std::fs::write(
            tmp.path().join("Msg.tla"),
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
            ),
        )
        .expect("Msg.tla");
        tmp
    }

    fn site_for(tmp: &tempfile::TempDir) -> SpecSite {
        locate_spec(tmp.path(), &tmp.path().join("ProvableRequirements")).expect("Spec located")
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
        let err = locate_spec(tmp.path(), &tmp.path().join("ProvableRequirements"))
            .expect_err("no Spec to check against");
        assert!(err.contains("Spec"), "{err}");
    }

    // Verifies: REQ029 — an existing file is NEVER clobbered; a name collision stops the run.
    #[test]
    fn an_existing_file_is_never_overwritten() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let victim = tmp.path().join("provreq_smoke.tla");
        std::fs::write(&victim, "\\* the operator's own module\n").expect("write");
        let site = SpecSite {
            dir: tmp.path().to_path_buf(),
            module: "Msg".into(),
        };
        let check = Check {
            name: "provreq_smoke".into(),
            module: "\\* generated\n".into(),
            cfg: "SPECIFICATION Spec\n".into(),
        };
        let Outcome::Inconclusive { reason } = run(&site, &check) else {
            panic!("a collision must not be treated as a verdict");
        };
        assert!(reason.contains("refusing to overwrite"), "{reason}");
        assert_eq!(
            std::fs::read_to_string(&victim).expect("read"),
            "\\* the operator's own module\n",
            "the operator's file must be untouched"
        );
    }
}
