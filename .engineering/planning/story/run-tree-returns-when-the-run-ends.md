---
format: aep.planning-md/3
id: story:run-tree-returns-when-the-run-ends
kind: story
status: implemented
title: run_tree with a cancel flag returns when its run ends, not at the watcher's next wake
summary: The cancel watcher waits on a channel the run's end closes, so run_tree no longer returns up to CANCEL_WATCH late
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:cancel-a-running-bound-program
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T16:10:18Z", actor: "agent:loom", revision: 3, decided_on: {"recorded":{"test_result":1}}}
- {from: "proposed", to: "active", at: "2026-11-26T16:10:18Z", actor: "agent:loom", revision: 4, decided_on: {"recorded":{"test_result":1}}}
- {from: "active", to: "implemented", at: "2026-11-26T16:10:18Z", actor: "agent:loom", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

`run_tree` called with a cancel flag returns as soon as its run ends, not up to `CANCEL_WATCH`
(100 ms) later. The watcher thread that bridges the caller's flag to the run's `Cancel` waits on a
channel with `recv_timeout(CANCEL_WATCH)` instead of sleeping; the run holds the channel's sender
and drops it however it returns (an `Ok`, an early refusal, a panic), which wakes the watcher at
once. The contract is unchanged: the watcher still reads the flag every `CANCEL_WATCH` and sets the
`Cancel` every port of the run holds.

## Why

Every `run_tree` with a cancel flag returned 0-100 ms late: the scope joined the watcher, and the
watcher was in `std::thread::sleep(CANCEL_WATCH)` when the run returned. A consumer that calls
`run_tree` once per step measured about 72 s of summed test time per test step from this, about
18 s of its forge test step's 112.7 s.

## Acceptance

- `a_run_with_a_cancel_flag_returns_when_it_ends` (`crates/loom-tree/tests/cancel_sec/mod.rs`):
  20 calls of `run_tree` with a cancel flag on a tree whose root is already terminated, and 20 on
  a store refused at once; the fastest of each 20 is below 50 ms (half of `CANCEL_WATCH`).
  Measured before the change: every terminated-tree call 100.05-100.16 ms (40 calls 2.40 s), so
  it fails; after it: 10-17 ms per call (40 calls 0.23 s), so it passes with a margin of about 5.
- The cancel tests of the same module still pass: a cancel set during a program or a
  `handoff_when` check ends the run `Cancelled` within its bound, and one run's cancel ends no
  other run in the process.

## Scope

- `crates/loom-tree/src/run.rs`: `run_tree`'s watcher (`recv_timeout` on an `mpsc` channel whose
  sender the run drops on return), `SetOnDrop` removed.
- `crates/loom-tree/tests/cancel_sec/mod.rs`: the new test and a shared `settings()` helper.
- `CHANGELOG.md`: one `### Fixed` bullet under `## [Unreleased]`.
