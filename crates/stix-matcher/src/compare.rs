//! Scalar comparison between a resolved `StixValue` and a pattern `Literal`.

use std::cmp::Ordering;

use stix_model::StixValue;
use stix_pattern::ast::Literal;

/// The string contents of a string-like literal, if any.
fn literal_str(lit: &Literal) -> Option<&str> {
    match lit {
        Literal::String(s) | Literal::Timestamp(s) | Literal::Binary(s) | Literal::Hex(s) => {
            Some(s)
        }
        _ => None,
    }
}

/// The numeric value of a numeric literal, if any.
fn literal_f64(lit: &Literal) -> Option<f64> {
    match lit {
        Literal::Integer(n) => Some(*n as f64),
        Literal::Float(f) => Some(*f),
        _ => None,
    }
}

/// Equality between a value and a literal, with int/float promotion and
/// string-typed-literal comparison.
pub fn value_eq_literal(value: &StixValue, lit: &Literal) -> bool {
    match (value, lit) {
        (StixValue::Bool(b), Literal::Boolean(l)) => b == l,
        _ => {
            if let (Some(v), Some(l)) = (value.as_str(), literal_str(lit)) {
                return v == l;
            }
            if let (Some(v), Some(l)) = (value.as_f64(), literal_f64(lit)) {
                return v == l;
            }
            false
        }
    }
}

/// Ordering between a value and a literal (numeric or string), or `None` if the
/// two are not comparable.
pub fn value_cmp_literal(value: &StixValue, lit: &Literal) -> Option<Ordering> {
    if let (Some(v), Some(l)) = (value.as_f64(), literal_f64(lit)) {
        return v.partial_cmp(&l);
    }
    if let (Some(v), Some(l)) = (value.as_str(), literal_str(lit)) {
        return Some(v.cmp(l));
    }
    None
}

/// Whether a value equals any member of a set literal (`IN`).
pub fn value_in_set(value: &StixValue, set: &[Literal]) -> bool {
    set.iter().any(|lit| value_eq_literal(value, lit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stix_model::StixValue;
    use stix_pattern::ast::Literal;

    #[test]
    fn string_equality() {
        assert!(value_eq_literal(
            &StixValue::String("a".into()),
            &Literal::String("a".into())
        ));
        assert!(!value_eq_literal(
            &StixValue::String("a".into()),
            &Literal::String("b".into())
        ));
    }

    #[test]
    fn numeric_equality_crosses_int_float() {
        assert!(value_eq_literal(
            &StixValue::Integer(3),
            &Literal::Integer(3)
        ));
        assert!(value_eq_literal(
            &StixValue::Integer(3),
            &Literal::Float(3.0)
        ));
        assert!(value_eq_literal(
            &StixValue::Float(3.0),
            &Literal::Integer(3)
        ));
        assert!(!value_eq_literal(
            &StixValue::Integer(3),
            &Literal::Integer(4)
        ));
    }

    #[test]
    fn typed_literals_compare_as_strings() {
        assert!(value_eq_literal(
            &StixValue::String("2020-01-01T00:00:00Z".into()),
            &Literal::Timestamp("2020-01-01T00:00:00Z".into())
        ));
        assert!(value_eq_literal(
            &StixValue::String("cafe".into()),
            &Literal::Hex("cafe".into())
        ));
    }

    #[test]
    fn ordering() {
        use std::cmp::Ordering;
        assert_eq!(
            value_cmp_literal(&StixValue::Integer(2), &Literal::Integer(5)),
            Some(Ordering::Less)
        );
        assert_eq!(
            value_cmp_literal(&StixValue::String("b".into()), &Literal::String("a".into())),
            Some(Ordering::Greater)
        );
        assert_eq!(
            value_cmp_literal(&StixValue::Bool(true), &Literal::Integer(1)),
            None
        );
    }

    #[test]
    fn membership() {
        let set = vec![Literal::Integer(1), Literal::Integer(2)];
        assert!(value_in_set(&StixValue::Integer(2), &set));
        assert!(!value_in_set(&StixValue::Integer(3), &set));
    }
}
