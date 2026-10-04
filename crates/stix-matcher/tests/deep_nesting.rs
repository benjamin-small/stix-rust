//! Deeply nested patterns and very long chains must not overflow the stack.
//!
//! Each pattern here runs the whole pipeline (parse, lower, validate, render,
//! AST JSON serialization, matching, `without_spans`, drop) on a thread with a
//! 256 KiB stack, a stand-in for constrained hosts such as wasm. Patterns at
//! [`MAX_NESTING`] in the worst shapes are what that limit was measured
//! against: if a change makes any stage use more stack per level, these fail.

use stix_matcher::match_bundle;
use stix_model::Bundle;
use stix_pattern::{ir, parse, MAX_NESTING};

const STACK: usize = 256 * 1024;

/// A leaf that matches the fixture bundle.
const LEAF: &str = "[ipv4-addr:value = '198.51.100.5']";
const TEST: &str = "ipv4-addr:value = '198.51.100.5'";

fn bundle() -> Bundle {
    Bundle::from_json_str(include_str!("fixtures/bundle.json")).unwrap()
}

/// Run `f` on a thread with a [`STACK`]-byte stack.
fn on_small_stack<F: FnOnce() + Send + 'static>(f: F) {
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

/// Every stage a pattern goes through, on the current thread.
fn full_pipeline(src: &str, bundle: &Bundle) {
    let ast = parse(src).unwrap_or_else(|e| panic!("{e}"));
    let program = ir::lower(&ast);
    program.validate().unwrap();
    let text = ir::render(&program);
    let json = serde_json::to_string(&ast).unwrap();
    assert!(!json.is_empty());
    // FOLLOWEDBY and qualifiers are not yet supported by the matcher; an
    // `Unsupported` error is a fine outcome, a crash is not.
    let _ = match_bundle(&ast, bundle);
    let stripped = ast.without_spans();
    let reparsed = parse(&text).unwrap_or_else(|e| panic!("{e}"));
    assert!(reparsed.without_spans() == stripped);
    drop(reparsed);
    drop(stripped);
    drop(program);
    drop(ast);
}

fn on_small_stack_full_pipeline(src: String) {
    on_small_stack(move || full_pipeline(&src, &bundle()));
}

// --- worst shapes, each with nesting exactly `n` ---

/// `n` observation-level groups, each holding a FOLLOWEDBY, an OR and an AND
/// node above the next group: the most AST levels per unit of nesting.
fn observation_groups(n: usize) -> String {
    let mut src = LEAF.to_string();
    for _ in 0..n {
        src = format!("{LEAF} FOLLOWEDBY {LEAF} OR {LEAF} AND ({src})");
    }
    src
}

/// Like [`observation_groups`] without FOLLOWEDBY, which the matcher rejects
/// up front; this is the deepest shape the matcher actually walks.
fn observation_groups_matchable(n: usize) -> String {
    let mut src = LEAF.to_string();
    for _ in 0..n {
        src = format!("{LEAF} OR {LEAF} AND ({src})");
    }
    src
}

/// `n` comparison-level groups, each holding an OR and an AND node.
fn comparison_groups(n: usize) -> String {
    let mut src = TEST.to_string();
    for _ in 0..n {
        src = format!("{TEST} OR {TEST} AND ({src})");
    }
    format!("[{src}]")
}

/// Observation-level groups around comparison-level groups.
fn mixed_groups(n: usize, outer: fn(usize, &str) -> String) -> String {
    let inner = comparison_groups(n - n / 2);
    outer(n / 2, &inner)
}

fn wrap_observation(n: usize, inner: &str) -> String {
    let mut src = inner.to_string();
    for _ in 0..n {
        src = format!("{LEAF} FOLLOWEDBY {LEAF} OR {LEAF} AND ({src})");
    }
    src
}

fn wrap_observation_matchable(n: usize, inner: &str) -> String {
    let mut src = inner.to_string();
    for _ in 0..n {
        src = format!("{LEAF} OR {LEAF} AND ({src})");
    }
    src
}

/// `n` qualifiers stacked on one observation.
fn stacked_qualifiers(n: usize) -> String {
    format!("{LEAF}{}", " REPEATS 2 TIMES".repeat(n))
}

/// `n / 2` groups, each qualified: chain nodes, groups and qualifiers mixed.
fn qualified_groups(n: usize) -> String {
    let mut src = LEAF.to_string();
    for _ in 0..n / 2 {
        src = format!("{LEAF} FOLLOWEDBY {LEAF} OR {LEAF} AND ({src}) WITHIN 1 SECONDS");
    }
    src
}

fn worst_shapes(n: usize) -> Vec<(&'static str, String)> {
    vec![
        ("observation groups", observation_groups(n)),
        (
            "observation groups (matchable)",
            observation_groups_matchable(n),
        ),
        ("comparison groups", comparison_groups(n)),
        ("mixed groups", mixed_groups(n, wrap_observation)),
        (
            "mixed groups (matchable)",
            mixed_groups(n, wrap_observation_matchable),
        ),
        ("stacked qualifiers", stacked_qualifiers(n)),
        ("qualified groups", qualified_groups(n)),
    ]
}

#[test]
fn worst_shapes_at_the_limit_survive_the_pipeline_on_a_small_stack() {
    for (name, src) in worst_shapes(MAX_NESTING) {
        eprintln!("shape: {name}");
        on_small_stack_full_pipeline(src);
    }
}

#[test]
fn worst_shapes_past_the_limit_are_rejected() {
    for (name, src) in worst_shapes(MAX_NESTING + 2) {
        let err = parse(&src).expect_err(name);
        assert_eq!(err.message, "pattern nests too deeply", "{name}");
    }
}

#[test]
fn huge_nesting_is_rejected_without_crashing() {
    on_small_stack(|| {
        for n in [10_000, 100_000] {
            let parens = format!("{}{LEAF}{}", "(".repeat(n), ")".repeat(n));
            let err = parse(&parens).unwrap_err();
            assert_eq!(err.message, "pattern nests too deeply");
            let parens = format!("[{}{TEST}{}]", "(".repeat(n), ")".repeat(n));
            let err = parse(&parens).unwrap_err();
            assert_eq!(err.message, "pattern nests too deeply");
        }
    });
}

#[test]
fn long_observation_or_chain_survives_the_pipeline_on_a_small_stack() {
    on_small_stack_full_pipeline(vec![LEAF; 100_000].join(" OR "));
}

#[test]
fn long_comparison_or_chain_survives_the_pipeline_on_a_small_stack() {
    on_small_stack_full_pipeline(format!("[{}]", vec![TEST; 100_000].join(" OR ")));
}

#[test]
fn long_chain_of_distinct_types_survives_the_pipeline_on_a_small_stack() {
    let tests: Vec<String> = (0..100_000).map(|i| format!("t-{i}:v = 1")).collect();
    on_small_stack_full_pipeline(format!("[{} OR {TEST}]", tests.join(" OR ")));
}
