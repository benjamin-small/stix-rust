//! `StixValue`: a uniform, JSON-shaped value the matcher can walk.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A dynamic STIX property value.
///
/// STIX timestamps, hex, and binary are carried as [`StixValue::String`] at this
/// layer; higher layers interpret them. Integers and floats are kept distinct so
/// numeric comparisons behave correctly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StixValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<StixValue>),
    Object(BTreeMap<String, StixValue>),
}

impl StixValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            StixValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            StixValue::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns the value as `f64`, promoting integers.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            StixValue::Float(f) => Some(*f),
            StixValue::Integer(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            StixValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[StixValue]> {
        match self {
            StixValue::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, StixValue>> {
        match self {
            StixValue::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, StixValue::Null)
    }
}

impl From<serde_json::Value> for StixValue {
    fn from(v: serde_json::Value) -> Self {
        use serde_json::Value;
        match v {
            Value::Null => StixValue::Null,
            Value::Bool(b) => StixValue::Bool(b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    StixValue::Integer(i)
                } else {
                    // Falls back to float for u64-too-big or fractional numbers.
                    StixValue::Float(n.as_f64().unwrap_or(0.0))
                }
            }
            Value::String(s) => StixValue::String(s),
            Value::Array(arr) => StixValue::List(arr.into_iter().map(StixValue::from).collect()),
            Value::Object(obj) => StixValue::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, StixValue::from(v)))
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_json_scalars() {
        assert_eq!(StixValue::from(serde_json::json!(null)), StixValue::Null);
        assert_eq!(
            StixValue::from(serde_json::json!(true)),
            StixValue::Bool(true)
        );
        assert_eq!(
            StixValue::from(serde_json::json!(42)),
            StixValue::Integer(42)
        );
        assert_eq!(
            StixValue::from(serde_json::json!(-7)),
            StixValue::Integer(-7)
        );
        assert_eq!(
            StixValue::from(serde_json::json!(2.5)),
            StixValue::Float(2.5)
        );
        assert_eq!(
            StixValue::from(serde_json::json!("hi")),
            StixValue::String("hi".to_string())
        );
    }

    #[test]
    fn from_json_nested() {
        let v = StixValue::from(serde_json::json!({"a": [1, "x"], "b": true}));
        match v {
            StixValue::Object(map) => {
                assert_eq!(
                    map.get("a"),
                    Some(&StixValue::List(vec![
                        StixValue::Integer(1),
                        StixValue::String("x".to_string())
                    ]))
                );
                assert_eq!(map.get("b"), Some(&StixValue::Bool(true)));
            }
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn accessors() {
        assert_eq!(StixValue::String("s".into()).as_str(), Some("s"));
        assert_eq!(StixValue::Integer(3).as_i64(), Some(3));
        assert_eq!(StixValue::Float(1.5).as_f64(), Some(1.5));
        assert_eq!(StixValue::Integer(3).as_f64(), Some(3.0));
        assert_eq!(StixValue::Bool(true).as_bool(), Some(true));
        assert!(StixValue::Null.is_null());
        assert_eq!(StixValue::String("s".into()).as_i64(), None);
    }
}
