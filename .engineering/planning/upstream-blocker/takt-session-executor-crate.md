---
format: aep.planning-md/3
id: upstream-blocker:takt-session-executor-crate
kind: upstream-blocker
status: open
title: takt has not yet built the standalone metaharness session-executor crate Loom is to host
relations:
- blocks: story:metaharness-session-executor
revision: 2
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
