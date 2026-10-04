//! Evaluation: leaf comparisons, comparison expressions, and observation expressions.

use std::collections::{BTreeMap, BTreeSet};

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
        values
            .iter()
            .any(|v| operator_holds(v, c.operator, &c.value))
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
    let mut types: Vec<&str> = Vec::new();
    collect_types(expr, &mut types, &mut BTreeSet::new());

    // Candidate objects per referenced type (indices into observation.objects).
    // A type with no candidate stays unbound, so its leaves evaluate to false;
    // only the types that do have candidates take part in the enumeration.
    let bound: Vec<(&str, Vec<usize>)> = types
        .into_iter()
        .map(|t| {
            let candidates: Vec<usize> = observation
                .objects
                .iter()
                .enumerate()
                .filter(|(_, o)| o.type_() == Some(t))
                .map(|(i, _)| i)
                .collect();
            (t, candidates)
        })
        .filter(|(_, candidates)| !candidates.is_empty())
        .collect();

    enumerate_bindings(&bound, |binding| {
        eval_tree(expr, observation, binding, store)
    })
}

/// Collect the distinct object types referenced by an expression's leaves, in
/// first-seen order. Recurses over nesting depth only (bounded by the parser);
/// chain operands are iterated.
fn collect_types<'e>(
    expr: &'e ComparisonExpression,
    out: &mut Vec<&'e str>,
    seen: &mut BTreeSet<&'e str>,
) {
    match expr {
        ComparisonExpression::Test(c) => {
            if seen.insert(&c.path.object_type) {
                out.push(&c.path.object_type);
            }
        }
        ComparisonExpression::And(xs) | ComparisonExpression::Or(xs) => {
            for x in xs {
                collect_types(x, out, seen);
            }
        }
    }
}

/// Try every assignment of one candidate object per type; return true as soon as
/// `predicate` accepts a binding. Every type in `bound` has at least one
/// candidate. Iterates like an odometer (the last type varies fastest), so the
/// number of types costs no stack.
fn enumerate_bindings(
    bound: &[(&str, Vec<usize>)],
    predicate: impl Fn(&BTreeMap<String, usize>) -> bool,
) -> bool {
    let mut choice = vec![0usize; bound.len()];
    let mut binding: BTreeMap<String, usize> = bound
        .iter()
        .map(|(t, candidates)| (t.to_string(), candidates[0]))
        .collect();
    loop {
        if predicate(&binding) {
            return true;
        }
        // Advance to the next assignment, or stop after the last one.
        let mut i = bound.len();
        loop {
            if i == 0 {
                return false;
            }
            i -= 1;
            let (t, candidates) = &bound[i];
            choice[i] += 1;
            if choice[i] == candidates.len() {
                choice[i] = 0;
            }
            binding.insert(t.to_string(), candidates[choice[i]]);
            if choice[i] != 0 {
                break;
            }
        }
    }
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
        // A chain is the left-associative `(a && b) && c`: short-circuiting
        // left to right, exactly as `all`/`any` do.
        ComparisonExpression::And(xs) => {
            xs.iter().all(|x| eval_tree(x, observation, binding, store))
        }
        ComparisonExpression::Or(xs) => {
            xs.iter().any(|x| eval_tree(x, observation, binding, store))
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
        ObservationExpression::Observation {
            expression: comparison,
            ..
        } => {
            let mut any = false;
            for (i, obs) in observations.iter().enumerate() {
                if eval_comparison_expression(comparison, obs, store) {
                    matched.push(i);
                    any = true;
                }
            }
            Ok(any)
        }
        // Chains are the left-associative binary form, which evaluates every
        // operand in order (no short-circuit, so each operand's matching
        // observations are recorded) and stops at the first error.
        ObservationExpression::And(xs) => {
            let mut all = true;
            for x in xs {
                all &= eval_observation_expression(x, observations, store, matched)?;
            }
            Ok(all)
        }
        ObservationExpression::Or(xs) => {
            let mut any = false;
            for x in xs {
                any |= eval_observation_expression(x, observations, store, matched)?;
            }
            Ok(any)
        }
        ObservationExpression::FollowedBy(_) => Err(MatchError::Unsupported(
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
                span: Default::default(),
            },
            operator,
            negated,
            value,
            span: Default::default(),
        }
    }

    fn lit(s: &str) -> ComparisonOperand {
        ComparisonOperand::Literal(Literal::String(s.to_string()))
    }

    #[test]
    fn equality_against_object() {
        let o =
            obj(serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}));
        let c = cmp(
            "ipv4-addr",
            "value",
            ComparisonOperator::Equal,
            false,
            lit("1.2.3.4"),
        );
        assert!(eval_comparison(&o, &c, None));

        let c2 = cmp(
            "ipv4-addr",
            "value",
            ComparisonOperator::Equal,
            false,
            lit("9.9.9.9"),
        );
        assert!(!eval_comparison(&o, &c2, None));
    }

    #[test]
    fn negation_inverts() {
        let o = obj(serde_json::json!({"type": "file", "id": "file--1", "name": "evil.exe"}));
        let c = cmp(
            "file",
            "name",
            ComparisonOperator::Equal,
            true,
            lit("evil.exe"),
        );
        assert!(!eval_comparison(&o, &c, None));
    }

    #[test]
    fn exists_checks_presence() {
        let o = obj(serde_json::json!({"type": "file", "id": "file--1", "name": "x"}));
        let present = cmp(
            "file",
            "name",
            ComparisonOperator::Exists,
            false,
            lit("ignored"),
        );
        assert!(eval_comparison(&o, &present, None));
        let absent = cmp(
            "file",
            "size",
            ComparisonOperator::Exists,
            false,
            lit("ignored"),
        );
        assert!(!eval_comparison(&o, &absent, None));
    }

    #[test]
    fn in_set_against_object() {
        let o =
            obj(serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "8.8.8.8"}));
        let set = ComparisonOperand::Set(vec![
            Literal::String("1.1.1.1".into()),
            Literal::String("8.8.8.8".into()),
        ]);
        let c = cmp("ipv4-addr", "value", ComparisonOperator::In, false, set);
        assert!(eval_comparison(&o, &c, None));
    }

    #[test]
    fn issubset_against_object() {
        let o = obj(
            serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "198.51.100.5"}),
        );
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
        let expr = ComparisonExpression::And(vec![
            test_expr(cmp(
                "file",
                "name",
                ComparisonOperator::Equal,
                false,
                lit("evil.exe"),
            )),
            test_expr(cmp(
                "file",
                "size",
                ComparisonOperator::Equal,
                false,
                ComparisonOperand::Literal(Literal::Integer(10)),
            )),
        ]);
        assert!(eval_comparison_expression(&expr, &matching, None));
        // No single file is both name=evil.exe AND size=10, so this must not match.
        assert!(!eval_comparison_expression(&expr, &split, None));
    }

    #[test]
    fn or_matches_either_branch() {
        let o = observation(vec![
            serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}),
        ]);
        let expr = ComparisonExpression::Or(vec![
            test_expr(cmp(
                "ipv4-addr",
                "value",
                ComparisonOperator::Equal,
                false,
                lit("9.9.9.9"),
            )),
            test_expr(cmp(
                "ipv4-addr",
                "value",
                ComparisonOperator::Equal,
                false,
                lit("1.2.3.4"),
            )),
        ]);
        assert!(eval_comparison_expression(&expr, &o, None));
    }

    fn eq_test(object_type: &str, value: &str) -> ComparisonExpression {
        test_expr(cmp(
            object_type,
            "value",
            ComparisonOperator::Equal,
            false,
            lit(value),
        ))
    }

    /// An n-ary chain evaluates exactly as the left-associative binary chain
    /// it stands for. The nested form below breaks the parser's flattening
    /// invariant on purpose, to serve as the reference.
    #[test]
    fn comparison_chains_match_their_left_assoc_binary_form() {
        let o = observation(vec![
            serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}),
            serde_json::json!({"type": "domain-name", "id": "domain-name--1", "value": "a.example"}),
        ]);
        let leaves = [
            eq_test("ipv4-addr", "1.2.3.4"),
            eq_test("ipv4-addr", "9.9.9.9"),
            eq_test("domain-name", "a.example"),
            eq_test("url", "absent"),
        ];
        for a in &leaves {
            for b in &leaves {
                for c in &leaves {
                    type Build = fn(Vec<ComparisonExpression>) -> ComparisonExpression;
                    for build in [ComparisonExpression::And as Build, ComparisonExpression::Or] {
                        let chain = build(vec![a.clone(), b.clone(), c.clone()]);
                        let nested = build(vec![build(vec![a.clone(), b.clone()]), c.clone()]);
                        assert_eq!(
                            eval_comparison_expression(&chain, &o, None),
                            eval_comparison_expression(&nested, &o, None),
                            "{chain:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn many_distinct_types_in_one_chain() {
        // Types absent from the observation are left unbound; a long chain of
        // them must neither recurse per type nor change the result.
        let o = observation(vec![
            serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.2.3.4"}),
        ]);
        let mut xs: Vec<ComparisonExpression> = (0..10_000)
            .map(|i| eq_test(&format!("type-{i}"), "x"))
            .collect();
        assert!(!eval_comparison_expression(
            &ComparisonExpression::Or(xs.clone()),
            &o,
            None
        ));
        xs.push(eq_test("ipv4-addr", "1.2.3.4"));
        assert!(eval_comparison_expression(
            &ComparisonExpression::Or(xs),
            &o,
            None
        ));
    }

    #[test]
    fn observation_chains_match_their_left_assoc_binary_form() {
        let observations = vec![
            observation(vec![
                serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"}),
            ]),
            observation(vec![
                serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--2", "value": "2.2.2.2"}),
            ]),
            observation(vec![
                serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--3", "value": "3.3.3.3"}),
            ]),
        ];
        let leaf = |v: &str| ObservationExpression::Observation {
            expression: Box::new(eq_test("ipv4-addr", v)),
            span: Default::default(),
        };
        let leaves = [leaf("1.1.1.1"), leaf("3.3.3.3"), leaf("9.9.9.9")];
        let run = |e: ObservationExpression| {
            eval_pattern(&Pattern { expression: e }, &observations, None).unwrap()
        };
        for a in &leaves {
            for b in &leaves {
                for c in &leaves {
                    type Build = fn(Vec<ObservationExpression>) -> ObservationExpression;
                    for build in [
                        ObservationExpression::And as Build,
                        ObservationExpression::Or,
                    ] {
                        let chain = build(vec![a.clone(), b.clone(), c.clone()]);
                        let nested = build(vec![build(vec![a.clone(), b.clone()]), c.clone()]);
                        // Same verdict and the same matched observations.
                        assert_eq!(run(chain.clone()), run(nested), "{chain:?}");
                    }
                }
            }
        }
        // OR reports the observations of every operand that matched.
        let r = run(ObservationExpression::Or(vec![
            leaf("1.1.1.1"),
            leaf("9.9.9.9"),
            leaf("3.3.3.3"),
        ]));
        assert_eq!(r.observations(), &[0, 2]);
    }

    use stix_pattern::parse;

    #[test]
    fn single_observation_matches_across_set() {
        let observations = vec![
            observation(vec![
                serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"}),
            ]),
            observation(vec![
                serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--2", "value": "1.2.3.4"}),
            ]),
        ];
        let pattern = parse("[ipv4-addr:value = '1.2.3.4']").unwrap();
        let result = eval_pattern(&pattern, &observations, None).unwrap();
        assert!(result.is_match());
        assert_eq!(result.observations(), &[1]);
    }

    #[test]
    fn observation_and_needs_both() {
        let observations = vec![
            observation(vec![
                serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"}),
            ]),
            observation(vec![
                serde_json::json!({"type": "domain-name", "id": "domain-name--1", "value": "evil.example"}),
            ]),
        ];
        let yes = parse("[ipv4-addr:value = '1.1.1.1'] AND [domain-name:value = 'evil.example']")
            .unwrap();
        assert!(eval_pattern(&yes, &observations, None).unwrap().is_match());

        let no = parse("[ipv4-addr:value = '1.1.1.1'] AND [domain-name:value = 'good.example']")
            .unwrap();
        assert!(!eval_pattern(&no, &observations, None).unwrap().is_match());
    }

    #[test]
    fn followedby_is_unsupported() {
        let observations = vec![observation(vec![
            serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"}),
        ])];
        let pattern =
            parse("[ipv4-addr:value = '1.1.1.1'] FOLLOWEDBY [ipv4-addr:value = '2.2.2.2']")
                .unwrap();
        let err = eval_pattern(&pattern, &observations, None).unwrap_err();
        assert!(matches!(err, crate::error::MatchError::Unsupported(_)));
    }

    #[test]
    fn qualifier_is_unsupported() {
        let observations = vec![observation(vec![
            serde_json::json!({"type": "ipv4-addr", "id": "ipv4-addr--1", "value": "1.1.1.1"}),
        ])];
        let pattern = parse("[ipv4-addr:value = '1.1.1.1'] REPEATS 2 TIMES").unwrap();
        let err = eval_pattern(&pattern, &observations, None).unwrap_err();
        assert!(matches!(err, crate::error::MatchError::Unsupported(_)));
    }
}
