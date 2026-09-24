//! Source extents for instructions that carry no span of their own.

use std::collections::HashSet;

use crate::error::Span;
use crate::ir::{Block, BlockId, InstrId, Op, Program, MAX_DEPTH};

impl Program {
    /// The source extent of an instruction: its own span when it has one,
    /// otherwise the union of its operands' extents.
    ///
    /// The boolean combinators carry no span of their own — their extent is
    /// exactly the span of the text their operands cover — so this computes it on
    /// demand rather than storing redundant state that structural edits would
    /// have to keep consistent.
    ///
    /// Returns `None` when neither the instruction nor anything it references has
    /// a span, and when `block` or `instr` names nothing in this program.
    ///
    /// `Observe` normally answers from its own span, which is the extent of the
    /// whole `[...]`; only when that is missing does this descend into the
    /// comparison block it names. Descent stops at [`MAX_DEPTH`] levels, matching
    /// [`Program::validate`], so this is safe to call on a program that has not
    /// been validated.
    ///
    /// # Example
    ///
    /// ```
    /// use stix_pattern::{ir, parse};
    ///
    /// let src = "[file:size > 1] AND [file:name = 'a']";
    /// let program = ir::lower(&parse(src).unwrap());
    /// // The `and` in `main` has no span of its own.
    /// let and = program.main.instructions[2].id;
    /// let span = program.span_of(program.main.id, and).unwrap();
    /// assert_eq!(&src[span.start..span.end], src);
    /// ```
    pub fn span_of(&self, block: BlockId, instr: InstrId) -> Option<Span> {
        let mut extent: Option<Span> = None;
        // A validated program is acyclic, but this is meant to be safe on one
        // that is not, so never visit the same instruction twice.
        let mut visited: HashSet<(BlockId, InstrId)> = HashSet::new();
        let mut stack: Vec<(BlockId, InstrId, u32)> = vec![(block, instr, 0)];

        while let Some((block_id, instr_id, depth)) = stack.pop() {
            if !visited.insert((block_id, instr_id)) {
                continue;
            }
            let Some(b) = self.block_including_main(block_id) else {
                continue;
            };
            let Some(i) = b.instruction(instr_id) else {
                continue;
            };
            if let Some(span) = i.span {
                extent = Some(union(extent, span));
                continue;
            }
            if depth >= MAX_DEPTH {
                continue;
            }
            let next = depth + 1;
            match &i.op {
                Op::Load { .. } => {}
                Op::Compare { lhs, .. } => stack.push((block_id, *lhs, next)),
                Op::And { lhs, rhs } | Op::Or { lhs, rhs } | Op::FollowedBy { lhs, rhs } => {
                    stack.push((block_id, *lhs, next));
                    stack.push((block_id, *rhs, next));
                }
                Op::Within { input, .. }
                | Op::Repeats { input, .. }
                | Op::StartStop { input, .. } => stack.push((block_id, *input, next)),
                Op::Yield { value } | Op::Ret { value } => stack.push((block_id, *value, next)),
                Op::Observe { block: target } => {
                    if let Some(cb) = self.block(*target) {
                        if let Some(Op::Yield { value }) = cb.terminator().map(|t| &t.op) {
                            stack.push((*target, *value, next));
                        }
                    }
                }
            }
        }
        extent
    }

    /// The block with this id, whether it is a comparison block or `main`.
    fn block_including_main(&self, id: BlockId) -> Option<&Block> {
        if self.main.id == id {
            Some(&self.main)
        } else {
            self.block(id)
        }
    }
}

/// The smallest span covering both inputs.
fn union(acc: Option<Span>, span: Span) -> Span {
    match acc {
        None => span,
        Some(a) => Span {
            start: a.start.min(span.start),
            end: a.end.max(span.end),
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::ir::{lower, BlockId, InstrId, Op};
    use crate::parse;

    /// The source text an instruction's extent covers.
    fn text_of(src: &str, block: BlockId, instr: InstrId) -> String {
        let program = lower(&parse(src).unwrap());
        let span = program
            .span_of(block, instr)
            .expect("instruction should have an extent");
        src[span.start..span.end].to_string()
    }

    #[test]
    fn a_combinator_extent_covers_both_operands() {
        let src = "[file:size > 1] AND [file:name = 'a']";
        let program = lower(&parse(src).unwrap());
        let and = program
            .main
            .instructions
            .iter()
            .find(|i| matches!(i.op, Op::And { .. }))
            .expect("an and");
        assert!(and.span.is_none(), "and should carry no span of its own");
        assert_eq!(text_of(src, program.main.id, and.id), src);
    }

    #[test]
    fn a_comparison_tier_combinator_extent_covers_both_operands() {
        let src = "[file:size > 1 AND file:name = 'a']";
        let program = lower(&parse(src).unwrap());
        let block = &program.blocks[0];
        let and = block
            .instructions
            .iter()
            .find(|i| matches!(i.op, Op::And { .. }))
            .expect("an and");
        assert_eq!(text_of(src, block.id, and.id), "file:size > 1 AND file:name = 'a'");
    }

    #[test]
    fn an_instruction_with_its_own_span_returns_it_unchanged() {
        let src = "[file:size > 1] AND [file:name = 'a']";
        let program = lower(&parse(src).unwrap());
        let observe = &program.main.instructions[0];
        assert_eq!(program.span_of(program.main.id, observe.id), observe.span);
        assert_eq!(text_of(src, program.main.id, observe.id), "[file:size > 1]");
    }

    #[test]
    fn a_terminator_extent_covers_the_whole_program() {
        let src = "[file:size > 1] OR [file:name = 'a']";
        let program = lower(&parse(src).unwrap());
        let ret = program.main.terminator().expect("a ret");
        assert!(ret.span.is_none());
        assert_eq!(text_of(src, program.main.id, ret.id), src);
    }

    #[test]
    fn nested_combinators_union_transitively() {
        let src = "[file:size > 1] OR [file:name = 'a'] OR [file:name = 'b']";
        let program = lower(&parse(src).unwrap());
        // The outer OR is the last combinator in `main`.
        let outer = program
            .main
            .instructions
            .iter()
            .rfind(|i| matches!(i.op, Op::Or { .. }))
            .expect("two ors");
        assert_eq!(text_of(src, program.main.id, outer.id), src);
    }

    #[test]
    fn an_unknown_block_or_instruction_has_no_extent() {
        let program = lower(&parse("[file:size > 1]").unwrap());
        assert_eq!(program.span_of(BlockId(999), InstrId(0)), None);
        assert_eq!(program.span_of(program.main.id, InstrId(999)), None);
    }

    #[test]
    fn a_deep_chain_terminates_rather_than_overflowing() {
        use crate::ir::MAX_DEPTH;

        // A chain far deeper than MAX_DEPTH, with no spans anywhere: descent must
        // stop at the bound and return `None` rather than recursing.
        let mut program = lower(&parse("[file:size > 1]").unwrap());
        for instr in &mut program.blocks[0].instructions {
            instr.span = None;
        }
        for instr in &mut program.main.instructions {
            instr.span = None;
        }
        let mut acc = program.main.instructions[0].id;
        for next in 100..100 + MAX_DEPTH * 4 {
            let id = InstrId(next);
            program.main.instructions.push(crate::ir::Instruction {
                id,
                op: Op::Within {
                    input: acc,
                    seconds: 1.0,
                },
                span: None,
            });
            acc = id;
        }
        assert_eq!(program.span_of(program.main.id, acc), None);
    }
}
