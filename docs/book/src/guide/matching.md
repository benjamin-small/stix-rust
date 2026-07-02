# Matching

`stix-matcher` evaluates a parsed pattern against **observations** and reports
whether — and where — it matched.

## Observations

An **observation** is a set of objects seen together, plus temporal metadata
(`first_observed`, `last_observed`, `number_observed`). This mirrors the STIX
semantics and the MITRE reference implementation: **each `observed-data` SDO is one
observation**, its `object_refs` naming the objects in it.

## Four entry points

All normalize to observations internally; pick by what you have:

| Entry point | Input | Use when |
| --- | --- | --- |
| `match_bundle(&pattern, &bundle)` | a whole `Bundle` | you have bundle JSON — the common case; derives observations from its `observed-data` SDOs |
| `match_observed_data(&pattern, &sdos, &store)` | `observed-data` SDOs + an `ObjectStore` | MITRE-compatible; you manage the store |
| `match_observations(&pattern, &observations)` | pre-built `Observation`s | you construct observations yourself |
| `match_scos(&pattern, &scos)` | a flat object list | quick tests; the list is treated as one observation |

The result is a `MatchResult`: `is_match()` plus `observations()` — the indices of
the observations that participated in the match.

## How a `[ … ]` block evaluates: binding enumeration

Within one observation, if a comparison expression references multiple constraints
on the **same object type**, they must be satisfied by **one object**, not spread
across several. The matcher implements this by *binding enumeration*: for each
distinct object type in the expression, it tries each candidate object of that type,
and the expression matches if some assignment makes the boolean tree true.

Concretely, the pattern `[file:name = 'evil.exe' AND file:size = 10]`:

```json
// MATCHES — one file satisfies both constraints
{ "objects": [ { "type": "file", "name": "evil.exe", "size": 10 } ] }

// DOES NOT MATCH — constraints hold only across two different files
{ "objects": [
  { "type": "file", "name": "evil.exe", "size": 99 },
  { "type": "file", "name": "ok.txt",   "size": 10 } ] }
```

Constraints on *different* types (`[ipv4-addr:value = … AND domain-name:value = …]`)
bind independently — one object per type, all within the same observation.

## Observation-level logic

- A `[ … ]` block matches if **any observation** in the input satisfies it.
- `[a] AND [b]` requires both blocks to be satisfied — possibly by **different**
  observations.
- `[a] OR [b]` requires either.
- `[a] FOLLOWEDBY [b]` and the `WITHIN`/`REPEATS`/`START..STOP` qualifiers
  **parse but return an explicit `Unsupported` error at match time** — they never
  silently pass. See [Limitations](../limitations.md).

## Operator semantics worth knowing

- `LIKE` uses SQL wildcards (`%`, `_`) and is **anchored** (whole-value).
- `MATCHES` is a regular expression, **unanchored**. An invalid regex never matches.
- `ISSUBSET` / `ISSUPERSET` operate on IP addresses and CIDR ranges (IPv4/IPv6,
  never mixed families).
- `EXISTS` tests that a path resolves to any value at all.
- `NOT` before an operator negates that single test.
- Numeric comparisons promote integers to floats; strings (including timestamps)
  compare lexicographically.
