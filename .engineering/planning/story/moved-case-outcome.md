---
format: aep.planning-md/3
id: story:moved-case-outcome
kind: story
status: implemented
title: An executor reports a moved case and the run is judged on the current frontier
relations:
- decomposes: epic:commission-core
- serves: vision:O1
- depends_on: story:ess-055-upgrade
- depends_on: story:effect-invocation
scope:
- confidence: cited
  path: crates/loom-commission-testkit/tests/executor_port.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/moved_case_outcome.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/runtime_loop.rs
- confidence: cited
  path: crates/loom-commission/src/outcome.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-executor/src/harness/governed.rs
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w1_runtime_stale.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w1p2_runtime_windows.rs
- confidence: cited
  path: docs/commission/contracts/commission-executor.md
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: generated/rust/commission/
revision: 15
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T07:33:43Z", actor: "human:timo", revision: 4, executor: "agent:loom", correlation: "wave/2026-10-07-w3"}
- {from: "proposed", to: "active", at: "2026-10-07T07:33:43Z", actor: "human:timo", revision: 5, executor: "agent:loom", correlation: "wave/2026-10-07-w3"}
- {from: "active", to: "implemented", at: "2026-10-08T00:33:49Z", actor: "human:timo", revision: 15, decided_on: {"recorded":{"test_result":1,"review_outcome":8,"verification":1}}}
---
## Outcome

An executor can report that the case moved under it, and the Commission runtime then judges the run
on the case's current frontier. This applies the decision of 2026-10-07 on
`decision-blocker:run-stale-outcome` (option C).

Today a Loom built with `Loom::with_governor` refuses a selection made at a revision the case has
since left (`stale-revision`) and returns `NoUsefulAction`; `run_until_blocked` then derives the run
outcome from the frontier the case left. Measured in
`crates/loom-executor/tests/adversary_w1_runtime_stale.rs` and
`crates/loom-executor/tests/adversary_w1p2_runtime_windows.rs`: a case completed while the selector
selects or while arguments are generated ends `NoAdmissibleAction` instead of `Completed`, and a
case that moved past an open obligation ends `NeedsExternalEvidence(["tests-pass"])` for the
superseded obligation.

After this story:

1. `commission.responsibility.ExecutorOutcome` has a variant by which an executor reports that the
   case moved, naming the revision it worked at.
2. On that variant `run_until_blocked` reloads the case and its frontier, as it does for a stale
   proposal (item 8 of its module documentation), and derives the outcome from the current frontier.
3. Loom returns that variant for its `stale-revision` refusal instead of `NoUsefulAction`.

## ESS first

- **Specification change (first commit, `ess/commission/domains/responsibility.yaml` only):** the
  new `ExecutorOutcome` variant and its payload (the revision the executor worked at). Modelled with
  `ess:specifying`, validated with the newest `ess` (`--strict-requires`).
- **Red on that commit:** `task commission:drift` fails against the committed
  `generated/rust/commission/`.
- **Then:** `task commission:generate`, the runtime reload, Loom's mapping, and the flipped
  assertions below.

## Acceptance

- `adversary_w1_runtime_stale.rs` and `adversary_w1p2_runtime_windows.rs` assert the outcomes this
  decision gives instead of today's: a case completed while the selector selects, or while
  arguments are generated, ends `Completed`; a case that moved past an open obligation ends on the
  obligation its current frontier holds, not `NeedsExternalEvidence(["tests-pass"])`. Both suites
  pass and no longer name `decision-blocker:run-stale-outcome` as open.
- A new case in `crates/loom-commission-testkit/tests/` drives `run_until_blocked` with a scripted
  executor that returns the new variant and a fake governor that moved the case: the runtime reloads
  once and derives from the current frontier.

## Scope

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`
- `crates/loom-commission/src/runtime.rs` (the reload after the new variant)
- `crates/loom-executor/src/` (the `stale-revision` mapping; inferred: `src/selection.rs` or the
  executor's outcome conversion)
- `crates/loom-executor/tests/adversary_w1_runtime_stale.rs`,
  `crates/loom-executor/tests/adversary_w1p2_runtime_windows.rs`
- `crates/loom-commission-testkit/tests/` (one new case; inferred file)

## Shared surface

Edits `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/` and
`crates/loom-commission/src/runtime.rs` like `story:effect-invocation`, so the two do not share a
wave; it depends on nothing that story adds.

## Source

`decision-blocker:run-stale-outcome` (its Options, the wave 2026-10-06-w1 measurements and the
decision); `review-result:adversary-w1-loom-selection-revalidation-pass-1` (F1) and pass 2 (D2).

## Scope confirmed

Read from `git diff --stat b8af5c2 eb10aa3` (the unit's five commits, 30 files) at the close of wave
2026-10-07-w3. Corrections to the `## Scope` section above:

| Scope line | Confidence then | What the unit changed |
|---|---|---|
| `crates/loom-executor/src/` | inferred (`src/selection.rs` or the outcome conversion) | `src/lib.rs` (the stale-revision mapping) and `src/harness/governed.rs` (`LoopExecutor`'s move rule); `src/selection.rs` unchanged |
| `crates/loom-commission-testkit/tests/` | inferred (one new case) | `moved_case_outcome.rs` (new), `adversary_w3_moved_case.rs` (new, adversary pass 1), `executor_port.rs`, `runtime_loop.rs`, `skeleton.rs`, `adversary2_executor_contract_doc.rs` |
| `crates/loom-commission/src/runtime.rs` | cited | as cited; also `src/outcome.rs` |
| `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/` | cited | as cited |
| not listed | — | `docs/commission/contracts/commission-executor.md`, `website/docs/reference/commission/types.mdx` (generated), `crates/loom-executor/tests/adversary_w1_selection_boundaries.rs`, `adversary_w1p2_selection_edges.rs`, `selection_revalidation.rs`, and the adversary files `adversary_w3_moved_case_loop.rs`, `adversary2_w3_moved_case_loop.rs`, `adversary3_w3_moved_case_loop.rs` |
