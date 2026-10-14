---
format: aep.planning-md/3
id: review-result:adv-w11-u3-pass-1
kind: review-result
status: active
title: Adversary review, loom w11 U3 typed step outputs, pass 1
relations:
- reviews: story:dw-typed-step-outputs
revision: 1
---
needs-revision

unit: U3 story:dw-typed-step-outputs, branch impl/dw-typed-step-outputs, implementation 1331b75; findings cover tree at 079b4b4 (1331b75 + the adversary test file)
verdict: NEEDS-CHANGE
cases: executed 0→25 (crates/loom-executor/tests/adversary_w11_u3.rs only; the brief restricts cargo to `--test adversary_w11_u3`, so the implementor's 7 cases in workflow_step_outputs.rs were not rerun here), red 1
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 (this review file)
needs-coordinator: none

## 1. Diff stat (1331b75..079b4b4)

    crates/loom-executor/tests/adversary_w11_u3.rs | 1202 ++++++++++++++++++++++++
    1 file changed, 1202 insertions(+)

Only a test file. Commit 079b4b4 `test: adversarial cases for a workflow step's typed outputs`.

## 2. Cases (crates/loom-executor/tests/adversary_w11_u3.rs)

Red (1):

- `the_last_steps_outputs_are_kept_when_the_governor_completes_the_case`: RED. The governor completes
  the case once `tests.run` is performed. The workflow's only step declares `passed: Boolean`, and its
  report holds `{"passed": true}`. The run ends `Completed` and `WorkflowExecutor::outputs()` is
  empty. Outputs are taken only on the executor's *next* call (`workflow.rs:585-591`), not when the
  effect is recorded, so a run that ends without another call never takes them. For the same
  reason a mistyped output on that last step is never reported. Captured output:

      thread 'the_last_steps_outputs_are_kept_when_the_governor_completes_the_case' panicked at crates/loom-executor/tests/adversary_w11_u3.rs:1192:5:
      assertion `left == right` failed: the performed step's outputs were never taken
        left: []
       right: [("test", "passed", Bool(true))]

Green (24). Each passed against 1331b75:

- type confusion: `an_integer_with_a_fraction_proposes_nothing` (1.5, [1, 2.5]),
  `a_whole_number_is_an_integer_and_keeps_its_spelling` (42, -3),
  `a_string_spelling_another_type_is_not_that_type`, `a_list_holds_only_items_of_its_type` (mixed,
  scalar, null item, empty list, JsonList mixed, object as JsonList),
  `null_is_present_but_typed_only_as_json`
- pointers: `pointer_escapes_are_unescaped_in_order` (`~01` is key `~1`),
  `the_empty_pointer_is_the_whole_report`, `a_lone_slash_is_the_empty_key`,
  `array_indices_follow_rfc_6901` (01, -, past the end, -1, +1),
  `a_numeric_key_is_a_member_of_an_object`, `a_malformed_escape_proposes_nothing`
- placement: `a_reference_deep_inside_the_arguments_is_resolved_in_place`,
  `a_resolved_value_shaped_like_a_reference_is_not_resolved_again`,
  `a_models_arguments_shaped_like_a_reference_are_not_resolved`,
  `a_judgement_steps_output_never_carries_the_models_value`
- provenance: `outputs_come_from_the_performed_attempt_after_a_refusal`,
  `two_steps_with_the_same_request_each_read_their_own_report`,
  `an_output_from_an_earlier_executors_run_is_not_this_runs` (a new executor sharing the old log)
- Commission still decides: `the_step_after_a_resolved_step_is_reached`,
  `a_resolved_request_is_still_gated_by_authority` (NeedsAuthority, request carries the resolved
  branch), `a_resolved_step_the_frontier_blocks_proposes_nothing`
- load: `a_reference_cycle_is_refused_at_load` (self and two-step), 
  `a_reference_to_an_undeclared_output_of_any_step_kind_is_refused`
- determinism: `outputs_are_identical_and_in_declared_order_across_reruns`

The first run of the file showed 22 red. That was my fixture's fault, not a finding: my governor
always listed `repository.merge` as approval-required, so Commission's approval gate ended runs
`AwaitingApproval`. I fixed the fixture to list the merge only in the authority case, after the
inspect, and the run after the fix gave the result above.

## 3. Suite run

    cd <tree> && CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=8 conductor build run --reserve 3G --repository loom -- cargo test -p b10x-loom-executor --locked --test adversary_w11_u3
    running 25 tests
    test result: FAILED. 24 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
    error: test failed, to rerun pass `-p b10x-loom-executor --test adversary_w11_u3`

(rustfmt was applied to the file after this run, with no change in meaning.)

## 4. Findings

| # | file:line | finding | verdict | origin | severity | what reaches it |
|---|---|---|---|---|---|---|
| 1 | crates/loom-executor/src/workflow.rs:585 | Outputs are taken when the executor is next called, not when `RecordingEffects` records the `Performed` effect, as the story's design says. A step performed last before the governor completes the case never has its outputs taken, and a mistyped output there is never reported. Fix: extract in `RecordingEffects::invoke`, or take the outputs when the effect is recorded and keep them in the log | NEEDS-CHANGE | introduced | warning | Commission's loop loads the case after every performed effect and ends the run `Completed` without calling the executor (runtime.rs item 1); measured by the red case at tests/adversary_w11_u3.rs:1192 |
| 2 | ess/domains/workflow.yaml (loom.workflow.JudgementStep) | The design says "a judgement step may list outputs to put into its instructions as data". Neither ESS nor `workflow.rs` models it, and the acceptance does not test it | CONFIRMED | introduced | warning | the story's `## Design` bullet 2; nothing implements it |
| 3 | crates/loom-executor/src/workflow.rs:310 | A judgement step may declare outputs, read from the report of an effect whose arguments the model wrote. A port that echoes its arguments into its report (a write that returns its path) carries a model-written value into a deterministic step's arguments. The CHANGELOG also notes that `$step`/`$output` member names in literal arguments can no longer be written, and there is no escape for them | INFEASIBLE | introduced | note | no echoing effect port found in the repository; constructed only |

## 5. Attacked and not broken

Integer, Boolean, String, list and null type confusion; RFC 6901 escapes and their order; the
empty pointer and `/`; array indices with a leading zero, `-`, a sign or past the end; a reference
deep inside the arguments; double resolution; a model's reference-shaped arguments; outputs after
a refusal; same-request steps; a log carried over from an earlier executor; authority and frontier
gating of a resolved request; advancing past a resolved step; self and cyclic references;
determinism and declared order across reruns. The implementor's tests in workflow_step_outputs.rs
assert literal values; I found no test that cannot fail.

## 6. Paths written outside the worktree

- ~/.cache/loom-w18/reviews/adv-w11-u3-pass-1.md (this file)
- ~/.cache/loom-w18/reviews/adv-w11-u3-commit-msg.txt (commit message draft, deleted after the commit)

## 7. Findings block

```findings
- file: crates/loom-executor/src/workflow.rs
  line: 585
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: outputs are taken on the executor's next call instead of when RecordingEffects records the performed effect, so a step performed last before the governor completes the case never has its outputs taken or type-checked
- file: ess/domains/workflow.yaml
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the design's judgement-step outputs listed into instructions as data are neither specified in ESS nor implemented
- file: crates/loom-executor/src/workflow.rs
  line: 310
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: a judgement step's declared outputs read the report of a model-argued effect, so a port that echoes its arguments passes a model-written value into a deterministic step, and literal $step/$output member names have no escape
```
