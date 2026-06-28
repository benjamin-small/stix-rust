//! Evaluation: leaf comparisons, comparison expressions, and observation expressions.

use stix_model::{ObjectStore, ObjectView, StixValue};
use stix_pattern::ast::{Comparison, ComparisonOperand, ComparisonOperator, Literal};

use crate::compare::{value_cmp_literal, value_eq_literal, value_in_set};
use crate::pattern_ops::{like_matches, regex_matches};
use crate::resolve::resolve_path;
use crate::subset::{is_subset, is_superset};

/// Evaluate a single `Comparison` against a single object, dereferencing through
/// `store` where the path requires it. Honors the leaf's `negated` flag.
pub fn eval_comparison(obj: &dyn ObjectView, c: &Comparison, store: Option<&ObjectStore>) -> bool {
    let values = resolve_path(obj, &c.path, store);

    let base = if c.operator == ComparisonOperator::Exists {
        !values.is_empty()
    } else {
        values.iter().any(|v| operator_holds(v, c.operator, &c.value))
    };

    base ^ c.negated
}

/// Whether a single resolved value satisfies a (non-EXISTS) operator + operand.
fn operator_holds(value: &StixValue, op: ComparisonOperator, operand: &ComparisonOperand) -> bool {
    use std::cmp::Ordering;

    // `IN` is the only operator that takes a set operand.
    if op == ComparisonOperator::In {
        return match operand {
            ComparisonOperand::Set(set) => value_in_set(value, set),
            ComparisonOperand::Literal(lit) => value_in_set(value, std::slice::from_ref(lit)),
        };
    }

    let lit = match operand {
        ComparisonOperand::Literal(l) => l,
        // A non-IN operator with a set operand is ill-formed; never matches.
        ComparisonOperand::Set(_) => return false,
    };

    match op {
        ComparisonOperator::Equal => value_eq_literal(value, lit),
        ComparisonOperator::NotEqual => !value_eq_literal(value, lit),
        ComparisonOperator::GreaterThan => value_cmp_literal(value, lit) == Some(Ordering::Greater),
        ComparisonOperator::GreaterThanOrEqual => matches!(
            value_cmp_literal(value, lit),
            Some(Ordering::Greater | Ordering::Equal)
        ),
        ComparisonOperator::LessThan => value_cmp_literal(value, lit) == Some(Ordering::Less),
        ComparisonOperator::LessThanOrEqual => matches!(
            value_cmp_literal(value, lit),
            Some(Ordering::Less | Ordering::Equal)
        ),
        ComparisonOperator::Like => string_op(value, lit, like_matches),
        ComparisonOperator::Matches => string_op(value, lit, regex_matches),
        ComparisonOperator::IsSubset => string_op(value, lit, is_subset),
        ComparisonOperator::IsSuperset => string_op(value, lit, is_superset),
        // Handled above / not reachable here.
        ComparisonOperator::In | ComparisonOperator::Exists => false,
    }
}

/// Apply a `(value_str, literal_str) -> bool` operator, requiring both sides to be
/// strings.
fn string_op(value: &StixValue, lit: &Literal, f: impl Fn(&str, &str) -> bool) -> bool {
    let v = match value.as_str() {
        Some(s) => s,
        None => return false,
    };
    let l = match lit {
        Literal::String(s) | Literal::Timestamp(s) | Literal::Binary(s) | Literal::Hex(s) => s,
        _ => return false,
    };
    f(v, l)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stix_model::StixObject;
    use stix_pattern::ast::{
        Comparison, ComparisonOperand, ComparisonOperator, Literal, ObjectPath, PathStep,
    };

    fn obj(json: serde_json::Value) -> StixObject {
        StixObject::from_json(json).unwrap()
    }

    fn cmp(
        object_type: &str,
        key: &str,
        operator: ComparisonOperator,
        negated: bool,
        value: ComparisonOperand,
    ) -> Comparison {
        Comparison {
            path: ObjectPath {
                object_type: object_type.to_string(),
                steps: vec![PathStep::Key(key.to_string())],
            },
            operator,
            negated,
            value,
        }
    }

    fn lit(s: &str) -> ComparisonOperand {
        ComparisonOperand::Literal(Literal::String(s.to_string()))
    }

    #[test]
    fn equality_against_object() {
        let o = obj(serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}));
        let c = cmp("ipv4-addr", "value", ComparisonOperator::Equal, false, lit("1.2.3.4"));
        assert!(eval_comparison(&o, &c, None));

        let c2 = cmp("ipv4-addr", "value", ComparisonOperator::Equal, false, lit("9.9.9.9"));
        assert!(!eval_comparison(&o, &c2, None));
    }

    #[test]
    fn negation_inverts() {
        let o = obj(serde_json::json!({"type": "file", "id": "file--1", "name": "evil.exe"}));
        let c = cmp("file", "name", ComparisonOperator::Equal, true, lit("evil.exe"));
        assert!(!eval_comparison(&o, &c, None));
    }

    #[test]
    fn exists_checks_presence() {
        let o = obj(serde_json::json!({"type": "file", "id": "file--1", "name": "x"}));
        let present = cmp("file", "name", ComparisonOperator::Exists, false, lit("ignored"));
        assert!(eval_comparison(&o, &present, None));
        let absent = cmp("file", "size", ComparisonOperator::Exists, false, lit("ignored"));
        assert!(!eval_comparison(&o, &absent, None));
    }

    #[test]
    fn in_set_against_object() {
        let o = obj(serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "8.8.8.8"}));
        let set = ComparisonOperand::Set(vec![
            Literal::String("1.1.1.1".into()),
            Literal::String("8.8.8.8".into()),
        ]);
        let c = cmp("ipv4-addr", "value", ComparisonOperator::In, false, set);
        assert!(eval_comparison(&o, &c, None));
    }

    #[test]
    fn issubset_against_object() {
        let o = obj(serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "198.51.100.5"}));
        let c = cmp(
            "ipv4-addr",
            "value",
            ComparisonOperator::IsSubset,
            false,
            ComparisonOperand::Literal(Literal::String("198.51.100.0/24".into())),
        );
        assert!(eval_comparison(&o, &c, None));
    }
}
