# Hackers (1995) STIX Dataset Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `datasets/hackers-1995/` — a valid, multi-hop STIX 2.1 bundle of the film *Hackers* with a wiki-quality narrative report — plus a CI test that validates every dataset in `datasets/`.

**Architecture:** A hand-authored JSON bundle (data, not code) under a new parent-owned `datasets/` directory, and one Rust integration test in the umbrella crate that parses each dataset with `stix-model`, checks STIX structural rules, re-runs the example patterns through the real matcher, and enforces the graph shape. The test lands first, proven against an in-test synthetic dataset; the dataset PR then gets validated by it.

**Tech Stack:** Rust (`stix` umbrella crate: `stix::model`, `stix::parse`, `stix::match_bundle`), `serde_json`, `regex`, `sha2`, `base64`; Python `stix2-validator` (authoring only).

**Spec:** `docs/superpowers/specs/2026-10-10-hackers-dataset-design.md`

## Global Constraints

- IDs are `<type>--<UUIDv4>` in lowercase hex; no ID is reused.
- Every SDO and SRO has `spec_version: "2.1"`, `created`, `modified`, with `modified >= created`.
- Timestamps are RFC 3339 UTC with millisecond precision: `YYYY-MM-DDTHH:MM:SS.mmmZ`.
- `bundle.json` is one STIX 2.1 `bundle` (`"type": "bundle"`, `bundle--<UUIDv4>` id, every object in `objects`) and carries no `spec_version`.
- Domains use the `.example` TLD; IPv4 addresses come from RFC 5737 (`192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`); URLs use those domains.
- The `file` SCO's `hashes.SHA-256` is the real SHA-256 of its `content_ref` artifact's decoded `payload_bin`.
- Relationship types and other enumerated properties use STIX 2.1 vocabulary values.
- Example patterns use no qualifiers (`WITHIN`, `REPEATS`, `START…STOP`).
- Narrative prose is original — not copied or closely paraphrased from Wikipedia or any other source.
- AGENTS.md boundaries: Task 1 edits only `crates/**` (rust-core); Tasks 2–4 edit only `datasets/**` and `AGENTS.md` (parent).
- Commits carry no attribution trailers.

## Review Focus

1. `datasets/` exists but holds no dataset directory (e.g. only a README) — the repo test must fail, not silently validate nothing. Pinned in Task 1 (`every_dataset_in_the_repo_is_valid`).
2. A dataset directory missing `patterns.json` — the failure must name the directory. Pinned in Task 1 (the repo test's `read_to_string` panics with the path).
3. A dangling reference nested inside an extension (e.g. `extensions.archive-ext.contains_refs`) — must be caught like a top-level one. Pinned in Task 1 (`finds_dangling_references_nested_in_extensions`).
4. Non-ASCII text (accents, emoji) in names and the narrative — must validate and must not confuse ID citation scanning. Pinned in Task 1 (`non_ascii_narrative_is_fine`).
5. The report's `object_refs` making it a graph hub — container refs must be excluded from the shape check. Pinned in Task 1 (the synthetic report cites objects yet the dataset passes).

---

### Task 1: Dataset validation test (rust-core, PR 1)

**Files:**
- Modify: `crates/stix/Cargo.toml` (`[dev-dependencies]`)
- Create: `crates/stix/tests/datasets.rs`

**Interfaces:**
- Consumes: `stix::model::Bundle::from_json_str(&str) -> Result<Bundle, _>`, `stix::parse(&str) -> Result<Pattern, ParseError>`, `stix::match_bundle(&Pattern, &Bundle) -> Result<MatchResult, MatchError>`, `MatchResult::{is_match() -> bool, observations() -> &[usize]}`. Observation indices count the bundle's `observed-data` objects in bundle order (`crates/stix-matcher/src/lib.rs` `match_bundle`).
- Produces: `fn validate(bundle_text: &str, patterns_text: &str) -> Vec<String>` (empty = valid) and the test `every_dataset_in_the_repo_is_valid`, which validates every directory under `<repo>/datasets/` (each must contain `bundle.json` and `patterns.json`). `patterns.json` format: JSON array of `{"name": str, "pattern": str, "expect": {"matched": bool, "observed_data": [observed-data id, …]}}`.

Branch: `git fetch origin && git checkout -b test/dataset-validation origin/main`.

- [ ] **Step 1: Add dev-dependencies**

In `crates/stix/Cargo.toml`, extend `[dev-dependencies]` to:

```toml
[dev-dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
regex = { workspace = true }
sha2 = "0.10"
base64 = "0.22"
```

(`regex` is already a `[workspace.dependencies]` entry in the root `Cargo.toml`; confirm with `grep -n regex Cargo.toml`.)

- [ ] **Step 2: Write the test file with the validator stubbed**

Create `crates/stix/tests/datasets.rs` with the fixtures and tests below and a `validate` that returns no errors (`fn validate(_: &str, _: &str) -> Vec<String> { Vec::new() }`), so the negative tests fail first.

```rust
//! Validates every dataset under the repository's `datasets/` directory, and
//! tests the validator itself against a small synthetic dataset.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------- the repo

fn repo_datasets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../datasets")
}

#[test]
fn every_dataset_in_the_repo_is_valid() {
    let dir = repo_datasets_dir();
    if !dir.exists() {
        eprintln!("no datasets/ directory; nothing to validate");
        return;
    }
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).expect("read datasets/") {
        let path = entry.expect("dir entry").path();
        if !path.is_dir() {
            continue;
        }
        count += 1;
        let read = |f: &str| {
            std::fs::read_to_string(path.join(f))
                .unwrap_or_else(|e| panic!("{}: cannot read {f}: {e}", path.display()))
        };
        let errs = validate(&read("bundle.json"), &read("patterns.json"));
        assert!(
            errs.is_empty(),
            "{} is invalid:\n  {}",
            path.display(),
            errs.join("\n  ")
        );
        eprintln!("validated {}", path.display());
    }
    assert!(count > 0, "datasets/ exists but contains no dataset directories");
}

// ------------------------------------------------------- synthetic dataset

const TA: &str = "threat-actor--00000000-0000-4000-8000-000000000001";
const MW: &str = "malware--00000000-0000-4000-8000-000000000002";
const IN: &str = "infrastructure--00000000-0000-4000-8000-000000000003";
const ID: &str = "identity--00000000-0000-4000-8000-000000000004";
const LO: &str = "location--00000000-0000-4000-8000-000000000005";
const DN: &str = "domain-name--00000000-0000-4000-8000-000000000006";
const OD: &str = "observed-data--00000000-0000-4000-8000-000000000007";
const FI: &str = "file--00000000-0000-4000-8000-000000000008";
const AR: &str = "artifact--00000000-0000-4000-8000-000000000009";
const RP: &str = "report--00000000-0000-4000-8000-00000000000a";
const T0: &str = "1995-09-15T00:00:00.000Z";
/// SHA-256 of the bytes `hello` (the artifact payload `aGVsbG8=`).
const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

fn rel_id(n: u32) -> String {
    format!("relationship--00000000-0000-4000-8000-0000000001{n:02}")
}

fn sdo(ty: &str, id: &str, extra: Value) -> Value {
    let mut o = json!({"type": ty, "spec_version": "2.1", "id": id, "created": T0, "modified": T0});
    o.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    o
}

fn rel(n: u32, relationship_type: &str, source: &str, target: &str) -> Value {
    sdo(
        "relationship",
        &rel_id(n),
        json!({"relationship_type": relationship_type, "source_ref": source, "target_ref": target}),
    )
}

/// A valid dataset whose graph is the chain location–identity–infrastructure–
/// malware–threat-actor (4 hops), with a branch to the observed domain.
fn good_bundle() -> Value {
    json!({
        "type": "bundle",
        "id": "bundle--00000000-0000-4000-8000-0000000000ff",
        "objects": [
            sdo("threat-actor", TA, json!({"name": "Zero Cool", "threat_actor_types": ["hacker"]})),
            sdo("malware", MW, json!({"name": "Da Vinci", "is_family": false})),
            sdo("infrastructure", IN, json!({"name": "Gibson"})),
            sdo("identity", ID, json!({"name": "Ellingson", "identity_class": "organization"})),
            sdo("location", LO, json!({"name": "New York", "country": "US"})),
            {"type": "domain-name", "spec_version": "2.1", "id": DN, "value": "a.example"},
            sdo("observed-data", OD, json!({
                "first_observed": T0, "last_observed": T0, "number_observed": 1, "object_refs": [DN]
            })),
            {"type": "artifact", "spec_version": "2.1", "id": AR, "payload_bin": "aGVsbG8="},
            {"type": "file", "spec_version": "2.1", "id": FI, "name": "garbage",
             "hashes": {"SHA-256": HELLO_SHA256}, "content_ref": AR},
            sdo("report", RP, json!({
                "name": "Case file", "published": T0,
                "description": format!("*Zero Cool* (`{TA}`) used *Da Vinci* (`{MW}`)."),
                "object_refs": [TA, MW]
            })),
            rel(1, "located-at", ID, LO),
            rel(2, "owns", ID, IN),
            rel(3, "targets", MW, IN),
            rel(4, "uses", TA, MW),
            rel(5, "consists-of", IN, DN),
        ]
    })
}

fn good_patterns() -> Value {
    json!([
        {"name": "hit", "pattern": "[domain-name:value = 'a.example']",
         "expect": {"matched": true, "observed_data": [OD]}},
        {"name": "miss", "pattern": "[domain-name:value = 'b.example']",
         "expect": {"matched": false, "observed_data": []}}
    ])
}

fn errors(bundle: &Value, patterns: &Value) -> Vec<String> {
    validate(&bundle.to_string(), &patterns.to_string())
}

fn objects_mut(bundle: &mut Value) -> &mut Vec<Value> {
    bundle["objects"].as_array_mut().unwrap()
}

fn find_mut<'a>(bundle: &'a mut Value, id: &str) -> &'a mut Value {
    objects_mut(bundle).iter_mut().find(|o| o["id"] == id).unwrap()
}

#[track_caller]
fn assert_flags(bundle: &Value, patterns: &Value, needle: &str) {
    let e = errors(bundle, patterns);
    assert!(
        e.iter().any(|x| x.contains(needle)),
        "expected an error containing {needle:?}, got {e:#?}"
    );
}

// -------------------------------------------------------- validator tests

#[test]
fn the_synthetic_dataset_is_valid() {
    let e = errors(&good_bundle(), &good_patterns());
    assert!(e.is_empty(), "{e:#?}");
}

#[test]
fn non_ascii_narrative_is_fine() {
    let mut b = good_bundle();
    find_mut(&mut b, RP)["description"] = json!(format!("Café 😀 — *Zero Cool* (`{TA}`)."));
    let e = errors(&b, &good_patterns());
    assert!(e.is_empty(), "{e:#?}");
}

#[test]
fn rejects_a_non_bundle() {
    let mut b = good_bundle();
    b["type"] = json!("grouping");
    assert_flags(&b, &good_patterns(), "type \"bundle\"");
}

#[test]
fn rejects_spec_version_on_the_bundle() {
    let mut b = good_bundle();
    b["spec_version"] = json!("2.1");
    assert_flags(&b, &good_patterns(), "must not carry spec_version");
}

#[test]
fn rejects_duplicate_ids() {
    let mut b = good_bundle();
    let dup = find_mut(&mut b, MW).clone();
    objects_mut(&mut b).push(dup);
    assert_flags(&b, &good_patterns(), "duplicate id");
}

#[test]
fn rejects_an_id_whose_prefix_is_not_its_type() {
    let mut b = good_bundle();
    find_mut(&mut b, MW)["type"] = json!("tool");
    assert_flags(&b, &good_patterns(), "id prefix");
}

#[test]
fn rejects_a_malformed_uuid() {
    let mut b = good_bundle();
    find_mut(&mut b, LO)["id"] = json!("location--not-a-uuid");
    assert_flags(&b, &good_patterns(), "malformed id");
}

#[test]
fn rejects_an_unknown_type() {
    let mut b = good_bundle();
    objects_mut(&mut b).push(json!({"type": "movie", "id": "movie--00000000-0000-4000-8000-0000000000aa"}));
    assert_flags(&b, &good_patterns(), "unknown type");
}

#[test]
fn rejects_missing_spec_version() {
    let mut b = good_bundle();
    find_mut(&mut b, TA).as_object_mut().unwrap().remove("spec_version");
    assert_flags(&b, &good_patterns(), "spec_version must be");
}

#[test]
fn rejects_modified_before_created() {
    let mut b = good_bundle();
    find_mut(&mut b, TA)["modified"] = json!("1995-09-14T00:00:00.000Z");
    assert_flags(&b, &good_patterns(), "modified is before created");
}

#[test]
fn rejects_timestamps_without_milliseconds() {
    let mut b = good_bundle();
    find_mut(&mut b, TA)["created"] = json!("1995-09-15T00:00:00Z");
    assert_flags(&b, &good_patterns(), "timestamp format");
}

#[test]
fn rejects_a_dangling_reference() {
    let mut b = good_bundle();
    find_mut(&mut b, OD)["object_refs"] = json!(["domain-name--00000000-0000-4000-8000-0000000000ee"]);
    assert_flags(&b, &good_patterns(), "unresolved reference object_refs");
}

#[test]
fn finds_dangling_references_nested_in_extensions() {
    let mut b = good_bundle();
    find_mut(&mut b, FI)["extensions"] = json!({
        "archive-ext": {"contains_refs": ["file--00000000-0000-4000-8000-0000000000ee"]}
    });
    assert_flags(&b, &good_patterns(), "unresolved reference contains_refs");
}

#[test]
fn rejects_a_relationship_type_outside_the_vocabulary() {
    let mut b = good_bundle();
    find_mut(&mut b, &rel_id(4))["relationship_type"] = json!("hangs-out-with");
    assert_flags(&b, &good_patterns(), "relationship_type");
}

#[test]
fn rejects_an_indicator_whose_pattern_does_not_parse() {
    let mut b = good_bundle();
    objects_mut(&mut b).push(sdo(
        "indicator",
        "indicator--00000000-0000-4000-8000-00000000000b",
        json!({"pattern": "[file:size >", "pattern_type": "stix", "valid_from": T0}),
    ));
    assert_flags(&b, &good_patterns(), "indicator pattern does not parse");
}

#[test]
fn rejects_a_file_hash_that_does_not_match_its_artifact() {
    let mut b = good_bundle();
    find_mut(&mut b, FI)["hashes"]["SHA-256"] = json!("00".repeat(32));
    assert_flags(&b, &good_patterns(), "does not match its artifact");
}

#[test]
fn rejects_a_pattern_whose_expectation_is_wrong() {
    let mut p = good_patterns();
    p[1]["expect"]["matched"] = json!(true);
    assert_flags(&good_bundle(), &p, "pattern \"miss\"");
}

#[test]
fn rejects_an_empty_pattern_list() {
    assert_flags(&good_bundle(), &json!([]), "no example patterns");
}

#[test]
fn rejects_a_hub_and_spoke_graph() {
    let mut b = good_bundle();
    for k in 0..4u32 {
        let dn = format!("domain-name--00000000-0000-4000-8000-0000000002{k:02}");
        objects_mut(&mut b).push(json!({"type": "domain-name", "spec_version": "2.1", "id": dn,
                                        "value": format!("hub{k}.example")}));
        objects_mut(&mut b).push(rel(10 + k, "consists-of", IN, &dn));
    }
    assert_flags(&b, &good_patterns(), "hub-and-spoke");
}

#[test]
fn rejects_a_shallow_graph() {
    let mut b = good_bundle();
    let (r1, r4) = (rel_id(1), rel_id(4));
    objects_mut(&mut b).retain(|o| o["id"] != r1.as_str() && o["id"] != r4.as_str());
    assert_flags(&b, &good_patterns(), "diameter");
}

#[test]
fn rejects_a_report_citing_an_unknown_object() {
    let mut b = good_bundle();
    find_mut(&mut b, RP)["description"] = json!("see `malware--00000000-0000-4000-8000-0000000000ee`");
    assert_flags(&b, &good_patterns(), "report cites unknown object");
}

#[test]
fn rejects_a_report_citing_an_object_missing_from_object_refs() {
    let mut b = good_bundle();
    find_mut(&mut b, RP)["object_refs"] = json!([TA]);
    assert_flags(&b, &good_patterns(), "not in object_refs");
}
```

- [ ] **Step 3: Run the tests to verify the negative tests fail**

Run: `cargo test -p stix-rust --test datasets`
Expected: `the_synthetic_dataset_is_valid`, `non_ascii_narrative_is_fine` and `every_dataset_in_the_repo_is_valid` pass; every `rejects_*` / `finds_*` test FAILS with "expected an error containing …".

- [ ] **Step 4: Implement `validate`**

Replace the stub with the real validator (append below the tests or above them; keep everything in this one file):

```rust
const SDO_TYPES: &[&str] = &[
    "attack-pattern", "campaign", "course-of-action", "grouping", "identity", "incident",
    "indicator", "infrastructure", "intrusion-set", "location", "malware", "malware-analysis",
    "note", "observed-data", "opinion", "report", "threat-actor", "tool", "vulnerability",
];
const SRO_TYPES: &[&str] = &["relationship", "sighting"];
const SCO_TYPES: &[&str] = &[
    "artifact", "autonomous-system", "directory", "domain-name", "email-addr", "email-message",
    "file", "ipv4-addr", "ipv6-addr", "mac-addr", "mutex", "network-traffic", "process",
    "software", "url", "user-account", "windows-registry-key", "x509-certificate",
];
const META_TYPES: &[&str] = &["extension-definition", "language-content", "marking-definition"];
/// STIX 2.1 relationship types (common: derived-from, duplicate-of, related-to;
/// plus every SDO-specific type defined in the spec).
const RELATIONSHIP_TYPES: &[&str] = &[
    "analysis-of", "attributed-to", "authored-by", "av-analysis-of", "based-on", "beacons-to",
    "characterizes", "communicates-with", "compromises", "consists-of", "controls", "delivers",
    "derived-from", "downloads", "drops", "duplicate-of", "dynamic-analysis-of", "exfiltrates-to",
    "exploits", "has", "hosts", "impersonates", "indicates", "investigates", "located-at",
    "mitigates", "originates-from", "owns", "related-to", "remediates", "static-analysis-of",
    "targets", "uses", "variant-of",
];
/// Types whose `object_refs` list contents rather than structure.
const CONTAINER_TYPES: &[&str] = &["report", "grouping", "note", "opinion"];

fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12].iter().zip(&parts).all(|(n, p)| {
            p.len() == *n && p.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        })
}

fn is_ms_timestamp(s: &str) -> bool {
    Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$").unwrap().is_match(s)
}

/// Every (`property`, referenced id) pair in `v`, at any depth.
fn refs(v: &Value) -> Vec<(String, String)> {
    fn walk(v: &Value, out: &mut Vec<(String, String)>) {
        match v {
            Value::Object(m) => {
                for (k, val) in m {
                    if k.ends_with("_ref") {
                        if let Some(s) = val.as_str() {
                            out.push((k.clone(), s.to_string()));
                        }
                    } else if k.ends_with("_refs") {
                        for s in val.as_array().into_iter().flatten().filter_map(Value::as_str) {
                            out.push((k.clone(), s.to_string()));
                        }
                    }
                    walk(val, out);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(v, &mut out);
    out
}

/// Validate one dataset; an empty result means it is valid.
fn validate(bundle_text: &str, patterns_text: &str) -> Vec<String> {
    let mut errs: Vec<String> = Vec::new();
    let bundle: Value = match serde_json::from_str(bundle_text) {
        Ok(v) => v,
        Err(e) => return vec![format!("bundle.json is not valid JSON: {e}")],
    };

    // 1. The bundle itself.
    if bundle.get("type").and_then(Value::as_str) != Some("bundle") {
        errs.push("top-level object must have type \"bundle\"".into());
    }
    let bundle_id_ok = bundle
        .get("id")
        .and_then(Value::as_str)
        .and_then(|i| i.strip_prefix("bundle--"))
        .is_some_and(is_uuid);
    if !bundle_id_ok {
        errs.push("bundle id must be bundle--<uuid> (malformed id)".into());
    }
    if bundle.get("spec_version").is_some() {
        errs.push("bundle must not carry spec_version".into());
    }
    let parsed = match stix::model::Bundle::from_json_str(bundle_text) {
        Ok(b) => Some(b),
        Err(e) => {
            errs.push(format!("stix-model failed to parse the bundle: {e}"));
            None
        }
    };
    let Some(objects) = bundle.get("objects").and_then(Value::as_array) else {
        errs.push("bundle has no objects array".into());
        return errs;
    };

    // 2-3. Ids, types, versioning.
    let mut by_id: HashMap<String, &Value> = HashMap::new();
    for o in objects {
        let ty = o["type"].as_str().unwrap_or("");
        let id = o["id"].as_str().unwrap_or("");
        let known = [SDO_TYPES, SRO_TYPES, SCO_TYPES, META_TYPES]
            .iter()
            .any(|l| l.contains(&ty))
            || ty.starts_with("x-");
        if !known {
            errs.push(format!("{id}: unknown type {ty:?}"));
        }
        match id.split_once("--") {
            Some((prefix, uuid)) => {
                if prefix != ty {
                    errs.push(format!("{id}: id prefix does not match type {ty:?}"));
                }
                if !is_uuid(uuid) {
                    errs.push(format!("{id}: malformed id"));
                }
            }
            None => errs.push(format!("{id:?}: malformed id")),
        }
        if by_id.insert(id.to_string(), o).is_some() {
            errs.push(format!("{id}: duplicate id"));
        }
        if SDO_TYPES.contains(&ty) || SRO_TYPES.contains(&ty) {
            if o["spec_version"].as_str() != Some("2.1") {
                errs.push(format!("{id}: spec_version must be \"2.1\""));
            }
            match (o["created"].as_str(), o["modified"].as_str()) {
                (Some(c), Some(m)) => {
                    for t in [c, m] {
                        if !is_ms_timestamp(t) {
                            errs.push(format!(
                                "{id}: timestamp format {t:?} is not YYYY-MM-DDTHH:MM:SS.mmmZ"
                            ));
                        }
                    }
                    if m < c {
                        errs.push(format!("{id}: modified is before created"));
                    }
                }
                _ => errs.push(format!("{id}: missing created/modified")),
            }
        }
    }

    // 4. Every reference resolves.
    for o in objects {
        let id = o["id"].as_str().unwrap_or("");
        for (key, target) in refs(o) {
            if !by_id.contains_key(&target) {
                errs.push(format!("{id}: unresolved reference {key} -> {target}"));
            }
        }
    }

    // 5-7, 10. Per-type checks.
    let id_re = Regex::new(
        r"\b[a-z][a-z0-9-]*--[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b",
    )
    .unwrap();
    for o in objects {
        let id = o["id"].as_str().unwrap_or("");
        match o["type"].as_str().unwrap_or("") {
            "relationship" => {
                let rt = o["relationship_type"].as_str().unwrap_or("");
                if !RELATIONSHIP_TYPES.contains(&rt) {
                    errs.push(format!(
                        "{id}: relationship_type {rt:?} is not in the STIX 2.1 vocabulary"
                    ));
                }
            }
            "indicator" if o["pattern_type"] == "stix" => {
                if let Err(e) = stix::parse(o["pattern"].as_str().unwrap_or("")) {
                    errs.push(format!("{id}: indicator pattern does not parse: {e}"));
                }
            }
            "file" => {
                let want = o["hashes"]["SHA-256"].as_str();
                let artifact = o["content_ref"].as_str().and_then(|r| by_id.get(r));
                if let (Some(want), Some(artifact)) = (want, artifact) {
                    let decoded = artifact["payload_bin"]
                        .as_str()
                        .map(|b| base64::engine::general_purpose::STANDARD.decode(b));
                    match decoded {
                        Some(Ok(bytes)) => {
                            let got = format!("{:x}", Sha256::digest(&bytes));
                            if !got.eq_ignore_ascii_case(want) {
                                errs.push(format!(
                                    "{id}: SHA-256 {want} does not match its artifact's payload ({got})"
                                ));
                            }
                        }
                        _ => errs.push(format!(
                            "{id}: content_ref artifact has no decodable payload_bin"
                        )),
                    }
                }
            }
            "report" => {
                let listed: HashSet<&str> = o["object_refs"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                for m in id_re.find_iter(o["description"].as_str().unwrap_or("")) {
                    let cited = m.as_str();
                    if !by_id.contains_key(cited) {
                        errs.push(format!("{id}: report cites unknown object {cited}"));
                    } else if !listed.contains(cited) {
                        errs.push(format!("{id}: report cites {cited} but it is not in object_refs"));
                    }
                }
            }
            _ => {}
        }
    }

    // 9. Graph shape: relationships are edges between their endpoints; other
    // references are edges from their holder, except container references.
    let mut edges: HashSet<(String, String)> = HashSet::new();
    let mut add_edge = |a: &str, b: &str| {
        if a != b && by_id.contains_key(a) && by_id.contains_key(b) {
            let (x, y) = if a < b { (a, b) } else { (b, a) };
            edges.insert((x.to_string(), y.to_string()));
        }
    };
    for o in objects {
        let id = o["id"].as_str().unwrap_or("");
        let ty = o["type"].as_str().unwrap_or("");
        if ty == "relationship" {
            add_edge(
                o["source_ref"].as_str().unwrap_or(""),
                o["target_ref"].as_str().unwrap_or(""),
            );
            continue;
        }
        for (key, target) in refs(o) {
            let container = key == "created_by_ref"
                || key == "object_marking_refs"
                || (key == "object_refs" && CONTAINER_TYPES.contains(&ty));
            if !container {
                add_edge(id, &target);
            }
        }
    }
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for (a, b) in &edges {
        adj.entry(a.as_str()).or_default().push(b.as_str());
        adj.entry(b.as_str()).or_default().push(a.as_str());
    }
    let limit = std::cmp::max(3, edges.len() / 3);
    if let Some((hub, n)) = adj.iter().map(|(k, v)| (*k, v.len())).max_by_key(|&(_, n)| n) {
        if n > limit {
            errs.push(format!(
                "graph is hub-and-spoke: {hub} has {n} edges (limit {limit} of {} total)",
                edges.len()
            ));
        }
    }
    let mut diameter = 0;
    for &start in adj.keys() {
        let mut dist: HashMap<&str, usize> = HashMap::from([(start, 0)]);
        let mut queue = VecDeque::from([start]);
        while let Some(node) = queue.pop_front() {
            let d = dist[node];
            for &next in &adj[node] {
                if !dist.contains_key(next) {
                    dist.insert(next, d + 1);
                    queue.push_back(next);
                }
            }
        }
        diameter = diameter.max(dist.values().copied().max().unwrap_or(0));
    }
    if diameter < 4 {
        errs.push(format!("graph diameter is {diameter}; at least 4 hops are required"));
    }

    // 8. Example patterns, through the real matcher.
    let patterns: Vec<Value> = match serde_json::from_str::<Value>(patterns_text) {
        Ok(Value::Array(a)) => a,
        Ok(_) => {
            errs.push("patterns.json must be a JSON array".into());
            Vec::new()
        }
        Err(e) => {
            errs.push(format!("patterns.json is not valid JSON: {e}"));
            Vec::new()
        }
    };
    if patterns.is_empty() {
        errs.push("patterns.json has no example patterns".into());
    }
    let observed_ids: Vec<&str> = objects
        .iter()
        .filter(|o| o["type"] == "observed-data")
        .filter_map(|o| o["id"].as_str())
        .collect();
    if let Some(parsed) = &parsed {
        for p in &patterns {
            let name = p["name"].as_str().unwrap_or("<unnamed>");
            let want_match = p["expect"]["matched"].as_bool();
            let mut want: Vec<&str> = p["expect"]["observed_data"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            want.sort_unstable();
            want.dedup();
            let ast = match stix::parse(p["pattern"].as_str().unwrap_or("")) {
                Ok(a) => a,
                Err(e) => {
                    errs.push(format!("pattern {name:?} does not parse: {e}"));
                    continue;
                }
            };
            match stix::match_bundle(&ast, parsed) {
                Ok(r) => {
                    let mut got: Vec<&str> = r
                        .observations()
                        .iter()
                        .filter_map(|&i| observed_ids.get(i).copied())
                        .collect();
                    got.sort_unstable();
                    got.dedup();
                    if Some(r.is_match()) != want_match || got != want {
                        errs.push(format!(
                            "pattern {name:?}: expected matched={want_match:?} {want:?}, got matched={} {got:?}",
                            r.is_match()
                        ));
                    }
                }
                Err(e) => errs.push(format!("pattern {name:?} failed to match: {e}")),
            }
        }
    }
    errs
}
```

- [ ] **Step 5: Run the tests to verify they all pass**

Run: `cargo test -p stix-rust --test datasets`
Expected: all 23 tests PASS; `every_dataset_in_the_repo_is_valid` prints "no datasets/ directory; nothing to validate".

If a test fails because of an API detail (e.g. `MatchError` lacks `Display`, or `Bundle` lives at a different path), adjust the call — not the assertion — and note it in the report. If a negative test cannot be made to fail for the stated reason, stop and report.

- [ ] **Step 6: Workspace checks**

Run: `cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --workspace`
Expected: clean.

- [ ] **Step 7: Commit and open PR 1**

```bash
git add crates/stix/Cargo.toml crates/stix/tests/datasets.rs
git commit -m "test(stix): validate every dataset under datasets/"
git push -u origin test/dataset-validation
gh pr create --title "test: validate STIX datasets under datasets/" --label area:rust-core --label type:feat --body-file <scratchpad>/pr-body-dataset-validation.md
```

The body describes the ten checks and the graph rule, and notes that with no `datasets/` directory the repo test validates nothing.

---

### Task 2: The Hackers bundle and example patterns (parent, PR 2)

**Files:**
- Create: `datasets/hackers-1995/bundle.json`
- Create: `datasets/hackers-1995/patterns.json`

**Interfaces:**
- Consumes: Task 1's validator, which is merged to main before this branch is cut. `patterns.json` uses the exact format in Task 1's Interfaces.
- Produces: a valid bundle whose `report` (id recorded in the task report) exists with `name`, `published`, `report_types`, `object_refs` and a one-paragraph summary `description`; Task 3 replaces that `description` with the full narrative.

Branch: `git fetch origin && git checkout -b data/hackers-1995 origin/main` (after PR 1 merges).

- [ ] **Step 1: Generate the fixed values**

```bash
python3 -c "import uuid; [print(uuid.uuid4()) for _ in range(120)]" > <scratchpad>/uuids.txt
printf '%s' "<the garbage file contents — 2-6 short original lines, see Step 2>" > <scratchpad>/garbage.bin
shasum -a 256 <scratchpad>/garbage.bin      # → the file SCO's hashes.SHA-256
base64 < <scratchpad>/garbage.bin            # → the artifact's payload_bin
```

- [ ] **Step 2: Author `bundle.json`**

Write one `bundle` (fresh `bundle--<uuid>`, no `spec_version`) containing, at minimum, every element of the spec's object map:

- **threat-actors:**
  - Zero Cool / Crash Override (Dade Murphy), with `aliases` holding both handles;
  - Acid Burn (Kate Libby), Cereal Killer, Lord Nikon, Phantom Phreak and Joey (Joey Pardella);
  - The Plague (Eugene Belford): insider, `primary_motivation` `personal-gain`.
- **intrusion-set** "The Elite", with each crew threat-actor `attributed-to` it.
- **identity** Ellingson Mineral Company (organization) `located-at` a **location** (New York City), and **identity** Agent Richard Gill (individual, Secret Service).
- **infrastructure** the Gibson, which Ellingson `owns`.
- **malware:**
  - the salami-slicing worm (worm);
  - the Da Vinci virus (virus), `authored-by` The Plague.
- **campaigns** The Plague's frame-up, `attributed-to` The Plague, and "Hack the Planet", `attributed-to` The Elite.
- **attack-patterns** with real MITRE ATT&CK `external_references`, at least:
  - social engineering / phishing for information (T1598);
  - exfiltration of the garbage file (e.g. T1041).
- At least one **tool**.
- **vulnerability**: fictional, with no CVE.
- **course-of-action** that `mitigates` the malware.
- **SCOs:**
  - `domain-name` `ellingson-mineral.example`;
  - `ipv4-addr` in `192.0.2.0/24`, linked with `resolves_to_refs`;
  - `url` on that domain;
  - the garbage `file` (SHA-256 from Step 1, `content_ref` to the `artifact` holding the base64 payload).
- **observed-data** objects referencing those SCOs, with **sightings** of the indicators `where_sighted_refs` Agent Gill's identity, `observed_data_refs` the observed-data.
- **indicators** with STIX patterns over those SCOs, `indicates` the malware or campaign.
- A **grouping** of the evidence, a **note** and/or **opinion** where natural, and the **report** (see Interfaces).

Rules:
- Wire relationships so they chain: threat-actor → intrusion-set → campaign → malware → infrastructure → identity → location, and sighting → observed-data → SCOs.
- Timestamps: `created`/`modified` set around the 1995 setting; `valid_from`, `first_observed` and `last_observed` consistent with the story.
- Each `name` matches how the film refers to the character or thing.

- [ ] **Step 3: Author `patterns.json`**

Include at least:
- one matching pattern per SCO type present;
- one compound pattern (`AND`/`OR`);
- one non-matching pattern;
- every indicator's `pattern`, each with its exact expected observed-data ids.

No qualifiers.

- [ ] **Step 4: Validate with the repo test**

Run: `cargo test -p stix-rust --test datasets every_dataset_in_the_repo_is_valid -- --nocapture`
Expected: PASS, printing `validated …/datasets/hackers-1995`. Fix the data until it passes (never the test).

- [ ] **Step 5: Run the OASIS validator**

```bash
python3 -m venv <scratchpad>/v && <scratchpad>/v/bin/pip install -q stix2-validator
<scratchpad>/v/bin/stix2_validator datasets/hackers-1995/bundle.json
```

Expected: no errors. Fix every warning you can; record any you deliberately keep (with the reason) and the final output for the PR body.

- [ ] **Step 6: Commit**

```bash
git add datasets/hackers-1995/bundle.json datasets/hackers-1995/patterns.json
git commit -m "data: Hackers (1995) STIX 2.1 bundle and example patterns"
```

---

### Task 3: The narrative report (parent, PR 2)

**Files:**
- Modify: `datasets/hackers-1995/bundle.json` (the `report`'s `description` and `object_refs`)

**Interfaces:**
- Consumes: Task 2's bundle and the report id.
- Produces: the final report.

- [ ] **Step 1: Write the narrative**

Replace the report's `description` with wiki-quality Markdown:
- **Sections**, at least:
  - `## Overview`
  - `## Cast` (characters, handles, roles)
  - `## Plot`, in acts: Zero Cool's 1988 crash and ban; the Gibson intrusion and the garbage file; The Plague's worm and the Da Vinci frame-up; "Hack the Planet"; arrests and exoneration
  - `## Aftermath`
- **Citations:** every object the text discusses is named exactly as its `name` (SCOs by their value) and cited inline by id, e.g. *Zero Cool* (`threat-actor--…`).
- **Original prose only:** written fresh, not copied or closely paraphrased from any source.
- **Length:** roughly 600–1,200 words.

- [ ] **Step 2: Sync `object_refs`**

Set `object_refs` to include every id the narrative cites, plus the evidence it summarizes.

- [ ] **Step 3: Validate**

Run: `cargo test -p stix-rust --test datasets every_dataset_in_the_repo_is_valid`
Expected: PASS (citation and object_refs checks included). Re-run `stix2_validator` and confirm there are no new findings.

- [ ] **Step 4: Commit**

```bash
git add datasets/hackers-1995/bundle.json
git commit -m "data: wiki-style narrative report for the Hackers dataset"
```

---

### Task 4: README, ownership row, PR 2 (parent)

**Files:**
- Create: `datasets/hackers-1995/README.md`
- Modify: `AGENTS.md` (Areas table)

**Interfaces:**
- Consumes: Tasks 2–3.
- Produces: PR 2.

- [ ] **Step 1: Write the README**

Cover:
- the film credit: *Hackers* (1995), United Artists; written by Rafael Moreno, directed by Iain Softley; the dataset is a fan-made illustration, not affiliated;
- a cast → STIX object table with ids;
- the conventions (reserved `.example` domains and RFC 5737 IPs, how the SHA-256 is derived, no pattern qualifiers);
- how to validate: `cargo test -p stix-rust --test datasets`;
- how to add another movie dataset (a sibling directory with the same three files).

- [ ] **Step 2: Add the ownership row**

Add to the AGENTS.md Areas table, after the TypeScript (wasm) row:

```markdown
| Datasets | parent | `datasets/**` | JSON (validated by `crates/stix/tests/datasets.rs`) | (none yet) |
```

- [ ] **Step 3: Commit and open PR 2**

```bash
git add datasets/hackers-1995/README.md AGENTS.md
git commit -m "docs: Hackers dataset README and datasets/ ownership"
git push -u origin data/hackers-1995
gh pr create --title "data: Hackers (1995) STIX 2.1 dataset" --label type:feat --body-file <scratchpad>/pr-body-hackers-dataset.md
```

The body includes:
- object counts by type;
- the graph diameter and max degree (from a local run);
- the `stix2_validator` output and any warnings deliberately kept.

CI's `validate` job runs Task 1's test against the dataset.
