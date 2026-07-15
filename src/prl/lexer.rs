//! A forgiving tokenizer. It never fails: any character it does not recognize becomes
//! a single-char [`Tok::Punct`], so the *parser* owns all error reporting and the raw
//! leaves the parser slices back out of source (durations, guards, `strength` lines)
//! can contain anything — `|`, `<=`, `!=` — without a lexing stage choking on them.
//!
//! Implements: REQ016 (mechanical gate part 1 — parse + type/name-check).

/// One lexical token with its source line and byte span (the span lets the parser
/// recover raw text for the deliberately-unparsed leaves).
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: Tok,
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    /// A `"…"` string literal (contents, unescaped).
    Str(String),
    Punct(char),
}

/// Tokenize `src`. Skips whitespace and `//` line comments; recognizes identifiers
/// (`[A-Za-z_][A-Za-z0-9_]*`), integers, `"…"` strings, and emits every other
/// character as a `Punct`.
pub fn lex(src: &str) -> Vec<Token> {
    let mut toks = Vec::new();
    let mut line = 1usize;
    let bytes = src.as_bytes();
    let mut chars = src.char_indices().peekable();

    while let Some(&(i, c)) = chars.peek() {
        if c == '\n' {
            line += 1;
            chars.next();
            continue;
        }
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        // Line comment `// …`
        if c == '/' && bytes.get(i + 1) == Some(&b'/') {
            while let Some(&(_, cc)) = chars.peek() {
                if cc == '\n' {
                    break;
                }
                chars.next();
            }
            continue;
        }
        if c == '"' {
            chars.next(); // opening quote
            let start = i;
            let mut s = String::new();
            let mut end = i + 1;
            for (j, cc) in chars.by_ref() {
                end = j + cc.len_utf8();
                if cc == '"' {
                    break;
                }
                s.push(cc);
            }
            toks.push(Token {
                kind: Tok::Str(s),
                line,
                start,
                end,
            });
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            let mut text = String::new();
            let mut end = i;
            while let Some(&(j, cc)) = chars.peek() {
                if cc.is_ascii_digit() {
                    text.push(cc);
                    end = j + cc.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            let val = text.parse::<i64>().unwrap_or(0);
            toks.push(Token {
                kind: Tok::Int(val),
                line,
                start,
                end,
            });
            continue;
        }
        if c == '_' || c.is_alphabetic() {
            let start = i;
            let mut text = String::new();
            let mut end = i;
            while let Some(&(j, cc)) = chars.peek() {
                if cc == '_' || cc.is_alphanumeric() {
                    text.push(cc);
                    end = j + cc.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            toks.push(Token {
                kind: Tok::Ident(text),
                line,
                start,
                end,
            });
            continue;
        }
        // Anything else: a single-char punct token.
        toks.push(Token {
            kind: Tok::Punct(c),
            line,
            start: i,
            end: i + c.len_utf8(),
        });
        chars.next();
    }
    toks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_idents_ints_and_punct() {
        let toks = lex("requirement r { k = 5 }");
        let kinds: Vec<_> = toks.iter().map(|t| t.kind.clone()).collect();
        assert_eq!(kinds[0], Tok::Ident("requirement".into()));
        assert_eq!(kinds[1], Tok::Ident("r".into()));
        assert_eq!(kinds[2], Tok::Punct('{'));
        assert_eq!(kinds[4], Tok::Punct('='));
        assert_eq!(kinds[5], Tok::Int(5));
    }

    #[test]
    fn never_fails_on_stray_chars() {
        // The `|` `<=` in an evidence line must not crash the lexer.
        let toks = lex("evidence: tla+ (bounded: |Message| <= 8)");
        assert!(toks.iter().any(|t| t.kind == Tok::Punct('|')));
        assert!(toks.iter().any(|t| t.kind == Tok::Punct('<')));
    }

    #[test]
    fn skips_line_comments_and_tracks_lines() {
        let toks = lex("a // comment\nb");
        assert_eq!(toks.len(), 2);
        assert_eq!(toks[0].line, 1);
        assert_eq!(toks[1].line, 2);
        assert_eq!(toks[1].kind, Tok::Ident("b".into()));
    }

    #[test]
    fn lexes_empty_string_literal() {
        let toks = lex(r#"r != """#);
        assert!(toks.iter().any(|t| t.kind == Tok::Str(String::new())));
    }
}
