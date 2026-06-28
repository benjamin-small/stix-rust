//! Recursive-descent parser: tokens -> AST.

use crate::ast::{ObjectPath, PathStep, Pattern};
use crate::error::{ParseError, Result, Span};
use crate::lexer::{Token, TokenKind};

pub(crate) struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    /// Length of the source string, used to build EOF spans.
    src_len: usize,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(tokens: &'a [Token], src: &'a str) -> Self {
        Parser {
            tokens,
            pos: 0,
            src_len: src.len(),
        }
    }

    // --- cursor helpers ---

    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos).map(|t| &t.kind)
    }

    fn current_span(&self) -> Span {
        match self.tokens.get(self.pos) {
            Some(t) => t.span,
            None => Span::new(self.src_len, self.src_len),
        }
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Consume the current token if its kind equals `want`.
    fn eat(&mut self, want: &TokenKind) -> bool {
        if self.peek() == Some(want) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, want: &TokenKind, what: &str) -> Result<()> {
        if self.eat(want) {
            Ok(())
        } else {
            Err(ParseError::new(
                format!("expected {what}"),
                self.current_span(),
            ))
        }
    }

    fn error_here(&self, msg: impl Into<String>) -> ParseError {
        ParseError::new(msg, self.current_span())
    }

    // --- object path ---

    pub(crate) fn parse_object_path(&mut self) -> Result<ObjectPath> {
        // object-type
        let object_type = match self.advance().map(|t| &t.kind) {
            Some(TokenKind::Identifier(s)) => s.clone(),
            _ => return Err(self.error_here("expected object type identifier")),
        };
        self.expect(&TokenKind::Colon, "':' after object type")?;

        // first path component (identifier or quoted string)
        let mut steps = Vec::new();
        steps.push(PathStep::Key(self.parse_key_component()?));

        // subsequent steps
        loop {
            match self.peek() {
                Some(TokenKind::Dot) => {
                    self.advance();
                    steps.push(PathStep::Key(self.parse_key_component()?));
                }
                Some(TokenKind::LBracket) => {
                    self.advance();
                    let step = match self.peek() {
                        Some(TokenKind::Star) => {
                            self.advance();
                            PathStep::AnyIndex
                        }
                        Some(TokenKind::Integer(n)) => {
                            let n = *n;
                            self.advance();
                            if n < 0 {
                                return Err(self.error_here("list index must be non-negative"));
                            }
                            PathStep::Index(n as u64)
                        }
                        _ => return Err(self.error_here("expected index or '*' in '[...]'")),
                    };
                    self.expect(&TokenKind::RBracket, "']' to close index")?;
                    steps.push(step);
                }
                _ => break,
            }
        }
        Ok(ObjectPath { object_type, steps })
    }

    fn parse_key_component(&mut self) -> Result<String> {
        match self.advance().map(|t| &t.kind) {
            Some(TokenKind::Identifier(s)) => Ok(s.clone()),
            Some(TokenKind::String(s)) => Ok(s.clone()),
            _ => Err(self.error_here("expected property name")),
        }
    }
}

/// Parse a complete pattern string into an AST.
/// (Observation/comparison parsing is added in later tasks.)
pub fn parse(src: &str) -> Result<Pattern> {
    let _ = crate::lexer::tokenize(src)?;
    Err(ParseError::new(
        "parse() implemented in a later task",
        Span::new(0, 0),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::PathStep;

    fn parse_path(src: &str) -> crate::ast::ObjectPath {
        let toks = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(&toks, src);
        p.parse_object_path().unwrap()
    }

    #[test]
    fn parses_simple_path() {
        let path = parse_path("ipv4-addr:value");
        assert_eq!(path.object_type, "ipv4-addr");
        assert_eq!(path.steps, vec![PathStep::Key("value".to_string())]);
    }

    #[test]
    fn parses_nested_keys() {
        let path = parse_path("file:hashes.MD5");
        assert_eq!(path.object_type, "file");
        assert_eq!(
            path.steps,
            vec![
                PathStep::Key("hashes".to_string()),
                PathStep::Key("MD5".to_string())
            ]
        );
    }

    #[test]
    fn parses_index_and_any_index() {
        let path = parse_path("network-traffic:protocols[0]");
        assert_eq!(
            path.steps,
            vec![
                PathStep::Key("protocols".to_string()),
                PathStep::Index(0)
            ]
        );

        let path = parse_path("x:list[*]");
        assert_eq!(
            path.steps,
            vec![PathStep::Key("list".to_string()), PathStep::AnyIndex]
        );
    }

    #[test]
    fn parses_quoted_key() {
        let path = parse_path("file:hashes.'SHA-256'");
        assert_eq!(
            path.steps,
            vec![
                PathStep::Key("hashes".to_string()),
                PathStep::Key("SHA-256".to_string())
            ]
        );
    }
}
