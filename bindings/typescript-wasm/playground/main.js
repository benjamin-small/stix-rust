import init, { Engine } from "./pkg/stix_wasm.js";
import { EXAMPLES } from "./examples.js";
import { irToMermaid, instructionsInOrder, isInstructionLine } from "./graph.js";
import { spanToRange, parseErrorSpan } from "./spans.js";

const $ = (id) => document.getElementById(id);
const input = $("pattern");
const backdrop = $("backdrop");
const errorBanner = $("parse-error");
const panes = $("panes");
const mermaid = globalThis.mermaid; // from mermaid.min.js; absent if it failed to load

const TABS = ["graph", "listing", "ast", "ir", "canonical"];
const TAB_KEY = "stix-playground-tab";
const DEBOUNCE_MS = 150;

let engine;
let current = null; // { text, instrs } from the last successful parse
let errorRange = null; // UTF-16 range of the current parse error
let graphSeq = 0;

const messageOf = (e) => (e instanceof Error ? e.message : String(e));

// Redraw the mirror behind the textarea, with at most one marked range.
function renderBackdrop(range, cls) {
  const text = input.value;
  backdrop.replaceChildren();
  if (!range) {
    backdrop.append(text + "\n");
  } else {
    const mark = document.createElement("mark");
    const empty = range.start === range.end;
    mark.className = empty ? `${cls} caret` : cls;
    mark.textContent = text.slice(range.start, range.end);
    backdrop.append(text.slice(0, range.start), mark, text.slice(range.end) + "\n");
  }
  backdrop.scrollTop = input.scrollTop;
}

// The resting state of the mirror: the parse error, if there is one.
function showMark() {
  renderBackdrop(errorRange, "err");
}

function highlightInstruction(ins) {
  if (!current || current.text !== input.value || !ins?.span) return showMark();
  renderBackdrop(spanToRange(current.text, ins.span), "hl");
}

// Mermaid gives each node element an id like "flowchart-i3-12"; "i3" is the
// node id graph.js assigned to instruction 3.
function nodeInstrId(el) {
  const m = el && /flowchart-i(\d+)-/.exec(el.id);
  return m ? Number(m[1]) : null;
}

function selectTab(name) {
  for (const b of document.querySelectorAll("[data-tab]")) {
    b.setAttribute("aria-selected", String(b.dataset.tab === name));
  }
  for (const p of document.querySelectorAll("[data-pane]")) p.hidden = p.dataset.pane !== name;
  try { localStorage.setItem(TAB_KEY, name); } catch { /* storage unavailable */ }
}

function clearPanes() {
  for (const p of document.querySelectorAll("[data-pane]")) p.replaceChildren();
}

function renderListing(listing) {
  const pre = $("listing");
  pre.replaceChildren();
  let k = 0;
  for (const line of listing.split("\n")) {
    const span = document.createElement("span");
    span.textContent = line + "\n";
    if (isInstructionLine(line)) {
      span.className = "instr";
      span.dataset.k = String(k++);
    }
    pre.append(span);
  }
}

async function renderGraph(ir, listing) {
  const seq = ++graphSeq;
  const el = $("graph");
  if (!mermaid) {
    el.textContent = "Mermaid failed to load, so the graph is unavailable.";
    return;
  }
  try {
    const source = irToMermaid(ir, listing);
    await mermaid.parse(source);
    const { svg } = await mermaid.render(`graph-${seq}`, source);
    if (seq === graphSeq) el.innerHTML = svg;
  } catch (e) {
    if (seq === graphSeq) el.textContent = `Graph rendering failed: ${messageOf(e)}`;
  }
}

function update() {
  const text = input.value;
  if (!text.trim()) {
    current = null;
    errorRange = null;
    errorBanner.hidden = true;
    panes.classList.remove("stale");
    clearPanes();
    showMark();
    return;
  }

  let pattern;
  try {
    pattern = engine.parsePattern(text);
  } catch (e) {
    const message = messageOf(e);
    const span = parseErrorSpan(message);
    errorRange = span ? spanToRange(text, span) : null;
    errorBanner.textContent = message.replace(/^\[\w+\]\s?/, "");
    errorBanner.hidden = false;
    panes.classList.add("stale"); // keep the last good output, dimmed
    showMark();
    return;
  }

  errorRange = null;
  errorBanner.hidden = true;
  panes.classList.remove("stale");
  try {
    const ir = pattern.ir;
    const listing = pattern.irListing;
    $("ast").textContent = JSON.stringify(pattern.ast, null, 2);
    $("ir").textContent = JSON.stringify(ir, null, 2);
    $("canonical").textContent = pattern.canonical;
    renderListing(listing);
    current = { text, instrs: instructionsInOrder(ir) };
    renderGraph(ir, listing);
  } finally {
    pattern.free();
  }
  showMark();
}

async function start() {
  try {
    await init();
    engine = new Engine();
  } catch (e) {
    const banner = $("load-error");
    banner.textContent = `Couldn't load the WebAssembly module: ${messageOf(e)}`;
    banner.hidden = false;
    document.querySelector("main").hidden = true;
    return;
  }

  mermaid?.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    suppressErrorRendering: true,
    theme: matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "default",
  });

  const select = $("examples");
  EXAMPLES.forEach((ex, i) => select.append(new Option(ex.label, String(i))));
  select.addEventListener("change", () => {
    input.value = EXAMPLES[Number(select.value)].pattern;
    update();
  });

  for (const b of document.querySelectorAll("[data-tab]")) {
    b.addEventListener("click", () => selectTab(b.dataset.tab));
  }
  let saved = null;
  try { saved = localStorage.getItem(TAB_KEY); } catch { /* storage unavailable */ }
  selectTab(TABS.includes(saved) ? saved : "graph");

  let timer;
  input.addEventListener("input", () => {
    errorRange = null;
    showMark();
    clearTimeout(timer);
    timer = setTimeout(update, DEBOUNCE_MS);
  });
  input.addEventListener("scroll", () => { backdrop.scrollTop = input.scrollTop; });

  $("listing").addEventListener("mouseover", (e) => {
    const k = e.target.closest?.("[data-k]")?.dataset.k;
    highlightInstruction(k === undefined ? null : current?.instrs[Number(k)]);
  });
  $("listing").addEventListener("mouseleave", showMark);
  $("graph").addEventListener("mouseover", (e) => {
    const id = nodeInstrId(e.target.closest?.("g.node"));
    highlightInstruction(id === null ? null : current?.instrs.find((i) => i.id === id));
  });
  $("graph").addEventListener("mouseleave", showMark);

  input.value = EXAMPLES[0].pattern;
  update();
}

start();
