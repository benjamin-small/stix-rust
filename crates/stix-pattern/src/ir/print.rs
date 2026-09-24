//! The human-readable IR listing.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::ast::ComparisonOperator;
use crate::ir::render::{escape_string, render_literal, render_path, render_seconds};
use crate::ir::{Block, BlockKind, InstrId, Op, Operand, Program};

/// Minimum width of the mnemonic field, so operands line up in a column.
///
/// A mnemonic at least this wide — `not issuperset` is 14 — is padded to its own
/// length plus one instead, since the operands still need a space in front of
/// them.
const MNEMONIC_WIDTH: usize = 12;

/// Width of a line's prefix, given the widest result name in the listing.
///
/// A prefix is two spaces of indent, the result name, and `" = "`, as in
/// `"  t0 = "`; a terminator names no result and gets that many spaces instead.
/// The name field is as wide as the longest name the listing uses, so the `=`
/// stays in one column once the ordinals reach two digits.
fn prefix_width(name_width: usize) -> usize {
    2 + name_width + 3
}

impl Program {
    /// Render this program as a human-readable listing.
    ///
    /// Comparison-block results are named `t0`, `t1`, … across all blocks in
    /// order; main-block results are named `o0`, `o1`, … . These names are
    /// display ordinals, not [`InstrId`](crate::ir::InstrId) values.
    pub fn to_listing(&self) -> String {
        let mut names: HashMap<InstrId, String> = HashMap::new();
        let mut t = 0usize;
        for b in &self.blocks {
            for i in &b.instructions {
                if produces_value(&i.op) {
                    names.insert(i.id, format!("t{t}"));
                    t += 1;
                }
            }
        }
        let mut o = 0usize;
        for i in &self.main.instructions {
            if produces_value(&i.op) {
                names.insert(i.id, format!("o{o}"));
                o += 1;
            }
        }

        let name_width = names.values().map(String::len).max().unwrap_or(2);

        let mut out = String::new();
        for b in &self.blocks {
            write_block(&mut out, b, &names, name_width);
            out.push('\n');
        }
        write_block(&mut out, &self.main, &names, name_width);
        out
    }
}

/// `false` for the terminators, which name no result.
fn produces_value(op: &Op) -> bool {
    !matches!(op, Op::Yield { .. } | Op::Ret { .. })
}

fn write_block(out: &mut String, b: &Block, names: &HashMap<InstrId, String>, name_width: usize) {
    match b.kind {
        BlockKind::Comparison => {
            let _ = writeln!(out, "block b{} (comparison):", b.id.0);
        }
        BlockKind::Main => {
            let _ = writeln!(out, "block main (observation):");
        }
    }
    for i in &b.instructions {
        let dest = match names.get(&i.id) {
            Some(n) => format!("  {n:<name_width$} = "),
            None => " ".repeat(prefix_width(name_width)),
        };
        let (mnemonic, operands) = describe(&i.op, names);
        let width = MNEMONIC_WIDTH.max(mnemonic.len() + 1);
        let padded = format!("{mnemonic:<width$}");
        if operands.is_empty() {
            let _ = writeln!(out, "{dest}{}", padded.trim_end());
        } else {
            let _ = writeln!(out, "{dest}{padded}{operands}");
        }
    }
}

fn name_of(id: InstrId, names: &HashMap<InstrId, String>) -> String {
    names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("%{}", id.0))
}

/// The mnemonic and operand text for one op.
fn describe(op: &Op, names: &HashMap<InstrId, String>) -> (String, String) {
    match op {
        Op::Load { path } => ("load".to_string(), render_path(path)),
        Op::Compare {
            operator,
            negated,
            lhs,
            rhs,
        } => {
            let mut m = String::new();
            if *negated {
                m.push_str("not ");
            }
            m.push_str(mnemonic_for(*operator));
            let operands = match rhs {
                Operand::Absent => name_of(*lhs, names),
                Operand::Literal(lit) => {
                    format!("{}, {}", name_of(*lhs, names), render_literal(lit))
                }
                Operand::Set(items) => {
                    let inner: Vec<String> = items.iter().map(render_literal).collect();
                    format!("{}, ({})", name_of(*lhs, names), inner.join(", "))
                }
            };
            (m, operands)
        }
        Op::And { lhs, rhs } => (
            "and".to_string(),
            format!("{}, {}", name_of(*lhs, names), name_of(*rhs, names)),
        ),
        Op::Or { lhs, rhs } => (
            "or".to_string(),
            format!("{}, {}", name_of(*lhs, names), name_of(*rhs, names)),
        ),
        Op::Yield { value } => ("yield".to_string(), name_of(*value, names)),
        Op::Observe { block } => ("observe".to_string(), format!("b{}", block.0)),
        Op::FollowedBy { lhs, rhs } => (
            "followedby".to_string(),
            format!("{}, {}", name_of(*lhs, names), name_of(*rhs, names)),
        ),
        Op::Within { input, seconds } => (
            "within".to_string(),
            format!("{}, {}", name_of(*input, names), render_seconds(*seconds)),
        ),
        Op::Repeats { input, count } => (
            "repeats".to_string(),
            format!("{}, {}", name_of(*input, names), count),
        ),
        Op::StartStop { input, start, stop } => (
            "startstop".to_string(),
            format!(
                "{}, '{}', '{}'",
                name_of(*input, names),
                escape_string(start),
                escape_string(stop)
            ),
        ),
        Op::Ret { value } => ("ret".to_string(), name_of(*value, names)),
    }
}

fn mnemonic_for(op: ComparisonOperator) -> &'static str {
    match op {
        ComparisonOperator::Equal => "eq",
        ComparisonOperator::NotEqual => "ne",
        ComparisonOperator::GreaterThan => "gt",
        ComparisonOperator::GreaterThanOrEqual => "ge",
        ComparisonOperator::LessThan => "lt",
        ComparisonOperator::LessThanOrEqual => "le",
        ComparisonOperator::In => "in",
        ComparisonOperator::Like => "like",
        ComparisonOperator::Matches => "matches",
        ComparisonOperator::IsSubset => "issubset",
        ComparisonOperator::IsSuperset => "issuperset",
        ComparisonOperator::Exists => "exists",
    }
}

#[cfg(test)]
mod tests {
    use crate::ir::lower;
    use crate::parse;

    #[test]
    fn prints_a_simple_comparison() {
        let prog = lower(&parse("[file:size > 1024]").unwrap());
        let expected = "\
block b1 (comparison):
  t0 = load        file:size
  t1 = gt          t0, 1024
       yield       t1

block main (observation):
  o0 = observe     b1
       ret         o0
";
        assert_eq!(prog.to_listing(), expected);
    }

    // NOTE: `WITHIN` qualifies only the SECOND observation, not the FOLLOWEDBY.
    // STIX 2.1 attaches qualifiers to the tightest production, so this parses as
    // FollowedBy(A, Within(B, 300)) — hence `within` precedes `followedby` here
    // and takes o1.
    #[test]
    fn prints_followedby_with_a_qualifier() {
        let src = "[ipv4-addr:value = '1.2.3.4' AND file:size > 1024] \
                   FOLLOWEDBY [domain-name:value = 'evil.example'] WITHIN 300 SECONDS";
        let prog = lower(&parse(src).unwrap());
        let expected = "\
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
";
        assert_eq!(prog.to_listing(), expected);
    }

    #[test]
    fn prints_exists_and_in() {
        let prog = lower(&parse("[EXISTS file:name]").unwrap());
        assert!(
            prog.to_listing().contains("t1 = exists      t0"),
            "{}",
            prog.to_listing()
        );

        let prog = lower(&parse("[ipv4-addr:value IN ('1.1.1.1', '8.8.8.8')]").unwrap());
        assert!(
            prog.to_listing()
                .contains("t1 = in          t0, ('1.1.1.1', '8.8.8.8')"),
            "{}",
            prog.to_listing()
        );
    }

    #[test]
    fn marks_negation() {
        let prog = lower(&parse("[file:name NOT = 'x']").unwrap());
        assert!(
            prog.to_listing().contains("t1 = not eq      t0, 'x'"),
            "{}",
            prog.to_listing()
        );
    }

    #[test]
    fn keeps_a_space_after_an_overlong_mnemonic() {
        // `not issubset` is exactly MNEMONIC_WIDTH and `not issuperset` is wider,
        // so fixed-width padding would run the mnemonic into its first operand.
        let prog = lower(&parse("[ipv4-addr:value NOT ISSUPERSET '10.0.0.0/8']").unwrap());
        let listing = prog.to_listing();
        assert!(
            listing.contains("t1 = not issuperset t0, '10.0.0.0/8'"),
            "{listing}"
        );

        let prog = lower(&parse("[ipv4-addr:value NOT ISSUBSET '10.0.0.0/8']").unwrap());
        let listing = prog.to_listing();
        assert!(
            listing.contains("t1 = not issubset t0, '10.0.0.0/8'"),
            "{listing}"
        );
    }

    #[test]
    fn columns_stay_aligned_past_the_tenth_ordinal() {
        // Six terms give twelve Load/Compare results plus five Ands = seventeen
        // comparison-tier results, so ordinals reach t16 and cross into two digits.
        let terms: Vec<String> = (0..6).map(|i| format!("file:size > {i}")).collect();
        let src = format!("[{}]", terms.join(" AND "));
        let prog = lower(&parse(&src).unwrap());
        let listing = prog.to_listing();
        assert!(listing.contains("t10 = "), "should reach t10:\n{listing}");

        // Every instruction line starts its mnemonic in the same column, whether it
        // names a result or not.
        // Block headers start at column zero; instruction lines are indented.
        let instruction_lines: Vec<&str> =
            listing.lines().filter(|l| l.starts_with("  ")).collect();
        let mnemonic_columns: Vec<usize> = instruction_lines
            .iter()
            .map(|l| match l.find(" = ") {
                // A line that names a result: the mnemonic follows `" = "`.
                Some(i) => i + 3,
                // A terminator: the mnemonic follows the blank prefix.
                None => l.len() - l.trim_start().len(),
            })
            .collect();
        assert!(
            mnemonic_columns.windows(2).all(|w| w[0] == w[1]),
            "mnemonics should share one column, got {mnemonic_columns:?}:\n{listing}"
        );

        // And the `=` signs line up with each other.
        let equals_columns: Vec<usize> = instruction_lines
            .iter()
            .filter_map(|l| l.find(" = "))
            .collect();
        assert!(
            equals_columns.windows(2).all(|w| w[0] == w[1]),
            "`=` should share one column, got {equals_columns:?}:\n{listing}"
        );
    }

    #[test]
    fn escapes_quotes_in_a_start_stop_listing() {
        let prog = lower(&parse(r"[file:name='a'] START t'2020\'x' STOP t'2021'").unwrap());
        let listing = prog.to_listing();
        assert!(
            listing.contains(r"startstop   o0, '2020\'x', '2021'"),
            "{listing}"
        );
    }
}
