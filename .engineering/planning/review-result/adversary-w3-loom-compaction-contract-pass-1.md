---
format: aep.planning-md/3
id: review-result:adversary-w3-loom-compaction-contract-pass-1
kind: review-result
status: active
title: Wave 2026-10-06-w3 adversary, loom story:compaction-contract, pass 1
relations:
- reviews: story:compaction-contract
revision: 1
---
unit: story:compaction-contract, head b9efe09 plus one untracked test file
verdict: red (1 adversary case red; the defect predates this unit and I could not show anyone reaches it, so INFEASIBLE; I found nothing this unit introduced)
cases: executed 629→641, red 12 (1 adversary case; the other 11 are ESS-gate cases broken by the `ess` 0.54.0 install, not by this unit)
origin: introduced 0, pre-existing 1, undecided 0
wrote-outside-worktree: 6 paths (part 6)
needs-coordinator: yes. `ess` 0.54.0 was installed at 19:54:51, after the implementor's gate ran at 19:44. `ess/ess-inputs.yaml` (at base too) says `requires: ess 0.53.0`, so `--strict-requires` now refuses it, and 11 cases in `ess_gate`, `adversary_ess_gate` and `adversary2_ess_gate` fail at the validate step. Getting the gate green again means moving loom to ESS 0.54.0, which is outside this unit.

**1. What I changed**

`git --no-pager diff --stat` is empty. `git status --short` shows one path, and it is a test file:
```
?? crates/loom-executor/tests/adversary_w3_compaction_contract.rs
```

**2. Cases added** (in-process scripted model, no network)

| Case | Asserts | Now |
|---|---|---|
| `two_compactions_in_one_run_are_two_records_each_priced_by_its_own_request` (:70) | two records, distinct ids, each with its own summary request's usage | green |
| `a_summary_turn_with_no_text_is_recorded_with_the_usage_it_reported` (:157) | the empty-summary path keeps the reported usage; next request carries the rev-2 catalogue | green |
| `a_summary_turn_that_failed_on_the_wire_is_recorded_without_usage` (:185) | record has usage `None`; instruction unchanged | green |
| `a_summary_turn_reporting_no_usage_is_recorded_without_usage` (:201) | `None`, not an estimate | green |
| `a_compaction_that_only_elided_is_recorded_without_usage` (:216) | one record per `Compacted` event, none with usage | green |
| `a_resumed_session_keeps_its_first_compaction_and_records_a_second` (:283) | Filed→Active resume: 2 ids, turn indices 1..4 | green |
| `each_compaction_is_recorded_on_the_session_of_the_run_that_made_it` (:342) | two sessions on one Loom | green |
| `a_compaction_behind_the_agent_executor_port_is_recorded` (:393) | `LoopExecutor` path records too | green |
| `a_frontier_that_moves_during_the_summary_request_is_the_one_offered_next` (:422) | governor reads `[false,false,true]`; next request has the moved catalogue | green |
| `a_summary_imitating_instructions_tools_and_the_marker_gains_nothing` (:476) | forged instruction, tool spec and marker: no selection, tools = catalogue, summary is one user item | green |
| `the_record_is_there_when_the_callers_sink_sees_the_compaction` (:577) | the record exists before the event reaches the caller's sink; no deadlock | green |
| `a_summary_longer_than_the_target_leaves_the_session_above_half_the_window` (:624) | acceptance 1 when the summary is long | **red** |

Reading the existing tests: nothing else reaches the compaction numbering, `Summarised::Failed(reported)`, or the order of record before event. The unit's suite would stay green under mutants of those three paths, and the cases above now catch them.

Red output, case run alone first:
`cargo test -p b10x-loom-executor --locked --test adversary_w3_compaction_contract -- --exact adversary_w3_a_summary_longer_than_the_target_leaves_the_session_above_half_the_window`
```
thread '...above_half_the_window' panicked at crates/loom-executor/tests/adversary_w3_compaction_contract.rs:647:5:
acceptance 1: after compaction the session is 4292 tokens of a 4000 token window (14596 bytes before the compaction, 17170 after)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.01s
```

**3. Full suite** (run after the cases existed)

`cargo test -p b10x-loom-executor --locked --no-fail-fast` → `EXIT=101`; 50 targets, 629 passed, 12 failed, 1 ignored. A first run without `--no-fail-fast` stopped at `adversary2_ess_gate`.
```
error: 4 targets failed:
    `-p b10x-loom-executor --test adversary2_ess_gate`
    `-p b10x-loom-executor --test adversary_ess_gate`
    `-p b10x-loom-executor --test adversary_w3_compaction_contract`
    `-p b10x-loom-executor --test ess_gate`
```
Every ESS-gate failure is this error: `requires ess 0.53.0 and this is ess 0.54.0, which is newer, and --strict-requires refuses a newer release`.

On "before" = 629: `gate.log` adds up to 639, but that includes a second `ess_gate` run (10 cases) from `task ess-gate`. The package suite itself ran 629, which matches the implementor's report.

**4. Findings**

| file:line | verdict | origin | finding |
|---|---|---|---|
| `crates/loom-executor/src/harness/turn_loop/mod.rs:3092` | INFEASIBLE | pre-existing | The only check on the summary is that it isn't empty, so a long summary is folded in even when it makes the conversation bigger. |

- **Measured:** 14,596 bytes went to 17,170 after compaction, which is 4,292 tokens against a 4,000-token window. The request after it is still sent. Acceptance 1 fails.
- **What reaches it:** a model that writes more summary than it was asked to fold. I found no caller or configuration that produces this. The summary request is limited only by `max_output_tokens_per_turn`, which defaults to `None`.
- **Base:** 268d37b has the same guard and fold (base lines 3084/3103/3106).
- **Fix (not applied):** treat a summary that doesn't shrink the folded items as `Failed(reported)`, and/or limit the summary request's output tokens to the target.

**5. Attacked and could not break**
- Two compactions per run, resume, two sessions, and the executor port: records, ids, ordering and usage are all correct.
- Usage is never estimated. Both wires leave unreported usage absent (`usage_from_response`, `usage_from_message`).
- A compaction is never recorded on a session the run doesn't hold: the run's claim gates it, and refusals are unreachable.
- A frontier change during the summary request is picked up; the summary request never reads the governor.
- Model-written summary text cannot reach instructions, tools, catalogue, revision or selection.
- The ESS `Compaction` lifecycle (`Recorded` only, `Active` sessions only) matches the generated code and `TurnRecord`.
- `adversary2_harness_port` limits: 4/4 pass.

**6. Paths written outside the worktree**
- `~/.cache/loom-waves-2026-10-06/w3/compaction-contract/adversary-1/suite.log`
- `~/.cache/loom-waves-2026-10-06/w3/compaction-contract/adversary-1/suite-nofailfast.log`
- `~/.cache/loom-waves-2026-10-06/w3/compaction-contract/adversary-1/suite-nofailfast.counts`
- `~/.cache/loom-waves-2026-10-06/w3/compaction-contract/adversary-1/gate.log.targets`
- `~/.cache/loom-waves-2026-10-06/w3/compaction-contract/adversary-1/suite-nofailfast.log.targets`
- `~/.cache/loom-waves-2026-10-06/w3/compaction-contract/gate.log.counts`: written by mistake outside my assigned subdirectory, then deleted.
- Build directory: `~/.cache/b10x-target/loom-w3-compaction-contract` (assigned). The ESS-gate tests write copies under its `tmp/`.

```findings
[{"file":"crates/loom-executor/src/harness/turn_loop/mod.rs","line":3092,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"pre-existing","message":"summarise folds any non-empty summary, so a summary longer than the target leaves the session above 50% of the window (4292 of 4000 tokens measured) and contradicts acceptance 1; no caller shown to reach it"}]
```
