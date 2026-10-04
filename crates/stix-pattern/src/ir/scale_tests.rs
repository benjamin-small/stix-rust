//! Complexity tests: render, validate and span_of must be linear in program
//! size. Programs here are built by hand so they can be far larger than any
//! pattern text a caller would write.

use crate::ast::{ComparisonOperator, Literal, ObjectPath, PathStep};
use std::time::Instant;

use crate::error::Span;
use crate::ir::index::ProgramIndex;
use crate::ir::instr::scans;
use crate::ir::{
    render, Block, BlockId, BlockKind, InstrId, Instruction, Op, Operand, Program, SCHEMA_VERSION,
};

fn instr(id: u32, op: Op, span: Option<Span>) -> Instruction {
    Instruction {
        id: InstrId(id),
        op,
        span,
    }
}

/// `n` observations `[file:size > 1]` joined left-associatively by `FOLLOWEDBY`:
/// `n` comparison blocks and a main block of `2n` instructions. The chain is
/// flat in `main`, so every operand and block lookup is a candidate for a
/// linear scan.
pub(crate) fn followedby_chain(n: u32) -> Program {
    let mut blocks = Vec::with_capacity(n as usize);
    for i in 0..n {
        let base = 3 * i;
        let path = ObjectPath {
            object_type: "file".to_string(),
            steps: vec![PathStep::Key("size".to_string())],
            span: Span::default(),
        };
        blocks.push(Block {
            id: BlockId(i),
            kind: BlockKind::Comparison,
            instructions: vec![
                instr(base, Op::Load { path }, None),
                instr(
                    base + 1,
                    Op::Compare {
                        operator: ComparisonOperator::GreaterThan,
                        negated: false,
                        lhs: InstrId(base),
                        rhs: Operand::Literal(Literal::Integer(1)),
                    },
                    None,
                ),
                instr(
                    base + 2,
                    Op::Yield {
                        value: InstrId(base + 1),
                    },
                    None,
                ),
            ],
        });
    }
    let mut next = 3 * n;
    let mut main = Vec::new();
    let mut acc: Option<InstrId> = None;
    for i in 0..n {
        let obs = InstrId(next);
        next += 1;
        // Only the observes carry spans, as `lower` would write them.
        let span = Some(Span {
            start: i as usize,
            end: i as usize + 1,
        });
        main.push(instr(obs.0, Op::Observe { block: BlockId(i) }, span));
        acc = Some(match acc {
            None => obs,
            Some(lhs) => {
                let id = InstrId(next);
                next += 1;
                main.push(instr(id.0, Op::FollowedBy { lhs, rhs: obs }, None));
                id
            }
        });
    }
    main.push(instr(
        next,
        Op::Ret {
            value: acc.expect("n > 0"),
        },
        None,
    ));
    Program {
        schema_version: SCHEMA_VERSION,
        blocks,
        main: Block {
            id: BlockId(n),
            kind: BlockKind::Main,
            instructions: main,
        },
    }
}

/// One observation whose comparison block ANDs `n` comparisons together
/// left-associatively: a single block of `3n` instructions, so every `Compare`
/// operand lookup is a candidate for a linear scan within it.
pub(crate) fn and_chain(n: u32) -> Program {
    let mut ins = Vec::new();
    let mut next = 0u32;
    let mut acc: Option<InstrId> = None;
    for _ in 0..n {
        let load = InstrId(next);
        let cmp = InstrId(next + 1);
        next += 2;
        let path = ObjectPath {
            object_type: "file".to_string(),
            steps: vec![PathStep::Key("size".to_string())],
            span: Span::default(),
        };
        ins.push(instr(load.0, Op::Load { path }, None));
        ins.push(instr(
            cmp.0,
            Op::Compare {
                operator: ComparisonOperator::GreaterThan,
                negated: false,
                lhs: load,
                rhs: Operand::Literal(Literal::Integer(1)),
            },
            None,
        ));
        acc = Some(match acc {
            None => cmp,
            Some(lhs) => {
                let id = InstrId(next);
                next += 1;
                ins.push(instr(id.0, Op::And { lhs, rhs: cmp }, None));
                id
            }
        });
    }
    ins.push(instr(
        next,
        Op::Yield {
            value: acc.expect("n > 0"),
        },
        None,
    ));
    let observe = InstrId(next + 1);
    Program {
        schema_version: SCHEMA_VERSION,
        blocks: vec![Block {
            id: BlockId(0),
            kind: BlockKind::Comparison,
            instructions: ins,
        }],
        main: Block {
            id: BlockId(1),
            kind: BlockKind::Main,
            instructions: vec![
                instr(
                    observe.0,
                    Op::Observe { block: BlockId(0) },
                    Some(Span { start: 0, end: 1 }),
                ),
                instr(observe.0 + 1, Op::Ret { value: observe }, None),
            ],
        },
    }
}

/// Run `f` and return how many linear-scan lookups it made on this thread.
fn scans_during<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let before = scans::count();
    let out = f();
    (out, scans::count() - before)
}

#[test]
fn the_chains_render_as_expected_at_small_size() {
    assert_eq!(
        render(&followedby_chain(3)),
        "[file:size > 1] FOLLOWEDBY [file:size > 1] FOLLOWEDBY [file:size > 1]"
    );
    assert_eq!(
        render(&and_chain(3)),
        "[file:size > 1 AND file:size > 1 AND file:size > 1]"
    );
}

#[test]
fn the_hot_paths_make_no_linear_scans_and_stay_fast_on_large_programs() {
    const N: u32 = 100_000;
    let started = Instant::now();

    // A flat chain in `main`, with one comparison block per observation.
    let fb = followedby_chain(N);
    let last = fb.main.instructions.last().unwrap().id;
    let (text, n) = scans_during(|| render(&fb));
    assert_eq!(n, 0, "render made linear scans");
    assert_eq!(text.matches("FOLLOWEDBY").count(), N as usize - 1);
    let (valid, n) = scans_during(|| fb.validate());
    assert_eq!(n, 0, "validate made linear scans");
    assert_eq!(valid, Ok(()));
    let (span, n) = scans_during(|| fb.span_of(fb.main.id, last));
    assert_eq!(n, 0, "span_of made linear scans");
    assert_eq!(
        span,
        Some(Span {
            start: 0,
            end: N as usize
        })
    );

    // One huge comparison block.
    let and = and_chain(N);
    let yield_id = and.blocks[0].instructions.last().unwrap().id;
    let (text, n) = scans_during(|| render(&and));
    assert_eq!(n, 0, "render made linear scans");
    assert_eq!(text.matches(" AND ").count(), N as usize - 1);
    let (valid, n) = scans_during(|| and.validate());
    assert_eq!(n, 0, "validate made linear scans");
    assert_eq!(valid, Ok(()));
    // No span anywhere in the block, so this walks all of it and finds none.
    let (span, n) = scans_during(|| and.span_of(BlockId(0), yield_id));
    assert_eq!(n, 0, "span_of made linear scans");
    assert_eq!(span, None);

    // Generous: a few hundred ms in debug on a laptop when linear, minutes when
    // quadratic.
    assert!(
        started.elapsed().as_secs() < 20,
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn the_scan_counter_counts_the_public_lookups() {
    let p = followedby_chain(2);
    let (_, n) = scans_during(|| {
        p.main.instruction(InstrId(0));
        p.block(BlockId(0));
    });
    assert_eq!(n, 2);
}

#[test]
fn an_index_resolves_duplicate_ids_like_the_scan_does() {
    let mut p = followedby_chain(2);
    // Two instructions in one block with the same id; the first must win.
    let dup = p.blocks[0].instructions[0].clone();
    let mut other = dup.clone();
    other.op = Op::Yield { value: dup.id };
    p.blocks[0].instructions.push(other);
    // And two blocks with one id.
    let mut second = p.blocks[1].clone();
    second.id = BlockId(0);
    p.blocks.push(second);

    let index = ProgramIndex::new(&p);
    for b in &p.blocks {
        for i in &b.instructions {
            assert_eq!(
                index.block(b.id).unwrap().instruction(i.id),
                p.block(b.id).unwrap().instruction(i.id)
            );
        }
    }
    assert!(std::ptr::eq(
        index.block(BlockId(0)).unwrap().block(),
        p.block(BlockId(0)).unwrap()
    ));
}
