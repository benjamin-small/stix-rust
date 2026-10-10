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
    assert!(
        count > 0,
        "datasets/ exists but contains no dataset directories"
    );
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
    o.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
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
    objects_mut(bundle)
        .iter_mut()
        .find(|o| o["id"] == id)
        .unwrap()
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
    objects_mut(&mut b)
        .push(json!({"type": "movie", "id": "movie--00000000-0000-4000-8000-0000000000aa"}));
    assert_flags(&b, &good_patterns(), "unknown type");
}

#[test]
fn rejects_missing_spec_version() {
    let mut b = good_bundle();
    find_mut(&mut b, TA)
        .as_object_mut()
        .unwrap()
        .remove("spec_version");
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
    find_mut(&mut b, OD)["object_refs"] =
        json!(["domain-name--00000000-0000-4000-8000-0000000000ee"]);
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
        objects_mut(&mut b).push(
            json!({"type": "domain-name", "spec_version": "2.1", "id": dn,
                                        "value": format!("hub{k}.example")}),
        );
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
    find_mut(&mut b, RP)["description"] =
        json!("see `malware--00000000-0000-4000-8000-0000000000ee`");
    assert_flags(&b, &good_patterns(), "report cites unknown object");
}

#[test]
fn rejects_a_report_citing_an_object_missing_from_object_refs() {
    let mut b = good_bundle();
    find_mut(&mut b, RP)["object_refs"] = json!([TA]);
    assert_flags(&b, &good_patterns(), "not in object_refs");
}

// -------------------------------------------------------------- validator

const SDO_TYPES: &[&str] = &[
    "attack-pattern",
    "campaign",
    "course-of-action",
    "grouping",
    "identity",
    "incident",
    "indicator",
    "infrastructure",
    "intrusion-set",
    "location",
    "malware",
    "malware-analysis",
    "note",
    "observed-data",
    "opinion",
    "report",
    "threat-actor",
    "tool",
    "vulnerability",
];
const SRO_TYPES: &[&str] = &["relationship", "sighting"];
const SCO_TYPES: &[&str] = &[
    "artifact",
    "autonomous-system",
    "directory",
    "domain-name",
    "email-addr",
    "email-message",
    "file",
    "ipv4-addr",
    "ipv6-addr",
    "mac-addr",
    "mutex",
    "network-traffic",
    "process",
    "software",
    "url",
    "user-account",
    "windows-registry-key",
    "x509-certificate",
];
const META_TYPES: &[&str] = &[
    "extension-definition",
    "language-content",
    "marking-definition",
];
/// STIX 2.1 relationship types (common: derived-from, duplicate-of, related-to;
/// plus every SDO-specific type defined in the spec).
const RELATIONSHIP_TYPES: &[&str] = &[
    "analysis-of",
    "attributed-to",
    "authored-by",
    "av-analysis-of",
    "based-on",
    "beacons-to",
    "characterizes",
    "communicates-with",
    "compromises",
    "consists-of",
    "controls",
    "delivers",
    "derived-from",
    "downloads",
    "drops",
    "duplicate-of",
    "dynamic-analysis-of",
    "exfiltrates-to",
    "exploits",
    "has",
    "hosts",
    "impersonates",
    "indicates",
    "investigates",
    "located-at",
    "mitigates",
    "originates-from",
    "owns",
    "related-to",
    "remediates",
    "static-analysis-of",
    "targets",
    "uses",
    "variant-of",
];
/// Types whose `object_refs` list contents rather than structure.
const CONTAINER_TYPES: &[&str] = &["report", "grouping", "note", "opinion"];

fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12].iter().zip(&parts).all(|(n, p)| {
            p.len() == *n
                && p.chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        })
}

fn is_ms_timestamp(s: &str) -> bool {
    Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$")
        .unwrap()
        .is_match(s)
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
                        for s in val
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                        {
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
                        errs.push(format!(
                            "{id}: report cites {cited} but it is not in object_refs"
                        ));
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
    if let Some((hub, n)) = adj
        .iter()
        .map(|(k, v)| (*k, v.len()))
        .max_by_key(|&(_, n)| n)
    {
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
        errs.push(format!(
            "graph diameter is {diameter}; at least 4 hops are required"
        ));
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
