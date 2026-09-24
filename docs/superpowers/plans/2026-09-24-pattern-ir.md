# Pattern IR (Three-Address Code) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a three-address intermediate representation of STIX patterns alongside the existing AST, plus a renderer that turns IR back into canonical pattern text, proven faithful by a round-trip property over the conformance corpus.

**Architecture:** A new `ir` module inside `stix-pattern`. `lower(&Pattern) -> Program` produces a `Program` of comparison blocks (each the per-binding evaluation unit for one `[...]` observation) plus one `main` block combining observations. Instructions are SSA — a value is named by the `InstrId` of the instruction producing it. `render(&Program) -> String` walks the IR back to canonical pattern text using operator precedence to decide parenthesization.

**Tech Stack:** Rust 2021, `serde` 1 (derive), `thiserror` 1, `serde_json` 1 (dev only). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-23-pattern-ir-design.md`

## Global Constraints

- **Crate:** all code lives in `crates/stix-pattern/`, except mechanical fixups in `crates/stix-matcher/` (Task 1). Both are `crates/**`, owned by the `rust-core` agent.
- **No new dependencies.** `serde`, `thiserror`, `serde_json` are already available via `[workspace.dependencies]`.
- **`#![warn(missing_docs)]` is enforced crate-wide.** Every `pub` item — including every struct field and enum variant — needs a doc comment. A missing one is a build warning that CI treats as failure.
- **Edition 2021**, `edition.workspace = true`. Do not add an MSRV or edition key.
- **`pub const SCHEMA_VERSION: u32 = 1;`** — the IR's serialized schema version.
- **Public fields** on `Program`, `Block`, `Instruction` (matching the existing AST style). Invariants live in `validate()`, not in constructors.
- **Dead instructions are legal.** `validate()` rejects dangling *references*, never unreferenced instructions.
- **Commit style:** conventional commits (`feat:`, `test:`, `docs:`, `refactor:`). Do not add attribution or co-author trailers to commit messages.
- **Verification command** for the whole workspace: `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`.

## Spec Deltas

Three corrections to the spec's data-model sketch, forced by implementation reality. Apply these; do not follow the spec's literal sketch where it conflicts.

1. **All `Op` variants are struct variants, not tuple variants.** The spec wrote `And(InstrId, InstrId)`. serde's internally-tagged representation (`#[serde(tag = "op")]`, which the spec requires) **cannot encode tuple variants** — it is a compile error. So `And { lhs, rhs }`, `Or { lhs, rhs }`, `FollowedBy { lhs, rhs }`, `Yield { value }`, `Ret { value }`.
2. **`Compare`'s operator field is named `operator`, not `op`.** A field named `op` collides with the `#[serde(tag = "op")]` discriminant. `operator` also matches the AST's `Comparison::operator`.
3. **`Operand::None` is renamed `Operand::Absent`.** `Operand::None` shadows `Option::None` under `use Operand::*` and reads as a bug at every call site.
4. **`validate()` gains a ninth rule: float literals must be finite.** The spec listed eight rules. STIX pattern syntax has no notation for `NaN` or `±inf` — the lexer at `lexer.rs:250-277` reads only digits and `.` — so a non-finite float in a hand-built IR is unrenderable. Rejecting it in `validate()` is better than emitting text that cannot be parsed back. Adds `IrError::NonFiniteFloat`.

All four deltas were verified empirically during planning, not assumed. Delta 1's exact compiler error is `#[serde(tag = "...")] cannot be used with tuple variants`.

---

### Task 1: Thread source spans through the AST

**Files:**
- Modify: `crates/stix-pattern/src/error.rs` (add `Default` to `Span`)
- Modify: `crates/stix-pattern/src/ast.rs` (4 span fields + `without_spans`)
- Modify: `crates/stix-pattern/src/parser.rs` (span helpers + 4 construction sites + test fixups)
- Modify: `crates/stix-matcher/src/eval.rs:219`, `crates/stix-matcher/src/eval.rs:267-275`, `crates/stix-matcher/src/resolve.rs:96-100` (mechanical fixups)

**Interfaces:**
- Consumes: nothing.
- Produces: `Comparison::span: Span`, `ObjectPath::span: Span`, `ObservationExpression::Observation { expression: Box<ComparisonExpression>, span: Span }`, `ObservationExpression::Qualified { expression, qualifier, span: Span }`, and `Pattern::without_spans(&self) -> Pattern`. Task 3 reads these spans; Task 7 uses `without_spans`.

This is the breaking change: `Observation` goes from a tuple variant to a struct variant, changing its JSON from `{"Observation": {...}}` to `{"Observation": {"expression": {...}, "span": {...}}}`.

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block at the bottom of `crates/stix-pattern/src/parser.rs`:

```rust
#[test]
fn records_spans_on_comparison_and_path() {
    let src = "[file:size > 1024]";
    let p = parse(src).unwrap();
    let ObservationExpression::Observation { expression, span } = &p.expression else {
        panic!("expected an observation");
    };
    // The observation span covers the whole `[...]` including brackets.
    assert_eq!(&src[span.start..span.end], "[file:size > 1024]");
    let ComparisonExpression::Test(c) = expression.as_ref() else {
        panic!("expected a test");
    };
    // The comparison span covers the property test, without the brackets.
    assert_eq!(&src[c.span.start..c.span.end], "file:size > 1024");
    // The path span covers only the object path.
    assert_eq!(&src[c.path.span.start..c.path.span.end], "file:size");
}

#[test]
fn records_span_on_qualifier_clause() {
    let src = "[file:name='a'] WITHIN 60 SECONDS";
    let p = parse(src).unwrap();
    let ObservationExpression::Qualified { span, .. } = &p.expression else {
        panic!("expected a qualified expression");
    };
    assert_eq!(&src[span.start..span.end], "WITHIN 60 SECONDS");
}

#[test]
fn without_spans_zeroes_every_span() {
    let a = parse("[file:size > 1024] WITHIN 60 SECONDS").unwrap();
    let b = parse("   [file:size > 1024]   WITHIN 60 SECONDS   ").unwrap();
    assert_ne!(a, b, "differing offsets should make the raw ASTs unequal");
    assert_eq!(
        a.without_spans(),
        b.without_spans(),
        "structurally identical patterns should compare equal without spans"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p stix-pattern records_spans_on_comparison_and_path`
Expected: FAIL to compile — `ObservationExpression::Observation` is a tuple variant, so the struct pattern does not match, and `Comparison` has no field `span`.

- [ ] **Step 3: Add `Default` to `Span`**

In `crates/stix-pattern/src/error.rs`, extend the derive on `Span`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// The inclusive start byte offset.
    pub start: usize,
    /// The exclusive end byte offset.
    pub end: usize,
}
```

- [ ] **Step 4: Add the span fields to the AST**

In `crates/stix-pattern/src/ast.rs`, add the import and the four changes:

```rust
use crate::error::Span;
```

```rust
pub struct Comparison {
    /// The object path on the left-hand side.
    pub path: ObjectPath,
    /// The comparison operator.
    pub operator: ComparisonOperator,
    /// `true` if a `NOT` preceded the operator.
    pub negated: bool,
    /// The right-hand-side operand.
    pub value: ComparisonOperand,
    /// Byte range of the whole property test in the source pattern.
    pub span: Span,
}
```

```rust
pub struct ObjectPath {
    /// The STIX object type before the colon (e.g. `file`).
    pub object_type: String,
    /// The property steps after the colon, in order.
    pub steps: Vec<PathStep>,
    /// Byte range of the path text in the source pattern.
    pub span: Span,
}
```

```rust
pub enum ObservationExpression {
    /// A single `[ comparisonExpr ]` observation.
    Observation {
        /// The comparison expression inside the brackets.
        expression: Box<ComparisonExpression>,
        /// Byte range of the whole `[...]` including the brackets.
        span: Span,
    },
    /// `AND` of two observation expressions.
    And(Box<ObservationExpression>, Box<ObservationExpression>),
    /// `OR` of two observation expressions.
    Or(Box<ObservationExpression>, Box<ObservationExpression>),
    /// `FOLLOWEDBY`: the left expression's observations precede the right's.
    FollowedBy(Box<ObservationExpression>, Box<ObservationExpression>),
    /// A sub-expression with a postfix [`Qualifier`] attached.
    Qualified {
        /// The qualified sub-expression.
        expression: Box<ObservationExpression>,
        /// The attached qualifier.
        qualifier: Qualifier,
        /// Byte range of the qualifier clause only (e.g. `WITHIN 60 SECONDS`).
        span: Span,
    },
}
```

`And`, `Or` and `FollowedBy` deliberately get no span — their extent is the union of their children's.

- [ ] **Step 5: Add `without_spans` to `ast.rs`**

Append to `crates/stix-pattern/src/ast.rs`, above the `mod tests` block:

```rust
/// A span of zero length at offset 0, used as the normalized value by
/// [`Pattern::without_spans`].
const ZERO_SPAN: Span = Span { start: 0, end: 0 };

impl Pattern {
    /// A copy of this pattern with every source span zeroed.
    ///
    /// Two patterns that differ only in whitespace or in where they appeared in
    /// a source string compare equal after this normalization. Used to state
    /// structural equality independent of byte offsets.
    pub fn without_spans(&self) -> Pattern {
        Pattern {
            expression: strip_observation_spans(&self.expression),
        }
    }
}

fn strip_observation_spans(e: &ObservationExpression) -> ObservationExpression {
    use ObservationExpression as O;
    match e {
        O::Observation { expression, .. } => O::Observation {
            expression: Box::new(strip_comparison_spans(expression)),
            span: ZERO_SPAN,
        },
        O::And(l, r) => O::And(
            Box::new(strip_observation_spans(l)),
            Box::new(strip_observation_spans(r)),
        ),
        O::Or(l, r) => O::Or(
            Box::new(strip_observation_spans(l)),
            Box::new(strip_observation_spans(r)),
        ),
        O::FollowedBy(l, r) => O::FollowedBy(
            Box::new(strip_observation_spans(l)),
            Box::new(strip_observation_spans(r)),
        ),
        O::Qualified {
            expression,
            qualifier,
            ..
        } => O::Qualified {
            expression: Box::new(strip_observation_spans(expression)),
            qualifier: qualifier.clone(),
            span: ZERO_SPAN,
        },
    }
}

fn strip_comparison_spans(e: &ComparisonExpression) -> ComparisonExpression {
    use ComparisonExpression as C;
    match e {
        C::Test(c) => C::Test(Comparison {
            path: ObjectPath {
                object_type: c.path.object_type.clone(),
                steps: c.path.steps.clone(),
                span: ZERO_SPAN,
            },
            operator: c.operator,
            negated: c.negated,
            value: c.value.clone(),
            span: ZERO_SPAN,
        }),
        C::And(l, r) => C::And(
            Box::new(strip_comparison_spans(l)),
            Box::new(strip_comparison_spans(r)),
        ),
        C::Or(l, r) => C::Or(
            Box::new(strip_comparison_spans(l)),
            Box::new(strip_comparison_spans(r)),
        ),
    }
}
```

- [ ] **Step 6: Add span helpers to the parser**

In `crates/stix-pattern/src/parser.rs`, add to the `// --- cursor helpers ---` section, after `current_span`:

```rust
    /// Span of the most recently consumed token, or a zero span at the start.
    fn prev_span(&self) -> Span {
        match self.pos.checked_sub(1).and_then(|i| self.tokens.get(i)) {
            Some(t) => t.span,
            None => Span::new(0, 0),
        }
    }

    /// A span from `start` to the end of the most recently consumed token.
    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.prev_span().end)
    }
```

- [ ] **Step 7: Populate spans at the four construction sites**

In `parse_object_path`, capture the start before consuming anything and attach the span at the end:

```rust
    pub(crate) fn parse_object_path(&mut self) -> Result<ObjectPath> {
        let start = self.current_span().start;
        // object-type
        let object_type = match self.advance().map(|t| &t.kind) {
```

and change the final `Ok(...)`:

```rust
        Ok(ObjectPath {
            object_type,
            steps,
            span: self.span_from(start),
        })
    }
```

In `parse_prop_test`, capture `start` at the very top (before the `LParen`/`EXISTS` checks) and attach it to both `Comparison` constructions:

```rust
    fn parse_prop_test(&mut self) -> Result<ComparisonExpression> {
        let start = self.current_span().start;

        // Parenthesized sub-expression
        if self.eat(&TokenKind::LParen) {
            let inner = self.parse_comparison_expression()?;
            self.expect(&TokenKind::RParen, "')' to close comparison group")?;
            return Ok(inner);
        }

        // EXISTS objectPath
        if self.eat(&TokenKind::Exists) {
            let path = self.parse_object_path()?;
            return Ok(ComparisonExpression::Test(Comparison {
                path,
                operator: ComparisonOperator::Exists,
                negated: false,
                // operand unused for EXISTS; use a benign placeholder.
                value: ComparisonOperand::Literal(Literal::Boolean(true)),
                span: self.span_from(start),
            }));
        }

        // objectPath NOT? operator operand
        let path = self.parse_object_path()?;
        let negated = self.eat(&TokenKind::Not);
        let operator = self.parse_comparison_operator()?;
        let value = if operator == ComparisonOperator::In {
            self.parse_set_literal()?
        } else {
            ComparisonOperand::Literal(self.parse_literal()?)
        };
        Ok(ComparisonExpression::Test(Comparison {
            path,
            operator,
            negated,
            value,
            span: self.span_from(start),
        }))
    }
```

In `parse_observation_primary`, capture the start before the bracket:

```rust
    fn parse_observation_primary(&mut self) -> Result<ObservationExpression> {
        let start = self.current_span().start;
        if self.eat(&TokenKind::LBracket) {
            let comp = self.parse_comparison_expression()?;
            self.expect(&TokenKind::RBracket, "']' to close observation")?;
            return Ok(ObservationExpression::Observation {
                expression: Box::new(comp),
                span: self.span_from(start),
            });
        }
```

In `parse_observation_qualified`, capture the qualifier keyword's start inside the loop, before the `match` advances past it:

```rust
    fn parse_observation_qualified(&mut self) -> Result<ObservationExpression> {
        let mut expr = self.parse_observation_primary()?;
        loop {
            let q_start = self.current_span().start;
            let qualifier = match self.peek() {
                // ... arms unchanged ...
                _ => break,
            };
            expr = ObservationExpression::Qualified {
                expression: Box::new(expr),
                qualifier,
                span: self.span_from(q_start),
            };
        }
        Ok(expr)
    }
```

- [ ] **Step 8: Fix the existing tests that construct or destructure these nodes**

Five mechanical edits. In `crates/stix-pattern/src/ast.rs`, `build_simple_comparison_pattern`:

```rust
        let path = ObjectPath {
            object_type: "ipv4-addr".to_string(),
            steps: vec![PathStep::Key("value".to_string())],
            span: Span::default(),
        };
        let comp = Comparison {
            path,
            operator: ComparisonOperator::Equal,
            negated: false,
            value: ComparisonOperand::Literal(Literal::String("1.2.3.4".to_string())),
            span: Span::default(),
        };
        let pattern = Pattern {
            expression: ObservationExpression::Observation {
                expression: Box::new(ComparisonExpression::Test(comp)),
                span: Span::default(),
            },
        };
        match pattern.expression {
            ObservationExpression::Observation { .. } => {}
            _ => panic!("expected observation"),
        }
```

In `crates/stix-pattern/src/parser.rs`, `parses_single_observation`: change `ObservationExpression::Observation(_) => {}` to `ObservationExpression::Observation { .. } => {}`.

In `crates/stix-pattern/src/parser.rs`, `parses_within_qualifier`: the struct pattern lists two fields and now needs `..`:

```rust
            ObservationExpression::Qualified {
                qualifier: Qualifier::Within { seconds },
                expression,
                ..
            } => {
```

and the inner arm already uses `..`, so it needs no change.

In `crates/stix-matcher/src/eval.rs:219`, change the match arm:

```rust
        ObservationExpression::Observation { expression, .. } => {
```

and rename the binding's uses inside that arm from `comparison` to `expression` (or bind as `expression: comparison` to keep the body untouched — prefer `expression: comparison` for a smaller diff).

In `crates/stix-matcher/src/eval.rs:267-275` and `crates/stix-matcher/src/resolve.rs:96-100`, add `span: Default::default(),` to the `Comparison` and `ObjectPath` literals in those test helpers, importing `stix_pattern::Span` if it is not already in scope.

- [ ] **Step 9: Run the tests to verify they pass**

Run: `cargo test --workspace`
Expected: PASS — all pre-existing tests plus the three new span tests. If `stix-matcher` fails to compile, the Step 8 fixups are incomplete; the compiler names every remaining site.

- [ ] **Step 10: Check clippy and commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
git add crates/stix-pattern/src crates/stix-matcher/src
git commit -m "feat(pattern)!: record source spans on AST nodes

Adds span fields to Comparison, ObjectPath, and the Observation and
Qualified observation-expression variants, plus Pattern::without_spans for
structural comparison independent of byte offsets.

BREAKING CHANGE: ObservationExpression::Observation becomes a struct
variant, changing its serialized form from {\"Observation\": {...}} to
{\"Observation\": {\"expression\": {...}, \"span\": {...}}}."
```

---

### Task 2: The IR data model

**Files:**
- Create: `crates/stix-pattern/src/ir/mod.rs`
- Create: `crates/stix-pattern/src/ir/instr.rs`
- Modify: `crates/stix-pattern/src/lib.rs` (add `pub mod ir;`)

**Interfaces:**
- Consumes: `ObjectPath`, `ComparisonOperator`, `Literal` from `ast.rs`; `Span` from `error.rs`.
- Produces: `SCHEMA_VERSION`, `InstrId(pub u32)`, `BlockId(pub u32)`, `Operand`, `Op`, `Instruction`, `BlockKind`, `Block`, `Program`. Every later task depends on these exact names.

Pure data — no logic beyond constructors. Note the three Spec Deltas apply here.

- [ ] **Step 1: Write the failing test**

Create `crates/stix-pattern/src/ir/instr.rs` with only this test module at the bottom (the types come in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ComparisonOperator, Literal, ObjectPath, PathStep};
    use crate::error::Span;

    fn sample_program() -> Program {
        let path = ObjectPath {
            object_type: "file".to_string(),
            steps: vec![PathStep::Key("size".to_string())],
            span: Span::default(),
        };
        Program {
            schema_version: SCHEMA_VERSION,
            blocks: vec![Block {
                id: BlockId(0),
                kind: BlockKind::Comparison,
                instructions: vec![
                    Instruction {
                        id: InstrId(0),
                        op: Op::Load { path },
                        span: None,
                    },
                    Instruction {
                        id: InstrId(1),
                        op: Op::Compare {
                            operator: ComparisonOperator::GreaterThan,
                            negated: false,
                            lhs: InstrId(0),
                            rhs: Operand::Literal(Literal::Integer(1024)),
                        },
                        span: None,
                    },
                    Instruction {
                        id: InstrId(2),
                        op: Op::Yield {
                            value: InstrId(1),
                        },
                        span: None,
                    },
                ],
            }],
            main: Block {
                id: BlockId(1),
                kind: BlockKind::Main,
                instructions: vec![
                    Instruction {
                        id: InstrId(3),
                        op: Op::Observe { block: BlockId(0) },
                        span: None,
                    },
                    Instruction {
                        id: InstrId(4),
                        op: Op::Ret {
                            value: InstrId(3),
                        },
                        span: None,
                    },
                ],
            },
        }
    }

    #[test]
    fn program_round_trips_through_json() {
        let p = sample_program();
        let json = serde_json::to_string(&p).unwrap();
        let back: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn ops_serialize_internally_tagged() {
        let p = sample_program();
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""op":"load""#), "got: {json}");
        assert!(json.contains(r#""op":"compare""#), "got: {json}");
        assert!(json.contains(r#""op":"observe""#), "got: {json}");
    }

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p stix-pattern program_round_trips_through_json`
Expected: FAIL to compile — the `ir` module is not declared and none of `Program`, `Block`, `Op` exist.

- [ ] **Step 3: Write the types**

Prepend to `crates/stix-pattern/src/ir/instr.rs` (above the test module):

```rust
//! The IR instruction set and its container types.

use serde::{Deserialize, Serialize};

use crate::ast::{ComparisonOperator, Literal, ObjectPath};
use crate::error::Span;

/// Version of the serialized IR schema. Checked by
/// [`Program::validate`](crate::ir::Program::validate).
pub const SCHEMA_VERSION: u32 = 1;

/// Identifies one instruction, and therefore the value it produces.
///
/// The IR is in SSA form: there is no separate destination field, so a value is
/// named by the id of the instruction that computes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstrId(pub u32);

/// Identifies one block within a [`Program`](crate::ir::Program).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlockId(pub u32);

/// Right-hand side of a [`Op::Compare`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Operand {
    /// A single literal value.
    Literal(Literal),
    /// A set of literals, for `IN`.
    Set(Vec<Literal>),
    /// No operand, for `EXISTS`.
    Absent,
}

/// One three-address operation.
///
/// Instructions in a [`BlockKind::Comparison`] block use the comparison-tier ops
/// (`Load`, `Compare`, `And`, `Or`, `Yield`); instructions in the
/// [`BlockKind::Main`] block use the observation-tier ops. `And` and `Or` are
/// shared between the tiers and take their evaluation semantics from the
/// enclosing block's kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Resolve an object path to a value.
    Load {
        /// The path to resolve.
        path: ObjectPath,
    },
    /// Apply a comparison operator to a loaded value and an operand.
    Compare {
        /// The comparison operator.
        operator: ComparisonOperator,
        /// `true` if a `NOT` preceded the operator in the source.
        negated: bool,
        /// The loaded value being compared.
        lhs: InstrId,
        /// The right-hand side.
        rhs: Operand,
    },
    /// Boolean conjunction of two values.
    And {
        /// Left operand.
        lhs: InstrId,
        /// Right operand.
        rhs: InstrId,
    },
    /// Boolean disjunction of two values.
    Or {
        /// Left operand.
        lhs: InstrId,
        /// Right operand.
        rhs: InstrId,
    },
    /// Terminator of a comparison block: its result.
    Yield {
        /// The value the block evaluates to.
        value: InstrId,
    },
    /// Evaluate a comparison block against an observation.
    Observe {
        /// The comparison block to evaluate.
        block: BlockId,
    },
    /// `FOLLOWEDBY`: the left result's observations precede the right's.
    FollowedBy {
        /// Left operand.
        lhs: InstrId,
        /// Right operand.
        rhs: InstrId,
    },
    /// `WITHIN <seconds> SECONDS` applied to a result.
    Within {
        /// The result being qualified.
        input: InstrId,
        /// The window length in seconds.
        seconds: f64,
    },
    /// `REPEATS <count> TIMES` applied to a result.
    Repeats {
        /// The result being qualified.
        input: InstrId,
        /// The required repetition count.
        count: u64,
    },
    /// `START <start> STOP <stop>` applied to a result.
    StartStop {
        /// The result being qualified.
        input: InstrId,
        /// Inclusive window start, RFC3339.
        start: String,
        /// Exclusive window stop, RFC3339.
        stop: String,
    },
    /// Terminator of the main block: the program's result.
    Ret {
        /// The value the program evaluates to.
        value: InstrId,
    },
}

/// One instruction: an id, an operation, and optionally where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instruction {
    /// Unique id, which also names this instruction's result.
    pub id: InstrId,
    /// The operation.
    pub op: Op,
    /// Byte range in the source pattern, when this was lowered from text.
    pub span: Option<Span>,
}

/// Which tier a block belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    /// A `[...]` observation's comparisons, evaluated once per binding set.
    Comparison,
    /// The observation-tier block combining observation results.
    Main,
}

/// A straight-line sequence of instructions ending in a terminator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    /// Unique id.
    pub id: BlockId,
    /// Which tier this block belongs to.
    pub kind: BlockKind,
    /// The instructions, in evaluation order.
    pub instructions: Vec<Instruction>,
}

/// A lowered pattern: comparison blocks plus the observation-tier block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    /// Serialized schema version; see [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The comparison blocks, in lowering order.
    pub blocks: Vec<Block>,
    /// The observation-tier block.
    pub main: Block,
}

impl Program {
    /// The comparison block with the given id, if present.
    pub fn block(&self, id: BlockId) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }
}

impl Block {
    /// The instruction with the given id, if present in this block.
    pub fn instruction(&self, id: InstrId) -> Option<&Instruction> {
        self.instructions.iter().find(|i| i.id == id)
    }

    /// The block's terminator — its last instruction — if it has one.
    pub fn terminator(&self) -> Option<&Instruction> {
        self.instructions.last()
    }
}
```

- [ ] **Step 4: Create the module root and declare it**

Create `crates/stix-pattern/src/ir/mod.rs`:

```rust
//! A three-address intermediate representation of a parsed pattern.
//!
//! [`lower`] turns a [`Pattern`](crate::ast::Pattern) into a [`Program`]: a set
//! of comparison blocks, one per `[...]` observation, plus a `main` block that
//! combines their results. Each comparison block is exactly the unit the matcher
//! enumerates binding sets over.
//!
//! The IR is in SSA form — an instruction's [`InstrId`] names the value it
//! produces, and there is no separate destination field.
//!
//! # Example
//!
//! ```
//! use stix_pattern::{parse, ir};
//!
//! let pattern = parse("[file:size > 1024]").unwrap();
//! let program = ir::lower(&pattern);
//! program.validate().expect("lowered programs are always valid");
//! assert_eq!(ir::render(&program), "[file:size > 1024]");
//! ```
//!
//! # Deserialization
//!
//! Deserializing a [`Program`] does **not** check its invariants. Call
//! [`Program::validate`] before using one that came from outside this process.

mod instr;
mod lower;
mod print;
mod render;
mod validate;

pub use instr::{
    Block, BlockId, BlockKind, InstrId, Instruction, Op, Operand, Program, SCHEMA_VERSION,
};
pub use lower::lower;
pub use render::render;
pub use validate::IrError;
```

Note: `lower`, `print`, `render` and `validate` do not exist yet, so comment out those four `mod`/`pub use` lines for now and uncomment each as its task lands. Add to `crates/stix-pattern/src/lib.rs`, after `pub mod error;`:

```rust
pub mod ir;
```

Do **not** add `pub use ir::*;` — the IR's names (`Program`, `Block`, `Operand`) are generic enough to collide, so they stay namespaced as `stix_pattern::ir::Program`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p stix-pattern --lib ir::`
Expected: PASS — three tests.

- [ ] **Step 6: Check clippy and commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
git add crates/stix-pattern/src/ir crates/stix-pattern/src/lib.rs
git commit -m "feat(pattern): add the IR data model

Program/Block/Instruction/Op in SSA form, internally-tagged for a readable
serialized schema, versioned by SCHEMA_VERSION."
```

---

### Task 3: Lowering AST to IR

**Files:**
- Create: `crates/stix-pattern/src/ir/lower.rs`
- Modify: `crates/stix-pattern/src/ir/mod.rs` (uncomment `mod lower;` and its `pub use`)

**Interfaces:**
- Consumes: everything from Task 2; AST types and their spans from Task 1.
- Produces: `pub fn lower(pattern: &Pattern) -> Program`. Tasks 4–7 all consume this.

Lowering is total and infallible. Ids are allocated from a single counter shared by blocks and instructions is **not** wanted — blocks and instructions have separate id spaces, each starting at 0.

- [ ] **Step 1: Write the failing test**

Create `crates/stix-pattern/src/ir/lower.rs` with only this test module (implementation in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::ComparisonOperator;
    use crate::ir::{BlockKind, Op, Operand, SCHEMA_VERSION};
    use crate::parse;

    #[test]
    fn lowers_a_single_comparison() {
        let p = parse("[file:size > 1024]").unwrap();
        let prog = lower(&p);

        assert_eq!(prog.schema_version, SCHEMA_VERSION);
        assert_eq!(prog.blocks.len(), 1);
        assert_eq!(prog.blocks[0].kind, BlockKind::Comparison);
        assert_eq!(prog.main.kind, BlockKind::Main);

        let ops: Vec<&Op> = prog.blocks[0].instructions.iter().map(|i| &i.op).collect();
        assert!(matches!(ops[0], Op::Load { .. }), "got {:?}", ops[0]);
        assert!(
            matches!(
                ops[1],
                Op::Compare {
                    operator: ComparisonOperator::GreaterThan,
                    negated: false,
                    ..
                }
            ),
            "got {:?}",
            ops[1]
        );
        assert!(matches!(ops[2], Op::Yield { .. }), "got {:?}", ops[2]);

        let main_ops: Vec<&Op> = prog.main.instructions.iter().map(|i| &i.op).collect();
        assert!(matches!(main_ops[0], Op::Observe { .. }));
        assert!(matches!(main_ops[1], Op::Ret { .. }));
    }

    #[test]
    fn compare_references_its_load() {
        let p = parse("[file:size > 1024]").unwrap();
        let prog = lower(&p);
        let load_id = prog.blocks[0].instructions[0].id;
        match &prog.blocks[0].instructions[1].op {
            Op::Compare { lhs, .. } => assert_eq!(*lhs, load_id),
            other => panic!("expected a compare, got {other:?}"),
        }
    }

    #[test]
    fn exists_lowers_to_an_absent_operand() {
        let p = parse("[EXISTS file:name]").unwrap();
        let prog = lower(&p);
        match &prog.blocks[0].instructions[1].op {
            Op::Compare { operator, rhs, .. } => {
                assert_eq!(*operator, ComparisonOperator::Exists);
                assert_eq!(*rhs, Operand::Absent);
            }
            other => panic!("expected a compare, got {other:?}"),
        }
    }

    #[test]
    fn in_lowers_to_a_set_operand() {
        let p = parse("[ipv4-addr:value IN ('1.1.1.1', '8.8.8.8')]").unwrap();
        let prog = lower(&p);
        match &prog.blocks[0].instructions[1].op {
            Op::Compare { rhs, .. } => match rhs {
                Operand::Set(items) => assert_eq!(items.len(), 2),
                other => panic!("expected a set, got {other:?}"),
            },
            other => panic!("expected a compare, got {other:?}"),
        }
    }

    #[test]
    fn each_observation_gets_its_own_block() {
        let p = parse("[file:name='a'] FOLLOWEDBY [file:name='b']").unwrap();
        let prog = lower(&p);
        assert_eq!(prog.blocks.len(), 2);
        let main_ops: Vec<&Op> = prog.main.instructions.iter().map(|i| &i.op).collect();
        assert!(matches!(main_ops[0], Op::Observe { .. }));
        assert!(matches!(main_ops[1], Op::Observe { .. }));
        assert!(matches!(main_ops[2], Op::FollowedBy { .. }));
        assert!(matches!(main_ops[3], Op::Ret { .. }));
    }

    #[test]
    fn nested_qualifiers_lower_inner_first() {
        let p = parse("[file:name='a'] REPEATS 2 TIMES WITHIN 60 SECONDS").unwrap();
        let prog = lower(&p);
        let main_ops: Vec<&Op> = prog.main.instructions.iter().map(|i| &i.op).collect();
        assert!(matches!(main_ops[0], Op::Observe { .. }));
        assert!(matches!(main_ops[1], Op::Repeats { count: 2, .. }));
        assert!(matches!(main_ops[2], Op::Within { .. }));
        assert!(matches!(main_ops[3], Op::Ret { .. }));
    }

    #[test]
    fn instructions_carry_source_spans() {
        let src = "[file:size > 1024]";
        let prog = lower(&parse(src).unwrap());
        for instr in &prog.blocks[0].instructions {
            // Yield is synthetic and has no span of its own.
            if matches!(instr.op, Op::Yield { .. }) {
                continue;
            }
            let span = instr.span.expect("lowered instruction should have a span");
            assert!(span.end <= src.len());
            assert!(span.start < span.end, "span should be non-empty");
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p stix-pattern --lib ir::lower`
Expected: FAIL to compile — `lower` does not exist.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/stix-pattern/src/ir/lower.rs`:

```rust
//! Lowering: AST to [`Program`].

use crate::ast::{
    Comparison, ComparisonExpression, ComparisonOperand, ComparisonOperator, ObservationExpression,
    Pattern, Qualifier,
};
use crate::error::Span;
use crate::ir::{
    Block, BlockId, BlockKind, InstrId, Instruction, Op, Operand, Program, SCHEMA_VERSION,
};

/// Lower a parsed pattern into its three-address representation.
///
/// This is total: every well-formed [`Pattern`] lowers, and the result always
/// satisfies [`Program::validate`](crate::ir::Program::validate).
pub fn lower(pattern: &Pattern) -> Program {
    let mut l = Lowerer {
        next_instr: 0,
        next_block: 0,
        blocks: Vec::new(),
    };
    let main_id = l.fresh_block();
    let mut main_instrs = Vec::new();
    let result = l.lower_observation(&pattern.expression, &mut main_instrs);
    let ret = l.push(&mut main_instrs, Op::Ret { value: result }, None);
    debug_assert!(main_instrs.last().map(|i| i.id) == Some(ret));
    Program {
        schema_version: SCHEMA_VERSION,
        blocks: l.blocks,
        main: Block {
            id: main_id,
            kind: BlockKind::Main,
            instructions: main_instrs,
        },
    }
}

struct Lowerer {
    next_instr: u32,
    next_block: u32,
    blocks: Vec<Block>,
}

impl Lowerer {
    fn fresh_block(&mut self) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block += 1;
        id
    }

    /// Append an instruction and return the id naming its result.
    fn push(&mut self, into: &mut Vec<Instruction>, op: Op, span: Option<Span>) -> InstrId {
        let id = InstrId(self.next_instr);
        self.next_instr += 1;
        into.push(Instruction { id, op, span });
        id
    }

    fn lower_observation(
        &mut self,
        e: &ObservationExpression,
        into: &mut Vec<Instruction>,
    ) -> InstrId {
        match e {
            ObservationExpression::Observation { expression, span } => {
                let block = self.lower_comparison_block(expression);
                self.push(into, Op::Observe { block }, Some(*span))
            }
            ObservationExpression::And(l, r) => {
                let lhs = self.lower_observation(l, into);
                let rhs = self.lower_observation(r, into);
                self.push(into, Op::And { lhs, rhs }, None)
            }
            ObservationExpression::Or(l, r) => {
                let lhs = self.lower_observation(l, into);
                let rhs = self.lower_observation(r, into);
                self.push(into, Op::Or { lhs, rhs }, None)
            }
            ObservationExpression::FollowedBy(l, r) => {
                let lhs = self.lower_observation(l, into);
                let rhs = self.lower_observation(r, into);
                self.push(into, Op::FollowedBy { lhs, rhs }, None)
            }
            ObservationExpression::Qualified {
                expression,
                qualifier,
                span,
            } => {
                let input = self.lower_observation(expression, into);
                let op = match qualifier {
                    Qualifier::Within { seconds } => Op::Within {
                        input,
                        seconds: *seconds,
                    },
                    Qualifier::Repeats { count } => Op::Repeats {
                        input,
                        count: *count,
                    },
                    Qualifier::StartStop { start, stop } => Op::StartStop {
                        input,
                        start: start.clone(),
                        stop: stop.clone(),
                    },
                };
                self.push(into, op, Some(*span))
            }
        }
    }

    /// Lower one `[...]` observation into a fresh comparison block, returning its id.
    fn lower_comparison_block(&mut self, e: &ComparisonExpression) -> BlockId {
        let id = self.fresh_block();
        let mut instrs = Vec::new();
        let result = self.lower_comparison(e, &mut instrs);
        self.push(&mut instrs, Op::Yield { value: result }, None);
        self.blocks.push(Block {
            id,
            kind: BlockKind::Comparison,
            instructions: instrs,
        });
        id
    }

    fn lower_comparison(
        &mut self,
        e: &ComparisonExpression,
        into: &mut Vec<Instruction>,
    ) -> InstrId {
        match e {
            ComparisonExpression::Test(c) => self.lower_test(c, into),
            ComparisonExpression::And(l, r) => {
                let lhs = self.lower_comparison(l, into);
                let rhs = self.lower_comparison(r, into);
                self.push(into, Op::And { lhs, rhs }, None)
            }
            ComparisonExpression::Or(l, r) => {
                let lhs = self.lower_comparison(l, into);
                let rhs = self.lower_comparison(r, into);
                self.push(into, Op::Or { lhs, rhs }, None)
            }
        }
    }

    fn lower_test(&mut self, c: &Comparison, into: &mut Vec<Instruction>) -> InstrId {
        let lhs = self.push(
            into,
            Op::Load {
                path: c.path.clone(),
            },
            Some(c.path.span),
        );
        // EXISTS carries a placeholder operand in the AST; the IR states its absence.
        let rhs = if c.operator == ComparisonOperator::Exists {
            Operand::Absent
        } else {
            match &c.value {
                ComparisonOperand::Literal(lit) => Operand::Literal(lit.clone()),
                ComparisonOperand::Set(items) => Operand::Set(items.clone()),
            }
        };
        self.push(
            into,
            Op::Compare {
                operator: c.operator,
                negated: c.negated,
                lhs,
                rhs,
            },
            Some(c.span),
        )
    }
}
```

Note the block-id ordering: `main` takes `BlockId(0)` because it is allocated first, and comparison blocks take 1, 2, … in lowering order. Do not "fix" this to make `main` last; the ids are opaque and Task 5 only checks uniqueness and that `Observe` targets a comparison block.

- [ ] **Step 4: Uncomment the module wiring**

In `crates/stix-pattern/src/ir/mod.rs`, uncomment:

```rust
mod lower;
```

```rust
pub use lower::lower;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p stix-pattern --lib ir::lower`
Expected: PASS — seven tests.

- [ ] **Step 6: Check clippy and commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
git add crates/stix-pattern/src/ir
git commit -m "feat(pattern): lower the AST to IR

lower(&Pattern) -> Program is total and infallible; one comparison block per
observation, spans copied from the AST onto the instructions they produced."
```

---

### Task 4: The listing printer

**Files:**
- Create: `crates/stix-pattern/src/ir/print.rs`
- Modify: `crates/stix-pattern/src/ir/mod.rs` (uncomment `mod print;`)

**Interfaces:**
- Consumes: `Program`, `Block`, `Op` from Task 2; `lower` from Task 3.
- Produces: `impl Program { pub fn to_listing(&self) -> String }`.

The spec listed five files under `ir/`; this is a sixth. The listing printer is a distinct responsibility from the pattern-text renderer (Task 6) and they share no code, so they get separate files.

Display naming: comparison-block results are numbered `t0`, `t1`, … sequentially across blocks in `blocks` order; main-block results are numbered `o0`, `o1`, … . These are display ordinals only, unrelated to `InstrId` values.

- [ ] **Step 1: Write the failing test**

Create `crates/stix-pattern/src/ir/print.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use crate::ir::lower;
    use crate::parse;

    #[test]
    fn prints_a_simple_comparison() {
        let prog = lower(&parse("[file:size > 1024]").unwrap());
        let expected = "\
block b1 (comparison):
  t0 = load        file:size
  t1 = gt          t0, 1024
       yield       t1

block main (observation):
  o0 = observe     b1
       ret         o0
";
        assert_eq!(prog.to_listing(), expected);
    }

    // NOTE: `WITHIN` qualifies only the SECOND observation, not the FOLLOWEDBY.
    // STIX 2.1 attaches qualifiers to the tightest production, so this parses as
    // FollowedBy(A, Within(B, 300)) — hence `within` precedes `followedby` below
    // and takes o1. Verified against the parser, not assumed.
    #[test]
    fn prints_followedby_with_a_qualifier() {
        let src = "[ipv4-addr:value = '1.2.3.4' AND file:size > 1024] \
                   FOLLOWEDBY [domain-name:value = 'evil.example'] WITHIN 300 SECONDS";
        let prog = lower(&parse(src).unwrap());
        let expected = "\
block b1 (comparison):
  t0 = load        ipv4-addr:value
  t1 = eq          t0, '1.2.3.4'
  t2 = load        file:size
  t3 = gt          t2, 1024
  t4 = and         t1, t3
       yield       t4

block b2 (comparison):
  t5 = load        domain-name:value
  t6 = eq          t5, 'evil.example'
       yield       t6

block main (observation):
  o0 = observe     b1
  o1 = observe     b2
  o2 = within      o1, 300
  o3 = followedby  o0, o2
       ret         o3
";
        assert_eq!(prog.to_listing(), expected);
    }

    #[test]
    fn prints_exists_and_in() {
        let prog = lower(&parse("[EXISTS file:name]").unwrap());
        assert!(prog.to_listing().contains("t1 = exists      t0"), "{}", prog.to_listing());

        let prog = lower(&parse("[ipv4-addr:value IN ('1.1.1.1', '8.8.8.8')]").unwrap());
        assert!(
            prog.to_listing().contains("t1 = in          t0, ('1.1.1.1', '8.8.8.8')"),
            "{}",
            prog.to_listing()
        );
    }

    #[test]
    fn marks_negation() {
        let prog = lower(&parse("[file:name NOT = 'x']").unwrap());
        assert!(prog.to_listing().contains("t1 = not eq      t0, 'x'"), "{}", prog.to_listing());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p stix-pattern --lib ir::print`
Expected: FAIL to compile — `to_listing` does not exist.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/stix-pattern/src/ir/print.rs`:

```rust
//! The human-readable IR listing.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::ast::ComparisonOperator;
use crate::ir::{Block, BlockKind, InstrId, Op, Operand, Program};

/// Width of the mnemonic field, so operands line up in a column.
///
/// Every line's prefix is exactly 7 characters — `"  t0 = "` for an instruction
/// that names a result, `"       "` for a terminator that does not — so padding
/// the mnemonic to a fixed width is all the alignment this format needs.
const MNEMONIC_WIDTH: usize = 12;

impl Program {
    /// Render this program as a human-readable listing.
    ///
    /// Comparison-block results are named `t0`, `t1`, … across all blocks in
    /// order; main-block results are named `o0`, `o1`, … . These names are
    /// display ordinals, not [`InstrId`](crate::ir::InstrId) values.
    pub fn to_listing(&self) -> String {
        let mut names: HashMap<InstrId, String> = HashMap::new();
        let mut t = 0usize;
        for b in &self.blocks {
            for i in &b.instructions {
                if produces_value(&i.op) {
                    names.insert(i.id, format!("t{t}"));
                    t += 1;
                }
            }
        }
        let mut o = 0usize;
        for i in &self.main.instructions {
            if produces_value(&i.op) {
                names.insert(i.id, format!("o{o}"));
                o += 1;
            }
        }

        let mut out = String::new();
        for b in &self.blocks {
            write_block(&mut out, b, &names);
            out.push('\n');
        }
        write_block(&mut out, &self.main, &names);
        out
    }
}

/// `false` for the terminators, which name no result.
fn produces_value(op: &Op) -> bool {
    !matches!(op, Op::Yield { .. } | Op::Ret { .. })
}

fn write_block(out: &mut String, b: &Block, names: &HashMap<InstrId, String>) {
    match b.kind {
        BlockKind::Comparison => {
            let _ = writeln!(out, "block b{} (comparison):", b.id.0);
        }
        BlockKind::Main => {
            let _ = writeln!(out, "block main (observation):");
        }
    }
    for i in &b.instructions {
        let dest = match names.get(&i.id) {
            Some(n) => format!("  {n} = "),
            None => "       ".to_string(),
        };
        let (mnemonic, operands) = describe(&i.op, names);
        let padded = format!("{mnemonic:<MNEMONIC_WIDTH$}");
        if operands.is_empty() {
            let _ = writeln!(out, "{dest}{}", padded.trim_end());
        } else {
            let _ = writeln!(out, "{dest}{padded}{operands}");
        }
    }
}

fn name_of(id: InstrId, names: &HashMap<InstrId, String>) -> String {
    names.get(&id).cloned().unwrap_or_else(|| format!("%{}", id.0))
}

/// The mnemonic and operand text for one op.
fn describe(op: &Op, names: &HashMap<InstrId, String>) -> (String, String) {
    match op {
        Op::Load { path } => ("load".to_string(), render_path(path)),
        Op::Compare {
            operator,
            negated,
            lhs,
            rhs,
        } => {
            let mut m = String::new();
            if *negated {
                m.push_str("not ");
            }
            m.push_str(mnemonic_for(*operator));
            let operands = match rhs {
                Operand::Absent => name_of(*lhs, names),
                Operand::Literal(lit) => format!("{}, {}", name_of(*lhs, names), crate::ir::render_literal(lit)),
                Operand::Set(items) => {
                    let inner: Vec<String> = items.iter().map(crate::ir::render_literal).collect();
                    format!("{}, ({})", name_of(*lhs, names), inner.join(", "))
                }
            };
            (m, operands)
        }
        Op::And { lhs, rhs } => (
            "and".to_string(),
            format!("{}, {}", name_of(*lhs, names), name_of(*rhs, names)),
        ),
        Op::Or { lhs, rhs } => (
            "or".to_string(),
            format!("{}, {}", name_of(*lhs, names), name_of(*rhs, names)),
        ),
        Op::Yield { value } => ("yield".to_string(), name_of(*value, names)),
        Op::Observe { block } => ("observe".to_string(), format!("b{}", block.0)),
        Op::FollowedBy { lhs, rhs } => (
            "followedby".to_string(),
            format!("{}, {}", name_of(*lhs, names), name_of(*rhs, names)),
        ),
        Op::Within { input, seconds } => (
            "within".to_string(),
            format!("{}, {}", name_of(*input, names), render_seconds(*seconds)),
        ),
        Op::Repeats { input, count } => (
            "repeats".to_string(),
            format!("{}, {}", name_of(*input, names), count),
        ),
        Op::StartStop { input, start, stop } => (
            "startstop".to_string(),
            format!("{}, '{}', '{}'", name_of(*input, names), start, stop),
        ),
        Op::Ret { value } => ("ret".to_string(), name_of(*value, names)),
    }
}

fn mnemonic_for(op: ComparisonOperator) -> &'static str {
    match op {
        ComparisonOperator::Equal => "eq",
        ComparisonOperator::NotEqual => "ne",
        ComparisonOperator::GreaterThan => "gt",
        ComparisonOperator::GreaterThanOrEqual => "ge",
        ComparisonOperator::LessThan => "lt",
        ComparisonOperator::LessThanOrEqual => "le",
        ComparisonOperator::In => "in",
        ComparisonOperator::Like => "like",
        ComparisonOperator::Matches => "matches",
        ComparisonOperator::IsSubset => "issubset",
        ComparisonOperator::IsSuperset => "issuperset",
        ComparisonOperator::Exists => "exists",
    }
}
```

`render_path`, `render_literal` and `render_seconds` are defined in Task 6's `render.rs` and made `pub(crate)` there. Until Task 6 lands, define them as temporary local copies in this file and delete them in Task 6 Step 4.

- [ ] **Step 4: Uncomment the module wiring**

In `crates/stix-pattern/src/ir/mod.rs`, uncomment `mod print;`. It exports no names of its own — `to_listing` is an inherent method on `Program`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p stix-pattern --lib ir::print`
Expected: PASS — four tests. The expected strings in Step 1 are the specification of the format: every line has a 7-character prefix and a 12-character mnemonic field. If a line disagrees, the bug is in `describe` or in `MNEMONIC_WIDTH`, not in the expected strings.

- [ ] **Step 6: Check clippy and commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
git add crates/stix-pattern/src/ir
git commit -m "feat(pattern): add the IR listing printer"
```

---

### Task 5: The validator

**Files:**
- Create: `crates/stix-pattern/src/ir/validate.rs`
- Modify: `crates/stix-pattern/src/ir/mod.rs` (uncomment `mod validate;` and its `pub use`)

**Interfaces:**
- Consumes: Task 2's types, Task 3's `lower`.
- Produces: `pub enum IrError` and `impl Program { pub fn validate(&self) -> Result<(), IrError> }`. Task 7 calls `validate`.

Dead instructions are legal. Dangling and forward references are not.

- [ ] **Step 1: Write the failing test**

Create `crates/stix-pattern/src/ir/validate.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        lower, Block, BlockId, BlockKind, InstrId, Instruction, Op, Operand, Program,
        SCHEMA_VERSION,
    };
    use crate::parse;

    fn valid() -> Program {
        lower(&parse("[file:size > 1024]").unwrap())
    }

    #[test]
    fn accepts_every_lowered_corpus_pattern() {
        let raw = include_str!("../../tests/fixtures/valid_patterns.txt");
        for line in raw.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let prog = lower(&parse(line).expect(line));
            assert!(prog.validate().is_ok(), "{line} -> {:?}", prog.validate());
        }
    }

    #[test]
    fn rejects_a_wrong_schema_version() {
        let mut p = valid();
        p.schema_version = 99;
        assert!(matches!(
            p.validate(),
            Err(IrError::UnsupportedSchemaVersion { found: 99, .. })
        ));
    }

    #[test]
    fn rejects_a_dangling_reference() {
        let mut p = valid();
        // Point the Yield at an id that does not exist.
        let last = p.blocks[0].instructions.len() - 1;
        p.blocks[0].instructions[last].op = Op::Yield {
            value: InstrId(999),
        };
        assert!(matches!(p.validate(), Err(IrError::UnknownValue { .. })));
    }

    #[test]
    fn rejects_a_forward_reference() {
        let mut p = valid();
        // Make the Load reference the Compare that follows it.
        let compare_id = p.blocks[0].instructions[1].id;
        p.blocks[0].instructions[0].op = Op::And {
            lhs: compare_id,
            rhs: compare_id,
        };
        assert!(matches!(p.validate(), Err(IrError::UnknownValue { .. })));
    }

    #[test]
    fn rejects_a_missing_terminator() {
        let mut p = valid();
        p.blocks[0].instructions.pop();
        assert!(matches!(p.validate(), Err(IrError::MissingTerminator { .. })));
    }

    #[test]
    fn rejects_a_misplaced_terminator() {
        let mut p = valid();
        let first_id = p.blocks[0].instructions[0].id;
        p.blocks[0].instructions.insert(
            1,
            Instruction {
                id: InstrId(500),
                op: Op::Yield { value: first_id },
                span: None,
            },
        );
        assert!(matches!(p.validate(), Err(IrError::MisplacedTerminator { .. })));
    }

    #[test]
    fn rejects_a_tier_violation() {
        let mut p = valid();
        let first_id = p.blocks[0].instructions[0].id;
        p.blocks[0].instructions.insert(
            1,
            Instruction {
                id: InstrId(501),
                op: Op::Within {
                    input: first_id,
                    seconds: 1.0,
                },
                span: None,
            },
        );
        assert!(matches!(p.validate(), Err(IrError::WrongTier { .. })));
    }

    #[test]
    fn rejects_an_operand_shape_mismatch() {
        let mut p = valid();
        match &mut p.blocks[0].instructions[1].op {
            Op::Compare { rhs, .. } => *rhs = Operand::Absent,
            other => panic!("expected a compare, got {other:?}"),
        }
        assert!(matches!(p.validate(), Err(IrError::OperandShape { .. })));
    }

    #[test]
    fn rejects_a_duplicate_instruction_id() {
        let mut p = valid();
        let dup = p.blocks[0].instructions[0].clone();
        p.blocks[0].instructions.insert(1, dup);
        assert!(matches!(p.validate(), Err(IrError::DuplicateInstrId { .. })));
    }

    #[test]
    fn rejects_observe_targeting_a_non_comparison_block() {
        let mut p = valid();
        let main_id = p.main.id;
        p.main.instructions[0].op = Op::Observe { block: main_id };
        assert!(matches!(p.validate(), Err(IrError::UnknownBlock { .. })));
    }

    #[test]
    fn allows_dead_instructions() {
        let mut p = valid();
        let first_id = p.blocks[0].instructions[0].id;
        // An unreferenced instruction is legal: an editor mid-edit produces these.
        p.blocks[0].instructions.insert(
            1,
            Instruction {
                id: InstrId(600),
                op: Op::And {
                    lhs: first_id,
                    rhs: first_id,
                },
                span: None,
            },
        );
        assert!(p.validate().is_ok(), "{:?}", p.validate());
    }

    #[test]
    fn rejects_a_duplicate_block_id() {
        let mut p = valid();
        let dup = p.blocks[0].clone();
        p.blocks.push(dup);
        assert!(matches!(p.validate(), Err(IrError::DuplicateBlockId { .. })));
    }

    #[test]
    fn rejects_non_finite_floats() {
        use crate::ast::Literal;

        // In a comparison operand.
        let mut p = valid();
        match &mut p.blocks[0].instructions[1].op {
            Op::Compare { rhs, .. } => *rhs = Operand::Literal(Literal::Float(f64::NAN)),
            other => panic!("expected a compare, got {other:?}"),
        }
        assert!(matches!(p.validate(), Err(IrError::NonFiniteFloat { .. })));

        // In a WITHIN window.
        let mut p = lower(&parse("[file:name='a'] WITHIN 60 SECONDS").unwrap());
        for instr in &mut p.main.instructions {
            if let Op::Within { seconds, .. } = &mut instr.op {
                *seconds = f64::INFINITY;
            }
        }
        assert!(matches!(p.validate(), Err(IrError::NonFiniteFloat { .. })));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p stix-pattern --lib ir::validate`
Expected: FAIL to compile — `IrError` and `validate` do not exist.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/stix-pattern/src/ir/validate.rs`:

```rust
//! Well-formedness checking for a [`Program`].

use std::collections::HashSet;

use thiserror::Error;

use crate::ast::{ComparisonOperator, Literal};
use crate::ir::{Block, BlockId, BlockKind, InstrId, Op, Operand, Program, SCHEMA_VERSION};

/// Whether a literal is renderable — i.e. not a non-finite float.
fn literal_is_finite(lit: &Literal) -> bool {
    match lit {
        Literal::Float(f) => f.is_finite(),
        _ => true,
    }
}

/// A way a [`Program`] can be malformed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IrError {
    /// The program's `schema_version` is not one this build understands.
    #[error("unsupported IR schema version {found} (expected {expected})")]
    UnsupportedSchemaVersion {
        /// The version found in the program.
        found: u32,
        /// The version this build supports.
        expected: u32,
    },
    /// An instruction referenced a value that is not defined earlier in its block.
    #[error("instruction {instr} references value {value}, which is not defined earlier in its block")]
    UnknownValue {
        /// The referencing instruction.
        instr: u32,
        /// The referenced value.
        value: u32,
    },
    /// `Observe` targeted a missing block, or one that is not a comparison block.
    #[error("instruction {instr} observes block {block}, which is not a comparison block")]
    UnknownBlock {
        /// The referencing instruction.
        instr: u32,
        /// The referenced block.
        block: u32,
    },
    /// A block does not end in the terminator its kind requires.
    #[error("block {block} does not end in {expected}")]
    MissingTerminator {
        /// The offending block.
        block: u32,
        /// The terminator the block's kind requires.
        expected: &'static str,
    },
    /// A terminator appeared somewhere other than the end of its block.
    #[error("block {block} has a terminator at instruction {instr}, which is not its last")]
    MisplacedTerminator {
        /// The offending block.
        block: u32,
        /// The misplaced terminator.
        instr: u32,
    },
    /// An op appeared in a block of the wrong kind.
    #[error("instruction {instr} is not valid in a {kind} block")]
    WrongTier {
        /// The offending instruction.
        instr: u32,
        /// The block kind it appeared in.
        kind: &'static str,
    },
    /// A `Compare`'s operand shape does not match its operator.
    #[error("instruction {instr} has the wrong operand shape: {detail}")]
    OperandShape {
        /// The offending instruction.
        instr: u32,
        /// What was expected.
        detail: &'static str,
    },
    /// Two instructions share an id.
    #[error("instruction id {id} is used more than once")]
    DuplicateInstrId {
        /// The repeated id.
        id: u32,
    },
    /// Two blocks share an id.
    #[error("block id {id} is used more than once")]
    DuplicateBlockId {
        /// The repeated id.
        id: u32,
    },
    /// A float that STIX pattern syntax cannot express.
    ///
    /// The patterning language has no notation for `NaN` or infinity, so such a
    /// value could never be rendered back to text.
    #[error("instruction {instr} holds a non-finite float, which pattern syntax cannot express")]
    NonFiniteFloat {
        /// The offending instruction.
        instr: u32,
    },
}

impl Program {
    /// Check that this program is well-formed.
    ///
    /// Lowered programs always pass. Run this on any program that was built by
    /// hand or deserialized, since deserialization does not check invariants.
    ///
    /// Unreferenced ("dead") instructions are allowed — only dangling and
    /// forward *references* are errors.
    pub fn validate(&self) -> Result<(), IrError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(IrError::UnsupportedSchemaVersion {
                found: self.schema_version,
                expected: SCHEMA_VERSION,
            });
        }

        let mut block_ids = HashSet::new();
        for b in self.blocks.iter().chain(std::iter::once(&self.main)) {
            if !block_ids.insert(b.id) {
                return Err(IrError::DuplicateBlockId { id: b.id.0 });
            }
        }

        let comparison_blocks: HashSet<BlockId> = self
            .blocks
            .iter()
            .filter(|b| b.kind == BlockKind::Comparison)
            .map(|b| b.id)
            .collect();

        let mut seen_instrs = HashSet::new();
        for b in self.blocks.iter().chain(std::iter::once(&self.main)) {
            self.check_block(b, &comparison_blocks, &mut seen_instrs)?;
        }
        Ok(())
    }

    fn check_block(
        &self,
        b: &Block,
        comparison_blocks: &HashSet<BlockId>,
        seen_instrs: &mut HashSet<InstrId>,
    ) -> Result<(), IrError> {
        let expected_terminator = match b.kind {
            BlockKind::Comparison => "a yield",
            BlockKind::Main => "a ret",
        };
        let kind_name = match b.kind {
            BlockKind::Comparison => "comparison",
            BlockKind::Main => "main",
        };

        let is_terminator = |op: &Op| match b.kind {
            BlockKind::Comparison => matches!(op, Op::Yield { .. }),
            BlockKind::Main => matches!(op, Op::Ret { .. }),
        };

        match b.instructions.last() {
            Some(last) if is_terminator(&last.op) => {}
            _ => {
                return Err(IrError::MissingTerminator {
                    block: b.id.0,
                    expected: expected_terminator,
                })
            }
        }

        // Values defined earlier in this block, in order.
        let mut defined: HashSet<InstrId> = HashSet::new();
        let last_index = b.instructions.len() - 1;

        for (index, instr) in b.instructions.iter().enumerate() {
            if !seen_instrs.insert(instr.id) {
                return Err(IrError::DuplicateInstrId { id: instr.id.0 });
            }

            // Terminators of either kind may only be last.
            if matches!(instr.op, Op::Yield { .. } | Op::Ret { .. }) && index != last_index {
                return Err(IrError::MisplacedTerminator {
                    block: b.id.0,
                    instr: instr.id.0,
                });
            }

            let tier_ok = match (&instr.op, b.kind) {
                (Op::Load { .. } | Op::Compare { .. } | Op::Yield { .. }, BlockKind::Comparison) => true,
                (
                    Op::Observe { .. }
                    | Op::FollowedBy { .. }
                    | Op::Within { .. }
                    | Op::Repeats { .. }
                    | Op::StartStop { .. }
                    | Op::Ret { .. },
                    BlockKind::Main,
                ) => true,
                // And/Or are shared between the tiers.
                (Op::And { .. } | Op::Or { .. }, _) => true,
                _ => false,
            };
            if !tier_ok {
                return Err(IrError::WrongTier {
                    instr: instr.id.0,
                    kind: kind_name,
                });
            }

            let mut check_value = |value: InstrId| -> Result<(), IrError> {
                if defined.contains(&value) {
                    Ok(())
                } else {
                    Err(IrError::UnknownValue {
                        instr: instr.id.0,
                        value: value.0,
                    })
                }
            };

            match &instr.op {
                Op::Load { .. } => {}
                Op::Compare {
                    operator,
                    lhs,
                    rhs,
                    ..
                } => {
                    check_value(*lhs)?;
                    let shape_ok = match (operator, rhs) {
                        (ComparisonOperator::Exists, Operand::Absent) => true,
                        (ComparisonOperator::Exists, _) => false,
                        (ComparisonOperator::In, Operand::Set(_)) => true,
                        (ComparisonOperator::In, _) => false,
                        (_, Operand::Literal(_)) => true,
                        (_, _) => false,
                    };
                    if !shape_ok {
                        return Err(IrError::OperandShape {
                            instr: instr.id.0,
                            detail: "EXISTS takes no operand, IN takes a set, others take a literal",
                        });
                    }
                    let finite = match rhs {
                        Operand::Literal(lit) => literal_is_finite(lit),
                        Operand::Set(items) => items.iter().all(literal_is_finite),
                        Operand::Absent => true,
                    };
                    if !finite {
                        return Err(IrError::NonFiniteFloat { instr: instr.id.0 });
                    }
                }
                Op::And { lhs, rhs } | Op::Or { lhs, rhs } | Op::FollowedBy { lhs, rhs } => {
                    check_value(*lhs)?;
                    check_value(*rhs)?;
                }
                Op::Yield { value } | Op::Ret { value } => check_value(*value)?,
                Op::Within { input, seconds } => {
                    check_value(*input)?;
                    if !seconds.is_finite() {
                        return Err(IrError::NonFiniteFloat { instr: instr.id.0 });
                    }
                }
                Op::Repeats { input, .. } | Op::StartStop { input, .. } => check_value(*input)?,
                Op::Observe { block } => {
                    if !comparison_blocks.contains(block) {
                        return Err(IrError::UnknownBlock {
                            instr: instr.id.0,
                            block: block.0,
                        });
                    }
                }
            }

            defined.insert(instr.id);
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Uncomment the module wiring**

In `crates/stix-pattern/src/ir/mod.rs`, uncomment:

```rust
mod validate;
```

```rust
pub use validate::IrError;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p stix-pattern --lib ir::validate`
Expected: PASS — thirteen tests.

- [ ] **Step 6: Check clippy and commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
git add crates/stix-pattern/src/ir
git commit -m "feat(pattern): add the IR validator

Typed IrError per rule. Dangling and forward references, missing or
misplaced terminators, tier violations, operand-shape mismatches and
duplicate ids are errors; dead instructions are allowed."
```

---

### Task 6: The renderer

**Files:**
- Create: `crates/stix-pattern/src/ir/render.rs`
- Modify: `crates/stix-pattern/src/ir/mod.rs` (uncomment `mod render;` and its `pub use`)
- Modify: `crates/stix-pattern/src/ir/print.rs` (delete the temporary local helpers from Task 4)

**Interfaces:**
- Consumes: Task 2's types, Task 3's `lower`.
- Produces: `pub fn render(program: &Program) -> String`, plus `pub(crate) fn render_path(path: &ObjectPath) -> String`, `pub(crate) fn render_literal(lit: &Literal) -> String`, `pub(crate) fn render_seconds(seconds: f64) -> String` used by Task 4's printer.

**Precedence, which is the whole difficulty.** Tightest to loosest, matching the parser:

| Tier | Level | Constructs |
|---|---|---|
| Comparison | 3 / 2 / 1 | `Compare` / `And` / `Or` |
| Observation | 5 / 4 / 3 / 2 / 1 | `Observe` / qualifiers / `And` / `Or` / `FollowedBy` |

A node is parenthesized when its level is **below** the minimum its parent demands. For a left-associative binary op at level `n`, render the **left** child with minimum `n` and the **right** child with minimum `n + 1`. The asymmetry is what makes `Or(a, Or(b, c))` render as `a OR (b OR c)` and therefore reparse to the same right-nested tree — without it, round-trip fails on right-nested same-operator expressions.

**Three literal-rendering traps**, each of which breaks round-trip if missed:

1. **Floats need decimal notation with a decimal point, and neither `{}` nor `{:?}` gives it.** `format!("{}", 60.0_f64)` yields `"60"`, which reparses as `Literal::Integer(60)`, not `Float`. `{:?}` fixes that (`"60.0"`) but goes **exponential** for extreme magnitudes — `1e-10`, `1e16`, `1.2345678901234568e17` — and the lexer at `lexer.rs:250-277` reads only digits and `.`, so exponent notation does not parse back. `0.0000000001` is a perfectly valid pattern literal that would break. A fixed precision does not work either: `{:.17}` renders `1e-300` as `"0.0"`, silently destroying the value. Use the `render_float` precision search given in Step 3, which was verified against `f64::MAX`, `f64::MIN_POSITIVE`, `5e-324` and `f64::EPSILON`.
2. **Strings must escape `\` and `'`**, in that order, matching the lexer at `lexer.rs:217-230`.
3. **Path keys must be quoted** unless they match `[A-Za-z_][A-Za-z0-9_]*` and are not a keyword. The lexer's `keyword()` is case-insensitive, so a key named `start` or `in` lexes as a keyword token and must be rendered `'start'`. Hyphens also force quoting, since `parse_key_component` accepts only `Identifier` or `String` and quoting is always safe.

- [ ] **Step 1: Write the failing test**

Create `crates/stix-pattern/src/ir/render.rs` with only this test module:

```rust
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
        assert_eq!(round("[file:size<>1]"), "[file:size != 1]");
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
        // `{:?}` would emit `1e-10` here, which this dialect's lexer cannot read.
        let src = "[file:size > 0.0000000001]";
        let text = round(src);
        assert!(!text.contains('e'), "must not use exponent notation: {text}");
        assert_round_trips(src);

        // A long whole number must not collapse to exponent form either.
        let src = "[file:size > 123456789012345680.0]";
        let text = round(src);
        assert!(!text.contains('e'), "must not use exponent notation: {text}");
        assert_round_trips(src);
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
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p stix-pattern --lib ir::render`
Expected: FAIL to compile — `render` does not exist.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/stix-pattern/src/ir/render.rs`:

```rust
//! Rendering: [`Program`] back to canonical STIX pattern text.

use crate::ast::{ComparisonOperator, Literal, ObjectPath, PathStep};
use crate::ir::{Block, BlockKind, InstrId, Op, Operand, Program};

/// Render a program as canonical STIX pattern text.
///
/// The output is canonical rather than a reproduction of any original source:
/// whitespace is normalized, `!=` is always used in place of `<>`, and
/// parentheses appear only where operator precedence requires them.
///
/// For any program produced by [`lower`](crate::ir::lower), reparsing this
/// output recovers the same AST up to spans:
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
/// Hand-built programs that share one value between two consumers render that
/// subexpression once per use, since pattern text has no way to name a shared
/// value.
pub fn render(program: &Program) -> String {
    let mut out = String::new();
    match program.main.terminator() {
        Some(t) => match &t.op {
            Op::Ret { value } => render_observation(program, &program.main, *value, 1, &mut out),
            // Not a well-formed program; render nothing rather than panicking.
            _ => {}
        },
        None => {}
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
            out.push_str(&format!(" START t'{start}' STOP t'{stop}'"));
        }
        // Not valid at this tier; skip rather than panic.
        _ => {}
    }
    if parens {
        out.push(')');
    }
}

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
fn escape_string(s: &str) -> String {
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
pub(crate) fn render_seconds(seconds: f64) -> String {
    if seconds.fract() == 0.0 && seconds.is_finite() {
        format!("{}", seconds as i64)
    } else {
        format!("{seconds}")
    }
}
```

- [ ] **Step 4: Wire the module and remove Task 4's temporary helpers**

In `crates/stix-pattern/src/ir/mod.rs`, uncomment:

```rust
mod render;
```

```rust
pub use render::render;
```

Then delete the temporary local copies of `render_path`, `render_literal` and `render_seconds` from `crates/stix-pattern/src/ir/print.rs`, and add at the top of that file:

```rust
use crate::ir::render::{render_literal, render_path, render_seconds};
```

Change the `crate::ir::render_literal(...)` call sites in `print.rs` to bare `render_literal(...)`, and make `mod render;` visible to `print` by declaring it `pub(crate) mod render;` in `mod.rs` if the plain `mod render;` does not resolve.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p stix-pattern --lib ir::`
Expected: PASS — all of Tasks 2–6's tests, including the seventeen render tests.

- [ ] **Step 6: Check clippy and commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
git add crates/stix-pattern/src/ir
git commit -m "feat(pattern): render IR back to canonical pattern text

Precedence-driven parenthesization with left-associative asymmetry so
right-nested same-operator expressions reparse identically. Floats keep
their decimal point; path keys are quoted when they would otherwise lex as
keywords or contain hyphens."
```

---

### Task 7: The corpus round-trip proof

**Files:**
- Create: `crates/stix-pattern/tests/ir_roundtrip.rs`

**Interfaces:**
- Consumes: `parse`, `Pattern::without_spans`, `ir::lower`, `ir::render`, `Program::validate`, `Program::to_listing`.
- Produces: nothing — this is the proof obligation for sub-project B.

This is the task the whole design rests on. It reuses the `lines` helper idiom from `tests/conformance.rs`.

- [ ] **Step 1: Write the failing test**

Create `crates/stix-pattern/tests/ir_roundtrip.rs`:

```rust
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
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p stix-pattern --test ir_roundtrip`
Expected: PASS — five tests.

If `every_corpus_pattern_round_trips` fails, the failure message names the pattern, the rendered text, and whether it was a parse failure or an AST mismatch. Do **not** relax the assertion or add the pattern to an exclusion list — a failure here means the IR or renderer lost information, which is precisely what this task exists to detect. Fix `render.rs` (most likely a precedence or literal-escaping bug) or `lower.rs`.

- [ ] **Step 3: Commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings
git add crates/stix-pattern/tests/ir_roundtrip.rs
git commit -m "test(pattern): prove the IR round trip over the conformance corpus

parse(render(lower(ast))) == ast modulo spans for every valid pattern, plus
render idempotence, span bounds, and listing coverage."
```

---

### Task 8: Documentation

**Files:**
- Modify: `crates/stix-pattern/src/lib.rs` (crate-doc example)
- Modify: `docs/book/src/guide/patterns.md:66-79` (regenerate the AST JSON, add an IR section)

**Interfaces:**
- Consumes: the full public IR surface.
- Produces: nothing code-facing.

The AST JSON example in the guide is the only place in the repo that pins the exact AST shape, and Task 1 broke it. Verified during planning: no binding test asserts exact AST JSON, so nothing under `bindings/**` needs to change.

- [ ] **Step 1: Regenerate the AST JSON example**

Run this to get the real output rather than hand-editing it:

```bash
cargo run -q --example print_ast 2>/dev/null || cat > /tmp/print_ast.rs <<'EOF'
fn main() {
    let p = stix_pattern::parse("[file:size > 1024]").unwrap();
    println!("{}", serde_json::to_string_pretty(&p).unwrap());
}
EOF
echo "If no example exists, get the JSON from a scratch test instead:"
echo 'cargo test -p stix-pattern --lib -- --nocapture print_ast_for_docs'
```

Simplest reliable route: add a temporary test that prints it, copy the output, then delete the test.

```rust
#[test]
fn print_ast_for_docs() {
    let p = crate::parse("[file:size > 1024]").unwrap();
    println!("{}", serde_json::to_string_pretty(&p).unwrap());
}
```

- [ ] **Step 2: Update the guide's AST section**

In `docs/book/src/guide/patterns.md`, replace the JSON block at lines 66-79 with the regenerated output. It will now include `span` objects and the restructured `Observation`, shaped like:

```json
{
  "expression": {
    "Observation": {
      "expression": {
        "Test": {
          "path": {
            "object_type": "file",
            "steps": [ { "Key": "size" } ],
            "span": { "start": 1, "end": 10 }
          },
          "operator": "GreaterThan",
          "negated": false,
          "value": { "Literal": { "Integer": 1024 } },
          "span": { "start": 1, "end": 17 }
        }
      },
      "span": { "start": 0, "end": 18 }
    }
  }
}
```

Use the *actual* printed offsets, not these.

- [ ] **Step 3: Add an IR section to the guide**

Append to `docs/book/src/guide/patterns.md`, after the parse-errors paragraph:

````markdown
## The three-address IR

Alongside the AST, a pattern can be lowered to a linear three-address
representation. Each `[...]` observation becomes a *comparison block* — the unit
the matcher enumerates binding sets over — and a `main` block combines the
observation results:

```rust
let pattern = stix::parse("[file:size > 1024] FOLLOWEDBY [file:name = 'a']").unwrap();
let program = stix::pattern::ir::lower(&pattern);
println!("{}", program.to_listing());
```

```text
block b1 (comparison):
  t0 = load        file:size
  t1 = gt          t0, 1024
       yield       t1

block b2 (comparison):
  t2 = load        file:name
  t3 = eq          t2, 'a'
       yield       t3

block main (observation):
  o0 = observe     b1
  o1 = observe     b2
  o2 = followedby  o0, o1
       ret         o2
```

The IR is in SSA form: an instruction's id names the value it produces, so there
is no separate destination field. Instructions carry the byte span of the source
text they came from.

`ir::render` goes the other way, producing *canonical* pattern text — normalized
whitespace, parentheses only where precedence needs them, and `!=` as the sole
spelling of not-equal (the grammar's other spelling, `<>`, is not currently
accepted by this crate's lexer; see
[issue #29](https://github.com/benjamin-small/stix-rust/issues/29)). Rendering
then reparsing recovers the same AST, which makes canonical text a usable
basis for comparing two patterns:

```rust
let text = stix::pattern::ir::render(&program);
assert_eq!(text, "[file:size > 1024] FOLLOWEDBY [file:name = 'a']");
```

Two caveats. The IR represents more than the matcher can execute —
`FOLLOWEDBY` and the qualifiers lower and render correctly but still return
`MatchError::Unsupported` when matched. And a `Program` that was deserialized or
built by hand should be checked with `Program::validate()` first, since
deserialization does not verify invariants.
````

- [ ] **Step 4: Extend the crate-level docs**

In `crates/stix-pattern/src/lib.rs`, extend the crate doc example so the IR is discoverable from the front page:

```rust
//! Lexer and parser for the STIX 2.1 patterning language.
//!
//! # Example
//!
//! ```
//! use stix_pattern::parse;
//!
//! let pattern = parse("[file:hashes.'SHA-256' = 'abc']").unwrap();
//! let json = serde_json::to_string(&pattern).unwrap();
//! assert!(json.contains("SHA-256"));
//! ```
//!
//! Patterns can also be lowered to a three-address [`ir::Program`] and rendered
//! back to canonical pattern text:
//!
//! ```
//! use stix_pattern::{ir, parse};
//!
//! let pattern = parse("[file:size>1024]").unwrap();
//! let program = ir::lower(&pattern);
//! assert_eq!(ir::render(&program), "[file:size > 1024]");
//! ```
```

- [ ] **Step 5: Verify the docs build and the doctests pass**

Run: `cargo test --workspace` (runs doctests) then `cargo doc -p stix-pattern --no-deps`
Expected: PASS, no `missing_docs` warnings.

If `mdbook` is installed, also run `mdbook build docs/book` and confirm it succeeds. If it is not installed, skip it — the CI docs workflow will build it.

- [ ] **Step 6: Final full verification and commit**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git add crates/stix-pattern/src/lib.rs docs/book/src/guide/patterns.md
git commit -m "docs: document the pattern IR and regenerate the AST example

The AST JSON example changed shape when Observation became a struct variant
and spans were added; regenerated from actual output. Adds a guide section
covering lowering, the listing format, canonical rendering and the
validate-after-deserialize rule."
```

---

## Self-Review

**1. Spec coverage.** Walking the spec section by section:

| Spec requirement | Task |
|---|---|
| Two-tier blocks | 2 (model), 3 (lowering) |
| Placement in `stix-pattern/src/ir/` | 2 |
| Instruction-is-its-own-destination (SSA) | 2 |
| `And`/`Or` shared across tiers | 2 (model), 5 (tier check allows both) |
| `negated` on `Compare` | 2, 3, 6 |
| Public fields | 2 |
| Worked example / listing printer | 4 |
| `lower` total and infallible | 3 |
| Four span changes (`Comparison`, `ObjectPath`, `Observation`, `Qualified`) | 1 |
| No spans on combinators | 1 |
| `Instruction::span` is `Option<Span>` | 2, 3 |
| Canonical rendering (whitespace, `!=`, minimal parens) | 6 |
| Round-trip property + `without_spans` | 1 (helper), 6 (unit), 7 (corpus) |
| Validation rules, all eight (plus the non-finite-float delta) | 5 |
| Dead instructions legal | 5 |
| `schema_version` checked | 5 |
| Internally-tagged ops | 2 |
| Deserialization does not validate, documented | 2 (module docs), 8 (guide) |
| Corpus round-trip test | 7 |
| Listing snapshots | 4 |
| Validation negatives | 5 |
| Span sanity | 3 (unit), 7 (corpus) |
| Non-goals respected (no matcher execution, no edit API, no FFI) | none add these |
| Docs snippet regeneration | 8 |

No gaps.

**2. Placeholder scan.** No `TBD`/`TODO`; every code step carries real code. Two steps intentionally defer detail to the compiler rather than guessing: Task 1 Step 8 ("the compiler names every remaining site") and Task 6 Step 4's `pub(crate) mod render;` fallback. Both are verification-driven, not unspecified work. Task 8 Step 1's "copy the printed output" is deliberate — hand-writing byte offsets into docs would be guessing.

**3. Type consistency.** Checked across tasks: `Operand::Absent` (not `None`) in Tasks 2, 3, 5, 6. `Op::Compare { operator, .. }` (not `op`) in 2, 3, 4, 5, 6. All `Op` variants are struct variants everywhere. `Program::block`, `Block::instruction`, `Block::terminator` defined in Task 2 and used in 5, 6, 7. `render_path`/`render_literal`/`render_seconds` are declared `pub(crate)` in Task 6 and consumed by Task 4, with the ordering hazard called out explicitly in both tasks. `SCHEMA_VERSION` defined in 2, used in 3 and 5. `Pattern::without_spans` defined in Task 1, used in 6 and 7.

`render_float` is defined `pub(crate)` in Task 6 and exercised by Task 6's own tests via `super::render_float`; `literal_is_finite` is private to Task 5's `validate.rs`.

**4. Empirical verification.** Four claims in this plan were checked by compiling and running them during planning rather than asserted from memory: serde's rejection of internally-tagged tuple variants (Delta 1), `{}` dropping the decimal point on whole floats, `{:?}` switching to exponent notation at extreme magnitudes, and the `render_float` precision search round-tripping `f64::MAX`, `f64::MIN_POSITIVE`, `f64::EPSILON` and `5e-324`. The fixed-precision approach that the first draft of this plan proposed was found to render `1e-300` as `"0.0"` and was replaced.

One ordering note for the executor: **Task 4 depends on helpers that Task 6 creates.** Task 4 Step 3 says to write temporary local copies and Task 6 Step 4 says to delete them. If you prefer, run Task 6 before Task 4 and skip both workarounds; nothing else depends on their order.
