// Spotlight-and-trace for a pattern match. The matched observed-data and what
// they reference glow; the trail outward — sightings of them, the indicators
// sighted, what those indicate or are based on, and one further hop of
// attribution, authorship or use — is traced. Everything else dims.

import { refsOf, isContainerRef } from "./bundle-graph.js";

const FROM_INDICATOR = new Set(["indicates", "based-on"]);
const ONWARD = new Set(["attributed-to", "authored-by", "uses"]);

export function traceMatch(objects, matchedObservedIds) {
  const byId = new Map(objects.map((o) => [o.id, o]));
  const nodes = new Map();
  const edges = new Set();
  const isNode = (id) => byId.has(id) && byId.get(id).type !== "relationship";
  const mark = (id, tier) => {
    if (isNode(id) && nodes.get(id) !== "glow") nodes.set(id, tier);
  };
  const edge = (source, target, label) => {
    if (isNode(source) && isNode(target) && source !== target) edges.add(`${source}|${target}|${label}`);
  };

  // Glow: the matched observed-data, the SCOs they reference, and those SCOs'
  // own structural references (e.g. a file's content_ref artifact).
  const matched = new Set(matchedObservedIds.filter((id) => byId.get(id)?.type === "observed-data"));
  for (const odId of matched) {
    mark(odId, "glow");
    for (const sco of byId.get(odId).object_refs ?? []) {
      if (!isNode(sco)) continue;
      mark(sco, "glow");
      edge(odId, sco, "object_refs");
      for (const [property, target] of refsOf(byId.get(sco))) {
        if (!isContainerRef(byId.get(sco).type, property) && isNode(target)) {
          mark(target, "glow");
          edge(sco, target, property);
        }
      }
    }
  }

  // Trace: sightings of the matched observed-data → indicators → targets → one hop.
  const indicators = new Set();
  for (const s of objects) {
    if (s.type !== "sighting") continue;
    const hits = (s.observed_data_refs ?? []).filter((r) => matched.has(r));
    if (hits.length === 0) continue;
    mark(s.id, "trace");
    for (const r of hits) edge(s.id, r, "observed_data_refs");
    if (isNode(s.sighting_of_ref)) {
      mark(s.sighting_of_ref, "trace");
      edge(s.id, s.sighting_of_ref, "sighting_of_ref");
      indicators.add(s.sighting_of_ref);
    }
  }
  const relationships = objects.filter((o) => o.type === "relationship");
  const targets = new Set();
  for (const r of relationships) {
    if (indicators.has(r.source_ref) && FROM_INDICATOR.has(r.relationship_type) && isNode(r.target_ref)) {
      mark(r.target_ref, "trace");
      edge(r.source_ref, r.target_ref, r.relationship_type);
      targets.add(r.target_ref);
    }
  }
  for (const r of relationships) {
    if (targets.has(r.source_ref) && ONWARD.has(r.relationship_type) && isNode(r.target_ref)) {
      mark(r.target_ref, "trace");
      edge(r.source_ref, r.target_ref, r.relationship_type);
    }
  }
  return { nodes, edges };
}
