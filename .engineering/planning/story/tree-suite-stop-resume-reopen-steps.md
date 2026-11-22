---
format: aep.planning-md/3
id: story:tree-suite-stop-resume-reopen-steps
kind: story
status: proposed
title: Suite steps stop, resume and reopen over a tree hold record written by tree hold and tree reopen
summary: tree hold and tree reopen write StopRecorded and StopLifted in the root log under the store lock; tree run on a held tree is refused TreeStopped; the suite runs stop, resume and reopen; Runbook's run-stop cycle runs; approve members moved out (D130)
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-suite-format
- depends_on: story:cancel-a-running-bound-program
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-suite
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: inferred
  path: crates/loom-cli/tests/tree_run/stop.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_test/mod.rs
- confidence: inferred
  path: crates/loom-runlog/src/lib.rs
- confidence: inferred
  path: crates/loom-tree/src/lib.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: inferred
  path: crates/loom-tree/src/stop.rs
- confidence: cited
  path: crates/loom-tree/src/suite/availability.rs
- confidence: cited
  path: crates/loom-tree/src/suite/decode.rs
- confidence: cited
  path: crates/loom-tree/src/suite/run.rs
- confidence: cited
  path: crates/loom-tree/tests/suite_format/mod.rs
- confidence: cited
  path: docs/contracts/run-log-v1.json
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/run-a-case-tree.md
- confidence: cited
  path: website/docs/reference/cli.md
revision: 27
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T13:08:58Z", actor: "agent:loom", revision: 27}
---
## Outcome

The suite runner runs the steps `stop`, `resume` and `reopen` over a hold record that the library
and two new CLI verbs, `tree hold` and `tree reopen`, write. Runbook's cycle `run-stop` waits on
them (Runbook `docs/cycles-suite.md:236`, branch `impl/runbook-test-suite`). Runbook calls the
story `story:tree-stop-record-and-labels`; the id is ours. The code names that id at
`crates/loom-tree/src/suite/availability.rs:55`, and no story of that id is in the store.

The `approve` members `result`, `reasons` and `deny`, which Runbook's other three waiting cycles
need, are `story:tree-suite-approve-result-and-reasons` (decision D130). That story merges
before this one.

**Security-relevant.** The story adds a record of who held a tree and why. A principal and a
reason from a command line or a suite file enter the run log. The hold is a record of a decision
and not a lock against the store's owner (see "What a hold is").

### What a hold is

Loom has no hold today: `TreeCommand` has `Open`, `Run`, `Approve`, `Close`, `Status`, `Verify`
and `Test` (`crates/loom-cli/src/lib.rs:266-293`). The run log's `Stopped` says why one
`tree run` returned (`ess/domains/runlog.yaml:181-198`); it is not a stop that holds. The verb
`tree stop` stays `story:tree-daemon`'s: there it ends a served tree's run after its current step
(`.engineering/planning/story/tree-daemon.md:33-36`). That is why the verb here is `tree hold`
(decision D129).

```
b10x-loom tree hold   --store <dir> --principal <p> --reason <text> [--at <instant>]
b10x-loom tree reopen --store <dir> --principal <p> [--at <instant>]
```

- **`tree hold`** takes the store lock and appends one `stop recorded` line (`StopRecorded`:
  `principal`, `reason`) to the root case's run log. The tree is held while the root's log holds
  a `stop recorded` line with no `stop lifted` line after it. The run log is the record; no other
  file holds it.
- **`tree run` on a held tree** is refused `TreeStopped` with exit 1 (`TREE_REFUSED`,
  `crates/loom-cli/src/lib.rs:497`).
  - The refusal comes after the store lock is taken (`crates/loom-tree/src/run.rs:190`) and
    before a step or a line is written.
  - It prints the hold's instant, principal and reason, and the `tree reopen` command that
    lifts it.
- **`tree reopen`** takes the store lock and appends one `stop lifted` line (`StopLifted`:
  `principal`, and `stop`, the `seq` of the line it lifts). After it, `tree run` goes on as it
  would have without the hold.
- **Twice.** `tree hold` on a held tree is refused `TreeAlreadyStopped`. `tree reopen` on a tree
  that is not held is refused `TreeNotStopped`. Neither writes anything.
- **Other commands.** `tree approve`, `tree close`, `tree status` and `tree verify` work on a
  held tree as on any other, and an approval does not lift a hold. `tree status` does not show
  the hold (Out of scope).
- **What a command accepts.**
  - `--reason` is 1 to 1024 bytes of UTF-8 with no control character.
  - `--principal` is checked as `tree approve` checks it: not empty, at most 200 bytes
    (`MAX_NAME_BYTES`, `crates/loom-tree/src/approve.rs:24`), no control character.
  - Anything else is refused `StopUnusable`, naming the member and the rule, before the store is
    opened.
- **The lock.** Both verbs are refused `StoreInUse`, naming the holder, while another writer
  (a `tree run`) holds the store.
- **Exit.** `tree hold` and `tree reopen` exit 0 when the line is written, 1 when refused (the
  refusal on standard error), 2 when the command line is not valid.
- **One write.** Each verb writes one line through the run log's append, which renames a whole
  new log over the old one (`crates/loom-runlog/src/lib.rs:11-19`). A restart leaves the line
  written whole or not at all.
- **Not a lock.** A log whose last lines were cut away whole still verifies
  (`crates/loom-runlog/src/lib.rs:45-48`). So a person with write access to the store can lift a
  hold by cutting its line while it is the last one. The hold records a decision; the store's
  owner is not locked out.

What these lines hold comes from the command line alone. No program output enters a hold
record.

### The suite steps

Runbook's `run-stop` cycle (Runbook `cycles/tree-suite.yaml:327-338`) is `open`,
`run rounds: 1 exit: 3`, `stop`, `resume exit: {not: 0}`, `reopen`, `run rounds: 1 exit: 3`.
Loom's own copy is `crates/loom-cli/tests/fixtures/tree-suite/runbook/cycles.suite.yaml:121-131`.
The step names are the format's (`crates/loom-tree/src/suite/decode.rs:667-683`) and stay as
they are.

- **`stop`** runs the library's hold (`tree hold`).
  - The principal is chosen as the `approve` step chooses it (`self.principal`,
    `crates/loom-tree/src/suite/run.rs:977`).
  - The reason is the step's new member `reason`. When it is absent, the reason is
    `suite step <label>`, where the label is the step's `name` or `steps[<i>]`.
- **`resume`** runs exactly one `tree run`, as one round of the `run` step does
  (`suite/run.rs:1071-1074`), passing the cycle's cancel flag. It answers no wait from the
  scripted entries.
  - Its exit is the run's (`stop_exit`, `suite/run.rs:623-629`).
  - A refused run gives exit 1 and the refusal's name (`StepEnd::refused`,
    `suite/run.rs:584-590`), so a held tree gives exit 1 and refusal `TreeStopped`.
- **`reopen`** runs the library's `tree reopen`, with the principal chosen as for `stop`.
- **`case`.** The three steps take no `case` member, because a hold belongs to the tree.
  `step_members` drops `case` for them (`decode.rs:701-706`), and `stop` gains `reason`. A `case`
  given to one of them is refused at decode as an unknown member.
- **Availability.** In `SUITE_AVAILABILITY` (`suite/availability.rs:72-109`), the steps `stop`,
  `resume` and `reopen` and the events `StopRecorded` and `StopLifted` become `Available`. The
  step `allow` and the event `Labelled` keep `Waits("story:tree-stop-record-and-labels")`. The
  `log` source reads the two new events like any other.
- **The refusal of a waiting kind.** The match arm at `suite/run.rs:1023-1036` keeps `Decide`,
  `Allow` and `Replay` only.

### Runbook's side

Runbook's `run-stop` check reads `files: {in: store, glob: "run-stopped.*.json"}` (Runbook
`cycles/tree-suite.yaml:338`), the old driver's file. Loom writes no such file. The check becomes
`{log: {case: ".", event: StopRecorded}}`, as Loom's copy already has it
(`runbook/cycles.suite.yaml:131`).

## Decisions

- **D1. Step shape.** Default **a**.
  - **a.** One step kind per verb, in the style of `approve`: `stop {principal?, reason?}`,
    `resume {}`, `reopen {principal?}`. These are the kinds the format already decodes.
  - **b.** One `stop` step with a member `then: resume | reopen`. That is a format change for a
    shape no Runbook cycle uses.
- **D2. Where the hold verbs come from.** Default **a**.
  - **a.** This story adds `tree hold` and `tree reopen` to the library and the CLI, and the
    steps call the library. A suite step and a person's command then share one path.
  - **b.** Library verbs only, with the CLI subcommands in a later story. That saves
    `crates/loom-cli/src/lib.rs` now, but Runbook's driver would have no command to hold a run
    with until then.
  - **c.** A separate story owns `tree hold` and `tree reopen`, and this story depends on it.
    That is one more edge over the same files.
- **D3. Which verb lifts the hold.** Default **a**.
  - **a.** `tree reopen`, Runbook's word and the step's name. No story in the store gives
    `tree reopen` another meaning.
  - **b.** `tree hold --lift`, one verb with a flag. The suite's `reopen` step would then call
    the flag.
- **D4. The exit of `tree run` on a held tree.** Default **a**.
  - **a.** Exit 1, refusal `TreeStopped`: nothing ran.
  - **b.** Exit 6, the status `story:tree-daemon` gives a tree its `tree stop` stopped. Runbook's
    cycle checks only `{not: 0}`.
- **D5. The stop step's reason.** Default **a**.
  - **a.** A new optional member `reason`, defaulting to `suite step <label>`.
  - **b.** No member; the fixed text only.

## ESS first

`ess/domains/runlog.yaml` comes first:

- `RunLogEvent` (`:36-52`) gains `StopRecorded: loom.runlog.StopRecorded` (principal, reason)
  and `StopLifted: loom.runlog.StopLifted` (principal, and `stop`, an Integer: the `seq` of the
  lifted line).
- These are the names the suite format already knows (`availability.rs:107-108`).

`ess/domains/tree.yaml` comes next:

- `TreeRefusalReason` (`:1112-1138`) gains `TreeStopped`, `TreeAlreadyStopped`,
  `TreeNotStopped` and `StopUnusable`.
- The suite step type gains `reason: Optional<String>`.

On that commit `task drift` fails: the generated `loom::runlog`, `loom::tree` and
`docs/contracts/run-log-v1.json` lag behind.

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step ends within 120 s.
- No test makes a network request or a model call, and no test reads a credential file.
- Fixtures are POSIX sh, never Python.
- A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30
  minutes).

Each test below fails on today's code for the reason given.

**`crates/loom-cli/tests/tree_run/stop.rs`**

This is a new `#[path]` module of `tree_run.rs`, beside `handoff_when` and `principal`
(`tree_run.rs:38-44`).

- Fixture: `fixtures/tree-two-level`, the `review@1` case `reviewed` brings to its first wait
  (`tree_run.rs:2251-2268`). Each test works on a fresh copy.
- Failing first: the subcommands do not exist, so clap exits 2.

1. `a_held_tree_refuses_to_run`.
   - After `tree hold --principal person:owner --reason "hold for review"`, `tree run` exits 1
     with `TreeStopped`.
   - It prints the principal, the reason and a `tree reopen` command.
   - Every run log of the tree is byte-identical to before the `tree run`.
2. `a_reopened_tree_runs_on`.
   - After `tree reopen --principal person:owner`, `tree run` exits 3 with the same decisions as
     a copy that was never held.
   - The root's run log, read with `log_lines` (`tree_run.rs:601-604`), holds one
     `StopRecorded` and one `StopLifted` line, and the `StopLifted` line's `stop` equals the
     `StopRecorded` line's `seq`.
   - It also catches the planted defect of writing the `StopLifted` line's own `seq`, or 0, as
     `stop`.
3. `a_hold_and_a_reopen_each_apply_once`.
   - A second `tree hold` exits 1 with `TreeAlreadyStopped`.
   - `tree reopen` on a tree that is not held exits 1 with `TreeNotStopped`.
   - Neither changes a byte of the store.
4. `an_approval_does_not_lift_a_hold`. `tree approve --action draft.review` on a held tree exits
   0, and `tree run` still exits 1 with `TreeStopped`.
5. `an_unusable_reason_or_principal_is_refused`.
   - Each of these is refused `StopUnusable`, naming the member: a `--reason` holding a line
     feed, a `--reason` of 1025 bytes, a `--principal` of 201 bytes, and a `--principal` holding
     the control character U+0007.
   - The store is byte-identical afterwards.
   - Before the store is opened: `tree hold` with `--store` naming a path that does not exist and
     a `--reason` holding a line feed is refused `StopUnusable`, not `NotATree`
     (`existing_store`, `crates/loom-tree/src/store.rs:190-202`, would give `NotATree`), and the
     path still does not exist afterwards.
   - It catches the planted defect of checking the members after the store is opened.
6. `the_other_commands_work_on_a_held_tree`.
   - `tree status` exits 0. Its cases, revisions, decisions and approvals equal those of an
     unheld copy.
   - `tree close --case root --outcome done` gives the same exit and refusal on the held tree as
     on the unheld copy: `OutcomeNotLegitimate` at the first wait.
   - `tree verify` exits 0 after `tree hold`, and again after `tree reopen`.
7. `hold_and_reopen_wait_for_no_writer`.
   - The test takes the store lock itself, as
     `a_second_writer_is_refused_store_in_use_naming_the_holder` does (`tree_run.rs:389-399`).
     While it holds it, `tree hold` exits 1 naming `StoreInUse`.
   - The same is done for `tree reopen` on a held copy.
   - The store is byte-identical afterwards.

**`crates/loom-cli/tests/tree_test/mod.rs`**

The fixture file `fixtures/tree-suite/stop.suite.yaml` has one cycle over `tour@1`. These tests
run `tree test` on it alone, not on the module's shared run, and each asserts its time under the
module's 5 s (`within_bound`, `tree_test/mod.rs:183-186`).

8. `the_stop_steps_run_as_runbooks_cycle`.
   - The cycle's steps are `open`, `run rounds: 1 exit: 3`, `stop`,
     `resume exit: {not: 0} refusal: TreeStopped`, `reopen` and `run rounds: 1 exit: 3`.
   - Its checks: a `log` count of `StopRecorded` is 1, and a `log` count of `StopLifted` is 1.
   - The cycle passes.
   - Failing first: today the cycle is reported pending and waits for
     `story:tree-stop-record-and-labels`.
9. `a_resume_of_a_running_tree_runs`. A cycle of `open` and `resume exit: 3` passes. `resume`
   is an ordinary `tree run` when no hold is recorded.

**`crates/loom-tree/tests/suite_format/mod.rs`**

10. `runbooks_cycle_shapes_decode` (`:1104-1128`) now expects only
    `("replay-unchanged", "replay")` as waiting. Failing first: it lists `bugfix-full`
    (`StopRecorded`) and `run-stop` (`stop`).
11. `the_stop_kinds_are_available`.
    - `SUITE_AVAILABILITY` holds the three steps and the two events as `Available`.
    - `allow` and `Labelled` still hold `Waits("story:tree-stop-record-and-labels")`.
    - A `stop` step with `case: x` is refused at decode, naming the member.

**Docs.** `website/docs/reference/cli.md` is regenerated, and `docs/contracts/run-log-v1.json`
lists the two events (`crates/loom-docs/src/run_log.rs`, check mode).

**Gate.** `task check` exits 0.

## Out of scope

- The `approve` members `result`, `reasons` and `deny`
  (`story:tree-suite-approve-result-and-reasons`).
- The step `allow` and the event `Labelled`. Runbook names no cycle that uses them, and what
  `allow` does is not defined anywhere in the plan.
- Labels at `tree open`, and `{work}` in a printed handoff, which the w11 draft bundled.
- `tree status` showing the hold. The `tree run` refusal prints it; a member of `TreeStatus`
  would change the `loom.tree-status` format.
- `tree stop` (`story:tree-daemon`).
- A hold that `tree run` records itself.
- Runbook's change of its `run-stop` check.

## Scope

Derived 2026-11-22 from the plan tree at `16fe70d4`; the cites were read at `c260bdd1`.
**Cited** means read at the line named; **inferred** means a reading that could be wrong.

- `crates/loom-tree/src/suite/run.rs` (`:584-590`, `:623-629`, `:977`, `:1023-1036`,
  `:1071-1074`) — cited.
- `crates/loom-tree/src/suite/decode.rs` (`:667-707`) — cited.
- `crates/loom-tree/src/suite/availability.rs` (`:55`, `:72-109`) — cited.
- `crates/loom-tree/src/run.rs`: the refusal after the lock (`:190`) — cited.
- `crates/loom-tree/src/stop.rs`: the hold and the reopen — inferred (a new file).
- `crates/loom-tree/src/lib.rs`: the exports — inferred.
- `crates/loom-tree/src/approve.rs` (`:24`) and `crates/loom-tree/src/store.rs` (`:190-202`), read
  and not changed — cited.
- `crates/loom-cli/src/lib.rs` (`TreeCommand`, `:266-293`; `:497`) and
  `crates/loom-cli/src/tree.rs`: the two subcommands — cited.
- `crates/loom-runlog/src/lib.rs`: event encoding — cited for the file, inferred for the place.
- `ess/domains/runlog.yaml` (`:36-52`) and `ess/domains/tree.yaml` (`:1112-1138`, the suite step
  type) — cited.
- `generated/rust/loom`, `generated/ess/loom` and `docs/contracts/run-log-v1.json` — cited.
- `crates/loom-cli/tests/tree_run.rs` (`:38-44`, `:601-604`) — cited.
- `crates/loom-cli/tests/tree_run/stop.rs` — inferred (a new file).
- `crates/loom-cli/tests/tree_test/mod.rs` and `crates/loom-cli/tests/fixtures/tree-suite` —
  cited.
- `crates/loom-tree/tests/suite_format/mod.rs` (`:1104-1128`) — cited.
- `website/docs/guides/run-a-case-tree.md` (`:509`), `website/docs/reference/cli.md` and
  `CHANGELOG.md` — cited.

## Edges

- `depends_on story:tree-suite-format`: the runner, the step kinds and the availability table
  this story fills in.
- `depends_on story:cancel-a-running-bound-program`: the `resume` step's `tree run` holds the
  cycle's cancel flag, as a `run` round does.
- **Order**, stated in prose with no depends_on:
  1. `story:person-action-runs-a-program`;
  2. the five suite stories: `suite-runs-cycles-in-parallel`,
     `suite-cycle-writes-a-workspace-file`, `suite-runs-cycles-under-one-cgroup-root`,
     `tree-test-command`, `tree-test-decide-and-requests`;
  3. `story:cancelled-step-may-run-again`;
  4. `story:handoff-check-holds-the-cancel`;
  5. `story:tree-suite-approve-result-and-reasons`;
  6. this story, which merges all their heads.
- **Would collide with**, no edge:
  - `story:person-action-runs-a-program` (active):
    - the two `ApproveRequest` literals at `suite/run.rs:973` and `:1143`, which it edits for
      `runner`;
    - `crates/loom-cli/src/lib.rs`, `crates/loom-cli/src/tree.rs`,
      `crates/loom-cli/tests/tree_run.rs`, `ess/domains/tree.yaml` and
      `crates/loom-tree/src/lib.rs`.
  - The five suite stories on `suite/run.rs` (the step match), `suite/availability.rs`,
    `tree_test/mod.rs`, `crates/loom-cli/src/tree.rs` and `crates/loom-cli/tests/tree_run.rs`
    (`:38-44`; `suite-runs-cycles-under-one-cgroup-root` adds a `#[path]` module there and in
    `crates/loom-cli/tests/main.rs`).
    - `story:suite-cycle-writes-a-workspace-file` also on `suite/decode.rs`
      (`step_kind`/`step_members`) and on the suite step type in `tree.yaml`.
    - `story:tree-test-command` and `story:tree-test-decide-and-requests` also on
      `crates/loom-cli/src/lib.rs` and the guide's table at `run-a-case-tree.md:509`.
  - `story:cancelled-step-may-run-again` on `crates/loom-tree/src/run.rs`.
  - `story:handoff-check-holds-the-cancel` on `crates/loom-tree/src/run.rs`,
    `ess/domains/runlog.yaml`, `docs/contracts/run-log-v1.json` and the guide.
  - `story:tree-suite-approve-result-and-reasons` on `suite/run.rs`, `suite/availability.rs`,
    `tree_test/mod.rs`, the `tree-suite` fixtures and `suite_format/mod.rs`.
  - `story:tree-library-exports-what-an-engine-needs` (active) on `crates/loom-tree/src/lib.rs`
    and `crates/loom-cli/src/tree.rs`.
  - `story:tree-daemon` (draft) keeps the verb `tree stop`, which ends a served tree's run after
    its current step. It may later write this story's hold record; that is its own decision.
