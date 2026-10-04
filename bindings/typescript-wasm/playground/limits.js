// Mermaid defaults (maxEdges 500, maxTextSize 50,000) reject the depth-40
// worst-shape pattern (measured 604 edges, 38,095 chars; see
// tests/playground-graph-limits.test.ts). These leave ~3x headroom over that
// but stay capped: a 2,000-term flat OR (~10,000 edges, ~675,000 chars) would
// likely hang the tab, so it falls back to "Graph rendering failed".
export const MERMAID_MAX_EDGES = 2000;
export const MERMAID_MAX_TEXT_SIZE = 200000;
