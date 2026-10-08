---
format: aep.planning-md/3
id: story:compaction-target-bound
kind: story
status: draft
title: A compaction leaves the session at or below its target, or the run stops by name
relations:
- serves: vision:O1
scope:
- confidence: inferred
  path: crates/loom-executor/src/harness/turn_loop/mod.rs
- confidence: inferred
  path: crates/loom-executor/tests/
revision: 2
---
## Outcome

After a compaction of a declared window, the session is at or below the target it compacts to (50 %
of the window), whatever the summary request returned, or the run stops by name instead of sending
the next request above the target.

`story:compaction-summary-bound` (wave 2026-10-07-w2, commit cff8690) settled one case: a summary
no shorter than the items it would replace is now elided. Its implementor reported three paths it
did not change, each of which can still leave the session above the target:

1. a summary shorter than the items it replaces, but not short enough to reach the target, is kept;
2. a summary request that fails on the wire leaves the items as they were
   (`a_summary_turn_that_fails_on_the_wire_leaves_the_run_alive`,
   `crates/loom-executor/src/harness/turn_loop/tests.rs:3084`, asserts the long text survives);
3. an empty summary leaves the items as they were.

## Acceptance

To be settled when the story is proposed, from these options:

- keep a summary only when the session after folding is at or below the target; otherwise elide
  the items it would replace;
- after any compaction that leaves the session above the target, end the run with a named stop
  instead of sending the request.

Each of the three paths above gets a case in `crates/loom-executor/tests/` that asserts the chosen
outcome.

## ESS first

No specification change expected: the compaction record (`loom.run.RecordCompaction`) keeps its
shape. If a named stop is chosen and it is a run outcome, it is a specification change first.

## Source

The report of the `story:compaction-summary-bound` implementor, wave 2026-10-07-w2 (Not done
section).
