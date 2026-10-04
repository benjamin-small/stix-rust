//! Abstract syntax tree for STIX 2.1 patterns. All nodes are serde-serializable.

use serde::{Deserialize, Serialize};

use crate::error::Span;

/// A complete parsed pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    /// The top-level observation expression.
    pub expression: ObservationExpression,
}

/// Observation-level expression tree.
/// `FOLLOWEDBY`/`AND`/`OR` combine observations; qualifiers attach to a sub-expression.
///
/// # Chain nodes
///
/// `And`, `Or` and `FollowedBy` are n-ary: a chain such as `a OR b OR c` is a
/// single node holding every operand in source order, so the tree's depth does
/// not grow with the length of a chain. Nodes produced by [`parse`](crate::parse)
/// uphold two invariants:
///
/// - the operand list has **at least two** elements;
/// - the **first** operand is never a node of the same operator. Left-nested
///   chains are flattened (`(a OR b) OR c` and `a OR b OR c` are both
///   `Or([a, b, c])`), while a right-nested group keeps its own node
///   (`a OR (b OR c)` is `Or([a, Or([b, c])])`).
///
/// A chain `Op([a, b, c])` means the left-associative `(a op b) op c`.
///
/// In JSON a two-operand chain serializes as `{"Or": [l, r]}`, and longer
/// chains as longer arrays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ObservationExpression {
    /// A single `[ comparisonExpr ]` observation.
    Observation {
        /// The comparison expression inside the brackets.
        expression: Box<ComparisonExpression>,
        /// Byte range of the whole `[...]` including the brackets.
        span: Span,
    },
    /// `AND` of two or more observation expressions (see [chain nodes](Self#chain-nodes)).
    And(Vec<ObservationExpression>),
    /// `OR` of two or more observation expressions (see [chain nodes](Self#chain-nodes)).
    Or(Vec<ObservationExpression>),
    /// `FOLLOWEDBY` of two or more observation expressions: each operand's
    /// observations precede the next's (see [chain nodes](Self#chain-nodes)).
    FollowedBy(Vec<ObservationExpression>),
    /// A sub-expression with a postfix [`Qualifier`] attached.
    Qualified {
        /// The qualified sub-expression.
        expression: Box<ObservationExpression>,
        /// The attached qualifier.
        qualifier: Qualifier,
        /// Byte range of the qualifier clause only (e.g. `WITHIN 60 SECONDS`).
        span: Span,
    },
}

/// Postfix qualifier on an observation expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Qualifier {
    /// `WITHIN <seconds> SECONDS`
    Within {
        /// The window length in seconds.
        seconds: f64,
    },
    /// `REPEATS <count> TIMES`
    Repeats {
        /// The required repetition count.
        count: u64,
    },
    /// `START <start> STOP <stop>` (RFC3339 timestamps, kept as strings here)
    StartStop {
        /// The inclusive window start timestamp.
        start: String,
        /// The exclusive window stop timestamp.
        stop: String,
    },
}

/// Comparison-level expression tree (inside `[ ]`).
///
/// `And` and `Or` are n-ary chain nodes with the same invariants as
/// [`ObservationExpression`]'s: at least two operands, and the first operand is
/// never a node of the same operator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComparisonExpression {
    /// A single property test.
    Test(Comparison),
    /// `AND` of two or more comparison expressions.
    And(Vec<ComparisonExpression>),
    /// `OR` of two or more comparison expressions.
    Or(Vec<ComparisonExpression>),
}

/// A single property test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// The object path on the left-hand side.
    pub path: ObjectPath,
    /// The comparison operator.
    pub operator: ComparisonOperator,
    /// `true` if a `NOT` preceded the operator.
    pub negated: bool,
    /// The right-hand-side operand.
    pub value: ComparisonOperand,
    /// Byte range of the whole property test in the source pattern.
    pub span: Span,
}

/// Right-hand side of a comparison: either a single literal or a set (for `IN`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComparisonOperand {
    /// A single literal value.
    Literal(Literal),
    /// A parenthesized set of literals (for `IN`).
    Set(Vec<Literal>),
}

/// The operator of a property test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOperator {
    /// `=`
    Equal,
    /// `!=` or `<>`
    NotEqual,
    /// `>`
    GreaterThan,
    /// `>=`
    GreaterThanOrEqual,
    /// `<`
    LessThan,
    /// `<=`
    LessThanOrEqual,
    /// `IN`: membership in a set of literals.
    In,
    /// `LIKE`: SQL-style wildcard match (`%`, `_`).
    Like,
    /// `MATCHES`: regular-expression match.
    Matches,
    /// `ISSUBSET`: IP address/range containment.
    IsSubset,
    /// `ISSUPERSET`: inverse IP address/range containment.
    IsSuperset,
    /// `EXISTS objectPath`; for this operator the operand is ignored.
    Exists,
}

/// An object path: `object-type:first.step[0].next`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectPath {
    /// The STIX object type before the colon (e.g. `file`).
    pub object_type: String,
    /// The property steps after the colon, in order.
    pub steps: Vec<PathStep>,
    /// Byte range of the path text in the source pattern.
    pub span: Span,
}

/// One step in an [`ObjectPath`].
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
    /// A single-quoted string literal.
    String(String),
    /// An integer literal.
    Integer(i64),
    /// A floating-point literal.
    Float(f64),
    /// A `true`/`false` literal.
    Boolean(bool),
    /// RFC3339 timestamp from a `t'...'` literal (kept as the inner string).
    Timestamp(String),
    /// Base64 payload from a `b'...'` literal (kept as the inner string).
    Binary(String),
    /// Hex payload from an `h'...'` literal (kept as the inner string).
    Hex(String),
}

/// A span of zero length at offset 0, used as the normalized value by
/// [`Pattern::without_spans`].
const ZERO_SPAN: Span = Span { start: 0, end: 0 };

impl Pattern {
    /// A copy of this pattern with every source span zeroed.
    ///
    /// Two patterns that differ only in whitespace or in where they appeared in
    /// a source string compare equal after this normalization. Used to state
    /// structural equality independent of byte offsets.
    pub fn without_spans(&self) -> Pattern {
        let mut pattern = self.clone();
        zero_observation_spans(&mut pattern.expression);
        pattern
    }
}

// The span-zeroing walks use an explicit stack, so neither nesting depth nor
// chain length costs them any call stack.

fn zero_observation_spans(root: &mut ObservationExpression) {
    use ObservationExpression as O;
    let mut pending = vec![root];
    while let Some(e) = pending.pop() {
        match e {
            O::Observation { expression, span } => {
                *span = ZERO_SPAN;
                zero_comparison_spans(expression);
            }
            O::And(xs) | O::Or(xs) | O::FollowedBy(xs) => pending.extend(xs.iter_mut()),
            O::Qualified {
                expression, span, ..
            } => {
                *span = ZERO_SPAN;
                pending.push(expression);
            }
        }
    }
}

fn zero_comparison_spans(root: &mut ComparisonExpression) {
    use ComparisonExpression as C;
    let mut pending = vec![root];
    while let Some(e) = pending.pop() {
        match e {
            C::Test(c) => {
                c.span = ZERO_SPAN;
                c.path.span = ZERO_SPAN;
            }
            C::And(xs) | C::Or(xs) => pending.extend(xs.iter_mut()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_comparison_pattern() {
        let path = ObjectPath {
            object_type: "ipv4-addr".to_string(),
            steps: vec![PathStep::Key("value".to_string())],
            span: Span::default(),
        };
        let comp = Comparison {
            path,
            operator: ComparisonOperator::Equal,
            negated: false,
            value: ComparisonOperand::Literal(Literal::String("1.2.3.4".to_string())),
            span: Span::default(),
        };
        let pattern = Pattern {
            expression: ObservationExpression::Observation {
                expression: Box::new(ComparisonExpression::Test(comp)),
                span: Span::default(),
            },
        };
        match pattern.expression {
            ObservationExpression::Observation { .. } => {}
            _ => panic!("expected observation"),
        }
    }

    fn obs(value: &str) -> ObservationExpression {
        ObservationExpression::Observation {
            expression: Box::new(ComparisonExpression::Test(Comparison {
                path: ObjectPath {
                    object_type: "x".to_string(),
                    steps: vec![PathStep::Key("v".to_string())],
                    span: Span::default(),
                },
                operator: ComparisonOperator::Equal,
                negated: false,
                value: ComparisonOperand::Literal(Literal::String(value.to_string())),
                span: Span::default(),
            })),
            span: Span::default(),
        }
    }

    /// The JSON of a two-operand chain is exactly what the old binary
    /// `Or(Box, Box)` produced; longer chains are longer arrays.
    #[test]
    fn chain_json_shape_is_pinned() {
        let leaf = |v: &str| serde_json::to_string(&obs(v)).unwrap();
        let two = ObservationExpression::Or(vec![obs("a"), obs("b")]);
        assert_eq!(
            serde_json::to_string(&two).unwrap(),
            format!("{{\"Or\":[{},{}]}}", leaf("a"), leaf("b"))
        );
        let three = ObservationExpression::FollowedBy(vec![obs("a"), obs("b"), obs("c")]);
        assert_eq!(
            serde_json::to_string(&three).unwrap(),
            format!(
                "{{\"FollowedBy\":[{},{},{}]}}",
                leaf("a"),
                leaf("b"),
                leaf("c")
            )
        );
        let back: ObservationExpression =
            serde_json::from_str(&serde_json::to_string(&three).unwrap()).unwrap();
        assert_eq!(back, three);
    }

    #[test]
    fn ast_round_trips_through_json() {
        let lit = Literal::Integer(42);
        let json = serde_json::to_string(&lit).unwrap();
        let back: Literal = serde_json::from_str(&json).unwrap();
        assert_eq!(lit, back);
    }
}
