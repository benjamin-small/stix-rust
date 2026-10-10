// The details panel: the report narrative by default; an object's details when
// one is selected. Links of the form #obj:<id> focus that object.

import { renderMarkdown, escapeHtml } from "./render.js";
import { labelOf } from "./bundle-graph.js";
import { FAMILY_COLOURS, FAMILY_SHAPES } from "./graph-view.js";

const KEY_PROPS = [
  "aliases", "roles", "threat_actor_types", "sophistication", "resource_level", "primary_motivation",
  "malware_types", "is_family", "tool_types", "infrastructure_types", "identity_class", "sectors",
  "region", "country", "pattern", "pattern_type", "valid_from", "indicator_types", "value", "hashes",
  "mime_type", "size", "first_observed", "last_observed", "number_observed", "first_seen", "last_seen",
  "count", "published", "report_types", "context", "opinion", "abstract",
];

export function createDetails(panel, objects, { Marked, onFocus }) {
  const byId = new Map(objects.map((o) => [o.id, o]));
  const known = new Set(byId.keys());
  const report = objects.find((o) => o.type === "report");
  const link = (id) =>
    byId.has(id) ? `<a href="#obj:${escapeHtml(id)}">${escapeHtml(labelOf(byId.get(id)))}</a>` : escapeHtml(id);
  const md = (text) => renderMarkdown(text ?? "", Marked, known);
  const value = (v) =>
    Array.isArray(v) ? v.map((x) => escapeHtml(String(x))).join(", ")
    : v && typeof v === "object" ? Object.entries(v).map(([k, x]) => `${escapeHtml(k)}: ${escapeHtml(String(x))}`).join("<br>")
    : escapeHtml(String(v));

  const legend = () =>
    `<div class="legend">${Object.entries(FAMILY_COLOURS)
      .map(([f, c]) => `<span><i style="background:${escapeHtml(c)}" data-shape="${escapeHtml(FAMILY_SHAPES[f])}"></i>${escapeHtml(f)}</span>`)
      .join("")}</div>`;

  function connections(id) {
    const out = [];
    for (const o of objects) {
      if (o.type === "relationship") {
        if (o.source_ref === id) out.push(`<li>${escapeHtml(o.relationship_type)} → ${link(o.target_ref)}</li>`);
        if (o.target_ref === id) out.push(`<li>${link(o.source_ref)} → ${escapeHtml(o.relationship_type)}</li>`);
      }
    }
    return out.length ? `<h4>Relationships</h4><ul>${out.join("")}</ul>` : "";
  }

  function showReport() {
    panel.innerHTML = report
      ? `<h2>${escapeHtml(report.name)}</h2>${md(report.description)}${legend()}`
      : `<p>No report in this bundle.</p>${legend()}`;
  }

  function show(id) {
    const o = byId.get(id);
    if (!o) return showReport();
    if (o.type === "report") return showReport();
    const props = KEY_PROPS.filter((k) => o[k] !== undefined)
      .map((k) => `<tr><th>${escapeHtml(k)}</th><td>${value(o[k])}</td></tr>`).join("");
    const refs = Object.entries(o)
      .filter(([k, v]) => (k.endsWith("_ref") && typeof v === "string") || (k.endsWith("_refs") && Array.isArray(v)))
      .map(([k, v]) => `<tr><th>${escapeHtml(k)}</th><td>${[].concat(v).map(link).join(", ")}</td></tr>`).join("");
    panel.innerHTML = `
      <p><a href="#report" class="back">← Back to report</a></p>
      <p class="type">${escapeHtml(o.type)}</p>
      <h2>${escapeHtml(labelOf(o))}</h2>
      ${o.description ? md(o.description) : ""}
      ${props || refs ? `<table>${props}${refs}</table>` : ""}
      ${connections(id)}
      <p class="id"><code>${escapeHtml(o.id)}</code></p>`;
  }

  panel.addEventListener("click", (e) => {
    const a = e.target.closest?.("a[href]");
    if (!a) return;
    const href = a.getAttribute("href");
    if (href === "#report") { e.preventDefault(); showReport(); return; }
    if (href.startsWith("#obj:")) { e.preventDefault(); const id = href.slice(5); show(id); onFocus?.(id); }
  });

  return { showReport, show };
}
