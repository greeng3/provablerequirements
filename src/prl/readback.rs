//! D12 read-back: a **deterministic** renderer from the PRL AST to readable CNL. This
//! is a pure pretty-printer — **never an LLM call**. That independence is the whole
//! point (D12): the forward translation is the untrusted LLM, so if the same model
//! also rendered the read-back it would faithfully restate its own misreading and the
//! human would rubber-stamp a spec gap. A trusted renderer surfaces the *actual formal
//! meaning* of what the gate accepted, so the human confirms intent against the real
//! artifact.
//!
//! Faithfulness over prose polish: boolean operands are parenthesized whenever
//! compound (never changing meaning), predicate applications and raw leaves
//! (durations, `with` guards, assume/strength/evidence) are shown as-is.
//!
//! Implements: REQ018 (D12 deterministic AST→CNL read-back renderer), REQ066 (a variable used as
//! a condition reads as one, so it cannot be confirmed as a predicate of the same name).

use super::ast::*;

/// Render a checked requirement to a canonical CNL read-back for human confirmation.
pub fn render(req: &Requirement) -> String {
    let mut lines = Vec::new();

    let category = if req.category.is_empty() {
        "unspecified".to_string()
    } else {
        req.category
            .iter()
            .map(category_word)
            .collect::<Vec<_>>()
            .join(" + ")
    };
    lines.push(format!(
        "Requirement `{}` — category: {category}.",
        req.name
    ));

    if !req.assume.is_empty() {
        lines.push(format!("Assuming {}.", req.assume.join("; ")));
    }

    lines.push("It requires that:".to_string());
    for prop in &req.require {
        lines.push(format!("  • {}", render_property(req, prop)));
    }

    if let Some(strength) = &req.strength {
        lines.push(format!("Expected verdict: {strength}."));
    }
    if let Some(evidence) = &req.evidence {
        lines.push(format!("Checked by: {evidence}."));
    }

    lines.join("\n")
}

fn category_word(c: &Category) -> String {
    match c {
        Category::Code => "code (1)",
        Category::Model => "model (2a)",
        Category::Runtime => "runtime monitor (2b)",
        Category::Ui => "UI (3)",
    }
    .to_string()
}

/// One claim in the operator's words, including **every** variable it ranges over — the `each`
/// binder they wrote and the free variables closed over implicitly (REQ059) alike. A read-back that
/// showed only the written binder would understate what is checked, and D12's whole job is that the
/// operator sees what the tool will actually do.
fn render_property(req: &Requirement, p: &Property) -> String {
    let binders = req.binders(p);
    let claim = format!(
        "{}{}",
        render_pattern(&p.pattern, &binders),
        render_scope(&p.scope, &binders)
    );
    if binders.is_empty() {
        return claim;
    }
    let named = binders
        .iter()
        .map(|b| match &b.sort {
            Some(sort) => format!("{} of type {sort}", b.var),
            None => format!("{} (no declared sort)", b.var),
        })
        .collect::<Vec<_>>()
        .join(" and each ");
    let closed = binders.iter().any(|b| !b.explicit);
    format!(
        "for each {named}, {claim}{}",
        if closed {
            " — every variable the claim mentions is quantified, over the sort the vocabulary \
             declares for it"
        } else {
            ""
        }
    )
}

fn render_pattern(p: &Pattern, vars: &[Binder]) -> String {
    match p {
        // Pattern operands use `parenthesized` so a compound operand is bracketed and
        // never runs ambiguously into the surrounding "… always holds" phrasing.
        Pattern::Never(e) => format!("{} never holds", parenthesized(e, vars)),
        Pattern::Always(e) => format!("{} always holds", parenthesized(e, vars)),
        Pattern::Eventually(e) => format!("eventually {} holds", parenthesized(e, vars)),
        Pattern::LeadsTo { from, to, within } => {
            let base = format!(
                "once {} holds, {} eventually holds",
                parenthesized(from, vars),
                parenthesized(to, vars)
            );
            match within {
                Some(t) => format!("{base} within {t}"),
                None => base,
            }
        }
        Pattern::Precedes { first, then } => format!(
            "every {} is preceded by {}",
            parenthesized(then, vars),
            parenthesized(first, vars)
        ),
        Pattern::OccursAtMost { event, k } => format!(
            "{} occurs at most {k} time{}",
            parenthesized(event, vars),
            if *k == 1 { "" } else { "s" }
        ),
        Pattern::CanReach(e) => {
            format!(
                "a state where {} holds is reachable",
                parenthesized(e, vars)
            )
        }
    }
}

fn render_scope(s: &Scope, vars: &[Binder]) -> String {
    match s {
        Scope::Globally => String::new(),
        Scope::Before(a) => format!(", before {}", render_atom(a, vars)),
        Scope::After(a) => format!(", after {}", render_atom(a, vars)),
        Scope::Between(a, b) => format!(
            ", between {} and {}",
            render_atom(a, vars),
            render_atom(b, vars)
        ),
    }
}

fn render_expr(e: &Expr, vars: &[Binder]) -> String {
    match e {
        Expr::Atom(a) => render_atom(a, vars),
        Expr::Not(inner) => format!("not {}", parenthesized(inner, vars)),
        Expr::And(l, r) => format!("{} and {}", parenthesized(l, vars), parenthesized(r, vars)),
        Expr::Or(l, r) => format!("{} or {}", parenthesized(l, vars), parenthesized(r, vars)),
    }
}

/// Render an operand, wrapping it in parentheses when it is compound. Atoms need no
/// parens; anything with a connective does, so the read-back is never ambiguous about
/// grouping.
fn parenthesized(e: &Expr, vars: &[Binder]) -> String {
    match e {
        Expr::Atom(a) => render_atom(a, vars),
        _ => format!("({})", render_expr(e, vars)),
    }
}

/// One atom. A bare name that is one of the claim's own variables is rendered as the condition it
/// is — `p is true`, not `p` — because `p` alone reads as a predicate the vocabulary declares, and
/// D12 is only worth having if the operator can tell those two apart (REQ066).
fn render_atom(a: &Atom, vars: &[Binder]) -> String {
    let base = if a.args.is_empty() {
        if vars.iter().any(|b| b.var == a.name) {
            format!("{} is true", a.name)
        } else {
            a.name.clone()
        }
    } else {
        format!("{}({})", a.name, a.args.join(", "))
    };
    match &a.guard {
        Some(g) => format!("{base} where {g}"),
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn readback(src: &str) -> String {
        render(&parse(src).expect("should parse"))
    }

    #[test]
    fn renders_header_with_category_words() {
        let out = readback("requirement r { category: 2a + 2b\n require { always ok } }");
        assert!(out.contains("Requirement `r`"));
        assert!(out.contains("category: model (2a) + runtime monitor (2b)"));
    }

    #[test]
    fn renders_the_worked_leads_to_example_faithfully() {
        let src = "requirement no_message_lost {
            vocabulary {
                event accepted(m: Message)
                state succeeded(m), dead_lettered(m: Message, reason: String)
            }
            assume { retries_bounded(N = 5) }
            require {
                each m: Message .
                    accepted(m) leads_to (succeeded(m) or dead_lettered(m, r) with r != \"\") within 30s
            }
        }";
        let out = readback(src);
        assert!(out.contains("Assuming retries_bounded(N = 5)."));
        assert!(out.contains("for each m of type Message,"));
        assert!(out.contains(
            "once accepted(m) holds, (succeeded(m) or dead_lettered(m, r) where r != \"\") eventually holds within 30s"
        ));
    }

    // Verifies: REQ059 — the read-back states the closure. A claim with no `each` still ranges
    // over its free variables, and D12 is only faithful if the operator sees the quantification
    // the harness will actually be built with, not just the binder they typed.
    #[test]
    fn readback_states_an_implicit_closure() {
        let out = readback(
            "requirement r { category: 1
             vocabulary { state proceeds(d: Decision, f: Flag) }
             require { always proceeds(d, f) } }",
        );
        assert!(
            out.contains("for each d of type Decision and each f of type Flag,"),
            "{out}"
        );
        assert!(
            out.contains("every variable the claim mentions is quantified"),
            "{out}"
        );
    }

    // Verifies: REQ059 — a variable the requirement never types is said to be untyped rather than
    // quietly dropped from the read-back, since it is the reason the claim will not lower.
    #[test]
    fn readback_names_a_variable_with_no_declared_sort() {
        let out = readback(
            "requirement r { category: 1
             vocabulary { state p(u) }
             require { always p(u) } }",
        );
        assert!(out.contains("for each u (no declared sort),"), "{out}");
    }

    #[test]
    fn renders_never_always_eventually() {
        assert!(readback("requirement r { require { never boom } }").contains("boom never holds"));
        assert!(readback("requirement r { require { always ok } }").contains("ok always holds"));
        assert!(readback("requirement r { require { eventually done } }")
            .contains("eventually done holds"));
    }

    #[test]
    fn renders_precedes_in_the_faithful_direction() {
        // `S precedes P` means every P is preceded by an S.
        let out = readback("requirement r { require { grant precedes use } }");
        assert!(out.contains("every use is preceded by grant"), "got: {out}");
    }

    #[test]
    fn renders_scopes() {
        let out = readback("requirement r { require { always p between open and close } }");
        assert!(
            out.contains("p always holds, between open and close"),
            "got: {out}"
        );
    }

    #[test]
    fn renders_occurs_at_most_with_pluralization() {
        assert!(
            readback("requirement r { require { retry occurs at most 1 times } }")
                .contains("retry occurs at most 1 time")
        );
        assert!(
            readback("requirement r { require { retry occurs at most 5 times } }")
                .contains("retry occurs at most 5 times")
        );
    }

    #[test]
    fn renders_can_reach() {
        assert!(readback("requirement r { require { can_reach shutdown } }")
            .contains("a state where shutdown holds is reachable"));
    }

    #[test]
    fn parenthesizes_compound_boolean_operands() {
        let out = readback("requirement r { require { always (a or (b and c)) } }");
        // The nested `and` is parenthesized; the outer `or` operands too.
        assert!(out.contains("(a or (b and c)) always holds"), "got: {out}");
    }

    // Verifies: REQ066 — a variable used as a condition reads as one. Rendered as a bare `p` it
    // would be indistinguishable from a predicate the vocabulary declares, and D12 exists so the
    // operator can tell what the tool will actually check.
    #[test]
    fn a_variable_used_as_a_condition_reads_as_a_condition() {
        let out = readback(
            "requirement r { category: 1
                vocabulary { state proceeds(d: Decision, p: Flag) }
                require { always (not proceeds(d, p) or p) } }",
        );
        assert!(out.contains("p is true"), "got: {out}");
        // The predicate application is untouched — only the bare variable changes.
        assert!(out.contains("proceeds(d, p)"), "got: {out}");
    }

    // Verifies: REQ066 — a bare name the claim does not bind is still rendered as itself, so this
    // reading cannot silently restate an ordinary nullary predicate as a condition.
    #[test]
    fn a_nullary_predicate_still_reads_as_a_predicate() {
        let out = readback(
            "requirement r { category: 1
                vocabulary { state supported }
                require { always supported } }",
        );
        assert!(out.contains("supported always holds"), "got: {out}");
        assert!(!out.contains("is true"), "got: {out}");
    }

    #[test]
    fn renders_strength_and_evidence_footers() {
        let src = "requirement r {
            require { always ok }
            strength: model_checked over Model
            evidence: tla+ (bounded: |M| <= 8)
        }";
        let out = readback(src);
        assert!(out.contains("Expected verdict: model_checked over Model."));
        assert!(out.contains("Checked by: tla+ (bounded: |M| <= 8)."));
    }
}
