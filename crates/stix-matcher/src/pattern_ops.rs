//! The `LIKE` (SQL wildcard) and `MATCHES` (regex) operators.

use regex::Regex;

/// STIX `LIKE`: `%` matches any run of characters, `_` matches exactly one. All
/// other characters match literally. Implemented by translating to an anchored
/// regex with every non-wildcard character escaped.
pub fn like_matches(value: &str, pattern: &str) -> bool {
    let mut regex = String::with_capacity(pattern.len() * 2 + 2);
    regex.push('^');
    for ch in pattern.chars() {
        match ch {
            '%' => regex.push_str(".*"),
            '_' => regex.push('.'),
            other => regex.push_str(&regex::escape(&other.to_string())),
        }
    }
    regex.push('$');
    match Regex::new(&regex) {
        Ok(re) => re.is_match(value),
        Err(_) => false,
    }
}

/// STIX `MATCHES`: PCRE-style regular-expression match (unanchored, like the
/// reference implementation). An invalid regex never matches.
pub fn regex_matches(value: &str, pattern: &str) -> bool {
    match Regex::new(pattern) {
        Ok(re) => re.is_match(value),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_percent_matches_any_run() {
        assert!(like_matches("foobar.evil.example", "%.evil.example"));
        assert!(!like_matches("foobar.good.example", "%.evil.example"));
    }

    #[test]
    fn like_underscore_matches_single_char() {
        assert!(like_matches("cat", "c_t"));
        assert!(!like_matches("coat", "c_t"));
    }

    #[test]
    fn like_escapes_regex_metachars() {
        // '.' in the pattern is a literal dot, not "any char".
        assert!(like_matches("a.b", "a.b"));
        assert!(!like_matches("axb", "a.b"));
    }

    #[test]
    fn matches_uses_regex() {
        assert!(regex_matches("invoice12", "invoice[0-9]+"));
        assert!(!regex_matches("invoice", "invoice[0-9]+"));
    }

    #[test]
    fn invalid_regex_does_not_match() {
        assert!(!regex_matches("anything", "("));
    }
}
