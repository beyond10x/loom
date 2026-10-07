---
format: aep.planning-md/3
id: review-result:adversary-w3-loom-moved-case-outcome-pass-2
kind: review-result
status: active
title: Wave 2026-10-07-w3 adversary, loom story:moved-case-outcome, pass 2
relations:
- reviews: story:moved-case-outcome
revision: 1
---
unit: story:moved-case-outcome, impl/moved-case-outcome at a5b2e2e (base b8af5c2), plus 1 untracked adversary test file
verdict: NEEDS-CHANGE
cases: executed 827→831, red 4
origin: introduced 1 / pre-existing 0 / undecided 2
wrote-outside-worktree: none
needs-coordinator: (1) confirm the origin of F4 and F5 against the base, which I could not build without a second build directory; (2) decide what F5 should return: `CaseMoved` for a proposal selected at a revision other than the handed one, or catalogues projected from the handed frontier

**1. `git --no-pager diff --stat`**: empty, so no tracked file changed. `git status --short` shows only my new test file:
```
?? crates/loom-executor/tests/adversary2_w3_moved_case_loop.rs
```

**2. Cases added.** I wrote all four before anything ran, then ran the file alone. All four were red on that first run, and they are still red now (they assert at `:486` after rustfmt). Each case first runs the pipeline Loom (`Loom::run`) on the same scenario. That reference reaches the decided outcome every time, and every precondition held.

| Case (`adversary2_w3_moved_case_loop.rs`) | Scenario | Expected | Measured |
|---|---|---|---|
| `:495` `…completed_before_the_first_turn…answers_in_prose` | The case completes after Commission reads revision 7 and before the loop's first turn. The loop offers the catalogue of 8, which is empty. The model answers in prose. | `Completed` | `NeedsExternalEvidence(["tests-pass"])` |
| `:525` `…moved_before_the_first_turn…answers_in_prose` | Same window. The case moves on to `review-approved`; the model answers in prose. | `NeedsExternalEvidence(["review-approved"])` | `NeedsExternalEvidence(["tests-pass"])` |
| `:557` `…moved_before_the_first_turn…when_the_model_calls` | Same window. The model calls `repository_edit` from the catalogue of 8. | `review-approved` | `NoAdmissibleAction` |
| `:595` `…stale_refusal_then_a_call_from_the_next_catalogue…` | The case moves during turn 1, and that turn's call is refused `stale-revision`. On turn 2 the model calls again from the catalogue of 8. | `review-approved` | `NoAdmissibleAction` |

Red output of the first run, verbatim:
```
LoopExecutor: NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["tests-pass"] }) after 0 step(s), 0 request(s), expected Completed(RunOutcomeCompleted { outcome: "done" }): the loop offered the catalogue of the revision the case moved to, and the run was judged on the frontier it left. Catalogues offered at revisions [8]; revalidations []
LoopExecutor: NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["tests-pass"] }) after 0 step(s), 0 request(s), expected NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["review-approved"] }): ... Catalogues offered at revisions [8]; revalidations []
LoopExecutor: NoAdmissibleAction(Unit(true)) after 1 step(s), 1 request(s), expected NeedsExternalEvidence(... ["review-approved"] ...): Loom proposed a selection admitted at the revision the case moved to, to a Run that holds the case at the revision it left. Catalogues offered at revisions [8]; revalidations [Admitted {...}]
LoopExecutor: NoAdmissibleAction(Unit(true)) after 1 step(s), 1 request(s), expected NeedsExternalEvidence(... ["review-approved"] ...): ... Catalogues offered at revisions [7, 8]; revalidations [StaleRevision { selection_stale: SelectionStale { ..., catalogue_revision: 7, case_revision: 8 } }, Admitted {...}]
test result: FAILED. 0 passed; 4 failed   EXIT=101
```

**3. Suite run, after the cases existed.**
- Command: `CARGO_INCREMENTAL=0 cargo test -p b10x-loom-commission-testkit -p b10x-loom-executor --locked --no-fail-fast`
- Result: 105 result lines, passed 827, failed 4, ignored 1, EXIT=101. The only failures are my four cases.
- The `before` figure of 827 is this same run with my binary left out.
- Pass 1's three cases are green, so nothing regressed.
- `rustfmt --check` on my file: EXIT=0. `cargo clippy -p b10x-loom-executor --all-targets --locked -- -D warnings`: EXIT=0.

**4. Findings** (they cover a5b2e2e)

- **F4. A run that ends without a proposal is reported as moved only after a stale-revision refusal.**
  - What fails: `crates/loom-executor/src/harness/governed.rs:1173` (`if stale_refused`).
  - The loop projects every turn's catalogue from the governor's current frontier, not from the frontier it was handed. So a run can be offered revision 8 with no refusal at all. `LoopExecutor` then answers `CompletedLocalReasoning`, and Commission judges the run on revision 7. A case the governor holds complete ends asking for evidence for `tests-pass`, which is the exact defect the story names.
  - It contradicts the unit's own contract text, `docs/commission/contracts/commission-executor.md:58`: "An executor that finds the case has moved … returns `CaseMoved`".
  - What reaches it: the case moves between Commission's frontier read (runtime item 3) and the loop's first catalogue refresh. A completed case's frontier lists no actions, so the model cannot call anything and prose is its only possible answer. As with pass 1's F2, nothing in the repo composes `LoopExecutor` with `run_until_blocked`.
  - Origin: undecided. From reading the base code, the base `LoopExecutor::run` returned the same outcome, but I did not run it.
- **F5. A proposal from a newer catalogue goes to a Run that holds the older revision.**
  - What fails: `governed.rs:1177` (`outcome => outcome`) passes on a `ProposedAction` that was selected and admitted at revision 8. Commission binds it to revision 7, finds it `Stale`, and the next iteration ends `NoAdmissibleAction` instead of on `review-approved`.
  - What reaches it: the denial after a stale refusal tells the model "choose from the tools of the next turn", and that turn's catalogue lists the action again. A model that follows the instruction takes this path. The a5b2e2e correction only covers a model that gives up and answers in prose.
  - Named fix (not applied): in `governed()`, compare each offered catalogue's `case_revision` with the handed frontier's. `LoopExecutor::run` would then answer `CaseMoved(handed)` for any ending other than `Suspended` when a catalogue was offered at another revision. That one change covers both F4 and F5.
  - Origin: undecided, for the same reason as F4.
- **F6. The resume path records a stale refusal that nothing ever reads.**
  - What fails: `governed.rs:948` sets `stale_refused` inside `resolve`. That code runs only when `resuming` is set, which happens only with `Start::Checkpoint`, and only `resume_loop` uses that (`recovery.rs:240`). `resume_loop` throws the flag away (`governed.rs:198`, `.0`).
  - So a resumed run whose case moved still returns `CompletedLocalReasoning` or `NoUsefulAction`. The mapping lives in `LoopExecutor`, not in `governed`.
  - What reaches it: nothing in the repo wraps `resume_loop` in an `AgentExecutor`, so this is INFEASIBLE.
  - Origin: introduced (the line is new in a5b2e2e).

**5. Attacked, not broken.** Each was a probe case that passed. I deleted the probe files afterwards; the logs are `probes-f1.log` and `probes-f2.log`.

| Attack | Result |
|---|---|
| F1: idle bound, two non-borne `CaseMoved` in a row | Same as two `NoUsefulAction`: `NoAdmissibleAction` after 2 calls, steps=1 |
| F1: budget of 1 | `Suspended Budget{max_steps:1}` with steps=1, the same as `NoUsefulAction` |
| F1: approval gate | `AwaitingApproval(["repository.merge"])`, the same as `NoUsefulAction` |
| F1: governor fails on the reload that decides the move | `LoopError Governor(GovernorUnavailable)`, Run suspended `ExternalAvailability` |
| F1: after the Run's own effect moved the case 7→8 | `CaseMoved` naming 7 or 8 is compared with the held revision 8, not borne out, and the run goes on to `NeedsHumanJudgment` (3 calls) |
| F1: case moved and came back | Commission's only read after the executor is the reload, so this is the same as pass 1's `HELD+2` case (green) |
| F2: two stale refusals (7→8→9), then prose | `review-approved`; the observation names `expected_case_revision` 7 |
| F2: stale refusal, then interrupt | `CaseMoved(7)` → `review-approved`; the session stays `Interrupted` |
| F2: stale refusal, then prose or a call, on a completed case | `Completed` |
| Which revision `CaseMoved` names | Both the pipeline and the loop name 7, the handed revision, in every probe |
| Other consumers of `ExecutorOutcome` | `loom-intake-slice` `LoomStep` has no governor, so it never sees `CaseMoved` |

**6. Paths written outside the worktree:** none.
- Logs are in `<worktree>/.engineering/drafts/scratch/adversary-2/`.
- `TMPDIR` pointed at `<worktree>/.engineering/drafts/scratch/adversary-2/tmp`.
- Builds went to the tree's own `target/`, with no downloads.
- I took a session lease on the tree (`adversary2-w3-moved`) and released it.

Case file: `~/.local/state/worktree/trees/b10x/loom/loom-20261007-w3-moved/crates/loom-executor/tests/adversary2_w3_moved_case_loop.rs`

```findings
- file: crates/loom-executor/src/harness/governed.rs
  line: 1173
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: undecided
  message: LoopExecutor reports CaseMoved only after a stale-revision refusal, so a run offered a catalogue at a revision other than the handed one that ends in prose is judged on the frontier the case left, and a complete case ends NeedsExternalEvidence(["tests-pass"])
- file: crates/loom-executor/src/harness/governed.rs
  line: 1177
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: undecided
  message: a proposal the model selects from a turn catalogue at the revision the case moved to, after a stale refusal or on an already-stale start, is proposed to a Run bound to the old revision and ends NoAdmissibleAction instead of on the current obligation review-approved
- file: crates/loom-executor/src/harness/governed.rs
  line: 948
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: the stale_refused write on the resume path is never read, because resume_loop discards the flag, so a resumed run whose case moved is never reported as CaseMoved
```
