---
format: aep.planning-md/3
id: review-result:adversary-w1-20261008-loom-moved-run-named-outcome-pass-1
kind: review-result
status: active
title: Wave 2026-10-08-w1 adversary, loom story:moved-run-named-outcome, pass 1
relations:
- reviews: story:moved-run-named-outcome
revision: 1
---
## Adversary pass 1, story:moved-run-named-outcome (wave 2026-10-08-w1)

Attacked `impl/moved-run-named-outcome` at 18cc585 (unit commits 8cb5cb7..190d685, plus the adversary's test commit).

- verdict: CONFIRMED, one red case, pre-existing
- cases: executed 1013→1018, red 1
- origin: introduced 0 / pre-existing 1 / undecided 0

Cases added in `crates/loom-commission-testkit/tests/adversary_w1_moved_run_named_outcome.rs`:

| case | asserts | state |
|---|---|---|
| adversary_a_stale_proposal_after_an_own_effect_is_bound_to_the_runs_start_revision | own effect moves 5→6, a stale proposal finds 7; `CaseMovedOn{5,7}` | green |
| adversary_a_reported_move_after_an_own_effect_is_bound_to_the_runs_start_revision | same, executor reports `CaseMoved`; `CaseMovedOn{5,7}` | green |
| adversary_proposing_nothing_on_a_moved_case_ends_case_moved_on_bound_to_the_runs_start | empty frontier at 6, nothing proposed, case at 7; `CaseMovedOn{5,7}` | green |
| adversary_a_found_move_whose_frontier_is_for_another_revision_admits_nothing | load says 8, frontier read says 9; `NoAdmissibleAction` | green |
| adversary_a_stale_proposal_on_the_last_budgeted_step_ends_case_moved_on | `step_budget = Some(1)`, the only step is stale; `CaseMovedOn{5,6}` | red |

Red output (`cargo test -p b10x-loom-commission-testkit --locked --test adversary_w1_moved_run_named_outcome`, exit 101):

```
  left: Suspended(RunOutcomeSuspended { reason: Budget(Object([("max_steps", Number("1"))])) })
 right: CaseMovedOn(RunOutcomeCaseMovedOn { bound_case_revision: 5, current_case_revision: 6 })
test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

Suite: `cargo test -p b10x-loom-commission-testkit -p b10x-loom-commission -p b10x-loom-intake-slice -p b10x-loom-executor -p b10x-loom-sdk --locked --no-fail-fast` exit 101, 1017 passed, 1 failed (the red case). `task commission:drift`, `task commission:docs-drift`, `task commission:conform`, `cargo fmt --all --check`, clippy on testkit `-D warnings`: exit 0.

Attacked, not broken: the 5 re-pinned cases swap only the variant and add both revisions, no assertion dropped; `found_moved` judges the frontier as handed and requires it to be for the loaded revision; Completed and NeedsExternalEvidence unchanged; the intake slice handles the variant as NothingAdmissible with a stop line naming both revisions; the SDK example's catch-all covers it; generated code and docs show no drift.

Noted, not raised: the stale path and the reported-move path differ for a moved frontier of only unperformable actions (NoAdmissibleAction vs NoPerformableAction) and for an open obligation (NoAdmissibleAction vs NeedsExternalEvidence); both predate this unit, the second is pinned by `crates/loom-executor/tests/adversary_w1_runtime_stale.rs:312`.

```findings
- file: crates/loom-commission/src/runtime.rs
  line: 455
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: a stale proposal on the last budgeted step ends Suspended(Budget) instead of CaseMovedOn because the budget check precedes the load that finds the move
```
