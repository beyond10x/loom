---
format: aep.planning-md/3
id: story:loom-sdk
kind: story
status: active
title: One SDK crate embeds the governed runtime, shown by one example
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:runtime-merge
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: README.md
- confidence: inferred
  path: crates/loom-sdk
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T01:38:43Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T01:38:43Z", actor: "human:timo", revision: 3}
---
## Outcome

One crate an application depends on to embed a governed agent: it re-exports the contracts, the
runtime and the executor, and documents one example.

## Acceptance

- An example under the SDK crate's `examples/` opens a case on `software-change@1`, runs the loop
  over fake models and stops at `ApprovalRequired`, depending on the SDK crate alone.
- `cargo doc -p <sdk>` builds with no broken intra-doc links.
- `task check` exits 0.

## Depends on

`story:runtime-merge`.

## Scope (inferred)

loom: new SDK crate, `Cargo.toml`.
