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
