# stix-pattern Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `stix-pattern` crate — a pure-Rust lexer + recursive-descent parser that turns a STIX 2.1 patterning-language string into a typed, serde-serializable AST, parsing the *complete* grammar (comparisons, AND/OR/FOLLOWEDBY, and WITHIN/REPEATS/START..STOP qualifiers).

**Architecture:** A Cargo workspace is scaffolded with `stix-pattern` as the first member. The crate is split into focused modules: `error` (ParseError with source spans), `ast` (all node types), `lexer` (string → tokens), and `parser` (tokens → AST via recursive descent + Pratt-style precedence). Parsing the full grammar now means later crates can match incrementally without re-touching the parser.

**Tech Stack:** Rust (edition 2021), `serde` + `serde_derive` for AST serialization, `thiserror` for error types. No parser-generator dependencies (hand-written, keeps bindings clean later).

---

## Reference: STIX 2.1 Patterning Grammar (cheat sheet)

Precedence, loosest to tightest, for **observation expressions**:
`FOLLOWEDBY` < `OR` < `AND` < (postfix qualifiers `REPEATS`/`WITHIN`/`START..STOP`) < `[ ... ]` / `( ... )`.

For **comparison expressions** (inside `[ ]`): `OR` < `AND` < propTest.

Grammar (simplified EBNF):

```
pattern              = observationExpr
observationExpr      = obsOr (FOLLOWEDBY obsOr)*
obsOr                = obsAnd (OR obsAnd)*
obsAnd               = obsQualified (AND obsQualified)*
obsQualified         = obsPrimary (qualifier)*
qualifier            = WITHIN floatOrInt SECONDS
                     | REPEATS int TIMES
                     | START timestamp STOP timestamp
obsPrimary           = '[' comparisonExpr ']' | '(' observationExpr ')'

comparisonExpr       = compAnd (OR compAnd)*
compAnd              = propTest (AND propTest)*
propTest             = '(' comparisonExpr ')'
                     | EXISTS objectPath
                     | objectPath NOT? op primitiveLiteral
                     | objectPath NOT? IN setLiteral
                     | objectPath NOT? (LIKE|MATCHES|ISSUBSET|ISSUPERSET) stringLiteral
op                   = '=' | '!=' | '<' | '<=' | '>' | '>='
objectPath           = objectType ':' firstPathComponent pathStep*
objectType           = identifier (may contain hyphens, e.g. ipv4-addr)
firstPathComponent   = identifier | stringLiteral
pathStep             = '.' (identifier | stringLiteral)        (key step)
                     | '[' int ']'                              (index step)
                     | '[' '*' ']'                              (any-index step)
setLiteral           = '(' primitiveLiteral (',' primitiveLiteral)* ')'
primitiveLiteral     = stringLit | intLit | floatLit | boolLit
                     | timestampLit | binaryLit | hexLit
```

Literal token forms:
- string: `'...'` (single-quoted; `\'` and `\\` escapes)
- timestamp: `t'2014-01-13T07:03:17Z'`
- binary (base64): `b'aGVsbG8='`
- hex: `h'1234abcd'`
- int: `42`, `-7`; float: `3.14`, `-0.5`
- bool: `true` / `false`

---

## File Structure

- `Cargo.toml` (workspace root) — declares the workspace + members.
- `.gitignore` — already exists (`/target`, `Cargo.lock`, `*.rs.bk`).
- `crates/stix-pattern/Cargo.toml` — crate manifest.
- `crates/stix-pattern/src/lib.rs` — module wiring + top-level `parse()` + re-exports.
- `crates/stix-pattern/src/error.rs` — `ParseError`, `Span`.
- `crates/stix-pattern/src/ast.rs` — all AST node types (serde-derived).
- `crates/stix-pattern/src/lexer.rs` — `Token`, `TokenKind`, `Lexer`, `tokenize()`.
- `crates/stix-pattern/src/parser.rs` — `Parser` (recursive descent), produces `ast::Pattern`.
- `crates/stix-pattern/tests/conformance.rs` — corpus-driven valid/invalid tests.
- `crates/stix-pattern/tests/fixtures/valid_patterns.txt` — one valid pattern per line.
- `crates/stix-pattern/tests/fixtures/invalid_patterns.txt` — one invalid pattern per line.

---

## Task 1: Workspace + crate scaffold

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/stix-pattern/Cargo.toml`
- Create: `crates/stix-pattern/src/lib.rs`

- [ ] **Step 1: Create the workspace root manifest**

Create `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/stix-pattern"]

[workspace.package]
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/bsmall/stix-rust"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
```

- [ ] **Step 2: Create the crate manifest**

Create `crates/stix-pattern/Cargo.toml`:

```toml
[package]
name = "stix-pattern"
version = "0.0.1"
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Lexer and parser for the STIX 2.1 patterning language."

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

- [ ] **Step 3: Create a placeholder lib.rs**

Create `crates/stix-pattern/src/lib.rs`:

```rust
//! Lexer and parser for the STIX 2.1 patterning language.

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: Verify the workspace builds and tests run**

Run: `cargo test -p stix-pattern`
Expected: compiles; `smoke::crate_builds` passes.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/stix-pattern/Cargo.toml crates/stix-pattern/src/lib.rs
git commit -m "feat(pattern): scaffold workspace and stix-pattern crate"
```

---

## Task 2: Error types

**Files:**
- Create: `crates/stix-pattern/src/error.rs`
- Modify: `crates/stix-pattern/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add to the bottom of `crates/stix-pattern/src/error.rs` (create the file with this test first):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_records_offsets() {
        let span = Span::new(3, 7);
        assert_eq!(span.start, 3);
        assert_eq!(span.end, 7);
    }

    #[test]
    fn parse_error_displays_with_span() {
        let err = ParseError::new("unexpected token", Span::new(5, 6));
        let msg = format!("{err}");
        assert!(msg.contains("unexpected token"), "got: {msg}");
        assert!(msg.contains('5'), "span start should appear: {msg}");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-pattern`
Expected: FAIL — `Span` / `ParseError` not found (and `error` module not declared yet).

- [ ] **Step 3: Implement error types**

At the top of `crates/stix-pattern/src/error.rs` (above the test module):

```rust
//! Error and source-span types for parsing.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A half-open byte range `[start, end)` into the original pattern string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }
}

/// An error produced while lexing or parsing a STIX pattern.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("parse error at bytes {}..{}: {message}", .span.start, .span.end)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        ParseError { message: message.into(), span }
    }
}

/// Convenience alias for results in this crate.
pub type Result<T> = std::result::Result<T, ParseError>;
```

In `crates/stix-pattern/src/lib.rs`, replace the `smoke` module with:

```rust
//! Lexer and parser for the STIX 2.1 patterning language.

pub mod error;

pub use error::{ParseError, Span};
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-pattern`
Expected: PASS — both error tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-pattern/src/error.rs crates/stix-pattern/src/lib.rs
git commit -m "feat(pattern): add ParseError and Span types"
```

---

## Task 3: AST node types

**Files:**
- Create: `crates/stix-pattern/src/ast.rs`
- Modify: `crates/stix-pattern/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-pattern/src/ast.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_comparison_pattern() {
        let path = ObjectPath {
            object_type: "ipv4-addr".to_string(),
            steps: vec![PathStep::Key("value".to_string())],
        };
        let comp = Comparison {
            path,
            operator: ComparisonOperator::Equal,
            negated: false,
            value: ComparisonOperand::Literal(Literal::String("1.2.3.4".to_string())),
        };
        let pattern = Pattern {
            expression: ObservationExpression::Observation(Box::new(
                ComparisonExpression::Test(comp),
            )),
        };
        match pattern.expression {
            ObservationExpression::Observation(_) => {}
            _ => panic!("expected observation"),
        }
    }

    #[test]
    fn ast_round_trips_through_json() {
        let lit = Literal::Integer(42);
        let json = serde_json::to_string(&lit).unwrap();
        let back: Literal = serde_json::from_str(&json).unwrap();
        assert_eq!(lit, back);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-pattern`
Expected: FAIL — AST types not found, `ast` module not declared.

- [ ] **Step 3: Implement the AST**

At the top of `crates/stix-pattern/src/ast.rs` (above the test module):

```rust
//! Abstract syntax tree for STIX 2.1 patterns. All nodes are serde-serializable.

use serde::{Deserialize, Serialize};

/// A complete parsed pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub expression: ObservationExpression,
}

/// Observation-level expression tree.
/// `FOLLOWEDBY`/`AND`/`OR` combine observations; qualifiers attach to a sub-expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ObservationExpression {
    /// A single `[ comparisonExpr ]` observation.
    Observation(Box<ComparisonExpression>),
    And(Box<ObservationExpression>, Box<ObservationExpression>),
    Or(Box<ObservationExpression>, Box<ObservationExpression>),
    FollowedBy(Box<ObservationExpression>, Box<ObservationExpression>),
    Qualified {
        expression: Box<ObservationExpression>,
        qualifier: Qualifier,
    },
}

/// Postfix qualifier on an observation expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Qualifier {
    /// `WITHIN <seconds> SECONDS`
    Within { seconds: f64 },
    /// `REPEATS <count> TIMES`
    Repeats { count: u64 },
    /// `START <start> STOP <stop>` (RFC3339 timestamps, kept as strings here)
    StartStop { start: String, stop: String },
}

/// Comparison-level expression tree (inside `[ ]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComparisonExpression {
    Test(Comparison),
    And(Box<ComparisonExpression>, Box<ComparisonExpression>),
    Or(Box<ComparisonExpression>, Box<ComparisonExpression>),
}

/// A single property test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub path: ObjectPath,
    pub operator: ComparisonOperator,
    /// `true` if a `NOT` preceded the operator.
    pub negated: bool,
    pub value: ComparisonOperand,
}

/// Right-hand side of a comparison: either a single literal or a set (for `IN`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComparisonOperand {
    Literal(Literal),
    Set(Vec<Literal>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    In,
    Like,
    Matches,
    IsSubset,
    IsSuperset,
    /// `EXISTS objectPath`; for this operator the operand is ignored.
    Exists,
}

/// An object path: `object-type:first.step[0].next`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectPath {
    pub object_type: String,
    pub steps: Vec<PathStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PathStep {
    /// `.key` or the first component after the colon.
    Key(String),
    /// `[n]`
    Index(u64),
    /// `[*]`
    AnyIndex,
}

/// A primitive literal value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    /// RFC3339 timestamp from a `t'...'` literal (kept as the inner string).
    Timestamp(String),
    /// Base64 payload from a `b'...'` literal (kept as the inner string).
    Binary(String),
    /// Hex payload from an `h'...'` literal (kept as the inner string).
    Hex(String),
}
```

In `crates/stix-pattern/src/lib.rs`, add the module and re-exports:

```rust
//! Lexer and parser for the STIX 2.1 patterning language.

pub mod ast;
pub mod error;

pub use ast::*;
pub use error::{ParseError, Span};
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p stix-pattern`
Expected: PASS — both AST tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-pattern/src/ast.rs crates/stix-pattern/src/lib.rs
git commit -m "feat(pattern): add serde-serializable pattern AST"
```

---

## Task 4: Lexer

**Files:**
- Create: `crates/stix-pattern/src/lexer.rs`
- Modify: `crates/stix-pattern/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-pattern/src/lexer.rs` with the test module first:

```rust
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
        assert_eq!(kinds("3.14"), vec![TokenKind::Float(3.14)]);
        assert_eq!(kinds("true false"), vec![TokenKind::Boolean(true), TokenKind::Boolean(false)]);
        assert_eq!(kinds("t'2014-01-13T07:03:17Z'"), vec![TokenKind::Timestamp("2014-01-13T07:03:17Z".to_string())]);
        assert_eq!(kinds("b'aGVsbG8='"), vec![TokenKind::Binary("aGVsbG8=".to_string())]);
        assert_eq!(kinds("h'1234abcd'"), vec![TokenKind::Hex("1234abcd".to_string())]);
    }

    #[test]
    fn lexes_string_escapes() {
        assert_eq!(kinds(r"'a\'b'"), vec![TokenKind::String("a'b".to_string())]);
        assert_eq!(kinds(r"'a\\b'"), vec![TokenKind::String(r"a\b".to_string())]);
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-pattern`
Expected: FAIL — lexer types/functions not found.

- [ ] **Step 3: Implement the lexer**

At the top of `crates/stix-pattern/src/lexer.rs` (above the test module):

```rust
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
        Lexer { src: src.as_bytes(), pos: 0, tokens: Vec::new() }
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
        self.tokens.push(Token { kind, span: Span::new(start, self.pos) });
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
            Err(ParseError::new("expected '=' after '!'", Span::new(start, start + 1)))
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
            let v: f64 = text
                .parse()
                .map_err(|_| ParseError::new("invalid float literal", Span::new(start, self.pos)))?;
            self.push(TokenKind::Float(v), start);
        } else {
            let v: i64 = text
                .parse()
                .map_err(|_| ParseError::new("invalid integer literal", Span::new(start, self.pos)))?;
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
```

In `crates/stix-pattern/src/lib.rs`, add:

```rust
pub mod lexer;
```

(Place it with the other `pub mod` lines; keep `ast`, `error` re-exports as they are.)

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p stix-pattern lexer`
Expected: PASS — all lexer tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-pattern/src/lexer.rs crates/stix-pattern/src/lib.rs
git commit -m "feat(pattern): add lexer with full token set and spans"
```

---

## Task 5: Parser core + object paths

**Files:**
- Create: `crates/stix-pattern/src/parser.rs`
- Modify: `crates/stix-pattern/src/lib.rs`

This task builds the parser struct, its cursor helpers, and object-path parsing. Comparison and observation parsing come in Tasks 6–7; we stub `parse()` minimally so the crate compiles, then flesh it out.

- [ ] **Step 1: Write the failing test**

Create `crates/stix-pattern/src/parser.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::PathStep;

    fn parse_path(src: &str) -> crate::ast::ObjectPath {
        let toks = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(&toks, src);
        let path = p.parse_object_path().unwrap();
        path
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
            vec![PathStep::Key("hashes".to_string()), PathStep::Key("MD5".to_string())]
        );
    }

    #[test]
    fn parses_index_and_any_index() {
        let path = parse_path("network-traffic:protocols[0]");
        assert_eq!(path.steps, vec![PathStep::Key("protocols".to_string()), PathStep::Index(0)]);

        let path = parse_path("x:list[*]");
        assert_eq!(path.steps, vec![PathStep::Key("list".to_string()), PathStep::AnyIndex]);
    }

    #[test]
    fn parses_quoted_key() {
        let path = parse_path("file:hashes.'SHA-256'");
        assert_eq!(
            path.steps,
            vec![PathStep::Key("hashes".to_string()), PathStep::Key("SHA-256".to_string())]
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-pattern parser`
Expected: FAIL — `Parser` not found.

- [ ] **Step 3: Implement the parser core + object-path parsing**

At the top of `crates/stix-pattern/src/parser.rs` (above the test module):

```rust
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
        Parser { tokens, pos: 0, src_len: src.len() }
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

    /// Consume the current token if its kind equals `want` (by discriminant for
    /// data-carrying kinds we use dedicated helpers instead).
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
/// (Filled in fully in later tasks; observation/comparison parsing added next.)
pub fn parse(src: &str) -> Result<Pattern> {
    let tokens = crate::lexer::tokenize(src)?;
    let mut parser = Parser::new(&tokens, src);
    let expression = parser.parse_observation_expression()?;
    if !parser.at_end() {
        return Err(parser.error_here("unexpected trailing tokens after pattern"));
    }
    Ok(Pattern { expression })
}
```

> Note: `parse()` references `parse_observation_expression`, which is added in Task 7. Until then the crate will not compile via `parse()`. To keep Task 5 green in isolation, temporarily comment out the body of `parse()` and return `Err(ParseError::new("not yet implemented", Span::new(0, 0)))`. Task 7 restores it. (If executing tasks strictly in order with commits, this temporary stub is committed in Task 5 and replaced in Task 7.)

Apply the temporary stub now — replace the `parse()` body with:

```rust
pub fn parse(src: &str) -> Result<Pattern> {
    let _ = crate::lexer::tokenize(src)?;
    Err(ParseError::new("parse() implemented in a later task", Span::new(0, 0)))
}
```

In `crates/stix-pattern/src/lib.rs`, add:

```rust
pub mod parser;

pub use parser::parse;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p stix-pattern parser`
Expected: PASS — all four object-path tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-pattern/src/parser.rs crates/stix-pattern/src/lib.rs
git commit -m "feat(pattern): add parser core and object-path parsing"
```

---

## Task 6: Comparison expression parsing

**Files:**
- Modify: `crates/stix-pattern/src/parser.rs`

- [ ] **Step 1: Write the failing test**

Add these tests inside the existing `mod tests` in `crates/stix-pattern/src/parser.rs`:

```rust
    use crate::ast::{Comparison, ComparisonExpression, ComparisonOperand, ComparisonOperator, Literal};

    fn parse_comp(src: &str) -> ComparisonExpression {
        let toks = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(&toks, src);
        p.parse_comparison_expression().unwrap()
    }

    #[test]
    fn parses_single_comparison() {
        let c = parse_comp("ipv4-addr:value = '1.2.3.4'");
        match c {
            ComparisonExpression::Test(Comparison { operator, negated, value, .. }) => {
                assert_eq!(operator, ComparisonOperator::Equal);
                assert!(!negated);
                assert_eq!(value, ComparisonOperand::Literal(Literal::String("1.2.3.4".into())));
            }
            _ => panic!("expected a single test"),
        }
    }

    #[test]
    fn parses_not_operator() {
        let c = parse_comp("file:size != 0");
        match c {
            ComparisonExpression::Test(Comparison { operator, value, .. }) => {
                assert_eq!(operator, ComparisonOperator::NotEqual);
                assert_eq!(value, ComparisonOperand::Literal(Literal::Integer(0)));
            }
            _ => panic!("expected test"),
        }
    }

    #[test]
    fn parses_not_keyword_prefix() {
        // `objectPath NOT op value` sets negated = true
        let c = parse_comp("file:name NOT = 'x'");
        match c {
            ComparisonExpression::Test(Comparison { negated, .. }) => assert!(negated),
            _ => panic!("expected test"),
        }
    }

    #[test]
    fn parses_in_set() {
        let c = parse_comp("ipv4-addr:value IN ('1.1.1.1', '8.8.8.8')");
        match c {
            ComparisonExpression::Test(Comparison { operator, value, .. }) => {
                assert_eq!(operator, ComparisonOperator::In);
                assert_eq!(
                    value,
                    ComparisonOperand::Set(vec![
                        Literal::String("1.1.1.1".into()),
                        Literal::String("8.8.8.8".into()),
                    ])
                );
            }
            _ => panic!("expected test"),
        }
    }

    #[test]
    fn parses_exists() {
        let c = parse_comp("EXISTS file:name");
        match c {
            ComparisonExpression::Test(Comparison { operator, path, .. }) => {
                assert_eq!(operator, ComparisonOperator::Exists);
                assert_eq!(path.object_type, "file");
            }
            _ => panic!("expected test"),
        }
    }

    #[test]
    fn comparison_and_binds_tighter_than_or() {
        // a OR b AND c  =>  a OR (b AND c)
        let c = parse_comp("file:name = 'a' OR file:name = 'b' AND file:size = 1");
        match c {
            ComparisonExpression::Or(_, right) => match *right {
                ComparisonExpression::And(_, _) => {}
                _ => panic!("right side of OR should be an AND"),
            },
            _ => panic!("top should be OR"),
        }
    }

    #[test]
    fn parses_parenthesized_comparison() {
        let c = parse_comp("(file:name = 'a' OR file:name = 'b') AND file:size = 1");
        match c {
            ComparisonExpression::And(left, _) => match *left {
                ComparisonExpression::Or(_, _) => {}
                _ => panic!("left of AND should be OR"),
            },
            _ => panic!("top should be AND"),
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p stix-pattern parser`
Expected: FAIL — `parse_comparison_expression` not found.

- [ ] **Step 3: Implement comparison-expression parsing**

Add these methods to `impl<'a> Parser<'a>` in `crates/stix-pattern/src/parser.rs`
(place them after `parse_key_component`). Add the needed imports to the existing
`use crate::ast::{...}` line: `Comparison, ComparisonExpression, ComparisonOperand,
ComparisonOperator, Literal`.

```rust
    // --- comparison expressions ---

    pub(crate) fn parse_comparison_expression(&mut self) -> Result<ComparisonExpression> {
        let mut left = self.parse_comparison_and()?;
        while self.eat(&TokenKind::Or) {
            let right = self.parse_comparison_and()?;
            left = ComparisonExpression::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_comparison_and(&mut self) -> Result<ComparisonExpression> {
        let mut left = self.parse_prop_test()?;
        while self.eat(&TokenKind::And) {
            let right = self.parse_prop_test()?;
            left = ComparisonExpression::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_prop_test(&mut self) -> Result<ComparisonExpression> {
        // Parenthesized sub-expression
        if self.eat(&TokenKind::LParen) {
            let inner = self.parse_comparison_expression()?;
            self.expect(&TokenKind::RParen, "')' to close comparison group")?;
            return Ok(inner);
        }

        // EXISTS objectPath
        if self.eat(&TokenKind::Exists) {
            let path = self.parse_object_path()?;
            return Ok(ComparisonExpression::Test(Comparison {
                path,
                operator: ComparisonOperator::Exists,
                negated: false,
                // operand unused for EXISTS; use a benign placeholder.
                value: ComparisonOperand::Literal(Literal::Boolean(true)),
            }));
        }

        // objectPath NOT? operator operand
        let path = self.parse_object_path()?;
        let negated = self.eat(&TokenKind::Not);
        let operator = self.parse_comparison_operator()?;
        let value = if operator == ComparisonOperator::In {
            self.parse_set_literal()?
        } else {
            ComparisonOperand::Literal(self.parse_literal()?)
        };
        Ok(ComparisonExpression::Test(Comparison { path, operator, negated, value }))
    }

    fn parse_comparison_operator(&mut self) -> Result<ComparisonOperator> {
        let op = match self.peek() {
            Some(TokenKind::Equal) => ComparisonOperator::Equal,
            Some(TokenKind::NotEqual) => ComparisonOperator::NotEqual,
            Some(TokenKind::GreaterThan) => ComparisonOperator::GreaterThan,
            Some(TokenKind::GreaterThanOrEqual) => ComparisonOperator::GreaterThanOrEqual,
            Some(TokenKind::LessThan) => ComparisonOperator::LessThan,
            Some(TokenKind::LessThanOrEqual) => ComparisonOperator::LessThanOrEqual,
            Some(TokenKind::In) => ComparisonOperator::In,
            Some(TokenKind::Like) => ComparisonOperator::Like,
            Some(TokenKind::Matches) => ComparisonOperator::Matches,
            Some(TokenKind::IsSubset) => ComparisonOperator::IsSubset,
            Some(TokenKind::IsSuperset) => ComparisonOperator::IsSuperset,
            _ => return Err(self.error_here("expected a comparison operator")),
        };
        self.advance();
        Ok(op)
    }

    fn parse_set_literal(&mut self) -> Result<ComparisonOperand> {
        self.expect(&TokenKind::LParen, "'(' to open set literal")?;
        let mut items = vec![self.parse_literal()?];
        while self.eat(&TokenKind::Comma) {
            items.push(self.parse_literal()?);
        }
        self.expect(&TokenKind::RParen, "')' to close set literal")?;
        Ok(ComparisonOperand::Set(items))
    }

    fn parse_literal(&mut self) -> Result<Literal> {
        let lit = match self.peek() {
            Some(TokenKind::String(s)) => Literal::String(s.clone()),
            Some(TokenKind::Integer(n)) => Literal::Integer(*n),
            Some(TokenKind::Float(f)) => Literal::Float(*f),
            Some(TokenKind::Boolean(b)) => Literal::Boolean(*b),
            Some(TokenKind::Timestamp(s)) => Literal::Timestamp(s.clone()),
            Some(TokenKind::Binary(s)) => Literal::Binary(s.clone()),
            Some(TokenKind::Hex(s)) => Literal::Hex(s.clone()),
            _ => return Err(self.error_here("expected a literal value")),
        };
        self.advance();
        Ok(lit)
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p stix-pattern parser`
Expected: PASS — all comparison tests pass (object-path tests still pass too).

- [ ] **Step 5: Commit**

```bash
git add crates/stix-pattern/src/parser.rs
git commit -m "feat(pattern): parse comparison expressions, sets, EXISTS, NOT"
```

---

## Task 7: Observation expression parsing + qualifiers + top-level parse()

**Files:**
- Modify: `crates/stix-pattern/src/parser.rs`

- [ ] **Step 1: Write the failing test**

Add these tests inside the existing `mod tests` in `crates/stix-pattern/src/parser.rs`:

```rust
    use crate::ast::{ObservationExpression, Qualifier};
    use crate::parser::parse;

    #[test]
    fn parses_single_observation() {
        let p = parse("[ipv4-addr:value = '1.2.3.4']").unwrap();
        match p.expression {
            ObservationExpression::Observation(_) => {}
            _ => panic!("expected single observation"),
        }
    }

    #[test]
    fn observation_and_binds_tighter_than_or() {
        // [a] OR [b] AND [c] => [a] OR ([b] AND [c])
        let p = parse("[file:name='a'] OR [file:name='b'] AND [file:size=1]").unwrap();
        match p.expression {
            ObservationExpression::Or(_, right) => match *right {
                ObservationExpression::And(_, _) => {}
                _ => panic!("right of OR should be AND"),
            },
            _ => panic!("top should be OR"),
        }
    }

    #[test]
    fn followedby_is_loosest() {
        // [a] FOLLOWEDBY [b] OR [c] => [a] FOLLOWEDBY ([b] OR [c])
        let p = parse("[file:name='a'] FOLLOWEDBY [file:name='b'] OR [file:name='c']").unwrap();
        match p.expression {
            ObservationExpression::FollowedBy(_, right) => match *right {
                ObservationExpression::Or(_, _) => {}
                _ => panic!("right of FOLLOWEDBY should be OR"),
            },
            _ => panic!("top should be FOLLOWEDBY"),
        }
    }

    #[test]
    fn parses_parenthesized_observation() {
        let p = parse("([file:name='a'] OR [file:name='b']) FOLLOWEDBY [file:size=1]").unwrap();
        match p.expression {
            ObservationExpression::FollowedBy(left, _) => match *left {
                ObservationExpression::Or(_, _) => {}
                _ => panic!("left should be OR"),
            },
            _ => panic!("top should be FOLLOWEDBY"),
        }
    }

    #[test]
    fn parses_within_qualifier() {
        let p = parse("[file:name='a'] REPEATS 2 TIMES WITHIN 60 SECONDS").unwrap();
        // Outermost qualifier is the last one parsed (WITHIN), wrapping REPEATS.
        match p.expression {
            ObservationExpression::Qualified { qualifier: Qualifier::Within { seconds }, expression } => {
                assert_eq!(seconds, 60.0);
                match *expression {
                    ObservationExpression::Qualified { qualifier: Qualifier::Repeats { count }, .. } => {
                        assert_eq!(count, 2);
                    }
                    _ => panic!("inner should be REPEATS"),
                }
            }
            _ => panic!("outer should be WITHIN"),
        }
    }

    #[test]
    fn parses_start_stop_qualifier() {
        let p = parse(
            "[file:name='a'] START t'2020-01-01T00:00:00Z' STOP t'2020-01-02T00:00:00Z'",
        )
        .unwrap();
        match p.expression {
            ObservationExpression::Qualified { qualifier: Qualifier::StartStop { start, stop }, .. } => {
                assert_eq!(start, "2020-01-01T00:00:00Z");
                assert_eq!(stop, "2020-01-02T00:00:00Z");
            }
            _ => panic!("expected START..STOP"),
        }
    }

    #[test]
    fn trailing_tokens_error() {
        assert!(parse("[file:name='a'] [file:name='b']").is_err());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p stix-pattern parser`
Expected: FAIL — `parse_observation_expression` not found; the temporary `parse()` stub returns an error.

- [ ] **Step 3: Implement observation parsing and restore parse()**

Add these methods to `impl<'a> Parser<'a>` in `crates/stix-pattern/src/parser.rs`
(after the comparison methods). Add to the `use crate::ast::{...}` line:
`ObservationExpression, Qualifier`.

```rust
    // --- observation expressions ---

    pub(crate) fn parse_observation_expression(&mut self) -> Result<ObservationExpression> {
        let mut left = self.parse_observation_or()?;
        while self.eat(&TokenKind::FollowedBy) {
            let right = self.parse_observation_or()?;
            left = ObservationExpression::FollowedBy(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_observation_or(&mut self) -> Result<ObservationExpression> {
        let mut left = self.parse_observation_and()?;
        while self.eat(&TokenKind::Or) {
            let right = self.parse_observation_and()?;
            left = ObservationExpression::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_observation_and(&mut self) -> Result<ObservationExpression> {
        let mut left = self.parse_observation_qualified()?;
        while self.eat(&TokenKind::And) {
            let right = self.parse_observation_qualified()?;
            left = ObservationExpression::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_observation_qualified(&mut self) -> Result<ObservationExpression> {
        let mut expr = self.parse_observation_primary()?;
        loop {
            let qualifier = match self.peek() {
                Some(TokenKind::Within) => {
                    self.advance();
                    let seconds = self.parse_number_as_f64()?;
                    self.expect(&TokenKind::Seconds, "SECONDS after WITHIN value")?;
                    Qualifier::Within { seconds }
                }
                Some(TokenKind::Repeats) => {
                    self.advance();
                    let count = self.parse_non_negative_int()?;
                    self.expect(&TokenKind::Times, "TIMES after REPEATS value")?;
                    Qualifier::Repeats { count }
                }
                Some(TokenKind::Start) => {
                    self.advance();
                    let start = self.parse_timestamp_string()?;
                    self.expect(&TokenKind::Stop, "STOP after START timestamp")?;
                    let stop = self.parse_timestamp_string()?;
                    Qualifier::StartStop { start, stop }
                }
                _ => break,
            };
            expr = ObservationExpression::Qualified {
                expression: Box::new(expr),
                qualifier,
            };
        }
        Ok(expr)
    }

    fn parse_observation_primary(&mut self) -> Result<ObservationExpression> {
        if self.eat(&TokenKind::LBracket) {
            let comp = self.parse_comparison_expression()?;
            self.expect(&TokenKind::RBracket, "']' to close observation")?;
            return Ok(ObservationExpression::Observation(Box::new(comp)));
        }
        if self.eat(&TokenKind::LParen) {
            let inner = self.parse_observation_expression()?;
            self.expect(&TokenKind::RParen, "')' to close grouped observation")?;
            return Ok(inner);
        }
        Err(self.error_here("expected '[' or '(' to start an observation"))
    }

    fn parse_number_as_f64(&mut self) -> Result<f64> {
        match self.peek() {
            Some(TokenKind::Integer(n)) => {
                let v = *n as f64;
                self.advance();
                Ok(v)
            }
            Some(TokenKind::Float(f)) => {
                let v = *f;
                self.advance();
                Ok(v)
            }
            _ => Err(self.error_here("expected a numeric value")),
        }
    }

    fn parse_non_negative_int(&mut self) -> Result<u64> {
        match self.peek() {
            Some(TokenKind::Integer(n)) if *n >= 0 => {
                let v = *n as u64;
                self.advance();
                Ok(v)
            }
            _ => Err(self.error_here("expected a non-negative integer")),
        }
    }

    fn parse_timestamp_string(&mut self) -> Result<String> {
        match self.peek() {
            Some(TokenKind::Timestamp(s)) => {
                let v = s.clone();
                self.advance();
                Ok(v)
            }
            _ => Err(self.error_here("expected a t'...' timestamp literal")),
        }
    }
```

Now restore the real `parse()` — replace the temporary stub body with:

```rust
pub fn parse(src: &str) -> Result<Pattern> {
    let tokens = crate::lexer::tokenize(src)?;
    let mut parser = Parser::new(&tokens, src);
    let expression = parser.parse_observation_expression()?;
    if !parser.at_end() {
        return Err(parser.error_here("unexpected trailing tokens after pattern"));
    }
    Ok(Pattern { expression })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p stix-pattern`
Expected: PASS — all parser tests pass, including the full-`parse()` cases.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-pattern/src/parser.rs
git commit -m "feat(pattern): parse observation expressions, qualifiers, top-level parse()"
```

---

## Task 8: Public API polish + AST round-trip integration test

**Files:**
- Modify: `crates/stix-pattern/src/lib.rs`
- Create: `crates/stix-pattern/tests/roundtrip.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/stix-pattern/tests/roundtrip.rs`:

```rust
use stix_pattern::{parse, Pattern};

#[test]
fn parse_then_serde_round_trip() {
    let src = "[ipv4-addr:value = '1.2.3.4'] FOLLOWEDBY [domain-name:value = 'evil.example'] WITHIN 300 SECONDS";
    let pattern = parse(src).expect("should parse");
    let json = serde_json::to_string(&pattern).expect("serialize");
    let back: Pattern = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(pattern, back);
}

#[test]
fn doc_example_in_lib_is_valid() {
    // Mirrors the crate-level doc example; guards against doc rot.
    let p = parse("[file:hashes.'SHA-256' = 'abc']").unwrap();
    let _ = serde_json::to_string(&p).unwrap();
}
```

- [ ] **Step 2: Run the test to verify it fails or passes**

Run: `cargo test -p stix-pattern --test roundtrip`
Expected: PASS if `parse` and `Pattern` are already re-exported from `lib.rs`. If it FAILS to compile (missing re-export), proceed to Step 3.

- [ ] **Step 3: Ensure clean public API + crate doc example**

Set `crates/stix-pattern/src/lib.rs` to:

```rust
//! Lexer and parser for the STIX 2.1 patterning language.
//!
//! # Example
//!
//! ```
//! use stix_pattern::parse;
//!
//! let pattern = parse("[file:hashes.'SHA-256' = 'abc']").unwrap();
//! let json = serde_json::to_string(&pattern).unwrap();
//! assert!(json.contains("SHA-256"));
//! ```

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use error::{ParseError, Span};
pub use parser::parse;
```

Add `serde_json` to `[dev-dependencies]` if the doc test needs it (it is already there from Task 1).

- [ ] **Step 4: Run all tests + doc tests**

Run: `cargo test -p stix-pattern`
Expected: PASS — unit tests, integration tests, and the doc test all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-pattern/src/lib.rs crates/stix-pattern/tests/roundtrip.rs
git commit -m "test(pattern): add serde round-trip integration test and crate doc example"
```

---

## Task 9: Conformance corpus (valid/invalid patterns)

**Files:**
- Create: `crates/stix-pattern/tests/fixtures/valid_patterns.txt`
- Create: `crates/stix-pattern/tests/fixtures/invalid_patterns.txt`
- Create: `crates/stix-pattern/tests/conformance.rs`

- [ ] **Step 1: Create the valid-pattern fixtures**

Create `crates/stix-pattern/tests/fixtures/valid_patterns.txt` (one pattern per line; lines starting with `#` are comments):

```
# Simple comparisons
[ipv4-addr:value = '198.51.100.1']
[file:size > 1024]
[file:name != 'benign.txt']
[domain-name:value LIKE '%.evil.example']
[email-message:subject MATCHES 'invoice[0-9]+']
# Boolean logic
[file:name = 'a' AND file:size = 1]
[file:name = 'a' OR file:name = 'b']
[(file:name = 'a' OR file:name = 'b') AND file:size = 1]
# NOT and EXISTS
[file:name NOT = 'x']
[EXISTS file:name]
# IN set
[ipv4-addr:value IN ('1.1.1.1', '8.8.8.8', '9.9.9.9')]
# Nested object paths
[file:hashes.'SHA-256' = 'aec070645fe53ee3b3763059376134f058cc337247c978add178b6ccdfb0019f']
[network-traffic:protocols[0] = 'tcp']
[x-custom:list[*] = 'y']
# Typed literals
[file:created = t'2020-01-01T00:00:00Z']
[artifact:payload_bin = b'aGVsbG8=']
[file:magic_number_hex = h'cafebabe']
[file:is_encrypted = true]
# Observation operators
[file:name='a'] AND [file:name='b']
[file:name='a'] OR [file:name='b']
[file:name='a'] FOLLOWEDBY [file:name='b']
([file:name='a'] OR [file:name='b']) FOLLOWEDBY [file:size=1]
# Qualifiers
[file:name='a'] WITHIN 60 SECONDS
[file:name='a'] REPEATS 5 TIMES
[file:name='a'] START t'2020-01-01T00:00:00Z' STOP t'2020-01-02T00:00:00Z'
[file:name='a'] REPEATS 2 TIMES WITHIN 60 SECONDS
```

- [ ] **Step 2: Create the invalid-pattern fixtures**

Create `crates/stix-pattern/tests/fixtures/invalid_patterns.txt`:

```
# Missing brackets
ipv4-addr:value = '1.2.3.4'
# Unterminated string
[ipv4-addr:value = '1.2.3.4]
# Missing operator
[file:name 'x']
# Missing object type
[:value = 'x']
# Bad qualifier (missing SECONDS)
[file:name='a'] WITHIN 60
# Trailing tokens
[file:name='a'] [file:name='b']
# Empty observation
[]
# Dangling AND
[file:name='a'] AND
# Unknown character
[file:name = @]
```

- [ ] **Step 3: Write the conformance test**

Create `crates/stix-pattern/tests/conformance.rs`:

```rust
use stix_pattern::parse;

fn lines(raw: &str) -> impl Iterator<Item = (usize, &str)> {
    raw.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
}

#[test]
fn all_valid_patterns_parse() {
    let raw = include_str!("fixtures/valid_patterns.txt");
    let mut failures = Vec::new();
    for (lineno, pat) in lines(raw) {
        if let Err(e) = parse(pat) {
            failures.push(format!("line {lineno}: `{pat}` -> {e}"));
        }
    }
    assert!(failures.is_empty(), "expected these to parse:\n{}", failures.join("\n"));
}

#[test]
fn all_invalid_patterns_reject() {
    let raw = include_str!("fixtures/invalid_patterns.txt");
    let mut leaks = Vec::new();
    for (lineno, pat) in lines(raw) {
        if parse(pat).is_ok() {
            leaks.push(format!("line {lineno}: `{pat}` should have failed but parsed"));
        }
    }
    assert!(leaks.is_empty(), "expected these to be rejected:\n{}", leaks.join("\n"));
}
```

- [ ] **Step 4: Run the conformance tests**

Run: `cargo test -p stix-pattern --test conformance`
Expected: PASS — every valid pattern parses; every invalid pattern is rejected. If a valid pattern fails or an invalid one slips through, fix the parser/lexer (not the test) unless the fixture line is genuinely wrong per the grammar.

- [ ] **Step 5: Commit**

```bash
git add crates/stix-pattern/tests/conformance.rs crates/stix-pattern/tests/fixtures/
git commit -m "test(pattern): add valid/invalid conformance corpus"
```

---

## Task 10: Lint, format, and final verification

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt --all`
Then review the diff: `git diff`.

- [ ] **Step 2: Clippy (treat warnings as errors)**

Run: `cargo clippy -p stix-pattern --all-targets -- -D warnings`
Expected: no warnings. Fix any that appear (e.g. needless clones, `match` that should be `if let`).

- [ ] **Step 3: Full test run**

Run: `cargo test -p stix-pattern`
Expected: all unit, integration, doc, and conformance tests PASS.

- [ ] **Step 4: Commit any fmt/clippy fixes**

```bash
git add -A
git commit -m "chore(pattern): fmt + clippy clean"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** This plan implements the `stix-pattern` crate from the spec —
  lexer, recursive-descent parser, full grammar (comparisons, AND/OR/FOLLOWEDBY,
  WITHIN/REPEATS/START..STOP), serde-serializable AST, `ParseError` with spans, and a
  conformance corpus. Object model, matcher, and umbrella crate are explicitly out of
  scope for this plan (Plans 2 and 3).
- **Type consistency:** `parse()` (free fn) and `Parser` (pub(crate)) are used
  consistently across Tasks 5–8; AST type/field names (`ObservationExpression`,
  `ComparisonExpression`, `Comparison.path/operator/negated/value`,
  `ObjectPath.object_type/steps`, `PathStep`, `Literal`, `Qualifier`) match between
  Task 3 and their uses in Tasks 5–7.
- **Stub callout:** Task 5 introduces a temporary `parse()` stub explicitly replaced in
  Task 7 — flagged so an out-of-order reader isn't surprised.
- **No placeholders:** every code step contains complete, compilable code.
```
