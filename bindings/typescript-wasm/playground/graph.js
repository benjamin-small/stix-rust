// Builds a Mermaid flowchart from a lowered IR program.
//
// Node labels are the program's own listing lines (`Pattern.irListing`), so
// literal and path formatting stays in Rust instead of being re-implemented here.

/** Every instruction: comparison blocks first, then main — the listing's order. */
export function instructionsInOrder(program) {
  return [...program.blocks, program.main].flatMap((b) => b.instructions);
}

/** Whether a listing line is an instruction (as opposed to a header or blank). */
export function isInstructionLine(line) {
  return /^ +\S/.test(line);
}

// An instruction line: indent, optional "name = ", a mnemonic (possibly
// "not <op>"), then the operands. Only the alignment padding between fields is
// dropped; the operands are kept verbatim, so literals keep their spaces.
const LINE_RE = /^ +(?:(\S+) += )?(not \S+|\S+)(?: +(.*))?$/;

/** The listing's instruction lines, in order, split into their fields. */
export function parseListing(listing) {
  return listing
    .split("\n")
    .filter(isInstructionLine)
    .map((line) => {
      const [, name = null, mnemonic, operands = ""] = LINE_RE.exec(line);
      const head = name ? `${name} = ${mnemonic}` : mnemonic;
      return { name, mnemonic, operands, label: operands ? `${head} ${operands}` : head };
    });
}

const ENTITIES = {
  "#": "#35;",
  '"': "#34;",
  "<": "#60;",
  ">": "#62;",
  "&": "#38;",
  "[": "#91;",
  "]": "#93;",
  "`": "#96;",
};

/** Escape text for use inside a quoted Mermaid label. */
export function escapeLabel(s) {
  return s.replace(/[#"<>&[\]`]/g, (c) => ENTITIES[c]);
}

const OPERAND_FIELDS = ["lhs", "rhs", "input", "value"];

// The instruction ids an op reads. A compare's `rhs` is a literal operand
// object, not an id, so only numeric fields count.
function operandsOf(op) {
  return OPERAND_FIELDS.map((f) => op[f]).filter((v) => typeof v === "number");
}

const CLASS_OF = {
  load: "test",
  compare: "test",
  and: "bool",
  or: "bool",
  observe: "observe",
  followed_by: "temporal",
  within: "temporal",
  repeats: "temporal",
  start_stop: "temporal",
  yield: "term",
  ret: "term",
};

const nodeId = (id) => `i${id}`;

/** Mermaid flowchart text for `program`, labelled from its `listing`. */
export function irToMermaid(program, listing) {
  const instrs = instructionsInOrder(program);
  const lines = parseListing(listing);
  if (lines.length !== instrs.length) {
    throw new Error(
      `listing has ${lines.length} instruction lines but the program has ${instrs.length} instructions`,
    );
  }
  const labelOf = new Map(instrs.map((ins, k) => [ins.id, lines[k].label]));

  const out = ["flowchart TB"];
  for (const block of [...program.blocks, program.main]) {
    const isMain = block.kind === "main";
    const title = isMain ? "main" : `block b${block.id} (comparison)`;
    out.push(`  subgraph ${isMain ? "blk_main" : `blk_${block.id}`}["${escapeLabel(title)}"]`);
    for (const ins of block.instructions) {
      const cls = CLASS_OF[ins.op.op] ?? "test";
      out.push(`    ${nodeId(ins.id)}["${escapeLabel(labelOf.get(ins.id))}"]:::${cls}`);
    }
    out.push("  end");
  }

  // A comparison block's value is its terminator's; `observe` reads it.
  const yieldOf = new Map(
    program.blocks.filter((b) => b.instructions.length).map((b) => [b.id, b.instructions.at(-1).id]),
  );
  for (const ins of instrs) {
    for (const src of operandsOf(ins.op)) out.push(`  ${nodeId(src)} --> ${nodeId(ins.id)}`);
    if (ins.op.op === "observe" && yieldOf.has(ins.op.block)) {
      out.push(`  ${nodeId(yieldOf.get(ins.op.block))} -.-> ${nodeId(ins.id)}`);
    }
  }

  out.push(
    "  classDef test stroke:#2f7ed8,stroke-width:2px",
    "  classDef bool stroke:#8250df,stroke-width:2px",
    "  classDef observe stroke:#1a7f37,stroke-width:2px",
    "  classDef temporal stroke:#bf8700,stroke-width:2px",
    "  classDef term stroke:#6e7781,stroke-width:1px,stroke-dasharray:4",
  );
  return out.join("\n");
}
