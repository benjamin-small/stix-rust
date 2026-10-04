# Limitations & Caveats

The sharp edges, in one place. Read this before production use.

1. **`FOLLOWEDBY` is not matched.** It parses, but evaluation returns an explicit
   `Unsupported` error. *Status: planned.*

2. **Temporal qualifiers are not matched.** `WITHIN`, `REPEATS`, and
   `START..STOP` parse but return `Unsupported` at match time. *Status: planned.*

3. **Timestamps compare as strings.** RFC3339 values are not parsed into instants:
   `2020-01-01T00:00:00Z` ≠ `2020-01-01T00:00:00.000Z` even though they denote the
   same moment. *Workaround: normalize timestamp formats on ingest (a custom-type
   hook is a good place).*

4. **Binding enumeration, not full constraint search.** Within an observation the
   matcher binds **one object per referenced type**. This gives correct
   "same object" semantics for the overwhelming majority of patterns, but exotic
   patterns needing several objects of the *same* type simultaneously may differ
   from the MITRE reference. *Status: full binding-set semantics are future work.*

5. **Reference paths need an `ObjectStore`.** A path through `src_ref` (or any
   `_ref`) resolves via the store; without one it resolves to nothing — a
   **non-match, not an error**. `match_bundle`/`match_scos` build the store for you.

6. **Typed objects synthesize properties.** What `property()` returns is not
   necessarily what the JSON contained (that's a feature — see
   [Custom Object Types](guide/custom-types.md)) — but don't assume a 1:1 mapping.

7. **`[*]` expands, per-element.** `x:list[*] = 'v'` passes if *any* element
   matches; there is no cross-step backtracking beyond that expansion.

8. **Integers and floats are distinct in storage,** and comparisons promote
   numerically — but a pattern's `5` and data's `"5"` (string) never match.

9. **`LIKE` is anchored; `MATCHES` is not.** `LIKE` must cover the whole value;
   `MATCHES` searches anywhere in it. An **invalid regex never matches** (no error).

10. **`ISSUBSET`/`ISSUPERSET` are IP/CIDR-only.** IPv4 or IPv6, never mixed
    families; unparseable input never matches. There is no generic string-set
    containment.

11. **Pattern nesting is capped at 40.** Parenthesized groups plus qualifiers
    (`WITHIN`, `REPEATS`, `START..STOP`) on any path through a pattern may total at
    most `stix_pattern::MAX_NESTING` (40); deeper patterns fail to parse with
    "pattern nests too deeply". Flat chains like `a OR b OR c OR …` are not
    limited.

Also worth knowing: custom-type hooks run **only at parse/import time** — if you
mutate an engine's registrations, previously parsed bundles are unaffected.
