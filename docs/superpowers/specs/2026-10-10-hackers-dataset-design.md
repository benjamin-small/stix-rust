# Hackers (1995) STIX 2.1 dataset — design

**Date:** 2026-10-10
**Status:** Approved in conversation; pending written-spec review

## Goal

A default STIX 2.1 dataset for demos and general use: the 1995 film *Hackers*
modelled as threat intelligence. It is a single, valid bundle that covers the
STIX 2.1 object types that fit the story naturally, forms a multi-hop graph
rather than hub-and-spoke, and carries a wiki-quality narrative of the plot in
its `report`. It is the first of possibly several movie datasets.

This spec covers **the dataset and its validation only**. Wiring it into the
pattern playground (loading the bundle, drawing the object graph, running the
entered pattern against it) is a separate, follow-up spec that builds on this
one.

## Non-goals

- Playground or demo integration (follow-up spec).
- Exhaustive type coverage. "Lean but accurate": include an object type when the
  plot gives it a natural role; the maintainer will ask for more after review.
- Meta objects (`marking-definition`, `extension-definition`,
  `language-content`) unless one falls out naturally.
- Running the OASIS Python validator in CI (it is run once during authoring).

## Layout and ownership

```
datasets/
  hackers-1995/
    bundle.json      # the STIX 2.1 bundle
    patterns.json    # example patterns with expected match results
    README.md        # cast → object map, conventions, attribution
```

Later movies are siblings: `datasets/<slug>-<year>/` with the same three files.

`datasets/` is new and outside every area, so it is **parent-owned**; AGENTS.md
gains a row for it. The validation test lives in the umbrella crate
(`crates/stix/tests/datasets.rs`, rust-core) because it needs the parser,
model and matcher together.

## Content

### Object map (lean: types the plot supports)

| Film element | STIX object(s) |
| --- | --- |
| Dade Murphy — "Zero Cool", later "Crash Override" | `threat-actor` (roles `hacker`, sophistication `expert`; `aliases` carry both handles; 1988 Zero Cool history in `description`) |
| Kate Libby — "Acid Burn"; "Cereal Killer"; "Lord Nikon"; "Phantom Phreak"; Joey Pardella | one `threat-actor` each |
| The crew | `intrusion-set` ("The Elite"), with each crew threat-actor `attributed-to` it |
| Eugene Belford — "The Plague" | `threat-actor` (insider, primary motivation `personal-gain`) |
| Ellingson Mineral Company | `identity` (organization; `sectors` from the closest `industry-sector-ov` value(s)), `located-at` a `location` (New York City) |
| Secret Service Agent Richard Gill | `identity` (individual) |
| The Gibson supercomputer | `infrastructure` (owned by Ellingson; `compromises`/`targets` edges from the actors and malware) |
| The salami-slicing worm (the real embezzlement) | `malware` (worm) |
| The Da Vinci virus (the threat to capsize the tanker fleet) | `malware` (virus), `authored-by`/`attributed-to` The Plague |
| The Plague's scheme to frame the kids | `campaign`, `attributed-to` The Plague |
| "Hack the Planet" counter-operation | `campaign` attributed to The Elite |
| Techniques (social engineering, exfiltration, …) | `attack-pattern`s with MITRE ATT&CK `external_references` (real technique IDs) |
| Hacker tooling | `tool` (and `uses` edges) |
| The garbage file — the evidence copied off the Gibson | `file` SCO with a real SHA-256 of its fictional contents, plus an `artifact` SCO carrying those contents as `payload_bin` |
| Network indicators | `domain-name`, `ipv4-addr`, `url` SCOs |
| Secret Service raids / detections | `observed-data` (referencing the SCOs) and `sighting`s |
| Detection logic | `indicator`s whose STIX patterns match the observed-data, `indicates` the malware/campaign |
| Mitigation | `course-of-action` `mitigates` the malware |
| The case file | `report` (see Narrative) and a `grouping` of the evidence |
| Analyst commentary | a `note` and/or `opinion` where natural |
| The vulnerability exploited on the Gibson | `vulnerability` (fictional, no CVE) |

Exact counts are the implementer's call within "lean": every listed element
appears; additional SDO types are added only where the plot supports them.

### Values and conventions

- **IDs:** `<type>--<UUIDv4>`, generated once and committed. No ID is reused.
- **`spec_version`:** `"2.1"` on every SDO and SRO.
- **Timestamps:** RFC 3339 UTC with millisecond precision, set around the film's
  1995 setting (the 1988 Zero Cool incident may be referenced in text);
  `modified` ≥ `created` everywhere.
- **Network values are reserved, so they can never collide with real hosts:**
  domains under `.example` (e.g. `ellingson-mineral.example`), IPv4 from the
  RFC 5737 documentation ranges (`192.0.2.0/24`, `198.51.100.0/24`,
  `203.0.113.0/24`), URLs on those domains.
- **Hashes:** the `file` SCO's `hashes.SHA-256` is the real SHA-256 of the bytes
  in the `artifact`'s `payload_bin` (base64 of short, original, fictional
  "garbage file" contents).
- **Vocabularies:** relationship types, `threat_actor_types`, `roles`,
  `sophistication`, `primary_motivation`, `malware_types`, `infrastructure_types`,
  `identity_class`, `sectors`, `indicator_types`, `report_types` etc. use the
  STIX 2.1 open-vocabulary values.
- **SCO IDs:** may be UUIDv4 (deterministic UUIDv5 per the spec's
  ID-contributing properties is allowed but not required).

### Graph shape

Relationships chain rather than radiate. For example:
threat-actor → intrusion-set → campaign → malware → infrastructure → identity → location;
sighting → observed-data → SCOs; indicator → malware; report → grouping → evidence.
Mechanical requirement (enforced by a test): treating every SRO endpoint and
every `*_ref`/`*_refs` property as an undirected edge, (a) some pair of objects
is at least 4 hops apart, and (b) no single object touches more than one third
of all edges.

### Narrative (the report)

The `report`'s `description` is a wiki-quality Markdown write-up of the film,
told in terms of the objects:

- Sections: overview; cast (characters, handles, roles); plot in acts (Zero
  Cool's 1988 crash and ban; the Gibson intrusion and the garbage file; The
  Plague's embezzlement worm and the Da Vinci frame-up; "Hack the Planet";
  arrests and exoneration); aftermath.
- Every character, organization, malware, infrastructure, campaign, indicator
  and evidence object the text discusses is named exactly as that object's
  `name` (SCOs by their key value, e.g. the domain) and cited by ID inline, e.g.
  *Zero Cool* (`threat-actor--…`).
- `object_refs` lists every object the narrative cites.
- **Original prose only:** written fresh, not copied or closely paraphrased from
  Wikipedia or any other source. The README credits the film (title, year,
  studio, writer, director) as the subject.

### Example patterns (`patterns.json`)

A JSON array; each entry:

```json
{
  "name": "Ellingson domain resolved to the documentation range",
  "pattern": "[domain-name:value = 'ellingson-mineral.example']",
  "expect": { "matched": true, "observed_data": ["observed-data--…"] }
}
```

At least one matching pattern per SCO type present, at least one compound
pattern (`AND`/`OR` within or across observations), and at least one
non-matching pattern. No qualifiers (`WITHIN`/`REPEATS`/`START…STOP`) — the
matcher does not support them yet. Every indicator's `pattern` also appears
here with its expected result.

## Validation (CI)

`crates/stix/tests/datasets.rs` discovers every directory under `datasets/`
(at runtime via `CARGO_MANIFEST_DIR`) and, per dataset, asserts:

1. `bundle.json` parses with `stix-model` (`Bundle::from_json_str`).
2. Every `id` is unique and its prefix equals the object's `type`; the UUID part
   is well-formed.
3. Every SDO/SRO has `spec_version == "2.1"`, `created` and `modified`, and
   `modified >= created`.
4. Every reference — `source_ref`, `target_ref`, `sighting_of_ref`,
   `observed_data_refs`, `where_sighted_refs`, `object_refs`,
   `created_by_ref`, and any other `*_ref`/`*_refs` property, at any depth —
   resolves to an object in the bundle.
5. Every `relationship_type` is in the STIX 2.1 relationship vocabulary (a
   checked-in allowlist in the test).
6. Every `indicator.pattern` parses with `stix-pattern`.
7. For every `file` SCO whose `hashes.SHA-256` corresponds to an `artifact`
   (linked via the file's `content_ref`), the hash equals SHA-256 of the
   artifact's decoded `payload_bin`.
8. Every `patterns.json` entry matches exactly as declared via `match_bundle`
   (observation indices mapped to observed-data IDs in bundle order).
9. The graph-shape requirement above.
10. The report narrative: every ID cited in the `report`'s `description` exists
    in the bundle and is in its `object_refs`.

A dev-dependency for SHA-256 and base64 is added to the umbrella crate's
`[dev-dependencies]` only.

During authoring, the implementer also runs the OASIS `stix2-validator`
(Python) once against `bundle.json`, fixes every error and warning it can
(documenting any it deliberately leaves), and records the output in the PR.

## Delivery

Two stacked PRs, merged in order:

1. **Dataset (parent):** `datasets/hackers-1995/` and the AGENTS.md row, with
   the `stix2-validator` output in the PR body.
2. **Validation test (rust-core):** `crates/stix/tests/datasets.rs` and the
   dev-dependencies, stacked on (1) so CI runs it against the real dataset.

The playground integration is the next spec.
