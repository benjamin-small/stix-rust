# Introduction

**stix-rust** is a Rust toolkit — with Python, Java, and TypeScript bindings — for
working with [STIX 2.1](https://oasis-open.github.io/cti-documentation/stix/intro.html),
the OASIS standard for representing cyber threat intelligence. It does three things:

1. **Parse** STIX patterning-language patterns into a typed, serializable AST.
2. **Import** STIX objects (SDOs, SCOs) and bundles into a flexible object model.
3. **Match** patterns against sets of observed objects and tell you what bound.

Try the [pattern playground](playground/) to parse a pattern in your browser right now.

## What is STIX?

STIX (Structured Threat Information eXpression) is a standardized JSON language for
describing threat intelligence — indicators, malware, observed data, and the
relationships between them. Two pieces matter most here:

- **STIX objects** — JSON documents like an `ipv4-addr`, a `file`, or an
  `observed-data` SDO, usually delivered together in a **bundle**.
- **The patterning language** — a query-like syntax used inside `indicator`
  objects to describe *what to look for*:

  ```text
  [ipv4-addr:value = '198.51.100.1' OR domain-name:value = 'evil.example']
  ```

stix-rust parses those patterns and evaluates them against observed objects to
answer: **"does this threat intelligence match what we saw?"**

## Shape of the toolkit

The core is five Rust crates with clean dependency edges; four language bindings
wrap a shared FFI facade:

| Layer | Pieces |
| --- | --- |
| Core crates | `stix-pattern` (parser), `stix-model` (objects), `stix-matcher` (engine), `stix` (umbrella) |
| FFI facade | `stix-ffi` — opaque handles + JSON deep structure, wrapped by every binding |
| Bindings | [Python](bindings/python.md) · [Java](bindings/java.md) · [TypeScript Node](bindings/typescript-node.md) · [TypeScript wasm](bindings/typescript-wasm.md) |

Every language gets the same conceptual API: an **Engine** that parses patterns and
bundles into handles, matches them, and accepts **custom object type** registrations
with validation/computed-property hooks.

## Status, honestly

The parser handles the **complete** STIX 2.1 patterning grammar. The matcher
implements all comparison operators, boolean logic, object-path resolution
(including reference traversal), and observation-level `AND`/`OR`.

Two grammar features **parse but do not yet match**: `FOLLOWEDBY` sequencing and
the temporal qualifiers (`WITHIN`, `REPEATS`, `START..STOP`). Reaching them at match
time returns an explicit *unsupported* error rather than silently passing. See
[Limitations & Caveats](limitations.md) for the full list of sharp edges — reading
that page before production use is strongly recommended.
