//! Id lookup tables, built once per call.
//!
//! [`Block::instruction`] and [`Program::block`] are linear scans, which is the
//! right shape for a one-off lookup but quadratic inside a loop over a large
//! program. Anything that resolves ids repeatedly builds one of these first.
//!
//! Both indexes keep the *first* entry for a repeated id, which is what the
//! scans they replace return, so an unvalidated program with duplicate ids
//! resolves exactly as before.

use std::collections::HashMap;

use crate::ir::{Block, BlockId, InstrId, Instruction, Program};

/// The instructions of one block, keyed by id.
pub(crate) struct BlockIndex<'p> {
    block: &'p Block,
    by_id: HashMap<InstrId, &'p Instruction>,
}

impl<'p> BlockIndex<'p> {
    pub(crate) fn new(block: &'p Block) -> Self {
        let mut by_id = HashMap::with_capacity(block.instructions.len());
        for i in &block.instructions {
            by_id.entry(i.id).or_insert(i);
        }
        BlockIndex { block, by_id }
    }

    /// The indexed block.
    pub(crate) fn block(&self) -> &'p Block {
        self.block
    }

    /// Same answer as [`Block::instruction`], in constant time.
    pub(crate) fn instruction(&self, id: InstrId) -> Option<&'p Instruction> {
        self.by_id.get(&id).copied()
    }
}

/// Every block of a program, keyed by id, each with its own instruction index.
pub(crate) struct ProgramIndex<'p> {
    main: BlockIndex<'p>,
    blocks: HashMap<BlockId, BlockIndex<'p>>,
}

impl<'p> ProgramIndex<'p> {
    pub(crate) fn new(program: &'p Program) -> Self {
        let mut blocks = HashMap::with_capacity(program.blocks.len());
        for b in &program.blocks {
            blocks.entry(b.id).or_insert_with(|| BlockIndex::new(b));
        }
        ProgramIndex {
            main: BlockIndex::new(&program.main),
            blocks,
        }
    }

    /// The `main` block.
    pub(crate) fn main(&self) -> &BlockIndex<'p> {
        &self.main
    }

    /// Same answer as [`Program::block`]: comparison blocks only.
    pub(crate) fn block(&self, id: BlockId) -> Option<&BlockIndex<'p>> {
        self.blocks.get(&id)
    }

    /// The block with this id, whether a comparison block or `main` (`main` wins
    /// a clash).
    pub(crate) fn block_including_main(&self, id: BlockId) -> Option<&BlockIndex<'p>> {
        if self.main.block.id == id {
            Some(&self.main)
        } else {
            self.block(id)
        }
    }
}
