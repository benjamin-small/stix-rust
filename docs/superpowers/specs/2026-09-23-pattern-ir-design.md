# Pattern IR (Three-Address Code) — Design

**Date:** 2026-09-23
**Status:** Approved (brainstorming complete; pending spec review)
**Scope:** Sub-projects **A** (IR core) and **B** (renderer + round-trip proof) of
the pattern-IR effort. Sub-projects C, D and E are named here for context and are
explicitly out of scope.
**Owner agent:** `rust-core` (the code lives under `crates/stix-pattern/`).

## Purpose

`stix-pattern` currently exposes exactly one representation of a parsed pattern:
the nested AST in `ast.rs`, surfaced downstream through `stix-ffi`'s
`Pattern::to_json` and every binding's `Pattern.ast`. A nested tree is the right
shape for a parser to produce and the wrong shape for three things this project
wants to do:

1. **Execute a pattern repeatedly.** The matcher walks the AST on every match. A
   linear, pre-resolved instruction list is compiled once and evaluated many times,
   and makes the observation-scoping rules explicit rather than implicit in the
   traversal.
2. **Analyse and interchange patterns.** Comparing, deduplicating, or translating
   patterns into other query languages needs a normalized form with a stable
   serialized schema. A tree with arbitrary nesting and redundant parentheses is a
   poor substrate for that.
3. **Drive a structural query editor.** An editor that breaks a query into
   individually addressable pieces needs stable identity per piece, a mapping back
   to the source text, and a way to turn the edited structure back into valid
   pattern text.

This design adds a **three-address intermediate representation** that serves all
three, and the **renderer** that closes the loop back to pattern text.

## Decomposition and where this spec sits

The full effort decomposes into five sub-projects. Each is independently useful:

| | Sub-project | What it is | Depends on |
|---|---|---|---|
| **A** | IR core | Instruction set, `Program` model, `lower`, validator, serde schema, listing printer | — |
| **B** | Renderer | `render(Program) -> String` plus the round-trip property proof | A |
| **C** | Structural edit API | Typed mutation ops with revalidation; makes IR authoritative | A, B |
| **D** | IR execution | `stix-matcher` evaluates a `Program`; compile-once/match-many | A |
| **E** | Surface exposure | IR access through `stix-ffi` and the four bindings | A, (C) |

**This spec covers A and B together.** B is bundled with A deliberately: the
round-trip property is the load-bearing proof that the IR is faithful. If
`parse(render(lower(ast))) != ast` on the corpus, the IR has lost information, and
every later sub-project would be built on that loss. Proving it now, while the
instruction set is still cheap to change, is the point.

## Key decisions

### Two-tier blocks, not a flat listing

STIX patterns are two-tier and the tiers have **different evaluation semantics**.
Comparisons inside `[...]` are evaluated per binding-set — the matcher enumerates
one object per type per observation — while observation-level operators combine
whole observation results. Any IR that flattens that distinction has to
reconstruct it in order to execute.

So a `Program` is a set of **comparison blocks** plus one **main block**.
Instructions *within* a block are strict three-address form. A comparison block is
exactly the unit the matcher enumerates bindings over, and it is also exactly the
"piece" a query editor displays as one observation. Both consumers get their
structure handed to them.

Two alternatives were considered and rejected:

- **Flat listing with `enter_obs`/`exit_obs` markers.** Purest classic 3AC and the
  simplest addressing model, but the matcher must rescan markers to rebuild
  evaluation units, an editor must infer grouping the IR does not state, and the
  validator must check marker balance.
- **Flat listing plus a side region table.** Fixes execution but introduces a
  cross-structure invariant — region bounds must stay consistent with the
  instruction list — that every structural edit in sub-project C would have to
  maintain. The worst of the three once the IR becomes mutable.

### Placement: a module in `stix-pattern`, not a new crate

The IR lives at `crates/stix-pattern/src/ir/`, not in a new `stix-ir` crate.

The structural case for a separate crate does not survive inspection:
`stix-matcher` already depends on `stix-pattern`, so sub-project D reaches
`stix_pattern::ir` with no new dependency, and the IR has no dependencies of its
own beyond the AST it lowers from. A new crate would cost a sixth crates.io publish
target, another node in the `release-crates.yml` DAG, and another version to hold
in lockstep on a project already shipped to three registries at 0.1.1.

If it outgrows this home, promotion to `stix-ir` with a `pub use` re-export from
`stix_pattern::ir` keeps every consumer compiling.

### An instruction is its own destination

There is no separate `dest` field. A value is named by the `InstrId` of the
instruction that produces it — SSA in the LLVM style. This gives one id space
instead of two, ids that stay stable under reordering, and — critically for
sub-project C — a structural edit that deletes an instruction leaves **dangling
references the validator can find**, rather than silently renumbered temporaries.

The listing printer still renders `t0 = load …`. `t0` is a display ordinal, not an
identity.

### `And`/`Or` are shared across tiers

Rather than duplicating them as `ObsAnd`/`ObsOr`, the enclosing `BlockKind`
disambiguates which evaluation semantics apply. Fewer opcodes, and the block
already carries the information.

### `negated` stays a field on `Compare`

STIX spells negation as `file:name NOT = 'x'` — the `NOT` is glued to the operator,
not a general boolean negation. Modelling it as a separate `Not` instruction would
make rendering lossy, because nothing would record whether to re-emit `NOT =` or
some other form.

## Architecture

```
crates/stix-pattern/src/
├── lib.rs          # + pub mod ir;
├── ast.rs          # + span fields (see "Spans" below)
├── error.rs        # unchanged (Span already lives here)
├── lexer.rs        # unchanged
├── parser.rs       # + span threading onto AST nodes
└── ir/
    ├── mod.rs      # Program, Block, re-exports, module docs
    ├── instr.rs    # Instruction, Op, Operand, InstrId, BlockId, BlockKind
    ├── lower.rs    # lower(&Pattern) -> Program
    ├── render.rs   # render(&Program) -> String
    └── validate.rs # Program::validate(), IrError
```

The five files under `ir/` are new. Three existing files are modified: `lib.rs`
gains `pub mod ir;`, and `ast.rs` and `parser.rs` gain spans.
`#![warn(missing_docs)]` is enforced crate-wide, so every public item needs a doc
comment.

## Data model

```rust
/// A lowered pattern: comparison blocks plus the observation-tier block.
pub struct Program {
    pub schema_version: u32,
    pub blocks: Vec<Block>,
    pub main: Block,
}

pub struct Block {
    pub id: BlockId,
    pub kind: BlockKind,
    pub instructions: Vec<Instruction>,
}

pub enum BlockKind { Comparison, Main }

pub struct Instruction {
    pub id: InstrId,
    pub op: Op,
    pub span: Option<Span>,
}

pub enum Op {
    // Comparison tier
    Load { path: ObjectPath },
    Compare { op: ComparisonOperator, negated: bool, lhs: InstrId, rhs: Operand },
    And(InstrId, InstrId),
    Or(InstrId, InstrId),
    Yield(InstrId),

    // Observation tier
    Observe { block: BlockId },
    FollowedBy(InstrId, InstrId),
    Within { input: InstrId, seconds: f64 },
    Repeats { input: InstrId, count: u64 },
    StartStop { input: InstrId, start: String, stop: String },
    Ret(InstrId),
}

pub enum Operand {
    Literal(Literal),
    Set(Vec<Literal>),
    /// For `EXISTS`, which takes no right-hand side.
    None,
}
```

`InstrId` and `BlockId` are newtypes over `u32`. `ObjectPath`, `ComparisonOperator`
and `Literal` are reused unchanged from `ast.rs`.

**Fields are public**, matching the existing AST exactly — every field on `Pattern`,
`Comparison` and `ObjectPath` is already `pub`. Invariants live in `validate()`
rather than in constructors. Sub-project C therefore gets construction for free and
only needs to add ergonomic mutation helpers. The trade-off is accepted knowingly:
nothing prevents a consumer building an invalid `Program`, so `validate()` is the
guard rather than the type system.

### Worked example

For:

```
[ipv4-addr:value = '1.2.3.4' AND file:size > 1024]
  FOLLOWEDBY [domain-name:value = 'evil.example']
  WITHIN 300 SECONDS
```

`to_listing()` produces:

```
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
```

Two things in that listing are easy to get wrong, and both were corrected after
being checked against the parser rather than reasoned about:

**`WITHIN` qualifies only the second observation, not the `FOLLOWEDBY`.** In the
STIX 2.1 grammar a qualifier attaches to `observationExpression` — the tightest
production, meaning a single bracketed observation or a parenthesized group — so
`[A] FOLLOWEDBY [B] WITHIN 300 SECONDS` parses as `FollowedBy(A, Within(B, 300))`.
That is why `within` precedes `followedby` in the listing and takes `o1` rather
than the `followedby` result. To scope the window over the whole sequence the
pattern must parenthesize it:

```
([ipv4-addr:value = '1.2.3.4'] FOLLOWEDBY [domain-name:value = 'evil.example'])
  WITHIN 300 SECONDS
```

which lowers to `within` taking the `followedby` result instead.

**Comparison blocks are numbered from 1, not 0.** `main` is allocated first and
takes `BlockId(0)`, so the comparison blocks are `b1`, `b2`, … . Block ids are
opaque and nothing depends on their values; only the listing's display reflects
them.

## Lowering

`lower(&Pattern) -> Program` is a **total, infallible** post-order walk. A
well-formed AST always lowers; there is no error path.

- Each `ObservationExpression::Observation` opens a new comparison block. The
  comparison tree inside lowers to `Load`/`Compare`/`And`/`Or`, terminated by
  `Yield` of the root result.
- `ComparisonExpression::Test` emits a `Load` of the path followed by a `Compare`
  referencing it. `Exists` emits `Compare` with `Operand::None`; `In` emits
  `Operand::Set`; every other operator emits `Operand::Literal`.
- The observation tier lowers into `main`: `Observation` becomes `Observe { block }`,
  `And`/`Or`/`FollowedBy` become their instructions, and `Qualified` becomes
  `Within`/`Repeats`/`StartStop` taking the qualified sub-expression's result as
  input. `main` is terminated by `Ret`.

## Spans

`Span` already exists in `error.rs` as a half-open byte range, but only `ParseError`
carries one — the parser has token spans and discards them.

This design **threads spans through the parser onto the AST nodes that produce
instructions**, and lowering copies them onto the `Instruction`s it emits. Spans
then become useful everywhere — error reporting, editors, and every binding —
rather than only inside the IR.

Precisely four AST changes, chosen to be minimal:

| AST node | Change | Covers | Lands on |
|---|---|---|---|
| `Comparison` | add `span: Span` | the whole property test | `Compare` |
| `ObjectPath` | add `span: Span` | just the path text | `Load` |
| `ObservationExpression::Observation` | tuple variant → struct variant `{ expression, span }` | the `[...]` extent | `Observe` |
| `ObservationExpression::Qualified` | add `span: Span` (already a struct variant) | the qualifier clause | `Within`/`Repeats`/`StartStop` |

**The combinator variants deliberately get no span.** `And`, `Or` and `FollowedBy`
at both tiers derive their extent as the union of their children's spans, so
storing one would be redundant state that structural editing must then keep
consistent. A `span_of(instr)` helper computes the union on demand.

Converting `Observation` from a tuple variant to a struct variant is the one
non-additive AST change in this design: its serialized form goes from
`{"Observation": {...}}` to `{"Observation": {"expression": {...}, "span": {...}}}`.
That is a breaking change to the AST JSON schema visible in all four bindings, and
it is the reason this work belongs in a 0.2.0 rather than a patch release.

Two consequences are accepted:

- **`Pattern.ast` JSON changes shape** in all four bindings. Three of the four
  changes are additive `span` fields, which consumers reading known keys survive;
  the `Observation` variant restructuring is **breaking**, so the release carrying
  this is a 0.2.0.

  Repo-internal fallout was measured and is small. **No binding test breaks** —
  all four assert the AST loosely (`isinstance(ast, dict)`,
  `ast.toString().contains("ipv4-addr")`, `typeof ast === "object"`), and
  `stix-ffi`'s `pattern_to_json_round_trips` is a serde round trip that is
  structure-agnostic. Exactly one place in the repo pins the exact AST JSON:
  the example at `docs/book/src/guide/patterns.md:66-79`, which must be
  regenerated. That file is outside every `AGENTS.md` ownership path, so it is a
  parent-owned edit rather than a subagent one.
- **The round-trip invariant needs a span normalizer** (see below).

The rejected alternative was a second parser entry point (`parse_to_ir`) that
lowers with spans as it parses, leaving the AST byte-identical. It was rejected
because instruction-building logic would then live in both `parser.rs` and
`lower.rs` and could drift.

`Instruction::span` is `Option<Span>` because programs constructed
programmatically — the normal case in sub-project C — have no source text.

## Rendering

`render(&Program) -> String` is a **canonicalizer**, not a reproducer:

- Normalized whitespace.
- `!=` always, never the equivalent `<>`.
- Parentheses emitted only where operator precedence demands them.

Canonical rendering delivers a meaningful part of the interchange goal directly:
two patterns are equivalent iff their canonical renderings are equal, which yields
comparison and deduplication without a separate normalization pass.

### The round-trip property

Textual identity (`render(lower(parse(s))) == s`) is **not** a goal and is not
achievable — whitespace and redundant parentheses legitimately differ. The
guaranteed and tested property is:

```
parse(render(lower(ast))) == ast          (modulo spans)
```

Render the IR, re-parse the text, recover the identical AST. This is the strongest
guarantee that actually holds, and it is precisely what sub-project C needs: if the
editor mutates IR and renders it, the output is valid STIX that means what the IR
says.

Because rendered text has different byte offsets than the original source, spans
necessarily differ even when structure is identical. A `Pattern::without_spans()`
normalizer (clearing spans throughout the tree) is therefore part of this design,
not an implementation detail, and the tested form of the invariant is:

```
parse(render(lower(ast))).without_spans() == ast.without_spans()
```

## Validation

`Program::validate() -> Result<(), IrError>` serves two consumers that lowering
does not: hand-built programs (sub-project C) and **programs deserialized from
JSON**, which are untrusted input the moment the interchange goal is real.

Rules:

- Every `InstrId` reference resolves to an instruction **earlier in the same block**.
- `Observe { block }` targets an existing block whose kind is `Comparison`.
- Each comparison block ends in exactly one `Yield`; `main` ends in exactly one
  `Ret`; terminators appear nowhere else.
- Tier ops stay in their tier: `Load`/`Compare`/`Yield` only in comparison blocks;
  `Observe`/`FollowedBy`/`Within`/`Repeats`/`StartStop`/`Ret` only in `main`.
- Operand shape matches the operator: `Exists` carries `Operand::None`, `In` carries
  `Operand::Set`, all others carry `Operand::Literal`.
- `InstrId`s are globally unique; `BlockId`s are unique.

**Dead instructions are legal.** An editor mid-edit routinely holds an orphaned
instruction it is about to rewire, and a validator that rejects that is a validator
that cannot run during editing. Dangling *references* are errors; unreferenced
instructions are not.

`IrError` is a `thiserror` enum with one variant per rule, carrying the offending
`InstrId`/`BlockId` so an editor can point at the problem.

## Serialization

`Program` and everything under it derive `Serialize`/`Deserialize`, matching the
AST's existing treatment.

- `schema_version: u32` is `1`, checked on deserialize; any other value is a typed
  error. Versioning from day one, because the IR schema becomes a public contract
  the moment sub-project E exposes it through the bindings.
- `Op` serializes **internally tagged** (`{"op": "compare", …}`) rather than serde's
  default externally-tagged form. Self-describing and readable matters more than
  compactness for a format whose purpose is consumption by other tools.
- Deserialization does not itself validate; callers run `validate()`. This is stated
  explicitly in the module docs, since skipping it on untrusted input is the
  foreseeable misuse.

## Testing

- **Corpus round-trip** (`tests/ir_roundtrip.rs`): every pattern in
  `tests/fixtures/valid_patterns.txt` — 26 of them — is lowered, validated,
  rendered, reparsed, and compared to the original AST modulo spans. This is the
  primary proof obligation of sub-project B.
- **Listing snapshots**: a handful of representative patterns (simple comparison,
  boolean nesting, `IN` set, `EXISTS`, `FOLLOWEDBY` with a qualifier) assert their
  exact `to_listing()` output, giving readable regression protection over the
  instruction set.
- **Validation negatives**: hand-built broken programs assert each `IrError`
  variant — dangling reference, forward reference, missing terminator, tier
  violation, operand-shape mismatch, duplicate id.
- **Span sanity**: for programs lowered from source, every instruction's span is
  in-bounds and non-empty.
- **Unit tests** per module, following the existing in-file `mod tests` convention.

## Non-goals

Explicitly out of scope for this spec:

- **Matcher execution over the IR** (sub-project D). The matcher continues to walk
  the AST; nothing about its behavior changes.
- **Mutation / edit API** (sub-project C). Public fields make construction possible;
  ergonomic and revalidating edit operations are separate work.
- **FFI and binding exposure** (sub-project E). No IR reaches `stix-ffi` or the
  bindings, and no binding code or test needs to change — see the measured fallout
  under "Spans". The only binding-visible effect is the changed `Pattern.ast`
  output shape, which adds no new API.

And one pre-existing gap this work does **not** close: the IR faithfully represents
more than the matcher can run. `FollowedBy`, `Within`, `Repeats` and `StartStop`
all lower and render correctly, while matching them still returns
`MatchError::Unsupported`. Representable is not the same as executable, and that
remains true after this lands.

## Success criteria

1. `lower` is total over the AST — every pattern in the conformance corpus lowers
   without panic.
2. `parse(render(lower(ast))).without_spans() == ast.without_spans()` holds for all
   26 corpus patterns.
3. `validate()` accepts every lowered program and rejects each hand-built violation
   with the correct typed error.
4. A `Program` survives a JSON serialize/deserialize round trip unchanged.
5. `cargo test` green workspace-wide, `cargo clippy` clean, `missing_docs` satisfied.
