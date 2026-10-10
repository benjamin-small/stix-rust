// The bundle graph: mounts @poietic-tech/graphing-library on a canvas, loads the
// bundle graph, styles nodes by STIX type family, and draws match tiers. The
// canvas-sizing recipe follows the library's docs/BROWSER.md.

const FAMILY_COLOURS = {
  actor: "#e05a5a", capability: "#9b6bd6", target: "#4aa3df",
  detection: "#e08a3c", context: "#8a8f98", observable: "#2bb5a5",
};
const FAMILY_SHAPES = {
  actor: "diamond", capability: "square", target: "circle",
  detection: "square", context: "circle", observable: "circle",
};
const GLOW = "#f5b301";

export { FAMILY_COLOURS, FAMILY_SHAPES };

const pixelRatio = () => Math.min(window.devicePixelRatio || 1, 2);

function sizeCanvas(canvas) {
  const ratio = pixelRatio();
  const w = Math.round((canvas.clientWidth || window.innerWidth || 1280) * ratio);
  const h = Math.round((canvas.clientHeight || window.innerHeight || 720) * ratio);
  if (canvas.width === w && canvas.height === h) return false;
  canvas.width = w;
  canvas.height = h;
  return true;
}

export async function createGraphView(canvas, graph, { onNodeClick, dark }) {
  const { mountGraph } = await import("./vendor/graphing-library/dist/mount.js");
  const { attachPointer } = await import("./vendor/graphing-library/dist/pointer.js");

  sizeCanvas(canvas);
  const client = await mountGraph(canvas.id);
  client.setPixelRatio(pixelRatio());
  client.setStyle({
    node_base: { color: FAMILY_COLOURS.context, radius: 9, shape: "circle", label_visible: true },
    node_rules: Object.entries(FAMILY_COLOURS).map(([family, color]) => ({
      when: { attr: "family", equals: family },
      set: { color, shape: FAMILY_SHAPES[family] },
    })),
    // Label level-of-detail: at the fitted zoom nodes are only a few screen px, so
    // the library default gate (8 px) would hide every label.
    labels: { min_screen_radius_px: 2, max_labels: 60, edge_min_screen_length_px: 40, max_edge_labels: 40 },
    edge_base: { color: dark ? "#5d6670" : "#a3abb5", width: 1.2, label_attr: "label", label_visible: false },
  });
  client.setBackground(dark ? "#0d1117" : "#f6f8fa");
  // reduced_motion makes the layout settle synchronously inside load(), so the
  // single fitView() below frames the settled extent, not the initial one.
  client.setAnimation({ reduced_motion: true });
  client.load(JSON.stringify(graph));
  client.fitView();
  const ids = new Set((graph.nodes ?? []).map((n) => n.id));
  const pointer = attachPointer(client, canvas, {
    onNodeClick: (hit) => { if (hit.kind === "node") onNodeClick?.(hit.id); },
  });
  const apply = () => {
    if (sizeCanvas(canvas)) client.resize(canvas.width, canvas.height);
    client.setPixelRatio(pixelRatio());
  };
  const ro = new ResizeObserver(apply);
  ro.observe(canvas);
  window.addEventListener("resize", apply);
  client.start();

  let tiers = null;
  const applyTiers = () => {
    client.setNodeStyler(
      tiers
        ? (id) => {
            // The engine calls a node styler as (id, role, attrs); node ids are STIX ids.
            const t = tiers.nodes.get(id);
            if (t === "glow") return { color: GLOW, radius: 14, opacity: 1, label_visible: true };
            if (t === "trace") return { radius: 11, opacity: 1, label_visible: true };
            return { opacity: 0.2, label_visible: false };
          }
        : null,
    );
    client.setEdgeStyler(
      tiers
        ? (attrs) =>
            tiers.edges.has(attrs.key)
              ? { color: GLOW, width: 2.5, opacity: 1, label_visible: true }
              : { opacity: 0.12 }
        : null,
    );
    client.raw.invalidate?.();
  };

  return {
    /** Show match tiers; `null` or an empty trace clears highlighting. */
    setTiers(t) {
      tiers = t && t.nodes.size > 0 ? t : null;
      applyTiers();
    },
    focus(id) {
      if (!ids.has(id)) return;
      client.selectId(id);
      client.focus(id);
    },
    destroy() {
      ro.disconnect();
      window.removeEventListener("resize", apply);
      pointer.detach();
      client.stop();
      client.dispose();
    },
  };
}
