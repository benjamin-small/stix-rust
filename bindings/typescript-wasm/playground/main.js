import init, { Engine } from "./pkg/stix_wasm.js";
import { EXAMPLES } from "./examples.js";
import { irToMermaid, instructionsInOrder, isInstructionLine } from "./graph.js";
import { spanToRange, parseErrorSpan } from "./spans.js";
import { nextTabIndex } from "./tabs.js";
import { bundleToGraph } from "./bundle-graph.js";
import { traceMatch } from "./trace.js";
import { createGraphView } from "./graph-view.js";
import { createDetails } from "./details.js";
import { Marked } from "./vendor/marked/marked.esm.js";
import { MERMAID_MAX_EDGES, MERMAID_MAX_TEXT_SIZE } from "./limits.js";

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
let bundle = null, objects = [], graphView = null, details = null, lastTiers = null;

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
    const selected = b.dataset.tab === name;
    b.setAttribute("aria-selected", String(selected));
    b.tabIndex = selected ? 0 : -1;
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

function clearMatch() {
  lastTiers = null;
  graphView?.setTiers(null);
  $("match-status").textContent = "";
}

function showGraphFallback() {
  $("bundle-graph").hidden = true;
  const fb = $("graph-fallback");
  fb.textContent = "This browser can't run the graph view (WebGL2 unavailable). The pattern tools and report still work.";
  fb.hidden = false;
}

function update() {
  const text = input.value;
  if (!text.trim()) {
    clearMatch();
    current = null;
    errorRange = null;
    errorBanner.hidden = true;
    panes.classList.remove("stale");
    clearPanes();
    showMark();
    return;
  }

  // Show a failure in the banner and dim the last good output. The underline
  // applies only when the message names a span.
  const showFailure = (e) => {
    const message = messageOf(e);
    const span = parseErrorSpan(message);
    errorRange = span ? spanToRange(text, span) : null;
    errorBanner.textContent = message.replace(/^\[\w+\]\s?/, "");
    errorBanner.hidden = false;
    panes.classList.add("stale"); // keep the last good output, dimmed
  };

  let pattern;
  try {
    pattern = engine.parsePattern(text);
  } catch (e) {
    clearMatch();
    showFailure(e);
    showMark();
    return;
  }

  try {
    // Read everything before touching the DOM so a failure leaves the panes
    // untouched (stale) rather than half-updated.
    const ir = pattern.ir;
    const listing = pattern.irListing;
    const ast = JSON.stringify(pattern.ast, null, 2);
    const irText = JSON.stringify(ir, null, 2);
    const canonical = pattern.canonical;
    errorRange = null;
    errorBanner.hidden = true;
    panes.classList.remove("stale");
    $("ast").textContent = ast;
    $("ir").textContent = irText;
    $("canonical").textContent = canonical;
    renderListing(listing);
    current = { text, instrs: instructionsInOrder(ir) };
    renderGraph(ir, listing);
    if (bundle) {
      try {
        const r = engine.matchBundle(pattern, bundle);
        const ids = Array.from(r.observedDataIds);
        r.free();
        lastTiers = traceMatch(objects, ids);
        $("match-status").textContent = ids.length
          ? `${ids.length} observation${ids.length === 1 ? "" : "s"} matched`
          : "no observations matched";
      } catch (e) {
        lastTiers = null;
        $("match-status").textContent = messageOf(e).replace(/^\[\w+\]\s?/, "");
      }
      graphView?.setTiers(lastTiers);
    }
  } catch (e) {
    showFailure(e);
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
    document.querySelector(".pattern-bar").hidden = true;
    return;
  }

  mermaid?.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    suppressErrorRendering: true,
    maxEdges: MERMAID_MAX_EDGES, // see limits.js
    maxTextSize: MERMAID_MAX_TEXT_SIZE,
    theme: matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "default",
  });

  let patterns = [];
  try {
    const [bt, pt] = await Promise.all(
      ["bundle.json", "patterns.json"].map(async (f) => {
        const res = await fetch(`datasets/hackers-1995/${f}`);
        if (!res.ok) throw new Error(`${f}: HTTP ${res.status}`);
        return res.text();
      }),
    );
    patterns = JSON.parse(pt);
    bundle = engine.parseBundle(bt);
    objects = JSON.parse(bt).objects;
  } catch (e) {
    bundle = null;
    objects = [];
    const banner = $("load-error");
    banner.textContent = `Couldn't load the Hackers dataset: ${messageOf(e)}`;
    banner.hidden = false;
  }

  if (bundle) {
    details = createDetails($("details"), objects, { Marked, onFocus: (id) => graphView?.focus(id) });
    details.showReport();
    for (const p of patterns) {
      const b = document.createElement("button");
      b.type = "button";
      b.textContent = p.name;
      b.addEventListener("click", () => { input.value = p.pattern; update(); });
      $("chips").append(b);
    }
    if (patterns.length) input.value = patterns[0].pattern;
  }
  if (!input.value) input.value = EXAMPLES[0].pattern;

  const select = $("examples");
  EXAMPLES.forEach((ex, i) => select.append(new Option(ex.label, String(i))));
  select.addEventListener("change", () => {
    input.value = EXAMPLES[Number(select.value)].pattern;
    update();
  });

  const tabButtons = [...document.querySelectorAll("[data-tab]")];
  for (const b of tabButtons) {
    b.addEventListener("click", () => selectTab(b.dataset.tab));
  }
  document.querySelector("[role=tablist]").addEventListener("keydown", (e) => {
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    const current = tabButtons.findIndex((b) => b.getAttribute("aria-selected") === "true");
    const next = nextTabIndex(current, e.key, tabButtons.length);
    if (next === null) return;
    e.preventDefault();
    selectTab(tabButtons[next].dataset.tab);
    tabButtons[next].focus();
  });
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

  update();

  if (bundle) {
    if (new URLSearchParams(location.search).has("nograph")) {
      showGraphFallback();
    } else {
      try {
        graphView = await createGraphView($("bundle-graph"), bundleToGraph(objects), {
          onNodeClick: (id) => details.show(id),
          dark: matchMedia("(prefers-color-scheme: dark)").matches,
        });
        graphView.setTiers(lastTiers);
      } catch (e) {
        showGraphFallback(e);
      }
    }
  }
}

start();
