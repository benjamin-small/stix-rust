//! Sub-project B's proof obligation: lowering then rendering is faithful.

use stix_pattern::ir::{lower, render};
use stix_pattern::parse;

fn lines(raw: &str) -> impl Iterator<Item = (usize, &str)> {
    raw.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
}

const CORPUS: &str = include_str!("fixtures/valid_patterns.txt");

#[test]
fn every_corpus_pattern_lowers_and_validates() {
    let mut failures = Vec::new();
    for (lineno, src) in lines(CORPUS) {
        let ast = parse(src).expect(src);
        let program = lower(&ast);
        if let Err(e) = program.validate() {
            failures.push(format!("line {lineno}: `{src}` -> {e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "lowered programs should validate:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_corpus_pattern_round_trips() {
    let mut failures = Vec::new();
    for (lineno, src) in lines(CORPUS) {
        let ast = parse(src).expect(src);
        let text = render(&lower(&ast));
        match parse(&text) {
            Ok(reparsed) => {
                if reparsed.without_spans() != ast.without_spans() {
                    failures.push(format!(
                        "line {lineno}: AST changed\n    src:      {src}\n    rendered: {text}"
                    ));
                }
            }
            Err(e) => failures.push(format!(
                "line {lineno}: rendered text did not parse\n    src:      {src}\n    rendered: {text}\n    error:    {e}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "parse(render(lower(ast))) should equal ast, modulo spans:\n{}",
        failures.join("\n")
    );
}

/// `Program::validate` enforces that each value has at most one consumer, which
/// is only a safe rule if `lower` never produces a shared value. Corpus patterns
/// combined pairwise exercise every combinator and qualifier over every leaf
/// shape, which is where sharing would show up if lowering ever introduced it.
#[test]
fn lowering_never_shares_a_value() {
    let patterns: Vec<&str> = lines(CORPUS).map(|(_, src)| src).collect();
    let mut failures = Vec::new();
    let mut checked = 0usize;
    for a in &patterns {
        for b in &patterns {
            for combined in [
                format!("({a}) AND ({b})"),
                format!("({a}) OR ({b})"),
                format!("({a}) FOLLOWEDBY ({b})"),
                format!("(({a}) OR ({b})) WITHIN 60 SECONDS"),
                format!("(({a}) AND ({b})) REPEATS 3 TIMES"),
            ] {
                let ast = parse(&combined).expect(&combined);
                checked += 1;
                if let Err(e) = lower(&ast).validate() {
                    failures.push(format!("{combined} -> {e}"));
                }
            }
        }
    }
    assert!(checked > 3000, "expected a broad sample, checked {checked}");
    assert!(
        failures.is_empty(),
        "lowered programs should never share a value:\n{}",
        failures.join("\n")
    );
}

#[test]
fn rendering_is_idempotent() {
    // Canonical output is a fixed point: rendering it again changes nothing.
    let mut failures = Vec::new();
    for (lineno, src) in lines(CORPUS) {
        let once = render(&lower(&parse(src).unwrap()));
        let twice = render(&lower(&parse(&once).unwrap()));
        if once != twice {
            failures.push(format!("line {lineno}: `{once}` -> `{twice}`"));
        }
    }
    assert!(
        failures.is_empty(),
        "rendering should be idempotent:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_lowered_instruction_has_an_in_bounds_span() {
    let mut failures = Vec::new();
    for (lineno, src) in lines(CORPUS) {
        let program = lower(&parse(src).unwrap());
        let blocks = program.blocks.iter().chain(std::iter::once(&program.main));
        for b in blocks {
            for instr in &b.instructions {
                let Some(span) = instr.span else { continue };
                if span.end > src.len() || span.start >= span.end {
                    failures.push(format!(
                        "line {lineno}: `{src}` has a bad span {:?} on {:?}",
                        span, instr.op
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "lowered spans should be non-empty and in bounds:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_corpus_pattern_prints_a_listing() {
    for (lineno, src) in lines(CORPUS) {
        let listing = lower(&parse(src).unwrap()).to_listing();
        assert!(
            listing.contains("block main (observation):"),
            "line {lineno}: `{src}` produced no main block:\n{listing}"
        );
        let last_line = listing
            .trim_end()
            .lines()
            .last()
            .unwrap_or_else(|| panic!("line {lineno}: listing should not be empty"));
        assert!(
            last_line.trim_start().starts_with("ret ")
                && last_line.trim_end().ends_with(|c: char| c.is_ascii_digit()),
            "line {lineno}: last line should be a `ret` terminator naming an operand: `{last_line}`"
        );
    }
}
