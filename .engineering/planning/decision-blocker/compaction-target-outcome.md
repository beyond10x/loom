---
format: aep.planning-md/3
id: decision-blocker:compaction-target-outcome
kind: decision-blocker
status: cleared
title: What a compaction does when it leaves the session above its target
relations:
- blocks: story:compaction-target-bound
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T23:16:39Z", actor: "human:timo", revision: 2}
---
## Question

What does a compaction do when it leaves the session above the target it compacts to (50 % of the
window)? Four paths do so on `main` at 5e3d0cb, all in
`crates/loom-executor/src/harness/turn_loop/mod.rs`: a summary shorter than the items it replaces
but above the target is kept (`:3174` compares against the replaced bytes, not the target); a
summary request that fails on the wire leaves the items (`:3129-3137`); an empty summary leaves the
items (`:3155-3162`); a fold judged too small or with no end is skipped (`:3092-3098`, treated as a
no-op at `:3024`).

Eliding cannot always reach the target: the task item (`FIRST_KEPT_ITEM`, `:997`), reasoning items
(`:3165-3169`), the protected tail (`:1217`), the newest item (`:1361`), and the instructions and
tool schemas counted in the provider's reported input (`:2989-2992`) are never removed.

## Options

- **A**: keep a summary only when the folded session is at or below the target, otherwise elide.
  A miss caused by the parts never removed stays silent, and a summary that beats elision is lost.
- **B**: any compaction that leaves the session above the target ends the run with a new
  `LoopStop` variant. Runs that complete today stop.
- **C**: elide on all four paths and keep the smaller of summary and elision; a new `LoopStop`
  variant ends the run only when the session is still above the trigger (80 %, `:947`) after
  eliding.

## Decided

Option C, 2026-10-09. Elide on all four paths and keep the smaller of the summary and the
elision. The run ends with a new, named `LoopStop` variant only when the session is still above
the 80 % trigger after eliding. `CHANGELOG.md` names the variant as a breaking change for callers
that match `LoopStop` exhaustively. The run's ending is `loom.run.RunEnding::Stopped`, and
`ess/domains/run.yaml` names the cause before the code changes.
