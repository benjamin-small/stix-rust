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
/// This is total for every [`Pattern`] that satisfies the AST invariants, which
/// [`parse`](crate::parse) guarantees. The result always satisfies
/// [`Program::validate`](crate::ir::Program::validate).
///
/// Each n-ary chain node becomes the left-associative chain of binary
/// operations it stands for, so `a OR b OR c` lowers to `Or(Or(a, b), c)`.
///
/// # Panics
///
/// If a chain node has no operands. This can only happen in hand-built or
/// deserialized [`Pattern`]s that violate the AST invariants.
pub fn lower(pattern: &Pattern) -> Program {
    let mut l = Lowerer {
        next_instr: 0,
        next_block: 0,
        blocks: Vec::new(),
    };
    let main_id = l.fresh_block();
    let mut main_instrs = Vec::new();
    let result = l.lower_observation(&pattern.expression, &mut main_instrs);
    l.push(&mut main_instrs, Op::Ret { value: result }, None);
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

/// An AST node that lowers to a single value: lets [`Lowerer::lower_chain`]
/// serve both the observation and the comparison level.
trait Lowerable {
    fn lower_into(&self, l: &mut Lowerer, into: &mut Vec<Instruction>) -> InstrId;
}

impl Lowerable for ObservationExpression {
    fn lower_into(&self, l: &mut Lowerer, into: &mut Vec<Instruction>) -> InstrId {
        l.lower_observation(self, into)
    }
}

impl Lowerable for ComparisonExpression {
    fn lower_into(&self, l: &mut Lowerer, into: &mut Vec<Instruction>) -> InstrId {
        l.lower_comparison(self, into)
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
            ObservationExpression::And(xs) => {
                self.lower_chain(xs, into, |lhs, rhs| Op::And { lhs, rhs })
            }
            ObservationExpression::Or(xs) => {
                self.lower_chain(xs, into, |lhs, rhs| Op::Or { lhs, rhs })
            }
            ObservationExpression::FollowedBy(xs) => {
                self.lower_chain(xs, into, |lhs, rhs| Op::FollowedBy { lhs, rhs })
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
            ComparisonExpression::And(xs) => {
                self.lower_chain(xs, into, |lhs, rhs| Op::And { lhs, rhs })
            }
            ComparisonExpression::Or(xs) => {
                self.lower_chain(xs, into, |lhs, rhs| Op::Or { lhs, rhs })
            }
        }
    }

    /// Lower an n-ary chain node as the left-associative binary chain it
    /// stands for: `[a, b, c]` becomes `op(op(a, b), c)`, emitted in the same
    /// order as the nested binary form would be. Iterates over the operands,
    /// so chain length costs no stack.
    fn lower_chain<E: Lowerable>(
        &mut self,
        operands: &[E],
        into: &mut Vec<Instruction>,
        op: fn(InstrId, InstrId) -> Op,
    ) -> InstrId {
        let (first, rest) = operands
            .split_first()
            .expect("chain nodes have at least two operands");
        let mut acc = first.lower_into(self, into);
        for operand in rest {
            let rhs = operand.lower_into(self, into);
            acc = self.push(into, op(acc, rhs), None);
        }
        acc
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
    fn chains_lower_to_left_associative_binary_ops() {
        let p = parse("[x:v=1] OR [x:v=2] OR [x:v=3]").unwrap();
        let prog = lower(&p);
        let main = &prog.main.instructions;
        let ops: Vec<&Op> = main.iter().map(|i| &i.op).collect();
        assert!(matches!(ops[0], Op::Observe { .. }));
        assert!(matches!(ops[1], Op::Observe { .. }));
        assert_eq!(
            *ops[2],
            Op::Or {
                lhs: main[0].id,
                rhs: main[1].id
            }
        );
        assert!(matches!(ops[3], Op::Observe { .. }));
        assert_eq!(
            *ops[4],
            Op::Or {
                lhs: main[2].id,
                rhs: main[3].id
            }
        );
        assert!(matches!(ops[5], Op::Ret { .. }));

        // The same holds inside a comparison block.
        let p = parse("[x:v=1 AND x:v=2 AND x:v=3]").unwrap();
        let block = &lower(&p).blocks[0].instructions;
        let ands: Vec<&Instruction> = block
            .iter()
            .filter(|i| matches!(i.op, Op::And { .. }))
            .collect();
        assert_eq!(ands.len(), 2);
        let Op::And { lhs, .. } = ands[1].op else {
            unreachable!()
        };
        assert_eq!(lhs, ands[0].id);
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
