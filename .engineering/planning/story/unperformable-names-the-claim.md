---
format: aep.planning-md/3
id: story:unperformable-names-the-claim
kind: story
status: draft
title: NoPerformableAction names a claim whose deciding actions have no performer
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:frontier-names-movers
revision: 1
---
## Outcome

A run with an open claim whose deciding actions all lack a performer and need no grant ends
`NoPerformableAction` naming that claim, even while other actions are performed. A claim no action
decides is external evidence and does not end the run.

## Why

takt's probe on 0.34.0 (takt `545c591`): a fixture whose `pr.open` had no performer ended
`IdleBound`, because `NoPerformableAction` fires only when no listed action is performed at all
(`crates/loom-commission/src/runtime.rs:1593-1597`, `2466-2507`; `src/outcome.rs:184-197`). Draft
until Timo wants the documented stop order changed.

## Acceptance

- `crates/loom-commission-testkit/tests/unperformable_names.rs::a_claim_only_an_unperformed_action_decides_ends_the_run_naming_it`
- `crates/loom-commission-testkit/tests/unperformable_names.rs::a_claim_no_action_decides_stays_external`
  (so `software.change/1` slice runs do not end early on review or provenance evidence)
- Unchanged: `unperformable_names.rs:227,264,291,323`, all of `adversary_w20261020w3_u1.rs`,
  `idle_bound_outcome.rs:266,310`.

## ESS first

`JudgePerformableActions` (`ess/commission/domains/responsibility.yaml:1865-1874`) gains a
`stranded_claims` input: the open claims whose `decided_by` is non-empty and all ungated and
unperformed. `task commission:conform` fails on that commit.

## Depends on

`story:frontier-names-movers`.

## Not in scope

The host-stop order (`story:needs-authority-before-host-stop`). It shares files with this story,
so the two run one after the other.

## Scope

- `crates/loom-commission/src/runtime.rs`, `src/outcome.rs`
- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`
- AGENTS.md :375-379 and the `NoPerformableAction` row; CHANGELOG Breaking line
