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
