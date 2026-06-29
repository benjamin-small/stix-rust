//! Opaque handles (`Pattern`, `Bundle`) and the plain `MatchOutcome` value.

/// Opaque handle around a parsed pattern AST.
#[derive(Debug, Clone)]
pub struct Pattern {
    inner: stix::pattern::Pattern,
}

impl Pattern {
    pub(crate) fn new(inner: stix::pattern::Pattern) -> Self {
        Pattern { inner }
    }

    pub(crate) fn inner(&self) -> &stix::pattern::Pattern {
        &self.inner
    }

    /// The pattern's AST serialized as compact JSON.
    pub fn to_json(&self) -> String {
        // Serialization of the AST is infallible in practice; fall back to "null".
        serde_json::to_string(&self.inner).unwrap_or_else(|_| "null".to_string())
    }
}

/// Opaque handle around an imported bundle.
#[derive(Debug, Clone)]
pub struct Bundle {
    inner: stix::model::Bundle,
}

impl Bundle {
    pub(crate) fn new(inner: stix::model::Bundle) -> Self {
        Bundle { inner }
    }

    pub(crate) fn inner(&self) -> &stix::model::Bundle {
        &self.inner
    }

    /// Number of objects in the bundle.
    pub fn object_count(&self) -> usize {
        self.inner.objects.len()
    }

    /// The object at `index` serialized as JSON, or `None` if out of range.
    pub fn object_json(&self, index: usize) -> Option<String> {
        let obj = self.inner.objects.get(index)?;
        Some(serde_json::to_string(obj).unwrap_or_else(|_| "null".to_string()))
    }
}

/// The outcome of a match: whether it matched and which observation indices bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchOutcome {
    pub matched: bool,
    pub observations: Vec<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_to_json_round_trips() {
        let inner = stix::parse("[ipv4-addr:value = '1.2.3.4']").unwrap();
        let handle = Pattern::new(inner.clone());
        let json = handle.to_json();
        let back: stix::pattern::Pattern = serde_json::from_str(&json).unwrap();
        assert_eq!(back, inner);
    }

    #[test]
    fn bundle_object_access() {
        let raw = r#"{"type":"bundle","id":"bundle--1","objects":[
            {"type":"ipv4-addr","id":"ipv4-addr--1","value":"1.2.3.4"}
        ]}"#;
        let inner = stix::model::Bundle::from_json_str(raw).unwrap();
        let handle = Bundle::new(inner);
        assert_eq!(handle.object_count(), 1);
        let obj_json = handle.object_json(0).unwrap();
        assert!(obj_json.contains("ipv4-addr--1"));
        assert!(handle.object_json(5).is_none());
    }

    #[test]
    fn match_outcome_fields() {
        let o = MatchOutcome {
            matched: true,
            observations: vec![0, 2],
        };
        assert!(o.matched);
        assert_eq!(o.observations, vec![0, 2]);
    }
}
