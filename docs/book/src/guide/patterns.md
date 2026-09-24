# Patterns

`stix-pattern` parses the **complete** STIX 2.1 patterning grammar into a typed AST.
This page covers what the language expresses and how the parser structures it.

## Anatomy of a pattern

```text
[file:name LIKE '%.exe' AND file:size > 1024] OR [ipv4-addr:value ISSUBSET '10.0.0.0/8']
```

A pattern is a tree of **observation expressions** — each `[ … ]` block is one
observation test — combined with `AND`, `OR`, and `FOLLOWEDBY`, optionally wrapped
in qualifiers. Inside the brackets live **comparison expressions**: property tests
combined with their own `AND`/`OR`.

## Supported constructs

| Construct | Examples |
| --- | --- |
| Comparison operators | `=` `!=` `<` `<=` `>` `>=` `IN` `LIKE` `MATCHES` `ISSUBSET` `ISSUPERSET` `EXISTS` |
| Negation | `file:name NOT = 'x'` |
| Boolean (inside `[ ]`) | `[a = 1 AND b = 2]`, grouping with `( )` |
| Object paths | `file:hashes.'SHA-256'`, `network-traffic:protocols[0]`, `x:list[*]` |
| Reference traversal | `network-traffic:src_ref.value` |
| Typed literals | `t'2020-01-01T00:00:00Z'` (timestamp), `b'aGk='` (base64), `h'cafe'` (hex) |
| Observation operators | `[a] AND [b]`, `[a] OR [b]`, `[a] FOLLOWEDBY [b]` |
| Qualifiers | `WITHIN 60 SECONDS`, `REPEATS 5 TIMES`, `START t'…' STOP t'…'` |

> `FOLLOWEDBY` and the qualifiers **parse** but are **not yet matched** — see
> [Limitations](../limitations.md).

## Precedence

Observation level, loosest to tightest:

```text
FOLLOWEDBY  <  OR  <  AND  <  qualifiers (postfix)  <  [ … ] / ( … )
```

Comparison level (inside `[ ]`): `OR` < `AND` < individual test. So
`[a = 1 OR b = 2 AND c = 3]` parses as `a = 1 OR (b = 2 AND c = 3)` — use
parentheses when in doubt.

## Object paths

A path starts with the **object type**, then walks properties:

- `.key` — property access (quote keys with special characters: `hashes.'SHA-256'`)
- `[0]` — list index
- `[*]` — *any* list element (the test passes if any element satisfies it)
- A step through a `_ref` property dereferences to the referenced object
  (requires an object store at match time; see [Matching](matching.md)).

## The AST is data

`parse()` returns a `Pattern` that is fully `serde`-serializable — useful for
tooling, caching, and the language bindings (every binding exposes the AST as a
native object/dict/Map). A small example:

```rust
let pattern = stix::parse("[file:size > 1024]").unwrap();
println!("{}", serde_json::to_string_pretty(&pattern).unwrap());
```

```json
{
  "expression": {
    "Observation": {
      "expression": {
        "Test": {
          "path": {
            "object_type": "file",
            "steps": [
              {
                "Key": "size"
              }
            ],
            "span": {
              "start": 1,
              "end": 10
            }
          },
          "operator": "GreaterThan",
          "negated": false,
          "value": {
            "Literal": {
              "Integer": 1024
            }
          },
          "span": {
            "start": 1,
            "end": 17
          }
        }
      },
      "span": {
        "start": 0,
        "end": 18
      }
    }
  }
}
```

Parse errors carry a byte-offset span into the source string:

```text
parse error at bytes 19..20: expected a literal value
```

## The three-address IR

Alongside the AST, a pattern can be lowered to a linear three-address
representation. Each `[...]` observation becomes a *comparison block* — the unit
the matcher enumerates binding sets over — and a `main` block combines the
observation results:

```rust
let pattern = stix::parse("[file:size > 1024] FOLLOWEDBY [file:name = 'a']").unwrap();
let program = stix::pattern::ir::lower(&pattern);
println!("{}", program.to_listing());
```

```text
block b1 (comparison):
  t0 = load        file:size
  t1 = gt          t0, 1024
       yield       t1

block b2 (comparison):
  t2 = load        file:name
  t3 = eq          t2, 'a'
       yield       t3

block main (observation):
  o0 = observe     b1
  o1 = observe     b2
  o2 = followedby  o0, o1
       ret         o2
```

The IR is in SSA form: an instruction's id names the value it produces, so there
is no separate destination field. Instructions carry the byte span of the source
text they came from.

`ir::render` goes the other way, producing *canonical* pattern text — normalized
whitespace, `!=` rather than `<>`, and parentheses only where precedence needs
them. Rendering then reparsing recovers the same AST, which makes canonical text
a usable basis for comparing two patterns:

```rust
let text = stix::pattern::ir::render(&program);
assert_eq!(text, "[file:size > 1024] FOLLOWEDBY [file:name = 'a']");
```

Two caveats. The IR represents more than the matcher can execute —
`FOLLOWEDBY` and the qualifiers lower and render correctly but still return
`MatchError::Unsupported` when matched. And a `Program` that was deserialized or
built by hand should be checked with `Program::validate()` first, since
deserialization does not verify invariants.
