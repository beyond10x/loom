---
format: aep.planning-md/3
id: upstream-blocker:takt-session-executor-crate
kind: upstream-blocker
status: cleared
title: takt has not yet built the standalone metaharness session-executor crate Loom is to host
relations:
- blocks: story:metaharness-session-executor
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-10-20T23:59:19Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":1}}}
---
## Waiting on

takt's story `story:harness-proposes-takt-performs` in beyond10x/takt
(<https://github.com/beyond10x/takt>), which builds the metaharness session executor as a
standalone crate: an `AgentExecutor` that runs a session through the metaharness library in a
scratch copy and returns its change as a `ProposedAction` (ADR 0099 point 3; ADR 0098 point 3).

## Clears when

All three hold, each read from takt's `main`:

1. the crate exists at a named commit, and that commit is written into this blocker's body;
2. `cargo metadata` in takt shows the crate depending on no takt package (only loom-sdk,
   metaharness and their dependencies);
3. its tests pass in takt's CI on that commit.

## Blocks

`story:metaharness-session-executor`, which moves the crate into Loom unchanged.

## Cleared (2026-10-21)

Read from takt's `main` at `7f2843bbc6bf70d0fc3b58b0ead2cac2401c6ef2`:

1. The crate exists: `crates/session-executor`, package `b10x-session-executor`, last changed in
   `c933f17f` (2026-10-20).
2. Its `[dependencies]` are `b10x-loom-sdk`, `metaharness`, `metaharness-b10x` and `serde_json`; no
   takt package.
3. takt's checks on that commit: `takt / task check`, `common / Security and privacy`, `Build
   documentation` and `publish / publish` completed with success.
