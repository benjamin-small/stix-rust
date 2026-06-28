//! Recursive-descent parser: tokens -> AST.

use crate::ast::{
    Comparison, ComparisonExpression, ComparisonOperand, ComparisonOperator, Literal, ObjectPath,
    PathStep, Pattern,
};
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
        Ok(ComparisonExpression::Test(Comparison {
            path,
            operator,
            negated,
            value,
        }))
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

    use crate::ast::{
        Comparison, ComparisonExpression, ComparisonOperand, ComparisonOperator, Literal,
    };

    fn parse_comp(src: &str) -> ComparisonExpression {
        let toks = crate::lexer::tokenize(src).unwrap();
        let mut p = Parser::new(&toks, src);
        p.parse_comparison_expression().unwrap()
    }

    #[test]
    fn parses_single_comparison() {
        let c = parse_comp("ipv4-addr:value = '1.2.3.4'");
        match c {
            ComparisonExpression::Test(Comparison {
                operator,
                negated,
                value,
                ..
            }) => {
                assert_eq!(operator, ComparisonOperator::Equal);
                assert!(!negated);
                assert_eq!(
                    value,
                    ComparisonOperand::Literal(Literal::String("1.2.3.4".into()))
                );
            }
            _ => panic!("expected a single test"),
        }
    }

    #[test]
    fn parses_not_operator() {
        let c = parse_comp("file:size != 0");
        match c {
            ComparisonExpression::Test(Comparison {
                operator, value, ..
            }) => {
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
            ComparisonExpression::Test(Comparison {
                operator, value, ..
            }) => {
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
}
