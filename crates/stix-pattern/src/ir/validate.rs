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
    #[error(
        "instruction {instr} references value {value}, which is not defined earlier in its block"
    )]
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
    /// One value is consumed by more than one operand.
    ///
    /// Pattern text has no way to name a shared value, so a shared
    /// subexpression would have to be re-rendered once per use — making output
    /// exponential in the number of instructions. Requiring each value to have
    /// at most one consumer keeps every value expression a tree, which is what
    /// [`render`](crate::ir::render) already assumes.
    ///
    /// This covers values only. A comparison *block* is not a value; its reuse
    /// is bounded separately, by [`IrError::BlockObservedTwice`].
    #[error("instruction {instr} references value {value}, which is already used elsewhere")]
    MultipleUses {
        /// The referencing instruction.
        instr: u32,
        /// The value that was already used.
        value: u32,
    },
    /// A comparison block is the target of more than one `Observe`.
    ///
    /// `Observe` names a block rather than a value, so the single-use rule does
    /// not cover it. Pattern text cannot name a shared block either: each
    /// `Observe` is rendered as its own copy of the block, so sharing one block
    /// among *k* observes makes output grow with *k* times the block's size —
    /// quadratic in the program. Requiring each block to be observed at most
    /// once keeps rendered size linear. [`lower`](crate::ir::lower) already
    /// allocates one block per observation.
    #[error("block {block} is observed more than once")]
    BlockObservedTwice {
        /// The block that was observed again.
        block: u32,
    },
    /// A `Compare`'s left-hand side is not a `Load`.
    ///
    /// The renderer takes the compared object path from the `Load` it names; any
    /// other instruction leaves the path missing from the rendered text.
    #[error("instruction {instr} compares value {lhs}, which is not a load")]
    ExpectedLoad {
        /// The offending instruction.
        instr: u32,
        /// The value it named as its left-hand side.
        lhs: u32,
    },
    /// An `EXISTS` comparison is marked negated.
    ///
    /// The grammar has no negated form of `EXISTS`, so the negation could not be
    /// rendered and would be silently dropped.
    #[error("instruction {instr} negates EXISTS, which pattern syntax cannot express")]
    NegatedExists {
        /// The offending instruction.
        instr: u32,
    },
    /// A block has the wrong [`BlockKind`](crate::ir::BlockKind) for its position.
    #[error("block {block} should be {expected}")]
    WrongBlockKind {
        /// The offending block.
        block: u32,
        /// The kind its position requires.
        expected: &'static str,
    },
}

impl Program {
    /// Check that this program is well-formed.
    ///
    /// Lowered programs always pass. Run this on any program that was built by
    /// hand or deserialized, since deserialization does not check invariants.
    ///
    /// Unreferenced ("dead") instructions are allowed — only dangling and
    /// forward *references* are errors. A value may have at most one consumer,
    /// however, and a comparison block at most one `Observe`: see
    /// [`IrError::MultipleUses`] and [`IrError::BlockObservedTwice`].
    ///
    /// Nesting depth is not limited: validation, [`render`](crate::ir::render)
    /// and [`Program::span_of`] are all iterative, so a deeply nested program
    /// cannot overflow the stack.
    pub fn validate(&self) -> Result<(), IrError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(IrError::UnsupportedSchemaVersion {
                found: self.schema_version,
                expected: SCHEMA_VERSION,
            });
        }

        if self.main.kind != BlockKind::Main {
            return Err(IrError::WrongBlockKind {
                block: self.main.id.0,
                expected: "the main block",
            });
        }
        for b in &self.blocks {
            if b.kind != BlockKind::Comparison {
                return Err(IrError::WrongBlockKind {
                    block: b.id.0,
                    expected: "a comparison block",
                });
            }
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
        let mut used_values = HashSet::new();
        let mut observed_blocks = HashSet::new();
        for b in self.blocks.iter().chain(std::iter::once(&self.main)) {
            self.check_block(
                b,
                &comparison_blocks,
                &mut seen_instrs,
                &mut used_values,
                &mut observed_blocks,
            )?;
        }
        Ok(())
    }

    fn check_block(
        &self,
        b: &Block,
        comparison_blocks: &HashSet<BlockId>,
        seen_instrs: &mut HashSet<InstrId>,
        used_values: &mut HashSet<InstrId>,
        observed_blocks: &mut HashSet<BlockId>,
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
                (
                    Op::Load { .. } | Op::Compare { .. } | Op::Yield { .. },
                    BlockKind::Comparison,
                ) => true,
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

            // Resolve one operand: it must name a value defined earlier in this
            // block, and no other operand anywhere may already have consumed it.
            let mut check_value = |value: InstrId| -> Result<(), IrError> {
                if !defined.contains(&value) {
                    return Err(IrError::UnknownValue {
                        instr: instr.id.0,
                        value: value.0,
                    });
                }
                if !used_values.insert(value) {
                    return Err(IrError::MultipleUses {
                        instr: instr.id.0,
                        value: value.0,
                    });
                }
                Ok(())
            };

            match &instr.op {
                Op::Load { .. } => {}
                Op::Compare {
                    operator,
                    negated,
                    lhs,
                    rhs,
                } => {
                    check_value(*lhs)?;
                    if !matches!(b.instruction(*lhs).map(|i| &i.op), Some(Op::Load { .. })) {
                        return Err(IrError::ExpectedLoad {
                            instr: instr.id.0,
                            lhs: lhs.0,
                        });
                    }
                    if *operator == ComparisonOperator::Exists && *negated {
                        return Err(IrError::NegatedExists { instr: instr.id.0 });
                    }
                    if let (ComparisonOperator::In, Operand::Set(items)) = (operator, rhs) {
                        if items.is_empty() {
                            return Err(IrError::OperandShape {
                                instr: instr.id.0,
                                detail: "IN takes a non-empty set",
                            });
                        }
                    }
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
                            detail:
                                "EXISTS takes no operand, IN takes a set, others take a literal",
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
                    if !observed_blocks.insert(*block) {
                        return Err(IrError::BlockObservedTwice { block: block.0 });
                    }
                }
            }

            defined.insert(instr.id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{lower, BlockKind, InstrId, Instruction, Op, Operand, Program};
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
        assert!(matches!(
            p.validate(),
            Err(IrError::MissingTerminator { .. })
        ));
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
        assert!(matches!(
            p.validate(),
            Err(IrError::MisplacedTerminator { .. })
        ));
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
        assert!(matches!(
            p.validate(),
            Err(IrError::DuplicateInstrId { .. })
        ));
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
        use crate::ast::{Literal, ObjectPath, PathStep};
        use crate::error::Span;

        let mut p = valid();
        // A whole unreferenced subtree is legal: an editor mid-edit produces these.
        // Its own operands are single-use, which is what keeps this about deadness
        // rather than about sharing.
        p.blocks[0].instructions.splice(
            1..1,
            [
                Instruction {
                    id: InstrId(600),
                    op: Op::Load {
                        path: ObjectPath {
                            object_type: "file".to_string(),
                            steps: vec![PathStep::Key("name".to_string())],
                            span: Span::default(),
                        },
                    },
                    span: None,
                },
                Instruction {
                    id: InstrId(601),
                    op: Op::Compare {
                        operator: ComparisonOperator::Equal,
                        negated: false,
                        lhs: InstrId(600),
                        rhs: Operand::Literal(Literal::String("dead".to_string())),
                    },
                    span: None,
                },
            ],
        );
        assert!(p.validate().is_ok(), "{:?}", p.validate());
        // The dead compare's value is consumed by nothing.
        assert_eq!(
            p.blocks[0]
                .instructions
                .iter()
                .filter(|i| matches!(i.op, Op::Compare { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn rejects_a_duplicate_block_id() {
        let mut p = valid();
        let dup = p.blocks[0].clone();
        p.blocks.push(dup);
        assert!(matches!(
            p.validate(),
            Err(IrError::DuplicateBlockId { .. })
        ));
    }

    #[test]
    fn rejects_a_block_observed_twice() {
        let mut p = lower(&parse("[file:size > 1] AND [file:size > 2]").unwrap());
        let first = p.blocks[0].id;
        let mut seen = 0;
        for i in &mut p.main.instructions {
            if let Op::Observe { block } = &mut i.op {
                *block = first;
                seen += 1;
            }
        }
        assert_eq!(seen, 2);
        assert_eq!(
            p.validate(),
            Err(IrError::BlockObservedTwice { block: first.0 })
        );
    }

    #[test]
    fn rejects_a_value_used_twice() {
        let mut p = valid();
        // The load is already consumed by the compare; make the yield take it too.
        let load_id = p.blocks[0].instructions[0].id;
        let last = p.blocks[0].instructions.len() - 1;
        p.blocks[0].instructions[last].op = Op::Yield { value: load_id };
        assert!(
            matches!(p.validate(), Err(IrError::MultipleUses { value: 0, .. })),
            "{:?}",
            p.validate()
        );
    }

    /// A program whose `main` block chains `levels` qualifiers onto one observation.
    fn qualifier_chain(levels: u32) -> Program {
        let mut p = valid();
        let observe = p.main.instructions[0].clone();
        let mut instrs = vec![observe.clone()];
        let mut acc = observe.id;
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

    #[test]
    fn accepts_a_program_nested_past_the_old_depth_limit() {
        // 257 levels deep: one past the 256-level limit removed by issue #32.
        let p = qualifier_chain(255);
        assert_eq!(p.validate(), Ok(()));
    }

    #[test]
    fn accepts_a_very_deeply_nested_program_on_a_small_stack() {
        // Validation is iterative, so depth costs nothing on the stack.
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let p = qualifier_chain(10_000);
                assert_eq!(p.validate(), Ok(()));
            })
            .expect("spawn test thread")
            .join()
            .expect("test thread panicked");
    }

    #[test]
    fn rejects_an_empty_in_set() {
        let mut p = lower(&parse("[ipv4-addr:value IN ('1.1.1.1')]").unwrap());
        match &mut p.blocks[0].instructions[1].op {
            Op::Compare { rhs, .. } => *rhs = Operand::Set(Vec::new()),
            other => panic!("expected a compare, got {other:?}"),
        }
        assert!(
            matches!(p.validate(), Err(IrError::OperandShape { .. })),
            "{:?}",
            p.validate()
        );
    }

    #[test]
    fn rejects_a_compare_whose_lhs_is_not_a_load() {
        use crate::ast::Literal;

        let mut p = valid();
        let compare_id = p.blocks[0].instructions[1].id;
        let chained = InstrId(700);
        p.blocks[0].instructions.insert(
            2,
            Instruction {
                id: chained,
                op: Op::Compare {
                    operator: ComparisonOperator::GreaterThan,
                    negated: false,
                    lhs: compare_id,
                    rhs: Operand::Literal(Literal::Integer(2)),
                },
                span: None,
            },
        );
        // Keep every value single-use: the yield now takes the chained compare.
        let last = p.blocks[0].instructions.len() - 1;
        p.blocks[0].instructions[last].op = Op::Yield { value: chained };
        assert!(
            matches!(p.validate(), Err(IrError::ExpectedLoad { instr: 700, .. })),
            "{:?}",
            p.validate()
        );
    }

    #[test]
    fn rejects_a_negated_exists() {
        let mut p = lower(&parse("[EXISTS file:name]").unwrap());
        match &mut p.blocks[0].instructions[1].op {
            Op::Compare { negated, .. } => *negated = true,
            other => panic!("expected a compare, got {other:?}"),
        }
        assert!(
            matches!(p.validate(), Err(IrError::NegatedExists { .. })),
            "{:?}",
            p.validate()
        );
    }

    #[test]
    fn rejects_a_main_block_of_the_wrong_kind() {
        let mut p = valid();
        p.main.kind = BlockKind::Comparison;
        assert!(
            matches!(p.validate(), Err(IrError::WrongBlockKind { .. })),
            "{:?}",
            p.validate()
        );
    }

    #[test]
    fn rejects_a_comparison_block_of_the_wrong_kind() {
        let mut p = valid();
        p.blocks[0].kind = BlockKind::Main;
        assert!(
            matches!(p.validate(), Err(IrError::WrongBlockKind { .. })),
            "{:?}",
            p.validate()
        );
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
