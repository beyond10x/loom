---
format: aep.planning-md/3
id: review-result:adversary-w2-loom-effect-invocation-pass-2
kind: review-result
status: active
title: Wave 2026-10-07-w2 adversary, loom story:effect-invocation, pass 2
relations:
- reviews: story:effect-invocation
revision: 1
---
unit: story:effect-invocation, branch impl/effect-invocation at 09c1c9e, plus one untracked adversary test file
verdict: NEEDS-CHANGE
cases: executed 157→160, red 2
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 (my worktree session lease record, acquired, refreshed and released through `worktree hook`)
needs-coordinator: finding 1 depends on how decision B is read. Does a `Blocked` unbound action that names a capability "need authority"? Either the filter changes or `website/docs/guides/run-an-intent.md:57` does, and that guide is the coordinator's file this wave.

**1. `git --no-pager diff --stat`:** empty, so no tracked file changed. `git status --short` shows one untracked file, `crates/loom-commission-testkit/tests/adversary2_w2_effect_invocation.rs`, which is a test file. Nothing outside the charter was touched.

**2. Cases added** (in `adversary2_w2_effect_invocation.rs`). This is the first run of the file alone, verbatim (`red-alone.log`):
```
test a_performed_effect_naming_no_attempt_is_observed_without_an_attempt ... ok
test the_merge_the_slice_lists_blocked_stays_in_the_frontier_the_executor_sees ... FAILED
test an_action_never_offered_does_not_take_the_run_off_its_approval_gate ... FAILED
---- the_merge_the_slice_lists_blocked_stays_in_the_frontier_the_executor_sees stdout ----
panicked at .../adversary2_w2_effect_invocation.rs:261:9:
the executor was handed a frontier without the gated `repository.merge` the governor lists blocked: [("repository.edit", Admissible), ("repository.inspect", Admissible), ("tests.run", Admissible)]
---- an_action_never_offered_does_not_take_the_run_off_its_approval_gate stdout ----
panicked at .../adversary2_w2_effect_invocation.rs:346:5:
  left: (NoAdmissibleAction(Unit(true)), 2)
 right: (AwaitingApproval(RunOutcomeAwaitingApproval { actions: ["service.deploy"] }), 1)
test result: FAILED. 1 passed; 2 failed
EXIT=101
```

| Case (line) | What it asserts | Now |
|---|---|---|
| `the_merge_the_slice_lists_blocked…` :229 | On the slice's frontier (merge `Blocked`, capability `repository.merge`; the port performs inspect, edit and tests.run), the executor is handed the merge | red |
| `an_action_never_offered_does_not_take…` :284 | A change only in an action the executor is never offered leaves the outcome and the executor-call count the same as a control run without that action | red |
| `a_performed_effect_naming_no_attempt…` :361 | A `Performed` with `attempt: None` is observed with no `attempt` member | green on the tree, red on the mutant (finding 3) |

**3. Suite run** (after the cases existed): `cargo test -p b10x-loom-commission-testkit --locked --no-fail-fast` gave EXIT=101. It executed 160: 158 passed, and the 2 failures are both mine. A second run with `adversary2_w2_effect_invocation` deselected (`--lib` plus the 84 other `--test` targets) executed 157 with EXIT=0. Pass 1's `adversary_w2_effect_invocation` passed 5/5, so none of pass 1's findings regressed.

**4. Findings** (they cover 09c1c9e plus my file)

| # | file:line | Measured | What reaches it | Verdict / origin |
|---|---|---|---|---|
| 1 | `crates/loom-commission/src/runtime.rs:613` | The filter keeps an unperformed action only when one of its entries is `ApprovalRequired`. A `Blocked` merge that names its capability is dropped (case :229, red). Decision B says "`repository.merge` on the local slice, stays in the frontier an executor sees". The story's filter removes only actions "that need no authority". Suggested fix: keep an unperformed action when any entry names a non-empty capability. | Every `b10x-loom run` on `software-change@1` before the tests pass. The governor lists merge `Blocked` with that capability (`crates/loom-governor/tests/governed_case.rs` asserts both), and `LocalEffects::performs` excludes merge (`effect.rs:202`). The next two points come from reading the code; I did not run them. The slice prints the frontier it was handed (`run.rs:505`), so the documented `repository.merge (blocked)` line (`run-an-intent.md:57`) disappears. A model that picks merge early is now told "`repository.merge` is not one of the candidate actions" (`run.rs:661`) instead of "is blocked: <reason>". The stop reasons do not change (see part 5). | NEEDS-CHANGE / introduced: base hands over the whole frontier (`git show 4d51820:…runtime.rs:459`) |
| 2 | `crates/loom-commission/src/runtime.rs:455` | The gate compares the governor's whole frontier, while the outcome is derived from the handed one. When only an unoffered action changes at the same revision, the executor is asked again about a frontier it was already shown. The run ends `NoAdmissibleAction` after 2 calls; the control run ends `AwaitingApproval(service.deploy)` after 1 (case :284). Suggested fix: compare the handed frontier in `unchanged`. | Nothing found. It needs two frontiers at one revision that differ only in an unoffered action. `CanonGovernor` is deterministic per revision except for freshness; on the slice the only unoffered action is a `Blocked` merge; no host builds `ConnectorEffects`. | INFEASIBLE / introduced (the split between gate and outcome is new in 09c1c9e) |
| 3 | `crates/loom-commission/src/runtime.rs:700` | Gap in the tests: no case checks that `attempt` is absent. A mutant that always pushes `attempt` (`Null` when `None`; `mutant-attempt-observation.patch`) leaves `effect_invocation` 4/4, `runtime_effect` 16/16, `runtime_loop` and every `adversary_*` file green. Case :361 fails on it: `left: Some(Null) right: None`. | Every effect observation from `LocalEffects` on the slice. | CONFIRMED / introduced |

**5. Attacked and could not break**
- **Outcome from the handed frontier versus the governor's:**
  - `Completed`: the completion check runs before the executor, so `derive` rule 1 is unreachable in the loop.
  - `NeedsExternalEvidence`: the obligations are cloned into the handed frontier.
  - `NeedsAuthority`: a gated action is kept with all its entries, so the capability matches.
  - The request and `last_frontier` bind only case and revision, which both frontiers share.
- **Merge gate on the slice:** at 09c1c9e, `loom-cli` `slice_run` passed 9/9 and `adversary_slice_run` 14/14 (EXIT=0).
- **Filtering by action:** an unbound action listed `Blocked` and `ApprovalRequired` is kept with both entries and refused, as the governor refuses it. Pass 1's case is green.
- **Connector path with no attempt:** `ConnectorEffects` turns it into an `EffectError` (the unit's own case). On the non-Connector path the runtime cannot check for an attempt; `LocalEffects` names none (`effect.rs:186`).

**6. Paths written outside the worktree:** one session-lease record, through `worktree hook`. Everything else is under `<worktree>/.engineering/drafts/scratch/adversary-2/`, which git ignores: the logs, the patch, and `mutant-attempt-observation/` with its own 471M `target/`. That copy's `Cargo.lock` was regenerated with `--offline`, because the root lock file refuses `--locked` in a smaller workspace. In the copy, the `ess_gate` and docs-contract tests fail only because `ess/` and `docs/` were not copied.

```findings
- file: crates/loom-commission/src/runtime.rs
  line: 613
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the offered filter keeps an unperformed action only when an entry is ApprovalRequired, so the slice's Blocked repository.merge that names its capability is hidden from the executor, against decision B and the documented frontier line
- file: crates/loom-commission/src/runtime.rs
  line: 455
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: the approval gate compares the governor's frontier while the outcome reads the handed one, so a change in an unoffered action re-asks the executor about an unchanged frontier and ends NoAdmissibleAction instead of AwaitingApproval
- file: crates/loom-commission/src/runtime.rs
  line: 700
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: always emitting an attempt member in the effect observation leaves the unit's suite green; only the adversary case asserts its absence when no Connector was invoked
```
