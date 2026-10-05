---
format: aep.planning-md/3
id: story:json-depth-bound
kind: story
status: implemented
title: No Loom build parses model JSON without a nesting limit
relations:
- decomposes: epic:runtime-consolidation
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: crates/loom-executor
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T07:10:02Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T07:10:02Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-05T07:17:18Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"verification":1}}}
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

## Close (wave 2026-10-05-w32)

The finding above was wrong; it was an inference, and the test disproves it. serde_json's
`unbounded_depth` feature only adds `Deserializer::disable_recursion_limit`; the default 128-level
limit stays (serde_json 1.0.151 `src/de.rs:63-67`, the method at `:215`), and nothing in Loom calls
the method (grep over `crates`, `generated`, `Cargo.toml`: no match).

`crates/loom-executor/tests/json_depth.rs` (12 cases) feeds JSON nested beyond 128 levels, and up to
100 000, through every place Loom decodes model or provider JSON (SSE payloads in both framings,
the Messages and Responses decoders, streamed tool and function-call arguments, the JSON exchange,
transcript replay) in a `--workspace` build and asserts a typed refusal naming the recursion limit.
All 12 pass on base `53cea8a`; a mutant that calls `disable_recursion_limit()` at the five parse
sites fails all 12. The tests are the guard; no code change was needed.
