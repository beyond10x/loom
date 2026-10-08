---
format: aep.planning-md/3
id: review-result:adversary-w3-loom-moved-case-outcome-pass-1
kind: review-result
status: active
title: Wave 2026-10-07-w3 adversary, loom story:moved-case-outcome, pass 1
relations:
- reviews: story:moved-case-outcome
revision: 1
---
unit: story:moved-case-outcome, impl/moved-case-outcome at 3002c07 (base b8af5c2), plus 2 untracked adversary test files
verdict: NEEDS-CHANGE
cases: executed 824→827, red 3
origin: introduced 2 / pre-existing 0 / undecided 1
wrote-outside-worktree: none
needs-coordinator: decide F2's expected outcome (the `LoopExecutor` route) and F3 (a moved case whose current frontier admits an action ends `NoAdmissibleAction`); confirm F2's origin against the base, which I could not build without a second build directory

**1. `git --no-pager diff --stat`**: empty, so no tracked file changed. `git status --short` shows only my two new test files:
```
?? crates/loom-commission-testkit/tests/adversary_w3_moved_case.rs
?? crates/loom-executor/tests/adversary_w3_moved_case_loop.rs
```

**2. Cases added. Each was run alone first and was red on that first run.**

| Case | Asserts | Now |
|---|---|---|
| `adversary_w3_moved_case.rs:143` `adversary_w3_a_move_the_governor_does_not_hold_does_not_end_the_run` | The governor holds the case at 7 throughout, and its frontier admits `tests.run`. The executor returns `CaseMoved{7}` and then `CaseMoved{9}`, followed by `NeedsHumanJudgment`. Expected: the run goes on and ends `NeedsHumanJudgment` after 2 executor calls. | red |
| `adversary_w3_moved_case_loop.rs:380` `…completed_during_the_model_call_ends_completed_through_the_run_loop` | Uses the scenario `adversary_w1_runtime_stale.rs` flips, run through `LoopExecutor`: the case completes during the model's call. Expected: `Completed`. The pipeline-Loom reference and the stale-revision precondition both pass. | red |
| `adversary_w3_moved_case_loop.rs:417` `…moved_during_the_model_call_ends_on_its_current_obligation_through_the_run_loop` | Same scenario, but the case moves past `tests-pass` to `review-approved`. Expected: `NeedsExternalEvidence(["review-approved"])`. The pipeline reference and the precondition both pass. | red |

Red output, verbatim:
```
CaseMoved naming revision 7, the governor holding the case at 7 throughout with `tests.run` admissible: the run ended NoAdmissibleAction(Unit(true)) after 1 executor call(s), expected NeedsHumanJudgment(...) after 2. ...
CaseMoved naming revision 9, ... the run ended NoAdmissibleAction(Unit(true)) after 1 executor call(s), expected NeedsHumanJudgment(...) after 2. ...
test result: FAILED. 0 passed; 1 failed   EXIT=101
LoopExecutor: NoAdmissibleAction(Unit(true)), expected Completed(RunOutcomeCompleted { outcome: "done" }): Loom refused the selection stale-revision, denied the CaseMoved to the model, and Commission judged the run on the frontier the case left
LoopExecutor: NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["tests-pass"] }), expected NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["review-approved"] }): ...
test result: FAILED. 0 passed; 2 failed   EXIT=101
```

**3. Suite run, after the cases existed.** Command: `CARGO_INCREMENTAL=0 cargo test -p b10x-loom-commission-testkit -p b10x-loom-executor --locked --no-fail-fast`. Result: 104 result lines, passed=824, failed=3, ignored=1, EXIT=101. The only failures are my 3 cases. The `before` figure of 824 comes from this same run with my two binaries left out; I did not run the suite before writing the cases. Clippy on both packages with `--all-targets -D warnings`: EXIT=0.

**4. Findings** (they cover 3002c07)

- **F1. The runtime believes the executor's claim of a move, not the governor.**
  - What fails: `crates/loom-commission/src/runtime.rs:500` sends every `CaseMoved` to `moved_outcome` (`:630`). That function never compares the revision it loads (`:641`) with the revision the Run holds. When the governor shows no move and the frontier admits an action, `Derived::Continue` becomes `NoAdmissibleAction` (`:658`). That outcome is false here.
  - It contradicts the unit's own texts: the ESS comment ("judges the run on them", meaning the governor's revision and frontier) and item 9's premise, "the Run holds the case at the revision it left".
  - What reaches it: any `AgentExecutor`, through the SDK re-export. Loom itself returns `CaseMoved` when its governor's frontier is older than the handed one (`adversary_w1_selection_boundaries.rs:182`, the unit's own flipped assertion). No in-repo caller combines `with_governor` with `run_until_blocked`.
  - Origin: introduced. Proposed fix: pass `held` into `moved_outcome`, and when `loaded == held` treat the outcome as `NoUsefulAction`.
- **F2. Loom's run loop still judges a moved case on the frontier it left.**
  - What fails: `crates/loom-executor/src/harness/governed.rs:874` turns `CaseMoved` into a denial to the model. When the model then answers in prose, Commission receives `CompletedLocalReasoning` and judges the run on the frontier the case left. So the decision's "Loom reports its stale-revision refusal through that variant" does not hold for `run_loop`. On the flipped suites' own scenario the outcomes are `NoAdmissibleAction` instead of `Completed`, and `["tests-pass"]` instead of `["review-approved"]`.
  - What reaches it: `LoopExecutor` is a public `AgentExecutor`; nothing in the repo composes it with `run_until_blocked`.
  - Origin: undecided. I did not build the base. From the code, the base `answer` (line 871) denied `NoUsefulAction` the same way, so the outcome is probably unchanged.
- **F3. A moved case whose current frontier admits an action still ends `NoAdmissibleAction`.** This is at `runtime.rs:658`, and `moved_case_outcome.rs` asserts it on purpose. It is the ambiguity the blocker was filed for ("reads the same as an empty frontier"). The acceptance line "ends on the obligation its current frontier holds" only holds when that frontier admits nothing.
  - The stale-proposal path the decision cites (ungoverned half of `adversary_w1_runtime_stale.rs`) ends `NoAdmissibleAction` for the same move. So whether the executor reported the move or the runtime found it changes the outcome. The implementor already flagged this.
  - Origin: introduced. Severity: note.

**5. Attacked, not broken.** Each was a probe case that passed; I deleted the probe file afterwards (log: `probes.log`).
- The case moves again during the reload: it ends `NoAdmissibleAction` if open, and `Completed` only when the governor says complete.
- The reload finds the case open with an empty frontier: not `Completed`.
- The governor fails during the reload: the result is a `LoopError` and the Run is `Suspended` with `ExternalAvailability`.
- A budget of 1: the `CaseMoved` step is not counted (`steps=0`), and the run ends on the reload.
- No other path ends `Completed` without the governor saying complete.
- The approval gate and idle rule cannot interact: `CaseMoved` returns before both.
- The handed-frontier filter on the reload is covered by the unit's own test 1.
- The observation payload is pinned by the unit's own test.
- The flipped assertions are strict equalities on the decided outcomes, not relaxed. The fixture gained `after` obligations, which makes the check stricter.
- Untested guard: the `frontier ≠ loaded` check at `runtime.rs:648` is only visible when the frontier at the newer revision admits nothing and has open obligations. I added no case for it.

**6. Paths written outside the worktree:** none. Logs are in `<worktree>/.engineering/drafts/scratch/adversary-1/`, and builds went to the tree's own `target/`. I acquired a session lease for this tree and released it.

```findings
- file: crates/loom-commission/src/runtime.rs
  line: 500
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: a CaseMoved the governor does not bear out (its revision equals the Run's) ends the run NoAdmissibleAction on a frontier that admits an action, trusting the executor's claim over the governor
- file: crates/loom-executor/src/harness/governed.rs
  line: 874
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: undecided
  message: through LoopExecutor a stale-revision refusal is denied to the model instead of reported, so the flipped suites' scenario still ends NoAdmissibleAction or NeedsExternalEvidence for the superseded tests-pass instead of Completed or review-approved
- file: crates/loom-commission/src/runtime.rs
  line: 658
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: a moved case whose current frontier admits an action ends NoAdmissibleAction, the empty-frontier ambiguity the decision-blocker was filed for
```
