---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-08-w4
kind: approval-record
status: draft
title: Wave 2026-10-08-w4 proposal accepted with no operator turn
tags:
- non-interactive
relations:
- decides: story:catalog-pipe-test-writer
revision: 2
---
# Wave 2026-10-08-w4 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. The dispatch names this wave.

- Unit: `story:catalog-pipe-test-writer` (serves `vision:O3`), test-only.
- Cause, reproduced outside cargo: `/bin/sh` is bash, which forks a child for `cat "$1" > "$2"`; the child blocks in `open` of the FIFO and `writer.kill()` reaches only the parent, so the child is reparented to PID 1.
- Other FIFO tests (`catalog_route_adversary.rs`, `evaluate.rs`) start no writer; out of scope.
- Excluded: `story:connectors-invoker` (blocked by `upstream-blocker:connectors-attempt-id`).
- Adversary: none (small test-only defect).
- Pre-flight: `/` 71G free, load 24.6, one worktree before the wave; builds limited to `b10x-loom-cli` with CARGO_INCREMENTAL=0.
- Commits authorised: 1 opening store commit, the unit commits on `impl/catalog-pipe-test-writer`, 1 merge into `wave/2026-10-08-w4`, 1 closing store commit, the merge into `main` through the pull request. No tag, no release.
