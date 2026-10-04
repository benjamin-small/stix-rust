//! Iterative parser using an explicit stack: tokens -> AST. Nesting is capped at [`MAX_NESTING`].

use crate::ast::{
    Comparison, ComparisonExpression, ComparisonOperand, ComparisonOperator, Literal, ObjectPath,
    ObservationExpression, PathStep, Pattern, Qualifier,
};
use crate::error::{ParseError, Result, Span};
use crate::lexer::{Token, TokenKind};

/// The deepest nesting [`parse`] accepts; deeper patterns are rejected with the
/// error "pattern nests too deeply".
///
/// Nesting is counted along each path from the root of the pattern to a leaf
/// as the number of parenthesized groups (at the observation level and inside
/// `[...]` alike) plus the number of qualifiers (`WITHIN`, `REPEATS`,
/// `START..STOP`), since each qualifier wraps its expression in another node.
/// Chains such as `a OR b OR c` add no nesting however long they are: they
/// parse into a single n-ary node.
///
/// The parser itself does not recurse per group. Lowering, the matcher, and
/// the AST's derived `Clone`, `PartialEq`, `Debug`, `Serialize` and drop glue
/// do recurse over nesting depth, so this bound is what keeps them within the
/// stack.
///
/// # How the value was chosen
///
/// By measurement, in a debug build on a thread with a 256 KiB stack (well
/// below wasm's 1 MiB default). The pipeline is parse, lower, validate,
/// render, `serde_json::to_string` of the AST, matching, `without_spans` and
/// drop. It was run on the worst shapes: observation-level groups with a
/// `FOLLOWEDBY`, an `OR` and an `AND` node between each pair of groups;
/// comparison-level groups; the two mixed; stacked qualifiers; and qualified
/// groups. The deepest shape survived about 56 levels, limited by
/// `serde_json` serialization of the AST. Every other stage and shape went
/// deeper. 40 keeps roughly 40% headroom for other platforms and toolchains.
/// `stix-matcher`'s `tests/deep_nesting.rs` reruns these shapes at this limit.
///
/// Real patterns nest a handful of levels at most.
pub const MAX_NESTING: usize = 40;

/// A parsed expression with its nesting height: the most groups and
/// qualifiers on any path from it down to a leaf.
type Parsed<T> = (T, usize);

/// An expression type parsed by [`Parser::parse_infix`]: its binary
/// operators, by precedence level (level 0 binds loosest), and its chain nodes.
trait Infix: Sized {
    /// Number of precedence levels.
    const LEVELS: usize;
    /// What `expect` reports when a group is not closed.
    const CLOSE_GROUP: &'static str;
    /// The precedence level of `token`, if it is one of this type's operators.
    fn level_of(token: &TokenKind) -> Option<usize>;
    /// The chain node at `level` with these operands (at least two).
    fn chain(level: usize, operands: Vec<Self>) -> Self;
    /// The operands of `self` if it is a chain node at `level`, else `self`.
    fn into_operands(self, level: usize) -> std::result::Result<Vec<Self>, Self>;
}

impl Infix for ObservationExpression {
    const LEVELS: usize = 3;
    const CLOSE_GROUP: &'static str = "')' to close grouped observation";

    fn level_of(token: &TokenKind) -> Option<usize> {
        match token {
            TokenKind::FollowedBy => Some(0),
            TokenKind::Or => Some(1),
            TokenKind::And => Some(2),
            _ => None,
        }
    }

    fn chain(level: usize, operands: Vec<Self>) -> Self {
        match level {
            0 => ObservationExpression::FollowedBy(operands),
            1 => ObservationExpression::Or(operands),
            _ => ObservationExpression::And(operands),
        }
    }

    fn into_operands(self, level: usize) -> std::result::Result<Vec<Self>, Self> {
        match (level, self) {
            (0, ObservationExpression::FollowedBy(xs))
            | (1, ObservationExpression::Or(xs))
            | (2, ObservationExpression::And(xs)) => Ok(xs),
            (_, e) => Err(e),
        }
    }
}

impl Infix for ComparisonExpression {
    const LEVELS: usize = 2;
    const CLOSE_GROUP: &'static str = "')' to close comparison group";

    fn level_of(token: &TokenKind) -> Option<usize> {
        match token {
            TokenKind::Or => Some(0),
            TokenKind::And => Some(1),
            _ => None,
        }
    }

    fn chain(level: usize, operands: Vec<Self>) -> Self {
        match level {
            0 => ComparisonExpression::Or(operands),
            _ => ComparisonExpression::And(operands),
        }
    }

    fn into_operands(self, level: usize) -> std::result::Result<Vec<Self>, Self> {
        match (level, self) {
            (0, ComparisonExpression::Or(xs)) | (1, ComparisonExpression::And(xs)) => Ok(xs),
            (_, e) => Err(e),
        }
    }
}

/// The top level or one open group in [`Parser::parse_infix`].
struct Frame<T> {
    /// Per precedence level, the operands so far of the chain being built at
    /// that level. A list is non-empty only while its operator is pending.
    chains: Vec<Vec<T>>,
    /// The greatest nesting height among the operands seen so far.
    height: usize,
}

impl<T: Infix> Frame<T> {
    fn new() -> Self {
        Frame {
            chains: (0..T::LEVELS).map(|_| Vec::new()).collect(),
            height: 0,
        }
    }

    /// Add `operand`, which an operator at `level` follows.
    fn push(&mut self, level: usize, operand: T) {
        let operand = self.close_above(level, operand);
        let chain = &mut self.chains[level];
        if chain.is_empty() {
            // The chain's first operand: splice in a left-nested chain of
            // the same operator, as in `(a OR b) OR c`.
            match operand.into_operands(level) {
                Ok(operands) => *chain = operands,
                Err(operand) => chain.push(operand),
            }
        } else {
            chain.push(operand);
        }
    }

    /// The whole expression, of which `operand` is the last operand.
    fn finish(&mut self, operand: T) -> T {
        let operand = self.close_above(0, operand);
        Self::close(0, std::mem::take(&mut self.chains[0]), operand)
    }

    /// Complete the chains at levels above `level`, of which `operand` is the
    /// last operand, giving the operand they form at `level`.
    fn close_above(&mut self, level: usize, mut operand: T) -> T {
        for l in (level + 1..T::LEVELS).rev() {
            operand = Self::close(l, std::mem::take(&mut self.chains[l]), operand);
        }
        operand
    }

    /// The chain at `level` of `operands` then `last`, or just `last` when no
    /// operator at that level was seen.
    fn close(level: usize, mut operands: Vec<T>, last: T) -> T {
        if operands.is_empty() {
            return last;
        }
        operands.push(last);
        T::chain(level, operands)
    }
}

pub(crate) struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    /// Length of the source string, used to build EOF spans.
    src_len: usize,
    /// Number of parenthesized groups currently open around the cursor.
    open_groups: usize,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(tokens: &'a [Token], src: &'a str) -> Self {
        Parser {
            tokens,
            pos: 0,
            src_len: src.len(),
            open_groups: 0,
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

    /// Span of the most recently consumed token, or a zero span at the start.
    fn prev_span(&self) -> Span {
        match self.pos.checked_sub(1).and_then(|i| self.tokens.get(i)) {
            Some(t) => t.span,
            None => Span::new(0, 0),
        }
    }

    /// A span from `start` to the end of the most recently consumed token.
    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.prev_span().end)
    }

    // --- object path ---

    pub(crate) fn parse_object_path(&mut self) -> Result<ObjectPath> {
        let start = self.current_span().start;
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
        Ok(ObjectPath {
            object_type,
            steps,
            span: self.span_from(start),
        })
    }

    fn parse_key_component(&mut self) -> Result<String> {
        match self.advance().map(|t| &t.kind) {
            Some(TokenKind::Identifier(s)) => Ok(s.clone()),
            Some(TokenKind::String(s)) => Ok(s.clone()),
            _ => Err(self.error_here("expected property name")),
        }
    }

    // --- infix expressions ---

    /// Error unless one more level of nesting fits around an expression of
    /// nesting `height`. Reported at the current token.
    fn check_nesting(&self, height: usize) -> Result<()> {
        if self.open_groups + height + 1 > MAX_NESTING {
            Err(self.error_here("pattern nests too deeply"))
        } else {
            Ok(())
        }
    }

    /// Parse an infix expression of `T` operands: `operand (op operand)*`
    /// with the operators and precedences `T` defines, and parenthesized
    /// groups. `leaf` parses an operand that is not a group; `postfix` applies
    /// any postfix clauses (qualifiers) to each operand, group or leaf.
    ///
    /// This does not recurse per group: open groups are frames on an explicit
    /// stack, so the parser's own stack use does not grow with nesting.
    ///
    /// Each chain becomes one n-ary node. If a chain's first operand is
    /// already a node of the same operator (a group, as in `(a OR b) OR c`),
    /// its operands are spliced in. Later operands are never flattened, so
    /// `a OR (b OR c)` keeps its inner node.
    fn parse_infix<T: Infix>(
        &mut self,
        mut leaf: impl FnMut(&mut Self) -> Result<Parsed<T>>,
        mut postfix: impl FnMut(&mut Self, Parsed<T>) -> Result<Parsed<T>>,
    ) -> Result<Parsed<T>> {
        let mut frames: Vec<Frame<T>> = vec![Frame::new()];
        loop {
            // Expecting an operand: open any groups, then parse a leaf.
            while self.peek() == Some(&TokenKind::LParen) {
                self.check_nesting(0)?;
                self.advance();
                self.open_groups += 1;
                frames.push(Frame::new());
            }
            let parsed = leaf(self)?;
            let (mut operand, mut height) = postfix(self, parsed)?;
            // After an operand: an operator continues the current group;
            // anything else ends it.
            loop {
                let frame = frames
                    .last_mut()
                    .expect("the top-level frame is never popped");
                frame.height = frame.height.max(height);
                if let Some(level) = self.peek().and_then(T::level_of) {
                    self.advance();
                    frame.push(level, operand);
                    break;
                }
                let expr = frame.finish(operand);
                let inner_height = frame.height;
                if frames.len() == 1 {
                    return Ok((expr, inner_height));
                }
                self.expect(&TokenKind::RParen, T::CLOSE_GROUP)?;
                frames.pop();
                self.open_groups -= 1;
                (operand, height) = postfix(self, (expr, inner_height + 1))?;
            }
        }
    }

    // --- comparison expressions ---

    pub(crate) fn parse_comparison_expression(&mut self) -> Result<Parsed<ComparisonExpression>> {
        self.parse_infix(
            |p: &mut Self| Ok((p.parse_prop_test()?, 0)),
            |_, parsed| Ok(parsed),
        )
    }

    fn parse_prop_test(&mut self) -> Result<ComparisonExpression> {
        let start = self.current_span().start;

        // EXISTS objectPath
        if self.eat(&TokenKind::Exists) {
            let path = self.parse_object_path()?;
            let test = ComparisonExpression::Test(Comparison {
                path,
                operator: ComparisonOperator::Exists,
                negated: false,
                // operand unused for EXISTS; use a benign placeholder.
                value: ComparisonOperand::Literal(Literal::Boolean(true)),
                span: self.span_from(start),
            });
            return Ok(test);
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
        let test = ComparisonExpression::Test(Comparison {
            path,
            operator,
            negated,
            value,
            span: self.span_from(start),
        });
        Ok(test)
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

    // --- observation expressions ---

    pub(crate) fn parse_observation_expression(&mut self) -> Result<Parsed<ObservationExpression>> {
        self.parse_infix(Self::parse_observation, Self::parse_qualifiers)
    }

    /// Apply any qualifiers that follow an operand. Each one wraps the
    /// expression in another node, so each counts toward the nesting limit.
    fn parse_qualifiers(
        &mut self,
        (mut expr, mut height): Parsed<ObservationExpression>,
    ) -> Result<Parsed<ObservationExpression>> {
        loop {
            let q_start = self.current_span().start;
            if matches!(
                self.peek(),
                Some(TokenKind::Within | TokenKind::Repeats | TokenKind::Start)
            ) {
                // Each qualifier wraps the expression in one more node.
                self.check_nesting(height)?;
            }
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
                span: self.span_from(q_start),
            };
            height += 1;
        }
        Ok((expr, height))
    }

    /// Parse one `[ comparisonExpr ]` observation. (Groups are handled by
    /// [`parse_infix`](Self::parse_infix), so only `[` can start one here.)
    fn parse_observation(&mut self) -> Result<Parsed<ObservationExpression>> {
        let start = self.current_span().start;
        if !self.eat(&TokenKind::LBracket) {
            return Err(self.error_here("expected '[' or '(' to start an observation"));
        }
        let (comp, height) = self.parse_comparison_expression()?;
        self.expect(&TokenKind::RBracket, "']' to close observation")?;
        let observation = ObservationExpression::Observation {
            expression: Box::new(comp),
            span: self.span_from(start),
        };
        Ok((observation, height))
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
}

/// Parse a complete pattern string into an AST.
pub fn parse(src: &str) -> Result<Pattern> {
    let tokens = crate::lexer::tokenize(src)?;
    let mut parser = Parser::new(&tokens, src);
    let (expression, _height) = parser.parse_observation_expression()?;
    if !parser.at_end() {
        return Err(parser.error_here("unexpected trailing tokens after pattern"));
    }
    Ok(Pattern { expression })
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
            vec![PathStep::Key("protocols".to_string()), PathStep::Index(0)]
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
        p.parse_comparison_expression().unwrap().0
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
            ComparisonExpression::Or(xs) => {
                assert_eq!(xs.len(), 2);
                assert!(matches!(xs[0], ComparisonExpression::Test(_)));
                assert!(
                    matches!(&xs[1], ComparisonExpression::And(ys) if ys.len() == 2),
                    "right side of OR should be an AND"
                );
            }
            _ => panic!("top should be OR"),
        }
    }

    #[test]
    fn parses_parenthesized_comparison() {
        let c = parse_comp("(file:name = 'a' OR file:name = 'b') AND file:size = 1");
        match c {
            ComparisonExpression::And(xs) => {
                assert_eq!(xs.len(), 2);
                assert!(
                    matches!(&xs[0], ComparisonExpression::Or(ys) if ys.len() == 2),
                    "left of AND should be OR"
                );
                assert!(matches!(xs[1], ComparisonExpression::Test(_)));
            }
            _ => panic!("top should be AND"),
        }
    }

    fn strip_comp(src: &str) -> ComparisonExpression {
        // Normalize spans by wrapping the comparison in an observation.
        let p = parse(&format!("[{src}]")).unwrap().without_spans();
        match p.expression {
            ObservationExpression::Observation { expression, .. } => *expression,
            _ => unreachable!(),
        }
    }

    #[test]
    fn comparison_chain_is_one_node() {
        let c = parse_comp("x:v = 1 OR x:v = 2 OR x:v = 3 OR x:v = 4");
        assert!(matches!(&c, ComparisonExpression::Or(xs) if xs.len() == 4));
        let c = parse_comp("x:v = 1 AND x:v = 2 AND x:v = 3");
        assert!(matches!(&c, ComparisonExpression::And(xs) if xs.len() == 3));
    }

    #[test]
    fn comparison_left_nested_group_is_flattened() {
        let flat = strip_comp("x:v = 1 OR x:v = 2 OR x:v = 3");
        assert_eq!(strip_comp("(x:v = 1 OR x:v = 2) OR x:v = 3"), flat);
        assert_eq!(strip_comp("((x:v = 1 OR x:v = 2)) OR x:v = 3"), flat);
        assert_eq!(
            strip_comp("(x:v = 1 AND x:v = 2) AND x:v = 3"),
            strip_comp("x:v = 1 AND x:v = 2 AND x:v = 3")
        );
    }

    #[test]
    fn comparison_right_nested_group_is_kept() {
        let c = strip_comp("x:v = 1 OR (x:v = 2 OR x:v = 3)");
        match c {
            ComparisonExpression::Or(xs) => {
                assert_eq!(xs.len(), 2);
                assert!(matches!(&xs[1], ComparisonExpression::Or(ys) if ys.len() == 2));
            }
            _ => panic!("top should be OR"),
        }
    }

    use crate::ast::{ObservationExpression, Qualifier};
    use crate::parser::parse;

    #[test]
    fn parses_single_observation() {
        let p = parse("[ipv4-addr:value = '1.2.3.4']").unwrap();
        match p.expression {
            ObservationExpression::Observation { .. } => {}
            _ => panic!("expected single observation"),
        }
    }

    #[test]
    fn observation_and_binds_tighter_than_or() {
        // [a] OR [b] AND [c] => [a] OR ([b] AND [c])
        let p = parse("[file:name='a'] OR [file:name='b'] AND [file:size=1]").unwrap();
        match p.expression {
            ObservationExpression::Or(xs) => {
                assert_eq!(xs.len(), 2);
                assert!(matches!(xs[0], ObservationExpression::Observation { .. }));
                assert!(
                    matches!(&xs[1], ObservationExpression::And(ys) if ys.len() == 2),
                    "right of OR should be AND"
                );
            }
            _ => panic!("top should be OR"),
        }
    }

    #[test]
    fn followedby_is_loosest() {
        // [a] FOLLOWEDBY [b] OR [c] => [a] FOLLOWEDBY ([b] OR [c])
        let p = parse("[file:name='a'] FOLLOWEDBY [file:name='b'] OR [file:name='c']").unwrap();
        match p.expression {
            ObservationExpression::FollowedBy(xs) => {
                assert_eq!(xs.len(), 2);
                assert!(matches!(xs[0], ObservationExpression::Observation { .. }));
                assert!(
                    matches!(&xs[1], ObservationExpression::Or(ys) if ys.len() == 2),
                    "right of FOLLOWEDBY should be OR"
                );
            }
            _ => panic!("top should be FOLLOWEDBY"),
        }
    }

    #[test]
    fn parses_parenthesized_observation() {
        let p = parse("([file:name='a'] OR [file:name='b']) FOLLOWEDBY [file:size=1]").unwrap();
        match p.expression {
            ObservationExpression::FollowedBy(xs) => {
                assert_eq!(xs.len(), 2);
                assert!(
                    matches!(&xs[0], ObservationExpression::Or(ys) if ys.len() == 2),
                    "left should be OR"
                );
                assert!(matches!(xs[1], ObservationExpression::Observation { .. }));
            }
            _ => panic!("top should be FOLLOWEDBY"),
        }
    }

    fn stripped(src: &str) -> ObservationExpression {
        parse(src).unwrap().without_spans().expression
    }

    #[test]
    fn observation_chains_are_one_node() {
        let p = stripped("[x:v=1] OR [x:v=2] OR [x:v=3]");
        assert!(matches!(&p, ObservationExpression::Or(xs) if xs.len() == 3));
        let p = stripped("[x:v=1] AND [x:v=2] AND [x:v=3] AND [x:v=4]");
        assert!(matches!(&p, ObservationExpression::And(xs) if xs.len() == 4));
        let p = stripped("[x:v=1] FOLLOWEDBY [x:v=2] FOLLOWEDBY [x:v=3]");
        assert!(matches!(&p, ObservationExpression::FollowedBy(xs) if xs.len() == 3));
    }

    #[test]
    fn observation_left_nested_group_is_flattened() {
        for op in ["OR", "AND", "FOLLOWEDBY"] {
            let flat = stripped(&format!("[x:v=1] {op} [x:v=2] {op} [x:v=3]"));
            assert_eq!(
                stripped(&format!("([x:v=1] {op} [x:v=2]) {op} [x:v=3]")),
                flat,
                "{op}"
            );
            assert_eq!(
                stripped(&format!("(([x:v=1] {op} [x:v=2])) {op} [x:v=3]")),
                flat,
                "{op}"
            );
        }
    }

    #[test]
    fn observation_right_nested_group_is_kept() {
        let p = stripped("[x:v=1] FOLLOWEDBY ([x:v=2] FOLLOWEDBY [x:v=3])");
        match p {
            ObservationExpression::FollowedBy(xs) => {
                assert_eq!(xs.len(), 2);
                assert!(matches!(&xs[1], ObservationExpression::FollowedBy(ys) if ys.len() == 2));
            }
            _ => panic!("top should be FOLLOWEDBY"),
        }
    }

    #[test]
    fn qualified_group_is_not_flattened() {
        let p = stripped("([x:v=1] OR [x:v=2]) WITHIN 5 SECONDS OR [x:v=3]");
        match p {
            ObservationExpression::Or(xs) => {
                assert_eq!(xs.len(), 2);
                assert!(matches!(xs[0], ObservationExpression::Qualified { .. }));
            }
            _ => panic!("top should be OR"),
        }
    }

    /// Run `f` on a thread with a small (256 KiB) stack, as a stand-in for
    /// constrained hosts such as wasm.
    fn on_small_stack<F: FnOnce() + Send + 'static>(f: F) {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(f)
            .unwrap()
            .join()
            .unwrap();
    }

    const LEAF: &str = "[x:v = 1]";

    fn obs_parens(n: usize) -> String {
        format!("{}{LEAF}{}", "(".repeat(n), ")".repeat(n))
    }

    fn cmp_parens(n: usize) -> String {
        format!("[{}x:v = 1{}]", "(".repeat(n), ")".repeat(n))
    }

    fn qualifiers(n: usize) -> String {
        format!("{LEAF}{}", " REPEATS 2 TIMES".repeat(n))
    }

    fn assert_too_deep(src: &str, at: usize, token: &str) {
        let err = parse(src).unwrap_err();
        assert_eq!(err.message, "pattern nests too deeply");
        assert_eq!(err.span.start, at, "error span: {:?}", err.span);
        assert_eq!(&src[err.span.start..err.span.end], token);
    }

    #[test]
    fn nesting_at_the_limit_parses() {
        parse(&obs_parens(MAX_NESTING)).unwrap();
        parse(&cmp_parens(MAX_NESTING)).unwrap();
        parse(&qualifiers(MAX_NESTING)).unwrap();
        // Observation-level and comparison-level groups share one budget.
        let h = MAX_NESTING / 2;
        let mixed = format!(
            "{}[{}x:v = 1{}]{}",
            "(".repeat(h),
            "(".repeat(MAX_NESTING - h),
            ")".repeat(MAX_NESTING - h),
            ")".repeat(h)
        );
        parse(&mixed).unwrap();
        let mixed_over = format!("({mixed})");
        assert_eq!(
            parse(&mixed_over).unwrap_err().message,
            "pattern nests too deeply"
        );
    }

    #[test]
    fn nesting_past_the_limit_is_an_error_at_the_crossing_token() {
        // The (MAX_NESTING + 1)-th '(' is where the limit is crossed.
        assert_too_deep(&obs_parens(MAX_NESTING + 1), MAX_NESTING, "(");
        assert_too_deep(&cmp_parens(MAX_NESTING + 1), MAX_NESTING + 1, "(");
        let src = qualifiers(MAX_NESTING + 1);
        let last = src.rfind("REPEATS").unwrap();
        assert_too_deep(&src, last, "REPEATS");
    }

    #[test]
    fn qualifiers_count_together_with_enclosing_groups() {
        // A group plus its qualifier is two levels.
        let level = |inner: &str| format!("({inner}) WITHIN 1 SECONDS");
        let mut ok = LEAF.to_string();
        for _ in 0..MAX_NESTING / 2 {
            ok = level(&ok);
        }
        parse(&ok).unwrap();
        let err = parse(&level(&ok)).unwrap_err();
        assert_eq!(err.message, "pattern nests too deeply");
        // Qualifiers stacked on successive groups cannot sneak past the limit,
        // even though only a few are ever stacked on one expression.
        let mut src = LEAF.to_string();
        for _ in 0..MAX_NESTING {
            src = format!("({src}) WITHIN 1 SECONDS WITHIN 1 SECONDS");
        }
        assert_eq!(parse(&src).unwrap_err().message, "pattern nests too deeply");
    }

    #[test]
    fn huge_nesting_is_an_error_not_a_crash() {
        on_small_stack(|| {
            for n in [10_000, 100_000] {
                for src in [obs_parens(n), cmp_parens(n), qualifiers(n)] {
                    let err = parse(&src).unwrap_err();
                    assert_eq!(err.message, "pattern nests too deeply");
                }
            }
        });
    }

    #[test]
    fn parses_within_qualifier() {
        let p = parse("[file:name='a'] REPEATS 2 TIMES WITHIN 60 SECONDS").unwrap();
        // Outermost qualifier is the last one parsed (WITHIN), wrapping REPEATS.
        match p.expression {
            ObservationExpression::Qualified {
                qualifier: Qualifier::Within { seconds },
                expression,
                ..
            } => {
                assert_eq!(seconds, 60.0);
                match *expression {
                    ObservationExpression::Qualified {
                        qualifier: Qualifier::Repeats { count },
                        ..
                    } => {
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
        let p = parse("[file:name='a'] START t'2020-01-01T00:00:00Z' STOP t'2020-01-02T00:00:00Z'")
            .unwrap();
        match p.expression {
            ObservationExpression::Qualified {
                qualifier: Qualifier::StartStop { start, stop },
                ..
            } => {
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

    #[test]
    fn records_spans_on_comparison_and_path() {
        let src = "[file:size > 1024]";
        let p = parse(src).unwrap();
        let ObservationExpression::Observation { expression, span } = &p.expression else {
            panic!("expected an observation");
        };
        // The observation span covers the whole `[...]` including brackets.
        assert_eq!(&src[span.start..span.end], "[file:size > 1024]");
        let ComparisonExpression::Test(c) = expression.as_ref() else {
            panic!("expected a test");
        };
        // The comparison span covers the property test, without the brackets.
        assert_eq!(&src[c.span.start..c.span.end], "file:size > 1024");
        // The path span covers only the object path.
        assert_eq!(&src[c.path.span.start..c.path.span.end], "file:size");
    }

    #[test]
    fn records_span_on_qualifier_clause() {
        let src = "[file:name='a'] WITHIN 60 SECONDS";
        let p = parse(src).unwrap();
        let ObservationExpression::Qualified { span, .. } = &p.expression else {
            panic!("expected a qualified expression");
        };
        assert_eq!(&src[span.start..span.end], "WITHIN 60 SECONDS");
    }

    #[test]
    fn without_spans_zeroes_every_span() {
        let a = parse("[file:size > 1024] WITHIN 60 SECONDS").unwrap();
        let b = parse("   [file:size > 1024]   WITHIN 60 SECONDS   ").unwrap();
        assert_ne!(a, b, "differing offsets should make the raw ASTs unequal");
        assert_eq!(
            a.without_spans(),
            b.without_spans(),
            "structurally identical patterns should compare equal without spans"
        );
    }
}
