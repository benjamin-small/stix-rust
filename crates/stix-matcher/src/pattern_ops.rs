//! The `LIKE` (SQL wildcard) and `MATCHES` (regex) operators.

use regex::Regex;

/// Tests whether `value` matches the SQL-style `LIKE` pattern.
///
/// `%` matches any run of characters and `_` exactly one; all other characters are
/// literal. The pattern is anchored (must match the whole value). An invalid
/// translation never matches.
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

/// Tests whether `value` matches the regular expression (the `MATCHES` operator).
///
/// The match is unanchored, mirroring the reference implementation. An invalid
/// regex never matches (returns false rather than erroring).
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
