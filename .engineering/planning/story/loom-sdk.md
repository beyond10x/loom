---
format: aep.planning-md/3
id: story:loom-sdk
kind: story
status: draft
title: One SDK crate embeds the governed runtime, shown by one example
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:runtime-merge
revision: 1
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
