---
format: aep.planning-md/3
id: story:every-printed-decision-command-runs
kind: story
status: proposed
title: Every decision tree run prints carries a command that, run unchanged, records that decision and exits 0
summary: one test per decision kind executes the printed command; a person action without a capability prints a command that runs
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:tree-run-command
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: cited
  path: crates/loom-tree/src/approve.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:37:58Z", actor: "agent:loom", revision: 5}
---
## Outcome

Every decision `tree run` prints carries a `command` that, run unchanged, records that decision
and exits 0. This holds for all four kinds of `loom.tree.DecisionKind`: `Approve`,
`ApproveAction`, `Close` and `Handoff` (`generated/rust/loom/src/tree.rs:76-87`). It includes a
person action whose protocol action needs no capability.

- `tree approve --capability` becomes optional when `--action` is given. The person action's
  command is built without `--capability` when the action needs none
  (`crates/loom-tree/src/run.rs:830-833`). Today `capability` is a required `String`
  (`crates/loom-cli/src/lib.rs:427-428`), so that command exits 2 (inferred from clap; the test
  proves it).
  - With `--action` and no `--capability`, `tree approve` takes the person action and records no
    authority entry.
  - Without `--action`, `--capability` stays required.
- The `Close` command names `b10x-loom tree close` (`run.rs:856-870`), which does not exist at
  this head (`TreeCommand`, `lib.rs:266-286`). The unmerged `impl/tree-run-command` adds it
  (`d13e36ec`).
- The `Handoff` command, `tree run --store <store>`, comes with the same branch (`db7a3d8b`).

This story is satisfied when the merged head of `impl/tree-run-command` already does all of the
above. Its test list below is then the Acceptance that proves it, and only the missing parts are
built. At `66396a6f` that branch adds `tree close` and `Handoff`, and still requires
`--capability` (`lib.rs:430-431` there). So the person action without a capability is the part
left.

Security-relevant: no. The commands record what the run already offers. No new authority is
granted: a person action without a capability needed none in the protocol.

## Why

Runbook runs each printed `command` as data between two `tree run` calls. A command that exits 2
or names a missing subcommand stops its driver.

The decision object (`crates/loom-tree/src/encode.rs:186-234`) has two kinds of member. `case`,
`kind`, `decides` and `command` are stable. `action`, `capability`, `subject`, `revision`, `path`,
`outcome` and `detail` are optional and only ever added to. Runbook should run `command`, never
parse `decides`, and treat `kind` as open: `story:tree-waits-open-decision-requests` adds a
request id and changes the `AwaitingAuthority` command (its lines 101-105).

## ESS first

`ess/` does not change: no noun or kind is added. The first commit holds the four tests below, of
which the `ApproveAction`-without-capability test is red at this head.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call. Each test runs one shared tree fixture to one wait (a store copy, not a whole tree per test,
where the fixture allows), runs the printed `command` as printed, and asserts exit 0 and the
recorded effect.

- `Approve`: the printed command records the authority entry, and the next `tree run` takes the
  action. Test: `the_printed_approve_command_runs`.
- `ApproveAction`, with a capability: the printed command takes the person action. Test:
  `the_printed_approve_action_command_runs`.
- `ApproveAction`, with no capability: the printed command has no `--capability`, exits 0, and
  records the person's record with no authority entry. Test:
  `the_printed_person_action_without_a_capability_runs`.
- `Close`: the printed command closes the case through the named outcome, and the tree is
  terminal. Test: `the_printed_close_command_runs`.
- `Handoff`: the printed command (`tree run --store`) runs and exits as the tree stops. Test:
  `the_printed_handoff_command_runs`.
- `tree approve` without `--action` and without `--capability` is refused by the command line
  (exit 2). Test: `approve_without_an_action_still_requires_a_capability`.

## Not covered

- Decision requests and their ids: `story:tree-waits-open-decision-requests`.
- A named result for a person action: `story:person-action-records-a-named-result`.
- When a handoff is offered: `story:handoff-when-a-program-decides`.

## Scope

- `crates/loom-cli/src/lib.rs`: `TreeCommand` at `:266-286`, `TreeApproveArgs` at `:419-442`.
  Cited.
- `crates/loom-tree/src/run.rs`: `waits` at `:786-890`, the approve command at `:818-840`, the
  close command at `:856-870`. Cited.
- `crates/loom-tree/src/approve.rs`: an action without a capability. Cited at `:110-145`; the
  change is inferred.
- `crates/loom-cli/tests/tree_run.rs`: next to
  `a_one_case_tree_runs_through_its_waits_to_its_outcome` at `:403`. Cited.

Edges:

- depends_on `story:tree-run-command`.
- Collides with `story:person-action-records-a-named-result` on `TreeApproveArgs` and
  `approve.rs`.
