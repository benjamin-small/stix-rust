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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ObservationExpression {
    /// A single `[ comparisonExpr ]` observation.
    Observation {
        /// The comparison expression inside the brackets.
        expression: Box<ComparisonExpression>,
        /// Byte range of the whole `[...]` including the brackets.
        span: Span,
    },
    /// `AND` of two observation expressions.
    And(Box<ObservationExpression>, Box<ObservationExpression>),
    /// `OR` of two observation expressions.
    Or(Box<ObservationExpression>, Box<ObservationExpression>),
    /// `FOLLOWEDBY`: the left expression's observations precede the right's.
    FollowedBy(Box<ObservationExpression>, Box<ObservationExpression>),
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComparisonExpression {
    /// A single property test.
    Test(Comparison),
    /// `AND` of two comparison expressions.
    And(Box<ComparisonExpression>, Box<ComparisonExpression>),
    /// `OR` of two comparison expressions.
    Or(Box<ComparisonExpression>, Box<ComparisonExpression>),
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
        Pattern {
            expression: strip_observation_spans(&self.expression),
        }
    }
}

fn strip_observation_spans(e: &ObservationExpression) -> ObservationExpression {
    use ObservationExpression as O;
    match e {
        O::Observation { expression, .. } => O::Observation {
            expression: Box::new(strip_comparison_spans(expression)),
            span: ZERO_SPAN,
        },
        O::And(l, r) => O::And(
            Box::new(strip_observation_spans(l)),
            Box::new(strip_observation_spans(r)),
        ),
        O::Or(l, r) => O::Or(
            Box::new(strip_observation_spans(l)),
            Box::new(strip_observation_spans(r)),
        ),
        O::FollowedBy(l, r) => O::FollowedBy(
            Box::new(strip_observation_spans(l)),
            Box::new(strip_observation_spans(r)),
        ),
        O::Qualified {
            expression,
            qualifier,
            ..
        } => O::Qualified {
            expression: Box::new(strip_observation_spans(expression)),
            qualifier: qualifier.clone(),
            span: ZERO_SPAN,
        },
    }
}

fn strip_comparison_spans(e: &ComparisonExpression) -> ComparisonExpression {
    use ComparisonExpression as C;
    match e {
        C::Test(c) => C::Test(Comparison {
            path: ObjectPath {
                object_type: c.path.object_type.clone(),
                steps: c.path.steps.clone(),
                span: ZERO_SPAN,
            },
            operator: c.operator,
            negated: c.negated,
            value: c.value.clone(),
            span: ZERO_SPAN,
        }),
        C::And(l, r) => C::And(
            Box::new(strip_comparison_spans(l)),
            Box::new(strip_comparison_spans(r)),
        ),
        C::Or(l, r) => C::Or(
            Box::new(strip_comparison_spans(l)),
            Box::new(strip_comparison_spans(r)),
        ),
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

    #[test]
    fn ast_round_trips_through_json() {
        let lit = Literal::Integer(42);
        let json = serde_json::to_string(&lit).unwrap();
        let back: Literal = serde_json::from_str(&json).unwrap();
        assert_eq!(lit, back);
    }
}
