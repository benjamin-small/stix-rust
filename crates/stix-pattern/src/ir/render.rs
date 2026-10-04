//! Rendering: [`Program`] back to canonical STIX pattern text.

use std::collections::HashSet;

use crate::ast::{ComparisonOperator, Literal, ObjectPath, PathStep};
use crate::ir::{Block, InstrId, Instruction, Op, Operand, Program};

/// Render a program as canonical STIX pattern text.
///
/// The output is canonical rather than a reproduction of any original source:
/// whitespace is normalized, `!=` is the canonical spelling of not-equal, and
/// parentheses appear only where operator precedence requires them. (The
/// grammar's other spelling, `<>`, is accepted by the lexer and rendered as
/// `!=`.)
///
/// For a program lowered from [`parse`](crate::parse), reparsing this output
/// recovers the same AST up to spans, including non-ASCII string literals:
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
/// The guarantee assumes the AST came from the parser: a hand-built `Comparison`
/// using `EXISTS` with an operand other than `Literal::Boolean(true)` — the
/// placeholder the parser writes — loses that operand, since the IR records
/// `EXISTS` as having none.
///
/// Hand-built programs that share one value between two consumers render that
/// subexpression once per use, since pattern text has no way to name a shared
/// value. [`Program::validate`](crate::ir::Program::validate) rejects those, so
/// check any program that did not come from `lower` before rendering it.
///
/// Rendering walks the program with an explicit work stack rather than by
/// recursion, so nesting depth is limited only by memory, never by the call
/// stack. It also terminates on a cyclic program, which `validate` rejects, by
/// skipping any operand that names one of its own ancestors.
pub fn render(program: &Program) -> String {
    let mut out = String::new();
    // Not a well-formed program if there is no terminator, or the terminator
    // is not `Ret`; render nothing rather than panicking.
    if let Some(Instruction {
        op: Op::Ret { value },
        ..
    }) = program.main.terminator()
    {
        Renderer::new(program).run(*value, &mut out);
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

/// The two expression tiers, which the renderer walks with separate rules.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Tier {
    Observation,
    Comparison,
}

/// One pending step of rendering.
///
/// The renderer is a loop over a stack of these rather than a recursive descent,
/// so a deeply nested program costs heap rather than stack. Because the stack is
/// last-in first-out, an instruction's pieces are pushed in the *reverse* of the
/// order they are written.
enum Work<'p> {
    /// Write the observation-tier value `value` of `main`, parenthesized when its
    /// operator binds more loosely than `min_prec`.
    Observation { value: InstrId, min_prec: u8 },
    /// Write the comparison-tier value `value` of `block`, parenthesized when its
    /// operator binds more loosely than `min_prec`.
    Comparison {
        block: &'p Block,
        value: InstrId,
        min_prec: u8,
    },
    /// Write fixed text.
    Text(&'static str),
    /// Write computed text.
    Owned(String),
    /// Mark an instruction as finished, i.e. no longer on the current path.
    Leave(Tier, InstrId),
}

/// The state of one [`render`] call.
struct Renderer<'p> {
    program: &'p Program,
    stack: Vec<Work<'p>>,
    /// The instructions currently being written: each one's ancestors, plus
    /// itself. An operand already on the path names one of its own ancestors,
    /// which only a cyclic (and so invalid) program can do; it is skipped so that
    /// rendering terminates. An acyclic program never hits this, so its output is
    /// unaffected.
    ///
    /// Keyed by tier rather than block: the observation tier lives only in
    /// `main`, and the comparison tier never leaves the one block being written.
    on_path: HashSet<(Tier, InstrId)>,
}

impl<'p> Renderer<'p> {
    fn new(program: &'p Program) -> Self {
        Renderer {
            program,
            stack: Vec::new(),
            on_path: HashSet::new(),
        }
    }

    /// Write the observation-tier expression rooted at `root`.
    fn run(mut self, root: InstrId, out: &mut String) {
        self.stack.push(Work::Observation {
            value: root,
            min_prec: 1,
        });
        while let Some(work) = self.stack.pop() {
            match work {
                Work::Observation { value, min_prec } => self.observation(value, min_prec, out),
                Work::Comparison {
                    block,
                    value,
                    min_prec,
                } => self.comparison(block, value, min_prec, out),
                Work::Text(text) => out.push_str(text),
                Work::Owned(text) => out.push_str(&text),
                Work::Leave(tier, value) => {
                    self.on_path.remove(&(tier, value));
                }
            }
        }
    }

    /// Start writing `value` at `tier`: open its parentheses if it needs them and
    /// schedule the matching close. Returns `false`, writing nothing, when `value`
    /// is already on the current path.
    fn enter(
        &mut self,
        tier: Tier,
        value: InstrId,
        prec: u8,
        min_prec: u8,
        out: &mut String,
    ) -> bool {
        if !self.on_path.insert((tier, value)) {
            return false;
        }
        self.stack.push(Work::Leave(tier, value));
        if prec < min_prec {
            out.push('(');
            self.stack.push(Work::Text(")"));
        }
        true
    }

    /// Schedule `lhs`, then `sep`, then `rhs`.
    fn binary(&mut self, lhs: Work<'p>, sep: &'static str, rhs: Work<'p>) {
        self.stack.push(rhs);
        self.stack.push(Work::Text(sep));
        self.stack.push(lhs);
    }

    /// Write the observation-tier expression rooted at `value`.
    ///
    /// Mirrors the parser's observation-expression cascade —
    /// `parse_observation_expression` (`FOLLOWEDBY`) → `parse_observation_or` →
    /// `parse_observation_and` → `parse_observation_qualified` →
    /// `parse_observation_primary` — with [`obs_prec`] standing in for the
    /// cascade's levels. `min_prec` is the tightest precedence the surrounding
    /// context accepts; a looser operator is parenthesized. The right operand is
    /// rendered one level tighter than the left, which is what makes these
    /// operators left-associative on the way out, matching the parser's loops.
    ///
    /// A change to the parser's precedence needs a matching change here, or
    /// rendering stops round-tripping.
    fn observation(&mut self, value: InstrId, min_prec: u8, out: &mut String) {
        let program = self.program;
        let Some(instr) = program.main.instruction(value) else {
            return;
        };
        if !self.enter(Tier::Observation, value, obs_prec(&instr.op), min_prec, out) {
            return;
        }
        let obs = |value: InstrId, min_prec: u8| Work::Observation { value, min_prec };
        match &instr.op {
            Op::Observe { block: target } => {
                out.push('[');
                self.stack.push(Work::Text("]"));
                if let Some(cb) = program.block(*target) {
                    if let Some(Op::Yield { value }) = cb.terminator().map(|t| &t.op) {
                        self.stack.push(Work::Comparison {
                            block: cb,
                            value: *value,
                            min_prec: 1,
                        });
                    }
                }
            }
            Op::FollowedBy { lhs, rhs } => self.binary(obs(*lhs, 1), " FOLLOWEDBY ", obs(*rhs, 2)),
            Op::Or { lhs, rhs } => self.binary(obs(*lhs, 2), " OR ", obs(*rhs, 3)),
            Op::And { lhs, rhs } => self.binary(obs(*lhs, 3), " AND ", obs(*rhs, 4)),
            Op::Within { input, seconds } => {
                self.stack.push(Work::Owned(format!(
                    " WITHIN {} SECONDS",
                    render_seconds(*seconds)
                )));
                self.stack.push(obs(*input, 4));
            }
            Op::Repeats { input, count } => {
                self.stack
                    .push(Work::Owned(format!(" REPEATS {count} TIMES")));
                self.stack.push(obs(*input, 4));
            }
            Op::StartStop { input, start, stop } => {
                self.stack.push(Work::Owned(format!(
                    " START t'{}' STOP t'{}'",
                    escape_string(start),
                    escape_string(stop)
                )));
                self.stack.push(obs(*input, 4));
            }
            // Not valid at this tier; skip rather than panic.
            _ => {}
        }
    }

    /// Write the comparison-tier expression rooted at `value`, i.e. the inside of
    /// one `[...]`.
    ///
    /// Mirrors the parser's comparison-expression cascade —
    /// `parse_comparison_expression` (`OR`) → `parse_comparison_and` →
    /// `parse_prop_test` — with [`cmp_prec`] standing in for the cascade's
    /// levels. `min_prec` and the one-level-tighter right operand work exactly as
    /// in [`Renderer::observation`].
    ///
    /// A change to the parser's precedence needs a matching change here, or
    /// rendering stops round-tripping.
    fn comparison(&mut self, block: &'p Block, value: InstrId, min_prec: u8, out: &mut String) {
        let Some(instr) = block.instruction(value) else {
            return;
        };
        if !self.enter(Tier::Comparison, value, cmp_prec(&instr.op), min_prec, out) {
            return;
        }
        let cmp = |value: InstrId, min_prec: u8| Work::Comparison {
            block,
            value,
            min_prec,
        };
        match &instr.op {
            Op::Or { lhs, rhs } => self.binary(cmp(*lhs, 1), " OR ", cmp(*rhs, 2)),
            Op::And { lhs, rhs } => self.binary(cmp(*lhs, 2), " AND ", cmp(*rhs, 3)),
            Op::Compare {
                operator,
                negated,
                lhs,
                rhs,
            } => render_compare(block, *operator, *negated, *lhs, rhs, out),
            // A bare Load or a non-comparison op: nothing renderable.
            _ => {}
        }
    }
}

/// Write one comparison, a leaf of the comparison tier.
fn render_compare(
    block: &Block,
    operator: ComparisonOperator,
    negated: bool,
    lhs: InstrId,
    rhs: &Operand,
    out: &mut String,
) {
    let path = match block.instruction(lhs).map(|i| &i.op) {
        Some(Op::Load { path }) => Some(path),
        _ => None,
    };
    if operator == ComparisonOperator::Exists {
        out.push_str("EXISTS ");
        if let Some(p) = path {
            out.push_str(&render_path(p));
        }
        return;
    }
    if let Some(p) = path {
        out.push_str(&render_path(p));
    }
    out.push(' ');
    if negated {
        out.push_str("NOT ");
    }
    out.push_str(operator_text(operator));
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
        let reparsed =
            parse(&text).unwrap_or_else(|e| panic!("rendered text failed to parse: {text}\n{e}"));
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
            assert!(
                s.contains('.'),
                "{v:?} rendered as {s}, needs a decimal point"
            );
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
        assert_eq!(
            round("[file:is_encrypted = true]"),
            "[file:is_encrypted = true]"
        );
    }

    #[test]
    fn quotes_path_keys_that_need_it() {
        assert_eq!(
            round("[file:hashes.'SHA-256' = 'abc']"),
            "[file:hashes.'SHA-256' = 'abc']"
        );
        assert_eq!(
            round("[file:hashes.MD5 = 'abc']"),
            "[file:hashes.MD5 = 'abc']"
        );
    }

    #[test]
    fn renders_index_steps() {
        assert_eq!(
            round("[network-traffic:protocols[0] = 'tcp']"),
            "[network-traffic:protocols[0] = 'tcp']"
        );
        assert_eq!(
            round("[x-custom:list[*] = 'y']"),
            "[x-custom:list[*] = 'y']"
        );
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
        assert_round_trips(
            r"[file:name='a'] START t'2020-01-01T00:00:00Z\'x' STOP t'2020-01-02T00:00:00Z'",
        );
    }

    #[test]
    fn non_ascii_strings_round_trip() {
        assert_round_trips("[file:name = 'café']");
        assert_round_trips("[file:name = '日本語 😀 it\\'s é']");
        assert_eq!(round("[file:name='café']"), "[file:name = 'café']");
    }

    #[test]
    fn diamond_not_equal_renders_as_bang_equal() {
        assert_eq!(round("[file:size<>1]"), "[file:size != 1]");
        assert_round_trips("[file:size <> 1]");
    }

    /// Run `f` on a thread with a deliberately small stack, so a test proves that
    /// the code under test does not recurse in proportion to program depth.
    fn on_small_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(f)
            .expect("spawn test thread")
            .join()
            .expect("test thread panicked");
    }

    /// Nesting far beyond what a recursive renderer survives on a small stack.
    const DEEP: u32 = 10_000;

    /// `[file:size > 1024]` with `levels` `WITHIN` qualifiers chained onto it.
    fn qualifier_chain(levels: u32) -> crate::ir::Program {
        use crate::ir::{InstrId, Instruction, Op};

        let mut p = lower(&parse("[file:size > 1024]").unwrap());
        let observe = p.main.instructions[0].clone();
        let mut acc = observe.id;
        let mut instrs = vec![observe];
        let mut next = 1000u32;
        for _ in 0..levels {
            let id = InstrId(next);
            next += 1;
            instrs.push(Instruction {
                id,
                op: Op::Within {
                    input: acc,
                    seconds: 1.0,
                },
                span: None,
            });
            acc = id;
        }
        instrs.push(Instruction {
            id: InstrId(next),
            op: Op::Ret { value: acc },
            span: None,
        });
        p.main.instructions = instrs;
        p
    }

    /// A single observation whose comparison block chains `terms` copies of
    /// `file:size > 1024` with `AND`, nested to the left or to the right.
    fn comparison_and_chain(terms: u32, right_nested: bool) -> crate::ir::Program {
        use crate::ast::{ComparisonOperator, Literal};
        use crate::ir::{InstrId, Instruction, Op, Operand};

        let mut p = lower(&parse("[file:size > 1024]").unwrap());
        let path = match &p.blocks[0].instructions[0].op {
            Op::Load { path } => path.clone(),
            other => panic!("expected a load, got {other:?}"),
        };
        let mut next = 1000u32;
        let mut fresh = || {
            next += 1;
            InstrId(next)
        };
        let mut instrs = Vec::new();
        let compare = |instrs: &mut Vec<Instruction>, fresh: &mut dyn FnMut() -> InstrId| {
            let load = fresh();
            instrs.push(Instruction {
                id: load,
                op: Op::Load { path: path.clone() },
                span: None,
            });
            let cmp = fresh();
            instrs.push(Instruction {
                id: cmp,
                op: Op::Compare {
                    operator: ComparisonOperator::GreaterThan,
                    negated: false,
                    lhs: load,
                    rhs: Operand::Literal(Literal::Integer(1024)),
                },
                span: None,
            });
            cmp
        };
        let mut acc = compare(&mut instrs, &mut fresh);
        for _ in 1..terms {
            let term = compare(&mut instrs, &mut fresh);
            let id = fresh();
            let (lhs, rhs) = if right_nested {
                (term, acc)
            } else {
                (acc, term)
            };
            instrs.push(Instruction {
                id,
                op: Op::And { lhs, rhs },
                span: None,
            });
            acc = id;
        }
        instrs.push(Instruction {
            id: fresh(),
            op: Op::Yield { value: acc },
            span: None,
        });
        p.blocks[0].instructions = instrs;
        p
    }

    #[test]
    fn a_deep_qualifier_chain_validates_and_renders_on_a_small_stack() {
        on_small_stack(|| {
            let p = qualifier_chain(DEEP);
            assert_eq!(p.validate(), Ok(()));
            let expected = format!(
                "[file:size > 1024]{}",
                " WITHIN 1 SECONDS".repeat(DEEP as usize)
            );
            assert_eq!(render(&p), expected);
        });
    }

    #[test]
    fn a_deep_left_nested_comparison_chain_renders_without_parens() {
        on_small_stack(|| {
            let p = comparison_and_chain(DEEP, false);
            assert_eq!(p.validate(), Ok(()));
            let expected = format!(
                "[{}]",
                vec!["file:size > 1024"; DEEP as usize].join(" AND ")
            );
            assert_eq!(render(&p), expected);
        });
    }

    #[test]
    fn a_deep_right_nested_comparison_chain_parenthesizes_every_level() {
        on_small_stack(|| {
            let p = comparison_and_chain(DEEP, true);
            assert_eq!(p.validate(), Ok(()));
            let x = "file:size > 1024";
            let wraps = DEEP as usize - 2;
            let expected = format!(
                "[{}{x} AND {x}{}]",
                format!("{x} AND (").repeat(wraps),
                ")".repeat(wraps)
            );
            assert_eq!(render(&p), expected);
        });
    }

    #[test]
    fn a_deep_right_nested_observation_chain_parenthesizes_every_level() {
        use crate::ir::{BlockKind, Instruction, Op};

        on_small_stack(|| {
            // Build `[a] OR ([a] OR ([a] OR ...))` by lowering one observation per
            // term and stitching their blocks together under a right-nested OR.
            let template = lower(&parse("[file:size > 1024]").unwrap());
            let mut p = template.clone();
            p.blocks.clear();
            p.main.instructions.clear();
            let mut next_instr = 100_000u32;
            let mut acc = None;
            for term in 0..DEEP {
                let mut b = template.blocks[0].clone();
                b.id = crate::ir::BlockId(1000 + term);
                for instr in &mut b.instructions {
                    let old = instr.id;
                    instr.id = crate::ir::InstrId(next_instr + old.0);
                    match &mut instr.op {
                        Op::Compare { lhs, .. } => lhs.0 += next_instr,
                        Op::Yield { value } => value.0 += next_instr,
                        _ => {}
                    }
                }
                next_instr += 10;
                assert_eq!(b.kind, BlockKind::Comparison);
                let observe = crate::ir::InstrId(next_instr);
                next_instr += 10;
                p.main.instructions.push(Instruction {
                    id: observe,
                    op: Op::Observe { block: b.id },
                    span: None,
                });
                p.blocks.push(b);
                acc = Some(match acc {
                    None => observe,
                    Some(prev) => {
                        let or = crate::ir::InstrId(next_instr);
                        next_instr += 10;
                        p.main.instructions.push(Instruction {
                            id: or,
                            op: Op::Or {
                                lhs: observe,
                                rhs: prev,
                            },
                            span: None,
                        });
                        or
                    }
                });
            }
            p.main.instructions.push(Instruction {
                id: crate::ir::InstrId(next_instr),
                op: Op::Ret {
                    value: acc.unwrap(),
                },
                span: None,
            });
            assert_eq!(p.validate(), Ok(()));
            let x = "[file:size > 1024]";
            let wraps = DEEP as usize - 2;
            let expected = format!(
                "{}{x} OR {x}{}",
                format!("{x} OR (").repeat(wraps),
                ")".repeat(wraps)
            );
            assert_eq!(render(&p), expected);
        });
    }

    #[test]
    fn a_cyclic_unvalidated_program_renders_without_hanging() {
        use crate::ir::Op;

        // An `and` that names itself: `validate` rejects this as a forward
        // reference, but `render` must still terminate on it.
        on_small_stack(|| {
            let mut p = lower(&parse("[file:size > 1] AND [file:size > 2]").unwrap());
            let and = p
                .main
                .instructions
                .iter_mut()
                .find(|i| matches!(i.op, Op::And { .. }))
                .expect("an and");
            let id = and.id;
            and.op = Op::And { lhs: id, rhs: id };
            assert!(p.validate().is_err());
            assert_eq!(render(&p), " AND ");
        });
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
