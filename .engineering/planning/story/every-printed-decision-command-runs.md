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
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:37:58Z", actor: "agent:loom", revision: 5}
---
## Outcome

Every decision `tree run` prints carries a `command` that works when run unchanged. This holds for
all four kinds of `loom.tree.DecisionKind`: `Approve`, `ApproveAction`, `Close` and `Handoff`
(`generated/rust/loom/src/tree.rs:76-87`).

- For `Approve`, `ApproveAction` and `Close`, the command records that decision and exits 0.
- For `Handoff`, the command is the next `tree run`, and it exits as that run does: 3, 4 or 0, as
  the tree stops (`story:tree-run-command`, its lines 324-325).
- A person action whose protocol action needs no capability is included.

What changes:

- `tree approve --capability` becomes optional when `--action` is given.
  - The person action's command is already built without `--capability` when the action needs
    none (`crates/loom-tree/src/run.rs:830-833`).
  - Today `capability` is a required `String` (`crates/loom-cli/src/lib.rs:427-428`), so that
    command exits 2.
  - With `--action` and no `--capability`, `tree approve` takes the person action and records no
    authority entry. That is allowed only when the protocol action needs no capability.
  - A person action that does need one, given without `--capability`, is refused
    `NotAPersonAction`, naming the capability, and nothing is written. The check is new, in
    `person_action` (`crates/loom-tree/src/approve.rs:130-170`). Today clap refuses that call
    with exit 2 before Loom reads it.
  - Without `--action`, `--capability` stays required.
- The `Close` command names `b10x-loom tree close` (`run.rs:856-870`), which does not exist at
  this head (`TreeCommand`, `lib.rs:266-286`). The unmerged `impl/tree-run-command` adds it
  (`d13e36ec`).
- The `Handoff` decision and its command, `tree run --store <store>`, come with the same branch
  (`db7a3d8b`).

This story is satisfied where the merged head of `impl/tree-run-command` already does the above.
Its test list below is the Acceptance that proves it, and only what is missing is built. At
`66396a6f` that branch adds `tree close` and `Handoff` and still requires `--capability`
(`lib.rs:430-431` there), so the person action without a capability is the part left.

Security-relevant: yes. `--capability` becomes optional on the command that records authority.
The new refusal keeps an action that needs a capability from being taken without one, and the
unit gets a security review.

## Why

Runbook runs each printed `command` as data between two `tree run` calls. A command that exits 2
or names a missing subcommand stops its driver.

The decision object (`crates/loom-tree/src/encode.rs:186-234`) has two kinds of member:

- `case`, `kind`, `decides` and `command` are stable;
- `action`, `capability`, `subject`, `revision`, `path`, `outcome` and `detail` are optional and
  only ever added.

Runbook runs `command`, never parses `decides`, and treats `kind` as open:
`story:tree-waits-open-decision-requests` adds a request id and changes the `AwaitingAuthority`
command (its lines 101-105).

## ESS first

`ess/` does not change: no noun or kind is added. The first commit holds the seven tests below.

- At the integration head `5db661b5`, four are red:
  - `the_printed_close_command_runs`, because `tree close` does not exist;
  - `the_printed_handoff_command_runs`, because no `Handoff` is printed;
  - `the_printed_person_action_without_a_capability_runs` and
    `a_person_action_that_needs_a_capability_is_refused_without_one`, because clap exits 2.
- At `impl/tree-run-command` `66396a6f`, only the last two are red.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call.

Each test brings one shared tree fixture to one wait, from a store copy rather than a whole tree
run per test where the fixture allows. It runs the printed `command` as printed, and asserts the
exit code and the recorded effect.

- **`Approve`.** Exit 0, and the case's run log gains an `AuthorityRecorded` line. The next
  `tree run` takes the action. Test: `the_printed_approve_command_runs`.
- **`ApproveAction` with a capability.** Exit 0, and the person's record is held. Test:
  `the_printed_approve_action_command_runs`.
- **`ApproveAction` with no capability.** The printed command has no `--capability`. It exits 0,
  records the person's record, and adds no `AuthorityRecorded` line. Test:
  `the_printed_person_action_without_a_capability_runs`.
- **A capability that is needed but not given.** `tree approve --action A` without
  `--capability`, for an `A` that needs one, exits 1, refused `NotAPersonAction` naming the
  capability, and the store is byte for byte unchanged. Test:
  `a_person_action_that_needs_a_capability_is_refused_without_one`.
- **`Close`.** Exit 0. The case is closed through the named outcome, its log gains a
  `Terminated` line, and the tree is terminal. Test: `the_printed_close_command_runs`.
- **`Handoff`.** The command runs `tree run --store <store>`. With the fixture
  `tour-leg-handoff.bindings.yaml` of `db7a3d8b`, whose writer writes nothing, it exits 3. The run
  log gains a `Stopped` line with stop `AwaitingPerson`, and the same `Handoff` decision is
  printed again. Test: `the_printed_handoff_command_runs`.
- **No action.** `tree approve` without `--action` and without `--capability` is refused by the
  command line with exit 2. Test: `approve_without_an_action_still_requires_a_capability`.

## Not covered

- Decision requests and their ids: `story:tree-waits-open-decision-requests`.
- A named result for a person action: `story:person-action-records-a-named-result`.
- When a handoff is offered: `story:handoff-when-a-program-decides`.

## Scope

- `crates/loom-cli/src/lib.rs`: `TreeCommand` at `:266-286`, `TreeApproveArgs` at `:419-442`.
  Cited.
- `crates/loom-tree/src/run.rs`: `waits` at `:786-890`, the approve command at `:818-840`, the
  close command at `:856-870`. Cited.
- `crates/loom-tree/src/approve.rs`: `person_action` at `:130-170`, `take_person_action` at
  `:174`. Cited.
- `crates/loom-cli/tests/tree_run.rs`, next to
  `a_one_case_tree_runs_through_its_waits_to_its_outcome` at `:403`. Cited.

Edges:

- depends_on `story:tree-run-command`.
- Collides with `story:person-action-records-a-named-result` on `TreeApproveArgs` and
  `approve.rs`. That story depends_on this one.
- Shares `waits` in `run.rs` (`:786-890`) and `tests/tree_run.rs` with
  `story:handoff-when-a-program-decides`. It shares `tests/tree_run.rs` with
  `story:tree-run-takes-a-delegated-cgroup`. The later unit merges the earlier head.
