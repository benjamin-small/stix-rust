// Turns a STIX bundle's objects into the node/edge graph the bundle view draws.
// Relationships become edges; every other *_ref / *_refs property becomes an
// edge from its holder, except container references — the same rule the
// dataset validator (crates/stix/tests/datasets.rs) uses — so a report or
// grouping does not become a hub.

const CONTAINER_TYPES = new Set(["report", "grouping", "note", "opinion"]);

const FAMILIES = {
  actor: ["threat-actor", "intrusion-set", "campaign"],
  capability: ["malware", "tool", "attack-pattern", "vulnerability", "course-of-action"],
  target: ["identity", "location", "infrastructure"],
  detection: ["indicator", "sighting", "observed-data", "malware-analysis", "incident"],
  context: ["report", "grouping", "note", "opinion"],
};

/** Every [property, id] reference in `value`, at any depth, in document order. */
export function refsOf(value) {
  const out = [];
  const walk = (v) => {
    if (Array.isArray(v)) {
      v.forEach(walk);
    } else if (v && typeof v === "object") {
      for (const [k, x] of Object.entries(v)) {
        if (k.endsWith("_ref") && typeof x === "string") out.push([k, x]);
        else if (k.endsWith("_refs") && Array.isArray(x)) {
          for (const s of x) if (typeof s === "string") out.push([k, s]);
        }
        walk(x);
      }
    }
  };
  walk(value);
  return out;
}

/** Whether a reference only lists contents rather than adding structure. */
export function isContainerRef(holderType, property) {
  return (
    property === "created_by_ref" ||
    property === "object_marking_refs" ||
    (property === "object_refs" && CONTAINER_TYPES.has(holderType))
  );
}

/**
 * Non-relationship objects that reference `id`, as `{ holder, property }`.
 * Relationships are covered by their source/target; container references are
 * skipped, as in the graph, so a report does not list every object.
 */
export function incomingRefs(objects, id) {
  const out = [];
  const seen = new Set();
  for (const o of objects) {
    if (o.type === "relationship") continue;
    for (const [property, target] of refsOf(o)) {
      const key = `${o.id}|${property}`;
      if (target === id && !isContainerRef(o.type, property) && !seen.has(key)) {
        seen.add(key);
        out.push({ holder: o, property });
      }
    }
  }
  return out;
}

/** The text a node shows. */
export function labelOf(o) {
  if (typeof o.name === "string") return o.name;
  if (typeof o.value === "string") return o.value;
  if (o.type === "observed-data" && o.first_observed) return `observed-data ${o.first_observed.slice(0, 10)}`;
  if (o.type === "sighting" && o.first_seen) return `sighting ${o.first_seen.slice(0, 10)}`;
  if (typeof o.abstract === "string") return o.abstract;
  if (o.type === "artifact" && o.mime_type) return `artifact (${o.mime_type})`;
  return o.type;
}

/** The colour/shape family for a STIX type. */
export function familyOf(type) {
  for (const [family, types] of Object.entries(FAMILIES)) if (types.includes(type)) return family;
  return "observable";
}

/** `{ nodes, edges }` in the graphing library's `load()` shape. */
export function bundleToGraph(objects) {
  const nodes = [];
  for (const o of objects) {
    if (o.type === "relationship") continue;
    nodes.push({ id: o.id, label: labelOf(o), attrs: { stix_id: o.id, stix_type: o.type, family: familyOf(o.type) } });
  }
  const nodeIds = new Set(nodes.map((n) => n.id));
  const edges = [];
  const seen = new Set();
  const addEdge = (source, target, label) => {
    if (source === target || !nodeIds.has(source) || !nodeIds.has(target)) return;
    const key = `${source}|${target}|${label}`;
    if (seen.has(key)) return;
    seen.add(key);
    edges.push({ id: key, source, target, attrs: { key, label } });
  };
  for (const o of objects) {
    if (o.type === "relationship") {
      addEdge(o.source_ref, o.target_ref, o.relationship_type);
      continue;
    }
    for (const [property, target] of refsOf(o)) {
      if (!isContainerRef(o.type, property)) addEdge(o.id, target, property);
    }
  }
  return { nodes, edges };
}
