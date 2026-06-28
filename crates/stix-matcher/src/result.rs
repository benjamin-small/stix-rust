//! The outcome of a match.

/// The result of evaluating a pattern against a set of observations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchResult {
    matched: bool,
    matched_observations: Vec<usize>,
}

impl MatchResult {
    /// A non-match (no observations).
    pub fn no_match() -> Self {
        MatchResult {
            matched: false,
            matched_observations: Vec::new(),
        }
    }

    /// A match, recording the indices of the observations that satisfied the pattern.
    pub fn matched(observations: Vec<usize>) -> Self {
        MatchResult {
            matched: true,
            matched_observations: observations,
        }
    }

    /// Whether the pattern matched.
    pub fn is_match(&self) -> bool {
        self.matched
    }

    /// Indices (into the input observation list) that participated in the match.
    pub fn observations(&self) -> &[usize] {
        &self.matched_observations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_match_is_false() {
        let r = MatchResult::no_match();
        assert!(!r.is_match());
        assert!(r.observations().is_empty());
    }

    #[test]
    fn matched_records_observations() {
        let r = MatchResult::matched(vec![0, 2]);
        assert!(r.is_match());
        assert_eq!(r.observations(), &[0, 2]);
    }
}
