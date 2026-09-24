//! Rendering: [`Program`] back to canonical STIX pattern text.

use crate::ast::{ComparisonOperator, Literal, ObjectPath, PathStep};
use crate::ir::{Block, InstrId, Instruction, Op, Operand, Program};

/// Render a program as canonical STIX pattern text.
///
/// The output is canonical rather than a reproduction of any original source:
/// whitespace is normalized, `!=` is the canonical spelling of not-equal, and
/// parentheses appear only where operator precedence requires them. (The
/// grammar's other spelling, `<>`, is not currently accepted by this crate's
/// lexer; see [issue #29](https://github.com/benjamin-small/stix-rust/issues/29).)
///
/// For a program lowered from [`parse`](crate::parse), reparsing this output
/// recovers the same AST up to spans, as long as the pattern is ASCII:
///
/// ```
/// use stix_pattern::{parse, ir};
///
/// let ast = parse("[file:size>1024] OR [file:name='a']").unwrap();
/// let text = ir::render(&ir::lower(&ast));
/// assert_eq!(text, "[file:size > 1024] OR [file:name = 'a']");
/// assert_eq!(parse(&text).unwrap().without_spans(), ast.without_spans());
/// ```
///
/// A non-ASCII string literal does **not** survive the round trip, because the
/// lexer decodes string literals as Latin-1; see
/// [issue #30](https://github.com/benjamin-small/stix-rust/issues/30). The
/// guarantee also assumes the AST came from the parser: a hand-built `Comparison`
/// using `EXISTS` with an operand other than `Literal::Boolean(true)` — the
/// placeholder the parser writes — loses that operand, since the IR records
/// `EXISTS` as having none.
///
/// Hand-built programs that share one value between two consumers render that
/// subexpression once per use, since pattern text has no way to name a shared
/// value. [`Program::validate`](crate::ir::Program::validate) rejects those, so
/// check any program that did not come from `lower` before rendering it.
pub fn render(program: &Program) -> String {
    let mut out = String::new();
    // Not a well-formed program if there is no terminator, or the terminator
    // is not `Ret`; render nothing rather than panicking.
    if let Some(Instruction {
        op: Op::Ret { value },
        ..
    }) = program.main.terminator()
    {
        render_observation(program, &program.main, *value, 1, &mut out);
    }
    out
}

/// Precedence of an observation-tier op: higher binds tighter.
fn obs_prec(op: &Op) -> u8 {
    match op {
        Op::FollowedBy { .. } => 1,
        Op::Or { .. } => 2,
        Op::And { .. } => 3,
        Op::Within { .. } | Op::Repeats { .. } | Op::StartStop { .. } => 4,
        _ => 5,
    }
}

/// Precedence of a comparison-tier op: higher binds tighter.
fn cmp_prec(op: &Op) -> u8 {
    match op {
        Op::Or { .. } => 1,
        Op::And { .. } => 2,
        _ => 3,
    }
}

/// Write the observation-tier expression rooted at `value`.
///
/// Mirrors the parser's observation-expression cascade —
/// `parse_observation_expression` (`FOLLOWEDBY`) → `parse_observation_or` →
/// `parse_observation_and` → `parse_observation_qualified` →
/// `parse_observation_primary` — with [`obs_prec`] standing in for the cascade's
/// levels. `min_prec` is the tightest precedence the surrounding context accepts;
/// a looser operator is parenthesized. The right operand is rendered one level
/// tighter than the left, which is what makes these operators left-associative on
/// the way out, matching the parser's loops.
///
/// A change to the parser's precedence needs a matching change here, or rendering
/// stops round-tripping.
fn render_observation(
    program: &Program,
    block: &Block,
    value: InstrId,
    min_prec: u8,
    out: &mut String,
) {
    let Some(instr) = block.instruction(value) else {
        return;
    };
    let prec = obs_prec(&instr.op);
    let parens = prec < min_prec;
    if parens {
        out.push('(');
    }
    match &instr.op {
        Op::Observe { block: target } => {
            out.push('[');
            if let Some(cb) = program.block(*target) {
                if let Some(Op::Yield { value }) = cb.terminator().map(|t| &t.op) {
                    render_comparison(cb, *value, 1, out);
                }
            }
            out.push(']');
        }
        Op::FollowedBy { lhs, rhs } => {
            render_observation(program, block, *lhs, 1, out);
            out.push_str(" FOLLOWEDBY ");
            render_observation(program, block, *rhs, 2, out);
        }
        Op::Or { lhs, rhs } => {
            render_observation(program, block, *lhs, 2, out);
            out.push_str(" OR ");
            render_observation(program, block, *rhs, 3, out);
        }
        Op::And { lhs, rhs } => {
            render_observation(program, block, *lhs, 3, out);
            out.push_str(" AND ");
            render_observation(program, block, *rhs, 4, out);
        }
        Op::Within { input, seconds } => {
            render_observation(program, block, *input, 4, out);
            out.push_str(" WITHIN ");
            out.push_str(&render_seconds(*seconds));
            out.push_str(" SECONDS");
        }
        Op::Repeats { input, count } => {
            render_observation(program, block, *input, 4, out);
            out.push_str(&format!(" REPEATS {count} TIMES"));
        }
        Op::StartStop { input, start, stop } => {
            render_observation(program, block, *input, 4, out);
            out.push_str(&format!(
                " START t'{}' STOP t'{}'",
                escape_string(start),
                escape_string(stop)
            ));
        }
        // Not valid at this tier; skip rather than panic.
        _ => {}
    }
    if parens {
        out.push(')');
    }
}

/// Write the comparison-tier expression rooted at `value`, i.e. the inside of one
/// `[...]`.
///
/// Mirrors the parser's comparison-expression cascade —
/// `parse_comparison_expression` (`OR`) → `parse_comparison_and` →
/// `parse_prop_test` — with [`cmp_prec`] standing in for the cascade's levels.
/// `min_prec` and the one-level-tighter right operand work exactly as in
/// [`render_observation`].
///
/// A change to the parser's precedence needs a matching change here, or rendering
/// stops round-tripping.
fn render_comparison(block: &Block, value: InstrId, min_prec: u8, out: &mut String) {
    let Some(instr) = block.instruction(value) else {
        return;
    };
    let prec = cmp_prec(&instr.op);
    let parens = prec < min_prec;
    if parens {
        out.push('(');
    }
    match &instr.op {
        Op::Or { lhs, rhs } => {
            render_comparison(block, *lhs, 1, out);
            out.push_str(" OR ");
            render_comparison(block, *rhs, 2, out);
        }
        Op::And { lhs, rhs } => {
            render_comparison(block, *lhs, 2, out);
            out.push_str(" AND ");
            render_comparison(block, *rhs, 3, out);
        }
        Op::Compare {
            operator,
            negated,
            lhs,
            rhs,
        } => {
            let path = match block.instruction(*lhs).map(|i| &i.op) {
                Some(Op::Load { path }) => Some(path),
                _ => None,
            };
            if *operator == ComparisonOperator::Exists {
                out.push_str("EXISTS ");
                if let Some(p) = path {
                    out.push_str(&render_path(p));
                }
            } else {
                if let Some(p) = path {
                    out.push_str(&render_path(p));
                }
                out.push(' ');
                if *negated {
                    out.push_str("NOT ");
                }
                out.push_str(operator_text(*operator));
                out.push(' ');
                match rhs {
                    Operand::Literal(lit) => out.push_str(&render_literal(lit)),
                    Operand::Set(items) => {
                        let inner: Vec<String> = items.iter().map(render_literal).collect();
                        out.push('(');
                        out.push_str(&inner.join(", "));
                        out.push(')');
                    }
                    Operand::Absent => {}
                }
            }
        }
        // A bare Load or a non-comparison op: nothing renderable.
        _ => {}
    }
    if parens {
        out.push(')');
    }
}

fn operator_text(op: ComparisonOperator) -> &'static str {
    match op {
        ComparisonOperator::Equal => "=",
        ComparisonOperator::NotEqual => "!=",
        ComparisonOperator::GreaterThan => ">",
        ComparisonOperator::GreaterThanOrEqual => ">=",
        ComparisonOperator::LessThan => "<",
        ComparisonOperator::LessThanOrEqual => "<=",
        ComparisonOperator::In => "IN",
        ComparisonOperator::Like => "LIKE",
        ComparisonOperator::Matches => "MATCHES",
        ComparisonOperator::IsSubset => "ISSUBSET",
        ComparisonOperator::IsSuperset => "ISSUPERSET",
        // Handled by the caller, which emits the prefix form.
        ComparisonOperator::Exists => "EXISTS",
    }
}

/// Render an object path, quoting the key steps that require it.
pub(crate) fn render_path(path: &ObjectPath) -> String {
    let mut out = String::new();
    out.push_str(&path.object_type);
    out.push(':');
    let mut first = true;
    for step in &path.steps {
        match step {
            PathStep::Key(k) => {
                if !first {
                    out.push('.');
                }
                out.push_str(&render_key(k));
            }
            PathStep::Index(n) => {
                out.push_str(&format!("[{n}]"));
            }
            PathStep::AnyIndex => out.push_str("[*]"),
        }
        first = false;
    }
    out
}

/// A path key, bare when it lexes as a plain identifier, quoted otherwise.
fn render_key(key: &str) -> String {
    let plain = !key.is_empty()
        && key
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !is_keyword(key);
    if plain {
        key.to_string()
    } else {
        format!("'{}'", escape_string(key))
    }
}

/// Whether a bare word would lex as a keyword rather than an identifier.
///
/// Mirrors `lexer::keyword`, which is case-insensitive.
fn is_keyword(word: &str) -> bool {
    matches!(
        word.to_ascii_uppercase().as_str(),
        "AND"
            | "OR"
            | "NOT"
            | "FOLLOWEDBY"
            | "LIKE"
            | "MATCHES"
            | "IN"
            | "ISSUBSET"
            | "ISSUPERSET"
            | "EXISTS"
            | "WITHIN"
            | "REPEATS"
            | "SECONDS"
            | "TIMES"
            | "START"
            | "STOP"
            | "TRUE"
            | "FALSE"
    )
}

/// Escape a string for a single-quoted STIX literal.
pub(crate) fn escape_string(s: &str) -> String {
    s.replace('\\', r"\\").replace('\'', r"\'")
}

/// The shortest decimal-notation rendering that parses back to exactly `v`.
///
/// Neither `{}` nor `{:?}` is usable here: `{}` drops the decimal point on whole
/// numbers, so the value reparses as an integer, and `{:?}` switches to exponent
/// notation for extreme magnitudes, which this dialect's lexer cannot read. The
/// search starts at one decimal place, so ordinary values are as short as
/// expected (`60.0`, `0.001`) and only extreme ones iterate.
///
/// `v` must be finite; [`Program::validate`](crate::ir::Program::validate)
/// rejects non-finite floats, which STIX pattern syntax cannot express.
pub(crate) fn render_float(v: f64) -> String {
    for p in 1..=1100usize {
        let s = format!("{v:.p$}");
        if s.parse::<f64>() == Ok(v) {
            return s;
        }
    }
    // Unreachable for finite f64; keeps the function total rather than panicking.
    format!("{v:.1}")
}

/// Render a literal in its source form.
pub(crate) fn render_literal(lit: &Literal) -> String {
    match lit {
        Literal::String(s) => format!("'{}'", escape_string(s)),
        Literal::Integer(n) => n.to_string(),
        Literal::Float(f) => render_float(*f),
        Literal::Boolean(b) => b.to_string(),
        Literal::Timestamp(s) => format!("t'{}'", escape_string(s)),
        Literal::Binary(s) => format!("b'{}'", escape_string(s)),
        Literal::Hex(s) => format!("h'{}'", escape_string(s)),
    }
}

/// Render a `WITHIN` window. The parser accepts an integer or a float here, so a
/// whole number renders without a decimal point.
///
/// Falls back to [`render_float`] when the value is not exactly representable as
/// an `i64`, because `as i64` saturates rather than failing — which would
/// silently change the number.
pub(crate) fn render_seconds(seconds: f64) -> String {
    if seconds.fract() == 0.0 && seconds.is_finite() {
        let as_int = seconds as i64;
        if as_int as f64 == seconds {
            return format!("{as_int}");
        }
    }
    render_float(seconds)
}

#[cfg(test)]
mod tests {
    use crate::ir::{lower, render};
    use crate::parse;

    /// Render the lowering of `src`.
    fn round(src: &str) -> String {
        render(&lower(&parse(src).unwrap()))
    }

    /// Assert that rendering then reparsing recovers the same AST.
    fn assert_round_trips(src: &str) {
        let original = parse(src).unwrap();
        let text = render(&lower(&original));
        let reparsed = parse(&text)
            .unwrap_or_else(|e| panic!("rendered text failed to parse: {text}\n{e}"));
        assert_eq!(
            reparsed.without_spans(),
            original.without_spans(),
            "round trip changed the AST\n  src:      {src}\n  rendered: {text}"
        );
    }

    #[test]
    fn renders_canonical_whitespace_and_operators() {
        assert_eq!(round("[file:name='a']"), "[file:name = 'a']");
        assert_eq!(round("[file:size != 1]"), "[file:size != 1]");
    }

    #[test]
    fn renders_exists_and_in() {
        assert_eq!(round("[EXISTS file:name]"), "[EXISTS file:name]");
        assert_eq!(
            round("[ipv4-addr:value IN ('1.1.1.1', '8.8.8.8')]"),
            "[ipv4-addr:value IN ('1.1.1.1', '8.8.8.8')]"
        );
    }

    #[test]
    fn renders_negation_in_operator_position() {
        assert_eq!(round("[file:name NOT = 'x']"), "[file:name NOT = 'x']");
    }

    #[test]
    fn omits_parens_where_precedence_makes_them_redundant() {
        assert_eq!(
            round("[(file:name = 'a') AND file:size = 1]"),
            "[file:name = 'a' AND file:size = 1]"
        );
    }

    #[test]
    fn adds_parens_where_precedence_demands_them() {
        assert_eq!(
            round("[(file:name = 'a' OR file:name = 'b') AND file:size = 1]"),
            "[(file:name = 'a' OR file:name = 'b') AND file:size = 1]"
        );
    }

    #[test]
    fn parenthesizes_a_right_nested_same_operator() {
        // Built via explicit parens so the AST is right-nested.
        assert_round_trips("[file:name = 'a' OR (file:name = 'b' OR file:name = 'c')]");
        assert_eq!(
            round("[file:name = 'a' OR (file:name = 'b' OR file:name = 'c')]"),
            "[file:name = 'a' OR (file:name = 'b' OR file:name = 'c')]"
        );
    }

    #[test]
    fn parenthesizes_a_qualified_boolean_observation() {
        // Qualifiers bind tighter than AND, so the AND needs parens.
        assert_round_trips("([file:name='a'] AND [file:name='b']) WITHIN 60 SECONDS");
        assert_eq!(
            round("([file:name='a'] AND [file:name='b']) WITHIN 60 SECONDS"),
            "([file:name = 'a'] AND [file:name = 'b']) WITHIN 60 SECONDS"
        );
    }

    #[test]
    fn renders_nested_qualifiers_without_parens() {
        assert_eq!(
            round("[file:name='a'] REPEATS 2 TIMES WITHIN 60 SECONDS"),
            "[file:name = 'a'] REPEATS 2 TIMES WITHIN 60 SECONDS"
        );
    }

    #[test]
    fn preserves_float_literals_as_floats() {
        assert_eq!(round("[file:size > 60.0]"), "[file:size > 60.0]");
        assert_round_trips("[file:size > 60.0]");
    }

    #[test]
    fn renders_extreme_floats_in_decimal_notation() {
        // The exponent-notation guarantee is asserted on the number itself in
        // float_rendering_round_trips_every_extreme; here the property that matters
        // is that a pattern carrying such a float survives the round trip at all.
        assert_round_trips("[file:size > 0.0000000001]");
        assert_round_trips("[file:size > 123456789012345680.0]");
    }

    #[test]
    fn float_rendering_stays_short_for_ordinary_values() {
        use super::render_float;
        assert_eq!(render_float(60.0), "60.0");
        assert_eq!(render_float(1.5), "1.5");
        assert_eq!(render_float(0.001), "0.001");
    }

    #[test]
    fn float_rendering_round_trips_every_extreme() {
        use super::render_float;
        for v in [
            0.0_f64,
            -1.0,
            1e-10,
            1e16,
            1e-300,
            f64::MAX,
            f64::MIN_POSITIVE,
            f64::EPSILON,
            5e-324,
        ] {
            let s = render_float(v);
            assert!(!s.contains('e'), "{v:?} rendered as {s}");
            assert!(s.contains('.'), "{v:?} rendered as {s}, needs a decimal point");
            assert_eq!(s.parse::<f64>().unwrap(), v, "{v:?} rendered as {s}");
        }
    }

    #[test]
    fn renders_typed_literals() {
        assert_eq!(
            round("[file:created = t'2020-01-01T00:00:00Z']"),
            "[file:created = t'2020-01-01T00:00:00Z']"
        );
        assert_eq!(
            round("[artifact:payload_bin = b'aGVsbG8=']"),
            "[artifact:payload_bin = b'aGVsbG8=']"
        );
        assert_eq!(
            round("[file:magic_number_hex = h'cafebabe']"),
            "[file:magic_number_hex = h'cafebabe']"
        );
        assert_eq!(round("[file:is_encrypted = true]"), "[file:is_encrypted = true]");
    }

    #[test]
    fn quotes_path_keys_that_need_it() {
        assert_eq!(
            round("[file:hashes.'SHA-256' = 'abc']"),
            "[file:hashes.'SHA-256' = 'abc']"
        );
        assert_eq!(round("[file:hashes.MD5 = 'abc']"), "[file:hashes.MD5 = 'abc']");
    }

    #[test]
    fn renders_index_steps() {
        assert_eq!(
            round("[network-traffic:protocols[0] = 'tcp']"),
            "[network-traffic:protocols[0] = 'tcp']"
        );
        assert_eq!(round("[x-custom:list[*] = 'y']"), "[x-custom:list[*] = 'y']");
    }

    #[test]
    fn escapes_quotes_and_backslashes_in_strings() {
        let src = r"[file:name = 'it\'s a \\ backslash']";
        assert_round_trips(src);
    }

    #[test]
    fn renders_start_stop() {
        assert_eq!(
            round("[file:name='a'] START t'2020-01-01T00:00:00Z' STOP t'2020-01-02T00:00:00Z'"),
            "[file:name = 'a'] START t'2020-01-01T00:00:00Z' STOP t'2020-01-02T00:00:00Z'"
        );
    }

    #[test]
    fn escapes_quotes_in_start_stop_timestamps() {
        // parse_timestamp_string does no RFC3339 validation, so a quote can reach
        // the IR and must be re-escaped on the way out.
        assert_round_trips(r"[file:name='a'] START t'2020-01-01T00:00:00Z\'x' STOP t'2020-01-02T00:00:00Z'");
    }

    /// Un-ignore this when #30 lands: it pins the one documented hole in the
    /// round-trip guarantee, so the gap surfaces rather than rotting.
    #[test]
    #[ignore = "blocked on lexer issue #30: string literals are decoded as Latin-1"]
    fn non_ascii_strings_round_trip() {
        assert_round_trips("[file:name = 'café']");
    }

    #[test]
    fn large_within_windows_survive_the_round_trip() {
        // `as i64` saturates; a whole number beyond i64::MAX must not be rendered
        // through the integer path.
        assert_round_trips("[file:name='a'] WITHIN 10000000000000000000.0 SECONDS");
        // The ordinary whole-number case still renders without a decimal point.
        assert_eq!(
            round("[file:name='a'] WITHIN 60 SECONDS"),
            "[file:name = 'a'] WITHIN 60 SECONDS"
        );
        // A fractional window keeps its fraction.
        assert_eq!(
            round("[file:name='a'] WITHIN 0.5 SECONDS"),
            "[file:name = 'a'] WITHIN 0.5 SECONDS"
        );
    }
}
