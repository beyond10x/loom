---
format: aep.planning-md/3
id: story:compaction-target-bound
kind: story
status: implemented
title: A compaction leaves the session at or below its target, or the run stops by name
relations:
- serves: vision:O1
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-executor/src/compaction.rs
- confidence: cited
  path: crates/loom-executor/src/harness/turn_loop/mod.rs
- confidence: cited
  path: crates/loom-executor/src/harness/turn_loop/tests.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w2_harness_loop_port.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w3_compaction_contract.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w4_20261009_compaction_target_bound.rs
- confidence: cited
  path: crates/loom-executor/tests/compaction_target_bound.rs
- confidence: cited
  path: ess/domains/run.yaml
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T23:16:40Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-08T23:16:40Z", actor: "human:timo", revision: 7}
- {from: "active", to: "implemented", at: "2026-10-09T16:30:02Z", actor: "human:timo", revision: 11, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

After a compaction of a declared window, the session is at or below the target it compacts to (50 %
of the window) whenever eliding can reach it, and a run whose session is still above the trigger
(80 %) after eliding stops by name instead of sending the next request
(`decision-blocker:compaction-target-outcome`, option C).

`story:compaction-summary-bound` (wave 2026-10-07-w2, commit cff8690) settled one case: a summary
no shorter than the items it would replace is now elided. Four paths still leave the session above
the target on `main` at 5e3d0cb (`crates/loom-executor/src/harness/turn_loop/mod.rs`):

1. a summary shorter than the items it replaces, but not short enough to reach the target, is kept
   (`:3174` compares against the replaced bytes, not the target);
2. a summary request that fails on the wire leaves the items as they were (`:3129-3137`;
   `a_summary_turn_that_fails_on_the_wire_leaves_the_run_alive`, `turn_loop/tests.rs:3084`, asserts
   the long text survives at `:3115-3121`);
3. an empty summary leaves the items as they were (`:3155-3162`);
4. a fold too small to summarise, or with no end, is skipped (`:3092-3098`, a no-op at `:3024`).

On each, the loop keeps a summary when the session after folding it is at or below the target;
otherwise it keeps the smaller of the summary and the elision of the folded items behind
`ELISION_MARKER` (in practice the elision; the note is about 340 bytes). A summary no smaller than
the items it replaces is never kept. The parts never removed (the task item, reasoning items, the
protected tail, the newest item, and the instructions and tool schemas in the provider's reported
input) can leave the session above the target; when the session is then still above the trigger,
the run ends with a new `LoopStop` variant naming the window, the target and the occupied tokens,
and no further request is sent. A failed or empty summary is still not a failed run.

## Acceptance

Each of the four paths has a case in `crates/loom-executor/tests/` asserting that after the
compaction the next request carries the elided session at or below the target; a summary that
brings the session to or below the target is kept, not elided; one case whose
unremovable parts exceed the trigger asserts the run ends with the new `LoopStop` variant, files its
session as `Stopped`, and sends no request after the compaction. `CHANGELOG.md` (**Unreleased**)
names the variant as a breaking change for exhaustive `LoopStop` matches.

## ESS first

`ess/domains/run.yaml` declares how a run ends (`loom.run.RunEnding`, `[Answered, Stopped,
Failed]`, `:63-69`); the new stop is a `Stopped` ending. The unit's first commit changes only
`ess/`: the comment of the `RunEnding` declaration names the cause (the session stays above its
compaction trigger after compaction), with `task drift` clean. The cause is not typed: a stop-cause
enum beside the hand-written `LoopStop` would be a second model; declaring `LoopStop` in ESS is
`story:loop-stop-model`. The red test is the story's new case for the stop.
`loom.run.RecordCompaction` keeps its shape.

## Source

The report of the `story:compaction-summary-bound` implementor, wave 2026-10-07-w2 (Not done
section); scoping 2026-10-09 added path 4.

## Delivered

## Delivered

- ESS first: 00648f4 changes only `ess/domains/run.yaml` (the `loom.run.RunEnding` declaration
  names the new cause of a `Stopped` ending); on it the 7 cases of
  `crates/loom-executor/tests/compaction_target_bound.rs` ran 1 passed, 6 failed.
- 9a77b56 implements the four paths and `LoopStop::ContextAboveTrigger { window, target, occupied }`.
- Keep rule as settled in the wave: a summary is kept when the session it leaves is at or below the
  target; above it, only when it is smaller than the elision note.
- Adversary pass 1 (`review-result:adversary-w1-20261009-loom-compaction-target-bound-pass-1`)
  found the session judged by its items alone; 4b603bc carries the instruction and tool-schema
  overhead the provider's count reveals, sizes the target and the stop on estimate plus overhead,
  and clears the stored count after a compaction that changed the conversation.
- Where it lands: `crates/loom-executor/src/harness/turn_loop/mod.rs` (`compact_run`, `summarise`,
  the stop check, `RunState.overhead`), `crates/loom-executor/src/compaction.rs` (module doc),
  `crates/loom-executor/src/harness/turn_loop/tests.rs`.

## Left open

- While the provider's count is current, the trigger does not add what the turn appended after it
  (the model's reply and tool results), so a request can go out at or above the trigger as the
  provider would count it; the adversary's boundary pins (exactly 800 stops, 799 does not) hold the
  present behaviour.
- Tool results smaller than the elision stub (about 150 bytes) grow when elided, and the elision
  warning overstates what was freed (pre-existing, `turn_loop/mod.rs`, the tool-result elision).
