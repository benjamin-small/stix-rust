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
//! [`Program::validate`] before using one that came from outside this process:
//! it is what rejects the shapes that would make [`render`] produce text that
//! does not parse, grow exponentially, or overflow the stack. See [`MAX_DEPTH`]
//! for the nesting limit it enforces.
//!
//! One gap remains, and `validate` does not close it: its single-use rule
//! constrains *value* references, not *block* references, so several `Observe`
//! instructions may target one comparison block and `render` will re-render that
//! block per use. Rendered output is therefore bounded quadratically rather than
//! linearly in the size of the program. See
//! [issue #33](https://github.com/benjamin-small/stix-rust/issues/33).

mod instr;
mod lower;
mod print;
mod render;
mod span;
mod validate;

pub use instr::{
    Block, BlockId, BlockKind, InstrId, Instruction, Op, Operand, Program, SCHEMA_VERSION,
};
pub use lower::lower;
pub use render::render;
pub use validate::{IrError, MAX_DEPTH};
