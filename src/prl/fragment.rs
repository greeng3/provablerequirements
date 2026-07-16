//! D2/D10 fragment check — does the requirement's **declared category** have an engine
//! that can *express* the patterns it uses?
//!
//! D2 makes each category a delimited fragment of the one core semantics, "each exactly
//! what one engine checks", and says that using an operator outside the target engine's
//! fragment is "a typed error surfaced to the author, never a silent approximation".
//! D10 names the failure `inapplicable` — a tool outside its competence envelope, whose
//! answer is the wrong *kind* of answer — and states that this "is exactly what D2's
//! typed fragments should prevent statically ... before anything runs".
//!
//! That is what this module does. Without it an out-of-fragment requirement gates clean
//! and then earns an `unknown / no-engine` verdict, which promises an engine could someday
//! answer it. For an out-of-fragment property no engine ever can, so the promise is false.
//!
//! Note what this makes unnecessary: **no `inapplicable` verdict reason is needed**, and
//! adding one would be dead code. A rejected candidate is never admitted, so it never
//! reaches [`crate::verdict`] to carry a reason at all. D10 reserves `inapplicable` for the
//! *residue* — subtle soundness-direction mismatches (a bounded checker asked to prove a
//! ∀ claim) "caught at aggregation and excluded" — which needs real engines and a per-tool
//! evidence map. Expressibility, the part decidable statically, is a pipeline type error
//! and belongs here.
//!
//! **Every declared category must be able to express every pattern.** A requirement may
//! span categories (`category: 2a + 2b`), and each declared category is a target engine
//! that will be asked the same claim — the same "every declared category" reading
//! [`crate::engine::readiness`] already uses.
//!
//! Only the two rules the design states outright are enforced (see `out_of_fragment`);
//! the doc's own open items list "the exact boundary of each engine fragment" as
//! unsettled, so the subtler boundaries stay permissive rather than guessed.
//!
//! Implements: REQ024 (fragment check — out-of-fragment category is a typed gate error).

use super::ast::*;
use super::error::GateError;

/// Check every declared category against every pattern used, returning a typed error per
/// out-of-fragment pairing (empty = every declared category can express the claim).
/// A category-less candidate is not checked — the gate does not guess a category (D3
/// inference is a later slice), and an undeclared category is reported as unroutable
/// downstream rather than silently assumed here.
pub fn check(req: &Requirement) -> Vec<GateError> {
    let mut errors = Vec::new();
    for category in &req.category {
        for prop in &req.require {
            if let Some((why, remedy)) = out_of_fragment(*category, &prop.pattern) {
                errors.push(GateError::OutOfFragment {
                    category: *category,
                    pattern: pattern_verb(&prop.pattern),
                    why,
                    remedy,
                    line: prop.line,
                });
            }
        }
    }
    errors
}

/// The fragment rule: `Some((why, remedy))` when `category`'s engine cannot express
/// `pattern`. Pure and total, so the boundary lives in exactly one place.
///
/// Two rules, each taken directly from the design rather than inferred:
///
/// 1. **Category 1 is the temporal-free fragment** — "1 → the temporal-free fragment
///    (pre/post/invariants) → Viper/deductive" (Core layer). `always`/`never` are safety
///    and land as invariants, so they are in-fragment; everything with a future-time or
///    ordering obligation is out.
/// 2. **`can_reach` is branching (CTL `EF`)** — "the branching-time need LTL cannot
///    express" (Surface vocabulary), and only 2a routes to a branching checker
///    (NuSMV). A deductive prover, a linear trace monitor, and a UI probe all lack the
///    state graph it quantifies over.
///
/// Deliberately NOT decided here (the doc lists the exact per-engine boundary as open):
/// whether unbounded liveness is monitorable at 2b/3. A finite trace can neither confirm
/// nor refute `eventually P`, which suggests only the metric form (`leads_to … within T`)
/// is decidable there — but that is a real design question, so 2b/3 stay permissive until
/// it is settled rather than silently narrowed.
fn out_of_fragment(category: Category, pattern: &Pattern) -> Option<(&'static str, &'static str)> {
    const RESTATE: &str =
        "declare category 2a (model) or 2b (runtime), or restate the claim as an invariant";
    const NEEDS_MODEL: &str =
        "declare category 2a (model) — reachability is checked over a model's state graph";

    match (category, pattern) {
        // Rule 1 — the code fragment is temporal-free; only invariants survive.
        (Category::Code, Pattern::Always(_) | Pattern::Never(_)) => None,
        (Category::Code, Pattern::Eventually(_) | Pattern::LeadsTo { .. }) => Some((
            "it is a liveness pattern, and the code fragment is temporal-free \
             (pre/post/invariants): a deductive prover checks a state predicate, not a \
             future-time obligation",
            RESTATE,
        )),
        (Category::Code, Pattern::Precedes { .. }) => Some((
            "it orders two claims over time, and the code fragment is temporal-free \
             (pre/post/invariants): a deductive prover sees one state, not a history",
            RESTATE,
        )),
        (Category::Code, Pattern::OccursAtMost { .. }) => Some((
            "it counts occurrences over time, and the code fragment is temporal-free \
             (pre/post/invariants): a deductive prover has no trace to count over",
            RESTATE,
        )),

        // Rule 2 — reachability is branching-time; only a model checker has the state graph.
        (Category::Model, Pattern::CanReach(_)) => None,
        (_, Pattern::CanReach(_)) => Some((
            "it is a branching-time (CTL `EF`) claim needing a model's state graph, which a \
             deductive prover, a linear trace monitor, and a UI probe all lack",
            NEEDS_MODEL,
        )),

        // 2a is finite model checking (TLA+/NuSMV) — the whole working set lowers to it.
        // 2b/3 stay permissive: see the open question in this module's docs.
        _ => None,
    }
}

/// The surface verb for a pattern, as the author wrote it — so an error quotes the token
/// the author must actually change.
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

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn errors_of(src: &str) -> Vec<GateError> {
        check(&parse(src).expect("should parse"))
    }

    // Verifies: REQ024 — the exact gap found while smoke-testing #36: a liveness claim
    // declared at category 1 gated clean and earned a misleading `no-engine` verdict.
    #[test]
    fn liveness_at_category_one_is_out_of_fragment() {
        let errs = errors_of(
            "requirement r { category: 1
                vocabulary { state logged_in state has_session }
                require { logged_in leads_to has_session } }",
        );
        assert!(errs.iter().any(|e| matches!(
            e,
            GateError::OutOfFragment {
                pattern: "leads_to",
                category: Category::Code,
                ..
            }
        )));
    }

    // Verifies: REQ024 — `always`/`never` are safety and land as invariants, so they ARE
    // expressible at category 1. The rule must not reject the code fragment's own claims.
    #[test]
    fn invariants_are_in_fragment_at_category_one() {
        let errs = errors_of(
            "requirement r { category: 1
                vocabulary { state balance_non_negative }
                require { always balance_non_negative } }",
        );
        assert!(
            errs.is_empty(),
            "an invariant is the code fragment: {errs:?}"
        );

        let errs = errors_of(
            "requirement r { category: 1
                vocabulary { state overdrawn }
                require { never overdrawn } }",
        );
        assert!(errs.is_empty(), "`never P` is `always not P`: {errs:?}");
    }

    // Verifies: REQ024 — every non-invariant pattern is rejected at category 1, so the
    // temporal-free rule is enforced comprehensively rather than for `leads_to` alone.
    #[test]
    fn every_temporal_pattern_is_out_of_fragment_at_category_one() {
        for (src, verb) in [
            ("require { eventually p }", "eventually"),
            ("require { p leads_to q }", "leads_to"),
            ("require { p precedes q }", "precedes"),
            ("require { p occurs at most 3 times }", "occurs at most"),
            ("require { can_reach p }", "can_reach"),
        ] {
            let full =
                format!("requirement r {{ category: 1 vocabulary {{ state p state q }} {src} }}");
            let errs = errors_of(&full);
            assert!(
                errs.iter().any(|e| matches!(
                    e,
                    GateError::OutOfFragment { pattern, .. } if *pattern == verb
                )),
                "`{verb}` must be out of fragment at category 1, got {errs:?}"
            );
        }
    }

    // Verifies: REQ024 — reachability needs a model's state graph, so it is expressible
    // at 2a and nowhere else.
    #[test]
    fn can_reach_is_only_expressible_at_the_model_category() {
        let clean = errors_of(
            "requirement r { category: 2a
                vocabulary { state recovered }
                require { can_reach recovered } }",
        );
        assert!(clean.is_empty(), "2a is the branching fragment: {clean:?}");

        for cat in ["2b", "3"] {
            let errs = errors_of(&format!(
                "requirement r {{ category: {cat}
                    vocabulary {{ state recovered }}
                    require {{ can_reach recovered }} }}"
            ));
            assert!(
                errs.iter().any(|e| matches!(
                    e,
                    GateError::OutOfFragment {
                        pattern: "can_reach",
                        ..
                    }
                )),
                "`can_reach` must be out of fragment at {cat}, got {errs:?}"
            );
        }
    }

    // Verifies: REQ024 — a multi-category requirement is checked against EVERY declared
    // category, so a claim expressible at one but not another is still a typed error.
    #[test]
    fn every_declared_category_must_express_the_claim() {
        let errs = errors_of(
            "requirement r { category: 2a + 2b
                vocabulary { state recovered }
                require { can_reach recovered } }",
        );
        assert_eq!(
            errs.len(),
            1,
            "expressible at 2a, not at 2b — exactly one error: {errs:?}"
        );
        assert!(matches!(
            errs[0],
            GateError::OutOfFragment {
                category: Category::Runtime,
                ..
            }
        ));
    }

    // Verifies: REQ024 — the gate does not guess a category (D3 inference is a later
    // slice), so a category-less candidate is not fragment-checked.
    #[test]
    fn category_less_candidate_is_not_fragment_checked() {
        let errs =
            errors_of("requirement r { vocabulary { state p state q } require { p leads_to q } }");
        assert!(
            errs.is_empty(),
            "no declared category to check against: {errs:?}"
        );
    }

    // Verifies: REQ024 — the error names the token the author must change and stays
    // actionable for the D11 generate-then-repair loop.
    #[test]
    fn out_of_fragment_error_is_actionable() {
        let errs = errors_of(
            "requirement r { category: 1
                vocabulary { state p state q }
                require { p leads_to q } }",
        );
        let text = errs[0].to_string();
        assert!(text.contains("`leads_to`"), "names the token: {text}");
        assert!(text.contains("category 1"), "names the category: {text}");
        assert!(text.contains("temporal-free"), "says why: {text}");
        assert!(text.contains("2a"), "offers a remedy: {text}");
    }
}
