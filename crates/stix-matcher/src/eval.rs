//! Evaluation: leaf comparisons, comparison expressions, and observation expressions.

use std::collections::BTreeMap;

use stix_model::{ObjectStore, ObjectView, StixValue};
use stix_pattern::ast::{
    Comparison, ComparisonExpression, ComparisonOperand, ComparisonOperator, Literal,
    ObservationExpression, Pattern,
};

use crate::compare::{value_cmp_literal, value_eq_literal, value_in_set};
use crate::error::MatchError;
use crate::observation::Observation;
use crate::pattern_ops::{like_matches, regex_matches};
use crate::resolve::resolve_path;
use crate::result::MatchResult;
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

/// Evaluate a comparison expression against one observation using binding
/// enumeration: each distinct referenced object-type is bound to one object of
/// that type from the observation (or none); the expression matches if some
/// binding makes the boolean tree true. This gives correct "same object" semantics
/// for `AND` within an observation while staying cheap (observations are small).
pub fn eval_comparison_expression(
    expr: &ComparisonExpression,
    observation: &Observation,
    store: Option<&ObjectStore>,
) -> bool {
    // Distinct object types referenced anywhere in the expression.
    let mut types: Vec<String> = Vec::new();
    collect_types(expr, &mut types);

    // Candidate objects per referenced type (indices into observation.objects).
    let candidates: Vec<Vec<usize>> = types
        .iter()
        .map(|t| {
            observation
                .objects
                .iter()
                .enumerate()
                .filter(|(_, o)| o.type_() == Some(t.as_str()))
                .map(|(i, _)| i)
                .collect()
        })
        .collect();

    // Enumerate one choice per type (or `None` when a type has no candidate).
    let mut binding: BTreeMap<String, usize> = BTreeMap::new();
    enumerate_bindings(&types, &candidates, 0, &mut binding, &|binding| {
        eval_tree(expr, observation, binding, store)
    })
}

/// Recursively collect distinct object types referenced by an expression's leaves.
fn collect_types(expr: &ComparisonExpression, out: &mut Vec<String>) {
    match expr {
        ComparisonExpression::Test(c) => {
            if !out.contains(&c.path.object_type) {
                out.push(c.path.object_type.clone());
            }
        }
        ComparisonExpression::And(a, b) | ComparisonExpression::Or(a, b) => {
            collect_types(a, out);
            collect_types(b, out);
        }
    }
}

/// Try every assignment of one candidate object per type; return true as soon as
/// `predicate` accepts a binding. Types with no candidates are simply absent from
/// the binding map (their leaves evaluate to false).
fn enumerate_bindings(
    types: &[String],
    candidates: &[Vec<usize>],
    idx: usize,
    binding: &mut BTreeMap<String, usize>,
    predicate: &dyn Fn(&BTreeMap<String, usize>) -> bool,
) -> bool {
    if idx == types.len() {
        return predicate(binding);
    }
    if candidates[idx].is_empty() {
        // No object of this type; leave it unbound and continue.
        return enumerate_bindings(types, candidates, idx + 1, binding, predicate);
    }
    for &obj_idx in &candidates[idx] {
        binding.insert(types[idx].clone(), obj_idx);
        if enumerate_bindings(types, candidates, idx + 1, binding, predicate) {
            binding.remove(&types[idx]);
            return true;
        }
    }
    binding.remove(&types[idx]);
    false
}

/// Evaluate the boolean tree under a fixed binding.
fn eval_tree(
    expr: &ComparisonExpression,
    observation: &Observation,
    binding: &BTreeMap<String, usize>,
    store: Option<&ObjectStore>,
) -> bool {
    match expr {
        ComparisonExpression::Test(c) => match binding.get(&c.path.object_type) {
            Some(&obj_idx) => eval_comparison(&observation.objects[obj_idx], c, store),
            None => false,
        },
        ComparisonExpression::And(a, b) => {
            eval_tree(a, observation, binding, store) && eval_tree(b, observation, binding, store)
        }
        ComparisonExpression::Or(a, b) => {
            eval_tree(a, observation, binding, store) || eval_tree(b, observation, binding, store)
        }
    }
}

/// Evaluate a whole pattern against a list of observations.
///
/// Phase 1: single observations and observation-level `AND`/`OR`. `FOLLOWEDBY` and
/// qualifiers (`WITHIN`/`REPEATS`/`START..STOP`) are parsed but return
/// `MatchError::Unsupported` rather than silently passing.
pub fn eval_pattern(
    pattern: &Pattern,
    observations: &[Observation],
    store: Option<&ObjectStore>,
) -> Result<MatchResult, MatchError> {
    let mut matched = Vec::new();
    let is_match =
        eval_observation_expression(&pattern.expression, observations, store, &mut matched)?;
    if is_match {
        matched.sort_unstable();
        matched.dedup();
        Ok(MatchResult::matched(matched))
    } else {
        Ok(MatchResult::no_match())
    }
}

/// Returns whether the observation expression matches, accumulating the indices of
/// observations that satisfied any `[ ... ]` leaf into `matched`.
fn eval_observation_expression(
    expr: &ObservationExpression,
    observations: &[Observation],
    store: Option<&ObjectStore>,
    matched: &mut Vec<usize>,
) -> Result<bool, MatchError> {
    match expr {
        ObservationExpression::Observation(comparison) => {
            let mut any = false;
            for (i, obs) in observations.iter().enumerate() {
                if eval_comparison_expression(comparison, obs, store) {
                    matched.push(i);
                    any = true;
                }
            }
            Ok(any)
        }
        ObservationExpression::And(a, b) => {
            let left = eval_observation_expression(a, observations, store, matched)?;
            let right = eval_observation_expression(b, observations, store, matched)?;
            Ok(left && right)
        }
        ObservationExpression::Or(a, b) => {
            let left = eval_observation_expression(a, observations, store, matched)?;
            let right = eval_observation_expression(b, observations, store, matched)?;
            Ok(left || right)
        }
        ObservationExpression::FollowedBy(_, _) => Err(MatchError::Unsupported(
            "FOLLOWEDBY sequencing is not yet implemented".to_string(),
        )),
        ObservationExpression::Qualified { .. } => Err(MatchError::Unsupported(
            "observation qualifiers (WITHIN/REPEATS/START..STOP) are not yet implemented"
                .to_string(),
        )),
    }
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

    use crate::observation::Observation;
    use stix_pattern::ast::ComparisonExpression;

    fn observation(objs: Vec<serde_json::Value>) -> Observation {
        Observation::new(objs.into_iter().map(obj).collect())
    }

    fn test_expr(c: Comparison) -> ComparisonExpression {
        ComparisonExpression::Test(c)
    }

    #[test]
    fn single_test_matches_some_object() {
        let o = observation(vec![
            serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}),
            serde_json::json!({"type": "domain-name", "id": "domain-name--1", "value": "evil.example"}),
        ]);
        let expr = test_expr(cmp(
            "domain-name",
            "value",
            ComparisonOperator::Equal,
            false,
            lit("evil.example"),
        ));
        assert!(eval_comparison_expression(&expr, &o, None));
    }

    #[test]
    fn and_requires_same_object_binding() {
        // Two constraints on `file` must be satisfied by ONE file object.
        let matching = observation(vec![
            serde_json::json!({"type": "file", "id": "file--1", "name": "evil.exe", "size": 10}),
        ]);
        let split = observation(vec![
            serde_json::json!({"type": "file", "id": "file--1", "name": "evil.exe", "size": 99}),
            serde_json::json!({"type": "file", "id": "file--2", "name": "ok.txt", "size": 10}),
        ]);
        let expr = ComparisonExpression::And(
            Box::new(test_expr(cmp(
                "file",
                "name",
                ComparisonOperator::Equal,
                false,
                lit("evil.exe"),
            ))),
            Box::new(test_expr(cmp(
                "file",
                "size",
                ComparisonOperator::Equal,
                false,
                ComparisonOperand::Literal(Literal::Integer(10)),
            ))),
        );
        assert!(eval_comparison_expression(&expr, &matching, None));
        // No single file is both name=evil.exe AND size=10, so this must not match.
        assert!(!eval_comparison_expression(&expr, &split, None));
    }

    #[test]
    fn or_matches_either_branch() {
        let o = observation(vec![
            serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}),
        ]);
        let expr = ComparisonExpression::Or(
            Box::new(test_expr(cmp(
                "ipv4-addr",
                "value",
                ComparisonOperator::Equal,
                false,
                lit("9.9.9.9"),
            ))),
            Box::new(test_expr(cmp(
                "ipv4-addr",
                "value",
                ComparisonOperator::Equal,
                false,
                lit("1.2.3.4"),
            ))),
        );
        assert!(eval_comparison_expression(&expr, &o, None));
    }

    use stix_pattern::parse;

    #[test]
    fn single_observation_matches_across_set() {
        let observations = vec![
            observation(vec![serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"})]),
            observation(vec![serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--2", "value": "1.2.3.4"})]),
        ];
        let pattern = parse("[ipv4-addr:value = '1.2.3.4']").unwrap();
        let result = eval_pattern(&pattern, &observations, None).unwrap();
        assert!(result.is_match());
        assert_eq!(result.observations(), &[1]);
    }

    #[test]
    fn observation_and_needs_both() {
        let observations = vec![
            observation(vec![serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"})]),
            observation(vec![serde_json::json!({"type": "domain-name", "id": "domain-name--1", "value": "evil.example"})]),
        ];
        let yes = parse("[ipv4-addr:value = '1.1.1.1'] AND [domain-name:value = 'evil.example']").unwrap();
        assert!(eval_pattern(&yes, &observations, None).unwrap().is_match());

        let no = parse("[ipv4-addr:value = '1.1.1.1'] AND [domain-name:value = 'good.example']").unwrap();
        assert!(!eval_pattern(&no, &observations, None).unwrap().is_match());
    }

    #[test]
    fn followedby_is_unsupported() {
        let observations =
            vec![observation(vec![serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"})])];
        let pattern = parse("[ipv4-addr:value = '1.1.1.1'] FOLLOWEDBY [ipv4-addr:value = '2.2.2.2']").unwrap();
        let err = eval_pattern(&pattern, &observations, None).unwrap_err();
        assert!(matches!(err, crate::error::MatchError::Unsupported(_)));
    }

    #[test]
    fn qualifier_is_unsupported() {
        let observations =
            vec![observation(vec![serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"})])];
        let pattern = parse("[ipv4-addr:value = '1.1.1.1'] REPEATS 2 TIMES").unwrap();
        let err = eval_pattern(&pattern, &observations, None).unwrap_err();
        assert!(matches!(err, crate::error::MatchError::Unsupported(_)));
    }
}
