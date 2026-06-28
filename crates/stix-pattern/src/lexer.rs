//! Lexer: converts a pattern string into a flat token stream.

use crate::error::{ParseError, Result, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Punctuation
    LBracket,
    RBracket,
    LParen,
    RParen,
    Colon,
    Dot,
    Comma,
    Star,
    // Comparison operators
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    // Keywords
    And,
    Or,
    Not,
    FollowedBy,
    Like,
    Matches,
    In,
    IsSubset,
    IsSuperset,
    Exists,
    Within,
    Repeats,
    Seconds,
    Times,
    Start,
    Stop,
    // Literals & identifiers
    Identifier(String),
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Timestamp(String),
    Binary(String),
    Hex(String),
}

/// Tokenize a STIX pattern string into a vector of tokens.
pub fn tokenize(src: &str) -> Result<Vec<Token>> {
    Lexer::new(src).run()
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer {
            src: src.as_bytes(),
            pos: 0,
            tokens: Vec::new(),
        }
    }

    fn run(mut self) -> Result<Vec<Token>> {
        while let Some(&c) = self.src.get(self.pos) {
            match c {
                b' ' | b'\t' | b'\r' | b'\n' => self.pos += 1,
                b'[' => self.punct(TokenKind::LBracket),
                b']' => self.punct(TokenKind::RBracket),
                b'(' => self.punct(TokenKind::LParen),
                b')' => self.punct(TokenKind::RParen),
                b':' => self.punct(TokenKind::Colon),
                b'.' if !self.peek_is_digit(1) => self.punct(TokenKind::Dot),
                b',' => self.punct(TokenKind::Comma),
                b'*' => self.punct(TokenKind::Star),
                b'=' => self.punct(TokenKind::Equal),
                b'!' => self.lex_bang()?,
                b'<' => self.lex_lt(),
                b'>' => self.lex_gt(),
                b'\'' => self.lex_string()?,
                b'-' | b'0'..=b'9' => self.lex_number()?,
                _ if is_ident_start(c) => self.lex_word_or_typed_literal()?,
                _ => {
                    return Err(ParseError::new(
                        format!("unexpected character '{}'", c as char),
                        Span::new(self.pos, self.pos + 1),
                    ))
                }
            }
        }
        Ok(self.tokens)
    }

    fn peek_is_digit(&self, ahead: usize) -> bool {
        matches!(self.src.get(self.pos + ahead), Some(b'0'..=b'9'))
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: Span::new(start, self.pos),
        });
    }

    fn punct(&mut self, kind: TokenKind) {
        let start = self.pos;
        self.pos += 1;
        self.push(kind, start);
    }

    fn lex_bang(&mut self) -> Result<()> {
        let start = self.pos;
        if self.src.get(self.pos + 1) == Some(&b'=') {
            self.pos += 2;
            self.push(TokenKind::NotEqual, start);
            Ok(())
        } else {
            Err(ParseError::new(
                "expected '=' after '!'",
                Span::new(start, start + 1),
            ))
        }
    }

    fn lex_lt(&mut self) {
        let start = self.pos;
        if self.src.get(self.pos + 1) == Some(&b'=') {
            self.pos += 2;
            self.push(TokenKind::LessThanOrEqual, start);
        } else {
            self.pos += 1;
            self.push(TokenKind::LessThan, start);
        }
    }

    fn lex_gt(&mut self) {
        let start = self.pos;
        if self.src.get(self.pos + 1) == Some(&b'=') {
            self.pos += 2;
            self.push(TokenKind::GreaterThanOrEqual, start);
        } else {
            self.pos += 1;
            self.push(TokenKind::GreaterThan, start);
        }
    }

    /// Reads a single-quoted string body starting at the opening quote.
    /// Returns the decoded contents and advances past the closing quote.
    fn read_quoted(&mut self) -> Result<String> {
        let start = self.pos;
        debug_assert_eq!(self.src.get(self.pos), Some(&b'\''));
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            match self.src.get(self.pos) {
                None => {
                    return Err(ParseError::new(
                        "unterminated string literal",
                        Span::new(start, self.pos),
                    ))
                }
                Some(b'\\') => match self.src.get(self.pos + 1) {
                    Some(b'\'') => {
                        out.push('\'');
                        self.pos += 2;
                    }
                    Some(b'\\') => {
                        out.push('\\');
                        self.pos += 2;
                    }
                    _ => {
                        out.push('\\');
                        self.pos += 1;
                    }
                },
                Some(b'\'') => {
                    self.pos += 1; // closing quote
                    return Ok(out);
                }
                Some(&b) => {
                    out.push(b as char);
                    self.pos += 1;
                }
            }
        }
    }

    fn lex_string(&mut self) -> Result<()> {
        let start = self.pos;
        let s = self.read_quoted()?;
        self.push(TokenKind::String(s), start);
        Ok(())
    }

    fn lex_number(&mut self) -> Result<()> {
        let start = self.pos;
        if self.src.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        let mut is_float = false;
        while let Some(&c) = self.src.get(self.pos) {
            match c {
                b'0'..=b'9' => self.pos += 1,
                b'.' => {
                    is_float = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        if is_float {
            let v: f64 = text.parse().map_err(|_| {
                ParseError::new("invalid float literal", Span::new(start, self.pos))
            })?;
            self.push(TokenKind::Float(v), start);
        } else {
            let v: i64 = text.parse().map_err(|_| {
                ParseError::new("invalid integer literal", Span::new(start, self.pos))
            })?;
            self.push(TokenKind::Integer(v), start);
        }
        Ok(())
    }

    /// Handles bare words (keywords, identifiers, booleans) AND the typed-literal
    /// prefixes `t'...'`, `b'...'`, `h'...'`.
    fn lex_word_or_typed_literal(&mut self) -> Result<()> {
        let start = self.pos;
        // Typed-literal prefix: a single letter t/b/h immediately followed by a quote.
        if matches!(self.src.get(self.pos), Some(b't' | b'b' | b'h'))
            && self.src.get(self.pos + 1) == Some(&b'\'')
        {
            let prefix = self.src[self.pos];
            self.pos += 1; // consume prefix letter
            let body = self.read_quoted()?;
            let kind = match prefix {
                b't' => TokenKind::Timestamp(body),
                b'b' => TokenKind::Binary(body),
                _ => TokenKind::Hex(body),
            };
            self.push(kind, start);
            return Ok(());
        }

        while let Some(&c) = self.src.get(self.pos) {
            if is_ident_continue(c) {
                self.pos += 1;
            } else {
                break;
            }
        }
        let word = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        let kind = keyword(word).unwrap_or_else(|| TokenKind::Identifier(word.to_string()));
        self.push(kind, start);
        Ok(())
    }
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

/// Identifier continuation allows hyphens so object types like `ipv4-addr` lex as one token.
fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

/// Map a bare word to a keyword token (case-insensitive), or `None` if it is an identifier.
fn keyword(word: &str) -> Option<TokenKind> {
    match word.to_ascii_uppercase().as_str() {
        "AND" => Some(TokenKind::And),
        "OR" => Some(TokenKind::Or),
        "NOT" => Some(TokenKind::Not),
        "FOLLOWEDBY" => Some(TokenKind::FollowedBy),
        "LIKE" => Some(TokenKind::Like),
        "MATCHES" => Some(TokenKind::Matches),
        "IN" => Some(TokenKind::In),
        "ISSUBSET" => Some(TokenKind::IsSubset),
        "ISSUPERSET" => Some(TokenKind::IsSuperset),
        "EXISTS" => Some(TokenKind::Exists),
        "WITHIN" => Some(TokenKind::Within),
        "REPEATS" => Some(TokenKind::Repeats),
        "SECONDS" => Some(TokenKind::Seconds),
        "TIMES" => Some(TokenKind::Times),
        "START" => Some(TokenKind::Start),
        "STOP" => Some(TokenKind::Stop),
        "TRUE" => Some(TokenKind::Boolean(true)),
        "FALSE" => Some(TokenKind::Boolean(false)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        tokenize(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn lexes_simple_comparison() {
        let k = kinds("[ipv4-addr:value = '1.2.3.4']");
        assert_eq!(
            k,
            vec![
                TokenKind::LBracket,
                TokenKind::Identifier("ipv4-addr".to_string()),
                TokenKind::Colon,
                TokenKind::Identifier("value".to_string()),
                TokenKind::Equal,
                TokenKind::String("1.2.3.4".to_string()),
                TokenKind::RBracket,
            ]
        );
    }

    #[test]
    fn lexes_keywords_case_insensitively() {
        assert_eq!(kinds("AND and"), vec![TokenKind::And, TokenKind::And]);
        assert_eq!(kinds("FOLLOWEDBY"), vec![TokenKind::FollowedBy]);
    }

    #[test]
    fn lexes_operators() {
        assert_eq!(
            kinds("= != < <= > >="),
            vec![
                TokenKind::Equal,
                TokenKind::NotEqual,
                TokenKind::LessThan,
                TokenKind::LessThanOrEqual,
                TokenKind::GreaterThan,
                TokenKind::GreaterThanOrEqual,
            ]
        );
    }

    #[test]
    fn lexes_literals() {
        assert_eq!(kinds("42"), vec![TokenKind::Integer(42)]);
        assert_eq!(kinds("-7"), vec![TokenKind::Integer(-7)]);
        assert_eq!(kinds("2.5"), vec![TokenKind::Float(2.5)]);
        assert_eq!(
            kinds("true false"),
            vec![TokenKind::Boolean(true), TokenKind::Boolean(false)]
        );
        assert_eq!(
            kinds("t'2014-01-13T07:03:17Z'"),
            vec![TokenKind::Timestamp("2014-01-13T07:03:17Z".to_string())]
        );
        assert_eq!(
            kinds("b'aGVsbG8='"),
            vec![TokenKind::Binary("aGVsbG8=".to_string())]
        );
        assert_eq!(
            kinds("h'1234abcd'"),
            vec![TokenKind::Hex("1234abcd".to_string())]
        );
    }

    #[test]
    fn lexes_string_escapes() {
        assert_eq!(kinds(r"'a\'b'"), vec![TokenKind::String("a'b".to_string())]);
        assert_eq!(
            kinds(r"'a\\b'"),
            vec![TokenKind::String(r"a\b".to_string())]
        );
    }

    #[test]
    fn unterminated_string_errors() {
        let err = tokenize("'oops").unwrap_err();
        assert!(err.message.contains("unterminated"), "got: {}", err.message);
    }

    #[test]
    fn tracks_spans() {
        let toks = tokenize("[a:b]").unwrap();
        assert_eq!(toks[0].span, crate::error::Span::new(0, 1)); // '['
    }
}
