//! Recursive-descent parser for the PRL block grammar. Produces a [`Requirement`] AST
//! or a list of [`GateError::Parse`] failures with source lines. The temporal-property
//! grammar (patterns, scopes, quantification, boolean atoms) is the concrete surface
//! from `docs/requirement-language.md`'s working set.
//!
//! Leaves left raw on purpose (arguments, `within` durations, `with` guards, `assume`
//! entries, `strength`/`evidence` bodies) are recovered by slicing source between the
//! bounding token offsets — cheap, and it keeps a relational/term grammar out of scope.
//! `// ponytail:` — those become structured when D13 grounding needs them.
//!
//! Implements: REQ016 (mechanical gate part 1 — parse + type/name-check).

use super::ast::*;
use super::error::GateError;
use super::lexer::{lex, Tok, Token};

/// Parse a candidate PRL block. Returns the AST, or every parse error found.
pub fn parse(src: &str) -> Result<Requirement, Vec<GateError>> {
    let mut p = Parser {
        src,
        toks: lex(src),
        pos: 0,
        errors: Vec::new(),
    };
    let req = p.requirement();
    match req {
        Some(r) if p.errors.is_empty() => Ok(r),
        _ => {
            if p.errors.is_empty() {
                p.errors.push(GateError::Parse {
                    message: "empty or unrecognized input".into(),
                    line: 1,
                });
            }
            Err(p.errors)
        }
    }
}

/// Keywords that terminate a raw `with`/`within` capture at brace/paren depth 0.
fn is_stop_kw(w: &str) -> bool {
    matches!(
        w,
        "and"
            | "or"
            | "leads_to"
            | "precedes"
            | "occurs"
            | "within"
            | "with"
            | "globally"
            | "before"
            | "after"
            | "between"
            | "never"
            | "always"
            | "eventually"
            | "can_reach"
            | "each"
    )
}

/// The section keywords of a `requirement { … }` block — boundaries a structured
/// section value (`category`) must not read past.
fn is_section_kw(w: &str) -> bool {
    matches!(
        w,
        "category" | "vocabulary" | "assume" | "require" | "strength" | "evidence"
    )
}

struct Parser<'a> {
    src: &'a str,
    toks: Vec<Token>,
    pos: usize,
    errors: Vec<GateError>,
}

impl<'a> Parser<'a> {
    // --- token cursor -------------------------------------------------------

    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.pos)
    }

    fn peek_ident(&self) -> Option<&str> {
        match self.peek().map(|t| &t.kind) {
            Some(Tok::Ident(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    fn peek_punct(&self) -> Option<char> {
        match self.peek().map(|t| &t.kind) {
            Some(Tok::Punct(c)) => Some(*c),
            _ => None,
        }
    }

    fn line(&self) -> usize {
        self.peek()
            .or_else(|| self.toks.last())
            .map(|t| t.line)
            .unwrap_or(1)
    }

    fn bump(&mut self) -> Option<Token> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eat_ident(&mut self, w: &str) -> bool {
        if self.peek_ident() == Some(w) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_punct(&mut self, c: char) -> bool {
        if self.peek_punct() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn error(&mut self, message: impl Into<String>, line: usize) {
        self.errors.push(GateError::Parse {
            message: message.into(),
            line,
        });
    }

    fn expect_punct(&mut self, c: char) -> bool {
        if self.eat_punct(c) {
            true
        } else {
            let line = self.line();
            self.error(format!("expected `{c}`"), line);
            false
        }
    }

    fn expect_ident(&mut self) -> Option<(String, usize)> {
        let line = self.line();
        match self.peek().map(|t| t.kind.clone()) {
            Some(Tok::Ident(s)) => {
                self.pos += 1;
                Some((s, line))
            }
            _ => {
                self.error("expected a name", line);
                None
            }
        }
    }

    // --- raw-source recovery ------------------------------------------------

    /// Raw text from `from` byte offset to the end of that source line, advancing the
    /// cursor past every token it covers. Used for the free-text `strength`/`evidence`.
    fn raw_line(&mut self, from: usize) -> String {
        let rest = &self.src[from..];
        let stop = from + rest.find('\n').unwrap_or(rest.len());
        while self.pos < self.toks.len() && self.toks[self.pos].start < stop {
            self.pos += 1;
        }
        self.src[from..stop].trim().to_string()
    }

    /// Raw `category:` value — like [`Self::raw_line`], but also stops at the next
    /// section keyword or a closing `}`. Category is a structured token list, so on a
    /// single-line block it must not swallow the sections that follow it.
    fn raw_category(&mut self, from: usize) -> String {
        let rest = &self.src[from..];
        let mut stop = from + rest.find('\n').unwrap_or(rest.len());
        for t in &self.toks[self.pos..] {
            if t.start >= stop {
                break;
            }
            let boundary = matches!(&t.kind, Tok::Punct('}'))
                || matches!(&t.kind, Tok::Ident(w) if is_section_kw(w));
            if boundary {
                stop = t.start;
                break;
            }
        }
        while self.pos < self.toks.len() && self.toks[self.pos].start < stop {
            self.pos += 1;
        }
        self.src[from..stop].trim().to_string()
    }

    /// Raw text until a depth-0 stop keyword / delimiter (`with` guard, `within`).
    fn raw_until_stop(&mut self) -> String {
        let Some(start) = self.peek().map(|t| t.start) else {
            return String::new();
        };
        let mut end = start;
        let mut depth = 0i32;
        while let Some(t) = self.peek() {
            match &t.kind {
                Tok::Punct('(') => depth += 1,
                Tok::Punct(')') if depth > 0 => depth -= 1,
                Tok::Punct(')') | Tok::Punct('}') | Tok::Punct(',') if depth == 0 => break,
                Tok::Ident(w) if depth == 0 && is_stop_kw(w) => break,
                _ => {}
            }
            end = t.end;
            self.pos += 1;
        }
        self.src[start..end].trim().to_string()
    }

    /// Raw inner text of a `{ … }` block (cursor must be at the `{`), consuming the
    /// matching close brace. Nested braces are balanced.
    fn raw_braced(&mut self) -> String {
        if !self.eat_punct('{') {
            return String::new();
        }
        let start = self.peek().map(|t| t.start).unwrap_or(0);
        let mut end = start;
        let mut depth = 1i32;
        while let Some(t) = self.peek() {
            match &t.kind {
                Tok::Punct('{') => depth += 1,
                Tok::Punct('}') => {
                    depth -= 1;
                    if depth == 0 {
                        self.pos += 1;
                        break;
                    }
                }
                _ => {}
            }
            end = t.end;
            self.pos += 1;
        }
        self.src[start..end].trim().to_string()
    }

    // --- grammar ------------------------------------------------------------

    fn requirement(&mut self) -> Option<Requirement> {
        if !self.eat_ident("requirement") {
            self.errors.push(GateError::MissingSection {
                section: "requirement <name> { … }",
            });
            return None;
        }
        let (name, _) = self.expect_ident()?;
        self.expect_punct('{');

        let mut req = Requirement {
            name,
            category: Vec::new(),
            vocabulary: Vec::new(),
            assume: Vec::new(),
            require: Vec::new(),
            strength: None,
            evidence: None,
        };
        let mut saw_require = false;

        loop {
            if self.peek().is_none() || self.eat_punct('}') {
                break;
            }
            let before = self.pos;
            match self.peek_ident() {
                Some("category") => {
                    self.pos += 1;
                    let colon_end = self.colon_end();
                    let line = self.line();
                    let raw = self.raw_category(colon_end);
                    req.category = self.parse_categories(&raw, line);
                }
                Some("vocabulary") => {
                    self.pos += 1;
                    req.vocabulary = self.vocabulary();
                }
                Some("assume") => {
                    self.pos += 1;
                    req.assume = split_top_commas(&self.raw_braced());
                }
                Some("require") => {
                    self.pos += 1;
                    saw_require = true;
                    req.require = self.require_block();
                }
                Some("strength") => {
                    self.pos += 1;
                    let colon_end = self.colon_end();
                    req.strength = Some(self.raw_line(colon_end));
                }
                Some("evidence") => {
                    self.pos += 1;
                    let colon_end = self.colon_end();
                    req.evidence = Some(self.raw_line(colon_end));
                }
                _ => {
                    let line = self.line();
                    self.error("expected a section (category/vocabulary/assume/require/strength/evidence) or `}`", line);
                    self.bump();
                }
            }
            // Guard against a stuck cursor on malformed input.
            if self.pos == before {
                self.bump();
            }
        }

        if !saw_require {
            self.errors
                .push(GateError::MissingSection { section: "require" });
        }
        Some(req)
    }

    /// Consume an expected `:` and return the byte offset just past it (for raw-line
    /// section values). Tolerant: a missing colon is reported but does not abort.
    fn colon_end(&mut self) -> usize {
        let end = self.peek().map(|t| t.end).unwrap_or_else(|| self.src.len());
        if !self.eat_punct(':') {
            let line = self.line();
            self.error("expected `:`", line);
        }
        end
    }

    fn parse_categories(&mut self, raw: &str, line: usize) -> Vec<Category> {
        let mut out = Vec::new();
        for part in raw.split('+') {
            let c = part.trim();
            if c.is_empty() {
                continue;
            }
            match c {
                "1" => out.push(Category::Code),
                "2a" => out.push(Category::Model),
                "2b" => out.push(Category::Runtime),
                "3" => out.push(Category::Ui),
                other => self.errors.push(GateError::BadCategory {
                    value: other.to_string(),
                    line,
                }),
            }
        }
        out
    }

    fn vocabulary(&mut self) -> Vec<Decl> {
        let mut decls = Vec::new();
        if !self.expect_punct('{') {
            return decls;
        }
        loop {
            if self.peek().is_none() || self.eat_punct('}') {
                break;
            }
            let before = self.pos;
            let line = self.line();
            match self.peek_ident() {
                Some("sort") => {
                    self.pos += 1;
                    if let Some((name, _)) = self.expect_ident() {
                        decls.push(Decl::Sort { name, line });
                    }
                }
                Some("event") | Some("state") => {
                    let is_event = self.peek_ident() == Some("event");
                    self.pos += 1;
                    // One keyword may introduce a comma-separated list of predicates.
                    loop {
                        let dline = self.line();
                        let Some((name, _)) = self.expect_ident() else {
                            break;
                        };
                        let params = if self.peek_punct() == Some('(') {
                            self.params()
                        } else {
                            Vec::new()
                        };
                        decls.push(if is_event {
                            Decl::Event {
                                name,
                                params,
                                line: dline,
                            }
                        } else {
                            Decl::State {
                                name,
                                params,
                                line: dline,
                            }
                        });
                        if !self.eat_punct(',') {
                            break;
                        }
                    }
                }
                Some("identity") => {
                    self.pos += 1;
                    let from = self.peek().map(|t| t.start).unwrap_or(0);
                    let raw = self.raw_line(from);
                    decls.push(Decl::Identity { raw, line });
                }
                _ => {
                    self.error("expected `sort`, `event`, `state`, or `identity`", line);
                    self.bump();
                }
            }
            if self.pos == before {
                self.bump();
            }
        }
        decls
    }

    /// Parse a `( name (: Type)? , … )` parameter list (cursor at `(`).
    fn params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        if !self.expect_punct('(') {
            return params;
        }
        loop {
            if self.peek().is_none() || self.eat_punct(')') {
                break;
            }
            let Some((name, _)) = self.expect_ident() else {
                self.bump();
                continue;
            };
            let ty = if self.eat_punct(':') {
                self.expect_ident().map(|(t, _)| t).unwrap_or_default()
            } else {
                String::new()
            };
            params.push(Param { name, ty });
            if !self.eat_punct(',') {
                self.expect_punct(')');
                break;
            }
        }
        params
    }

    fn require_block(&mut self) -> Vec<Property> {
        let mut props = Vec::new();
        if !self.expect_punct('{') {
            return props;
        }
        loop {
            if self.peek().is_none() || self.eat_punct('}') {
                break;
            }
            let before = self.pos;
            if let Some(prop) = self.property() {
                props.push(prop);
            }
            if self.pos == before {
                let line = self.line();
                self.error("could not parse a property here", line);
                self.bump();
            }
        }
        props
    }

    fn property(&mut self) -> Option<Property> {
        let line = self.line();
        let quantifier = if self.eat_ident("each") {
            let (var, _) = self.expect_ident()?;
            self.expect_punct(':');
            let (sort, _) = self.expect_ident()?;
            // Accept the middot `·` or an ASCII `.` separator.
            if !self.eat_punct('·') {
                self.expect_punct('.');
            }
            Some(Quantifier { var, sort })
        } else {
            None
        };
        let pattern = self.pattern()?;
        let scope = self.scope();
        Some(Property {
            quantifier,
            pattern,
            scope,
            line,
        })
    }

    fn pattern(&mut self) -> Option<Pattern> {
        match self.peek_ident() {
            Some("never") => {
                self.pos += 1;
                Some(Pattern::Never(self.expr()?))
            }
            Some("always") => {
                self.pos += 1;
                Some(Pattern::Always(self.expr()?))
            }
            Some("eventually") => {
                self.pos += 1;
                Some(Pattern::Eventually(self.expr()?))
            }
            Some("can_reach") => {
                self.pos += 1;
                Some(Pattern::CanReach(self.expr()?))
            }
            _ => {
                let lhs = self.expr()?;
                let line = self.line();
                match self.peek_ident() {
                    Some("leads_to") => {
                        self.pos += 1;
                        let to = self.expr()?;
                        let within = if self.eat_ident("within") {
                            Some(self.raw_until_stop())
                        } else {
                            None
                        };
                        Some(Pattern::LeadsTo {
                            from: lhs,
                            to,
                            within,
                        })
                    }
                    Some("precedes") => {
                        self.pos += 1;
                        let then = self.expr()?;
                        Some(Pattern::Precedes { first: lhs, then })
                    }
                    Some("occurs") => {
                        self.pos += 1;
                        self.eat_ident("at");
                        self.eat_ident("most");
                        let k = self.expect_int()?;
                        self.eat_ident("times");
                        Some(Pattern::OccursAtMost { event: lhs, k })
                    }
                    _ => {
                        self.error(
                            "expected a pattern verb (`leads_to`, `precedes`, `occurs at most`)",
                            line,
                        );
                        None
                    }
                }
            }
        }
    }

    fn expect_int(&mut self) -> Option<u32> {
        let line = self.line();
        match self.peek().map(|t| t.kind.clone()) {
            Some(Tok::Int(n)) if n >= 0 => {
                self.pos += 1;
                Some(n as u32)
            }
            _ => {
                self.error("expected a non-negative integer", line);
                None
            }
        }
    }

    fn scope(&mut self) -> Scope {
        match self.peek_ident() {
            Some("globally") => {
                self.pos += 1;
                Scope::Globally
            }
            Some("before") => {
                self.pos += 1;
                self.atom().map(Scope::Before).unwrap_or(Scope::Globally)
            }
            Some("after") => {
                self.pos += 1;
                self.atom().map(Scope::After).unwrap_or(Scope::Globally)
            }
            Some("between") => {
                self.pos += 1;
                let a = self.atom();
                self.eat_ident("and");
                let b = self.atom();
                match (a, b) {
                    (Some(a), Some(b)) => Scope::Between(a, b),
                    _ => Scope::Globally,
                }
            }
            _ => Scope::Globally,
        }
    }

    // --- expressions: not > and > or ---------------------------------------

    fn expr(&mut self) -> Option<Expr> {
        let mut left = self.and_expr()?;
        while self.eat_ident("or") {
            let right = self.and_expr()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn and_expr(&mut self) -> Option<Expr> {
        let mut left = self.unary()?;
        while self.eat_ident("and") {
            let right = self.unary()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn unary(&mut self) -> Option<Expr> {
        if self.eat_ident("not") {
            Some(Expr::Not(Box::new(self.unary()?)))
        } else if self.peek_punct() == Some('(') {
            self.pos += 1;
            let e = self.expr()?;
            self.expect_punct(')');
            Some(e)
        } else {
            self.atom().map(Expr::Atom)
        }
    }

    fn atom(&mut self) -> Option<Atom> {
        let (name, line) = self.expect_ident()?;
        let args = if self.peek_punct() == Some('(') {
            self.pos += 1;
            self.arglist()
        } else {
            Vec::new()
        };
        let guard = if self.eat_ident("with") {
            Some(self.raw_until_stop())
        } else {
            None
        };
        Some(Atom {
            name,
            args,
            guard,
            line,
        })
    }

    /// Parse comma-separated raw argument terms (cursor just past `(`), consuming `)`.
    fn arglist(&mut self) -> Vec<String> {
        let mut args = Vec::new();
        let mut cur_start: Option<usize> = None;
        let mut cur_end = 0usize;
        let mut depth = 0i32;
        while let Some(t) = self.peek() {
            match &t.kind {
                Tok::Punct('(') => {
                    depth += 1;
                    if cur_start.is_none() {
                        cur_start = Some(t.start);
                    }
                    cur_end = t.end;
                    self.pos += 1;
                }
                Tok::Punct(')') if depth > 0 => {
                    depth -= 1;
                    cur_end = t.end;
                    self.pos += 1;
                }
                Tok::Punct(')') => {
                    if let Some(s) = cur_start.take() {
                        args.push(self.src[s..cur_end].trim().to_string());
                    }
                    self.pos += 1;
                    break;
                }
                Tok::Punct(',') if depth == 0 => {
                    if let Some(s) = cur_start.take() {
                        args.push(self.src[s..cur_end].trim().to_string());
                    }
                    self.pos += 1;
                }
                _ => {
                    if cur_start.is_none() {
                        cur_start = Some(t.start);
                    }
                    cur_end = t.end;
                    self.pos += 1;
                }
            }
        }
        args
    }
}

/// Split raw text on commas that sit outside any parentheses (for `assume` entries).
fn split_top_commas(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, c) in raw.char_indices() {
        match c {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth -= 1,
            ',' if depth == 0 => {
                let piece = raw[start..i].trim();
                if !piece.is_empty() {
                    out.push(piece.to_string());
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = raw[start..].trim();
    if !last.is_empty() {
        out.push(last.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_requirement() {
        let r = parse("requirement r { require { always ok } }").unwrap();
        assert_eq!(r.name, "r");
        assert_eq!(r.require.len(), 1);
        assert!(matches!(r.require[0].pattern, Pattern::Always(_)));
        assert_eq!(r.require[0].scope, Scope::Globally);
    }

    #[test]
    fn missing_requirement_keyword_is_an_error() {
        let errs = parse("foo { }").unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, GateError::MissingSection { section } if section.starts_with("requirement"))));
    }

    #[test]
    fn missing_require_section_is_flagged() {
        let errs = parse("requirement r { category: 1 }").unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, GateError::MissingSection { section: "require" })));
    }

    #[test]
    fn parses_category_list() {
        let r = parse("requirement r { category: 2a + 2b\n require { always ok } }").unwrap();
        assert_eq!(r.category, vec![Category::Model, Category::Runtime]);
    }

    // A single-line block must not let `category` swallow the sections that follow it
    // (it stops at the next section keyword, not just the newline).
    #[test]
    fn category_stops_at_next_section_on_one_line() {
        let r = parse(
            "requirement r { category: 2b vocabulary { state ok(x) } require { always ok(x) } }",
        )
        .unwrap();
        assert_eq!(r.category, vec![Category::Runtime]);
        assert_eq!(r.vocabulary.len(), 1);
        assert_eq!(r.require.len(), 1);
    }

    #[test]
    fn bad_category_is_reported() {
        let errs = parse("requirement r { category: 9z\n require { always ok } }").unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, GateError::BadCategory { value, .. } if value == "9z")));
    }

    #[test]
    fn parses_vocabulary_with_typed_and_untyped_params() {
        let src = "requirement r {
            vocabulary {
                sort Message
                event accepted(m: Message)
                state succeeded(m), dead_lettered(m: Message, reason: String)
            }
            require { always accepted }
        }";
        let r = parse(src).unwrap();
        // sort + 3 predicates (one event, two states via the comma list)
        assert_eq!(r.vocabulary.len(), 4);
        let names: Vec<_> = r
            .vocabulary
            .iter()
            .filter_map(|d| match d {
                Decl::Event { name, .. } | Decl::State { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["accepted", "succeeded", "dead_lettered"]);
    }

    #[test]
    fn parses_quantified_leads_to_with_within_and_guard() {
        let src = "requirement no_message_lost {
            vocabulary {
                event accepted(m: Message)
                state succeeded(m), dead_lettered(m: Message, reason: String)
            }
            require {
                each m: Message .
                    accepted(m) leads_to (succeeded(m) or dead_lettered(m, r) with r != \"\") within 30s
            }
        }";
        let r = parse(src).unwrap();
        let prop = &r.require[0];
        assert_eq!(prop.quantifier.as_ref().unwrap().var, "m");
        assert_eq!(prop.quantifier.as_ref().unwrap().sort, "Message");
        match &prop.pattern {
            Pattern::LeadsTo { from, to, within } => {
                assert!(matches!(from, Expr::Atom(a) if a.name == "accepted"));
                assert!(matches!(to, Expr::Or(_, _)));
                assert_eq!(within.as_deref(), Some("30s"));
                // The `with` guard rode along on the dead_lettered atom.
                let mut guards = Vec::new();
                to.for_each_atom(&mut |a| {
                    if let Some(g) = &a.guard {
                        guards.push(g.clone());
                    }
                });
                assert_eq!(guards, vec!["r != \"\""]);
            }
            other => panic!("expected leads_to, got {other:?}"),
        }
    }

    #[test]
    fn parses_scopes_and_precedes() {
        let r = parse("requirement r { require { grant precedes use between open and close } }")
            .unwrap();
        assert!(matches!(r.require[0].pattern, Pattern::Precedes { .. }));
        match &r.require[0].scope {
            Scope::Between(a, b) => {
                assert_eq!(a.name, "open");
                assert_eq!(b.name, "close");
            }
            other => panic!("expected between scope, got {other:?}"),
        }
    }

    #[test]
    fn parses_occurs_at_most() {
        let r = parse("requirement r { require { retry occurs at most 5 times } }").unwrap();
        match r.require[0].pattern {
            Pattern::OccursAtMost { k, .. } => assert_eq!(k, 5),
            ref other => panic!("expected occurs-at-most, got {other:?}"),
        }
    }

    #[test]
    fn keeps_strength_and_evidence_raw() {
        let src = "requirement r {
            require { always ok }
            strength: model_checked over Model, monitored(deadline = 30s)
            evidence: tla+ (bounded: |Message| <= 8), monpoly(stream = queue.events)
        }";
        let r = parse(src).unwrap();
        assert_eq!(
            r.strength.as_deref(),
            Some("model_checked over Model, monitored(deadline = 30s)")
        );
        assert!(r.evidence.as_deref().unwrap().contains("|Message| <= 8"));
    }

    #[test]
    fn parses_assume_entries() {
        let src = "requirement r { assume { retries_bounded(N = 5), fairness = WF }
            require { always ok } }";
        let r = parse(src).unwrap();
        assert_eq!(r.assume, vec!["retries_bounded(N = 5)", "fairness = WF"]);
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
    }
}
