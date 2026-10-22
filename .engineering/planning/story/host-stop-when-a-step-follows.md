---
format: aep.planning-md/3
id: story:host-stop-when-a-step-follows
kind: story
status: implemented
title: A host stop stops a Run only where a step would follow the authority checks
relations:
- serves: vision:governed-autonomy
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T04:32:07Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-22T04:32:08Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-22T06:44:23Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

`LoopContext::host_stop` stops a Run only where a step would follow the admissibility and
authority checks. A frontier that admits no action without a grant ends the run as the authority
or admissibility outcome says (`NeedsAuthority`, `NoAdmissibleAction`, `AwaitingApproval`), never
`HostStop`, and a granted request is still never performed past a host stop.

## Why

- takt `dependency-blocker:loom-runtime-stops`, § Against Loom 0.33.0, row DiskLow: on 0.33.0
  `host_stop` is asked before the executor call (`crates/loom-commission/src/runtime.rs:1527` at
  tag `0.33.0`), while with `approval_gate: false` approval arrives as `NeedsAuthority` after a
  proposal. A case only a grant can move ends `HostStop` (takt exit 8) where takt ends
  `ApprovalRequired` (exit 4), against takt `crates/takt/tests/free_disk.rs:509-528`.
- The orchestrator, under the grant: "takt (blocker "Against Loom 0.33.0"): ask host_stop only
  when a step follows the approval/admissibility checks, so takt keeps exit 4 on low disk (takt
  crates/takt/tests/free_disk.rs:509-528). Add it to wave w2 as a story."

## Design

- Before the executor call, `host_stop` is asked only when the handed frontier lists an action
  `Admissible`. When it lists none (every listed action needs a grant), the executor runs; a
  proposal the provider withholds ends `NeedsAuthority`, a proposal of nothing ends as today.
- A proposal the provider allows, when the host stop was not asked this iteration, asks it after
  the allow and before the effect is invoked; a stop there ends the Run suspended for `HostStop`
  with nothing invoked, as a stop before the executor does.
- The deadline (`RunBudget.deadline`) keeps its place before the executor; only the host stop
  moves. A pending request is still asked the host stop before it is invoked again.
- With the approval gate armed, `AwaitingApproval` already comes before the host stop
  (`runtime.rs:1513-1526` at `0.33.0`); unchanged.

## ESS first

`ess/commission/domains/responsibility.yaml`: the text of `LoopContext.host_stop` and of
`SuspensionReason` `HostStop` states the new point, if either states one. When no declaration
states the order, the story says so in its first commit and the red commit is the new test.

## Acceptance

- A frontier listing only approval-required actions, with `approval_gate: false` and a host stop
  that always stops: the run ends `NeedsAuthority` naming the request, the executor ran once, and
  `HostStop` is not the outcome (red on `3d56bca9`).
- The same frontier with a provider that allows the request: the run ends suspended for
  `HostStop`, the effect is never invoked.
- A frontier with an admissible action: the host stop is asked before the executor, which is not
  called, as on 0.33.0.
- `adversary_a_host_stop_comes_before_a_frontier_admitting_nothing` and the host-stop tests of
  wave 2026-10-22-w1 hold, or are changed in the same commit with the reason.

## Scope

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/` (if the text
  changes), `crates/loom-commission/src/runtime.rs`
- `crates/loom-commission-testkit/tests/host_stop.rs`,
  `crates/loom-commission-testkit/tests/adversary_w20261022w1_host_stop.rs`
- `CHANGELOG.md`, `AGENTS.md` (§ Commission, the host-stop order), `website/docs/` pages that state
  the order
