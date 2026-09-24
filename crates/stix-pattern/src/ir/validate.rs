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

            let check_value = |value: InstrId| -> Result<(), IrError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{lower, InstrId, Instruction, Op, Operand, Program};
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
