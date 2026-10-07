---
format: aep.planning-md/3
id: review-result:adversary-w2-loom-effect-invocation-pass-1
kind: review-result
status: active
title: Wave 2026-10-07-w2 adversary, loom story:effect-invocation, pass 1
relations:
- reviews: story:effect-invocation
revision: 1
---
```
unit: story:effect-invocation, branch impl/effect-invocation at 68bc8b9, plus one untracked adversary test file
verdict: NEEDS-CHANGE
cases: executed 151→156, red 3
origin: introduced 4 / pre-existing 1 / undecided 0
wrote-outside-worktree: 1 (worktree session lease record, acquired and released through `worktree hook`)
needs-coordinator: whether derive should read the offered frontier or the governor's frontier (finding 2) is a design call on the story
```

**1. `git --no-pager diff --stat`:** empty, so no tracked file changed. `git status --short` shows one untracked file: `crates/loom-commission-testkit/tests/adversary_w2_effect_invocation.rs`. It is a test file, so nothing outside the charter was touched.

**2. Cases added** (in `<worktree>/crates/loom-commission-testkit/tests/adversary_w2_effect_invocation.rs`). First run of the file alone, verbatim (`red-alone.log`):
```
---- a_frontier_offering_nothing_admissible_does_not_spend_the_step_budget ----
  left: (Suspended(RunOutcomeSuspended { reason: Budget(Object([("max_steps", Number("4"))])) }), 4)
 right: (NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["tests.pass"] }), 1)
---- a_frontier_offering_nothing_admissible_ends_needing_external_evidence ----
  left: (NoAdmissibleAction(Unit(true)), 2)
 right: (NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["tests.pass"] }), 1)
---- the_frontier_handed_over_admits_each_kept_action_as_the_governor_frontier_does ----
the frontier handed to the executor admits `repository.merge` differently from the governor's frontier
  left: NeedsAuthority(AdmissionNeedsAuthority { capability: "repository.merge" })
 right: Refused(AdmissionRefused { action: "repository.merge", reasons: ["implementation.verified is Unknown, required True"] })
test result: FAILED. 2 passed; 3 failed
```

| Case (line) | Asserts | Now |
|---|---|---|
| `the_frontier_handed_over_admits_…` :193 | every action kept for the executor is admitted exactly as the governor's frontier admits it | red |
| `…does_not_spend_the_step_budget` :268 | with budget 4 and nothing admissible offered: `NeedsExternalEvidence` after 1 executor call | red |
| `…ends_needing_external_evidence` :328 | the same frontier with no budget | red |
| `a_binding_keyed_to_another_commission…` :377 | a binding whose key names another commission is refused | green; red against the mutant (finding 3) |
| `an_unbound_request_reaching_connector_effects…` :399 | the no-binding refusal path the implementor called unreachable | green: reachable through a wrapper port; it refuses and invokes nothing |

**3. Suite run:** `cargo test -p b10x-loom-commission-testkit --locked --no-fail-fast` gave EXIT=101. 156 executed, 153 passed, 3 failed, all in `adversary_w2_effect_invocation`. A second run with that file deselected gave 151 executed, EXIT=0.

**4. Findings** (they cover 68bc8b9 plus my file)

| # | file:line | Measured | What reaches it | Verdict / origin |
|---|---|---|---|---|
| 1 | `crates/loom-commission/src/runtime.rs:603` | `offered` filters entry by entry. An unbound action listed both `Blocked` and `ApprovalRequired` loses its `Blocked` entry. The executor then admits it as `NeedsAuthority`, so Loom would list it "admissible once authorized", while the governor refuses it. This breaks the least-authority rule in `docs/commission/contracts/frontier.md`. Fix: filter by action, and keep every entry of a kept action. | Nothing found. The shipped `CanonGovernor` lists each declared action once (`crates/loom-governor/src/lib.rs:581-607`); only the contract allows duplicates. | INFEASIBLE / introduced. At base the executor got the whole frontier (`git show 4d51820:…runtime.rs:459`). |
| 2 | `crates/loom-commission/src/runtime.rs:554` | The outcome is derived from the governor's frontier, so an admissible action that is unbound, ungated and never offered still counts. With a budget, the run calls the executor 4 times and ends `Suspended(Budget)`. Without one, it ends `NoAdmissibleAction` after 2 calls. Both should be `NeedsExternalEvidence` after 1 call. | Nothing found. No host builds `ConnectorEffects`. On the slice, LocalEffects performs every ungated `software.change/1` action. Every `incident-response/1` frontier lists gated actions, so the approval gate ends the run first. | INFEASIBLE / introduced. This is the implementor's flag, now tested by a program. |
| 3 | `crates/loom-commission/src/ports/connector.rs:96` | A test gap. With the key half of the check removed (`mutant-key-check.patch`), the unit's `effect_invocation` still passes 3/3. My case at :377 fails on the mutant. | A host composition passing such a binding; none exists yet. | CONFIRMED / introduced |
| 4 | `crates/loom-commission/src/runtime.rs:444` | Step 4 ends `NoPerformableAction` before the filter runs. A frontier whose remaining actions are only gated and unbound never reaches the gate. Story item 2 says only a frontier "left with no action" ends this way. The code matches the decision's "as today"; the story's wording does not. | LocalEffects on a non-`software.change` protocol (`local` is false). | CONFIRMED / pre-existing (identical at base, lines 439-445) |
| 5 | `crates/loom-intake-slice/src/effect.rs:187` | The slice reports its attempt as `local:<request id>`, in a type the spec documents as a Connectors `AttemptRecord` id (`responsibility.yaml:278`). | Every effect observation from `b10x-loom run`. | CONFIRMED / introduced (implementor-flagged) |

**5. Attacked and could not break**
- **Slice still stops at the merge gate:** `loom-cli` `slice_run` passed 9/9 and `adversary_slice_run` 14/14, all asserting `stopped: ApprovalRequired (repository.merge)`. `b10x-loom-intake-slice` exited 0.
- **No Connectors dependency:** all 19 Connectors packages are named `connectors` or `connectors-*`, which the guard's matcher covers, and `executor_port` is green.
- **Stale request, deny and approval-required invoke nothing; read actions take the same governor calls as consequential ones:** the unit's cases assert both what happens and what does not.
- **No attempt on a refused request:** `attempt` exists only on `Performed`.

**6. Paths written outside the worktree:** one session-lease record, written by `worktree hook session-start` and released by `session-end`. Everything else is under `<worktree>/.engineering/drafts/scratch/adversary-1/`, which git ignores: the logs, the patch, and `mutant-key-check/` with its own 42M `target/`.

```findings
- file: crates/loom-commission/src/runtime.rs
  line: 603
  category: contract-drift
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: the per-entry filter drops a Blocked entry and keeps an ApprovalRequired one for the same unbound action, so the executor admits as NeedsAuthority an action the governor's frontier refuses
- file: crates/loom-commission/src/runtime.rs
  line: 554
  category: acceptance
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: run outcome is derived from the governor's whole frontier, so an unbound ungated action never offered keeps the run going until the idle rule (NoAdmissibleAction) or the step budget (Suspended Budget) instead of NeedsExternalEvidence
- file: crates/loom-commission/src/ports/connector.rs
  line: 96
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: removing the binding-key half of the other-commission check leaves the unit's suite green; only the adversary case catches it
- file: crates/loom-commission/src/runtime.rs
  line: 444
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: step 4 ends NoPerformableAction before the filter, so gated unbound actions never reach the gate when nothing is performed, unlike story item 2's "a frontier left with no action"
- file: crates/loom-intake-slice/src/effect.rs
  line: 187
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: LocalEffects fills ConnectorAttemptId, documented as a Connectors AttemptRecord id, with a synthetic local:<request id>
```
