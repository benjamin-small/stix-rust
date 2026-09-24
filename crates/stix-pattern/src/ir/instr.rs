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
