---
format: aep.planning-md/3
id: story:json-depth-bound
kind: story
status: draft
title: No Loom build parses model JSON without a nesting limit
relations:
- decomposes: epic:runtime-consolidation
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

No Loom build parses model or provider JSON without serde_json's nesting limit.

## Finding

Wave 2026-10-05-w19, implementor of `story:import-commission`: ESS's `ess-conformance` crate, which
`b10x-commission-conformance` depends on, enables serde_json's `unbounded_depth` and `raw_value`.
Cargo unifies features across the packages of one invocation, so every `--workspace` build of Loom
now compiles `b10x-loom` with `unbounded_depth` (`cargo tree --workspace -e features -i serde_json`
shows the path). `unbounded_depth` removes the 128-level recursion limit, so deeply nested JSON from
a model or provider can exhaust the stack. `-p b10x-loom` builds and `cargo install` of one package
are unaffected. No Loom test covers it. Inferred, not observed: no deep-JSON run was made.

## Acceptance

- A test in Loom feeds JSON nested beyond 128 levels through the provider decoding path in a
  `--workspace` test build and asserts a typed refusal, not a stack overflow.
- Either ESS drops `unbounded_depth` from `ess-conformance` (an upstream fix, released and pinned),
  or Loom's decoding bounds depth itself.
