use stix_pattern::parse;

fn lines(raw: &str) -> impl Iterator<Item = (usize, &str)> {
    raw.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
}

#[test]
fn all_valid_patterns_parse() {
    let raw = include_str!("fixtures/valid_patterns.txt");
    let mut failures = Vec::new();
    for (lineno, pat) in lines(raw) {
        if let Err(e) = parse(pat) {
            failures.push(format!("line {lineno}: `{pat}` -> {e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "expected these to parse:\n{}",
        failures.join("\n")
    );
}

#[test]
fn all_invalid_patterns_reject() {
    let raw = include_str!("fixtures/invalid_patterns.txt");
    let mut leaks = Vec::new();
    for (lineno, pat) in lines(raw) {
        if parse(pat).is_ok() {
            leaks.push(format!("line {lineno}: `{pat}` should have failed but parsed"));
        }
    }
    assert!(
        leaks.is_empty(),
        "expected these to be rejected:\n{}",
        leaks.join("\n")
    );
}
