---
format: aep.planning-md/3
id: story:tree-status-names-what-it-read
kind: story
status: proposed
title: tree status lists the protocols it used, says when a case's run log did not verify, and names the budget that ended a run
summary: protocols[] filled from the catalog with the sha256 of each protocol's own bytes, a log member per case, an optional budget member on the stopped line (Cancel, MaxSteps, MaxWallSeconds); after child-record-in-the-run-log, before concurrent-confined-runs
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:tree-run-command
- depends_on: story:open-children-from-arguments-from
- depends_on: story:child-record-in-the-run-log
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T04:36:53Z", actor: "agent:loom", revision: 4}
---
## Outcome

`tree status` names three things it reads today without saying so:

- **Protocols.** `protocols[]` holds one entry per protocol the tree's cases use, from the catalog
  the status was read with, in the shape of `loom.tree.TreeProtocol`: `registration`
  (`name@major`), `protocol`, `revision`, `file` (`ProtocolSource.path`) and `sha256`, the
  SHA-256 of the protocol's own document bytes (`ProtocolDefinition.sha256`, which `add_yaml`
  computes from the YAML as read). `stored_catalog` still refuses a `--protocols` store, so in
  this build the catalog is always the host one.
- **Log.** Each case entry gains `log`, a `loom.tree.StatusLog`:
  - `verified: true` when the run log verifies or is missing (`verify` reads that as no line);
  - otherwise `verified: false` with `refusal` (a `loom.runlog.RunLogRefusal` variant), `line`
    and `detail`, as `verify` gives them.
  The entry then holds no stop, approvals or actions from that log; `tree status` prints, exit 0.
- **Budget.** A `stopped` line with stop `BudgetSpent` gains `budget`, the budget that ended the
  run. The case entry carries it as `budget`, from its last `stopped` line. Other stops hold none.

Loom's budgets, in Loom's terms, each with its exact `budget` spelling and `TreeRun.detail`:

- Cancel handle, `run.rs:162-163`: `Cancel`; detail `the run was cancelled`.
- Step budget `max_steps`, `run.rs:164-165`: `MaxSteps`; detail `the run took its <n> steps`.
- Wall time `max_wall_seconds`, `run.rs:166-170`: `MaxWallSeconds`; detail
  `the run's <n> s of wall time are spent`.

Two more limits, and why neither gets a spelling:

- An action's run limit `budget.max_runs` (`ess/domains/bindings.yaml:121-127`, checked at
  `crates/loom-bindings/src/program.rs:273-279`) does not end a tree run `BudgetSpent`. It is the
  program refusal `RunsSpent`, written in that action's `action finished` line as `refusal`.
- Loom starts no model in a tree run, so there is no model-call budget. An Agent binding is
  refused before anything runs (`an_agent_binding_is_refused_before_anything_runs`), so its
  `max_turns` and `max_tokens` never apply.

Runbook agreed to the three changes and maps these spellings on its side. The unit confirms the
list with Runbook before it starts.

## Why

Runbook's reader found three gaps:

- `protocols[]` is always `Vec::new()` (`status.rs:165`), though `tree.yaml:290-292` says it lists
  protocols. No reader can tell which protocol bytes a case was decided under.
- `log_lines` (`status.rs:171-174`) turns a log that does not verify into no line. A broken log
  reads as a case that never stopped, approved or acted, and nothing says so.
- A `BudgetSpent` stop writes only `stop` and `next` (`run.rs:188-195`). Which budget ended the
  run is only in `TreeRun.detail`, so the log and the status lose it.

Format versions:

- **`loom.tree-status/1`:** additive. `read_status` reads only the members it names
  (`read.rs:168-200`), so an old reader reads a new status. `protocols[]` entries keep the
  members `read.rs:152-160` requires. The format stays `loom.tree-status/1`.
- **Run log:** not additive for an older reader. The decoder is closed (`closed_at`,
  `crates/loom-runlog/src/lib.rs:586`) and refuses a `stopped` line with `budget`. This is
  handled as in `story:child-record-in-the-run-log`: the contract stays `run-log-v1`, the line
  holds `budget` only on a `BudgetSpent` stop, and `CHANGELOG.md` says older readers refuse
  those lines. `run-log-v2` is the alternative, for Runbook to choose.

## ESS first

`ess/` changes, then regenerated (`task drift`, `task docs-generate`), in its own first commit:

- `ess/domains/tree.yaml`:
  - new enum `loom.tree.SpentBudget` with variants `[Cancel, MaxSteps, MaxWallSeconds]`;
  - new struct `loom.tree.StatusLog` with `verified`, `refusal`, `line` and `detail`;
  - `CaseStatus` gains `log` and `budget: Optional<loom.tree.SpentBudget>`;
  - the comment of `TreeStatus.protocols` says the list comes from the catalog.
- `ess/domains/runlog.yaml`: `loom.runlog.Stopped` gains `budget: Optional<String>`. Its comment
  names the three spellings and says `budget` is present exactly when `stop` is `BudgetSpent`.

ESS field types cannot say "present exactly when". The unit records that rule as the red check
`a_stopped_line_with_a_budget_and_another_stop_is_refused` in `crates/loom-runlog`.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no model call, no
network. Each test is written first and fails first.

The fixture is shared, never a whole tree run per test: the two-level tree's store after one run
with `max_steps: 1`, committed beside `tests/fixtures/tree-two-level` with the digest of its
sources. One `slow_` test regenerates it and compares bytes; each fast test copies it and runs
only `tree status`. The unit reports each new test's time and names any test over 5 s.

Tests:

- `protocols[]` has one entry per protocol the snapshot's cases use, each with the
  `ProtocolDefinition.sha256` of its YAML bytes; a protocol two cases use is listed once.
  Planted defect: restoring `Vec::new()` turns it red. Test:
  `a_status_names_each_protocol_its_cases_use_with_its_digest`.
- Every case's `log` in the snapshot is `verified: true`. After a changed byte in the child's log
  that case shows `verified: false`, `refusal: BrokenChain`, the line and the detail, the root
  stays verified, and `tree status` exits 0. Planted defect: restoring `unwrap_or_default` in
  `log_lines` turns it red. Test: `a_status_names_a_log_that_does_not_verify`.
- A missing log reads as `verified: true` with no stop. Test:
  `a_status_reads_a_missing_log_as_verified_and_empty`.
- With the cancel handle set before the first step, the run takes no step and writes `stopped`
  with `budget: Cancel`. Test: `a_cancelled_run_names_its_budget_in_the_stopped_line`.
- The snapshot's `BudgetSpent` line holds `budget: MaxSteps`, and its status entry carries it.
  Planted defect: leaving `budget` out of the line turns it red. Test:
  `a_step_budget_stop_names_max_steps_in_the_log_and_the_status`.
- A run with `max_wall_seconds: 1` writes `budget: MaxWallSeconds`. It is timed and stays under
  5 s. Test: `a_wall_time_stop_names_max_wall_seconds_in_the_stopped_line`.
- `verify` refuses as `BrokenChain`, naming the line, a `stopped` line with `budget` and another
  stop, and a `budget` outside the three spellings. Tests:
  `a_stopped_line_with_a_budget_and_another_stop_is_refused`,
  `a_stopped_line_with_an_unknown_budget_is_refused`.
- An old log, as fixed bytes, still verifies, and its `stopped` lines read with `budget` `None`.
  Test: `an_old_log_without_a_budget_still_verifies`.
- An old status, as fixed bytes, is read by `read_status`. A new status, printed and read back,
  gives the same value. Tests: `an_old_status_without_log_and_budget_reads` and the existing
  `the_printed_status_is_the_value_of_tree_status`.
- The spellings are the list in this story. Runbook maps them, and none of Runbook's own spellings
  appears in Loom's code, ESS or docs. Test: `the_budget_spellings_are_the_three_of_the_story`.

## Out of scope

- Building `tree open --protocols` and reading `tree-protocols.json`.
- A new budget, a model-call budget, or making `RunsSpent` end a tree run.
- `tree verify`'s output, and Runbook's mapping of the spellings.

## Scope

- `crates/loom-tree/src/status.rs`, cited: `PROTOCOLS_FILE` `:29`, `stored_catalog` `:63-75`,
  the case loop `:110-155`, `protocols` `:165`, `log_lines` `:171-174`, `last_stop` `:177`.
- `crates/loom-tree/src/run.rs`, cited: the budget checks `:162-172`, the `BudgetSpent` stop
  `:188-203`, the other `stopped` line `:366-374`.
- `crates/loom-tree/src/read.rs`, cited: `case` at `:130`, `protocol` at `:152-160`,
  `read_status` at `:168-200`.
- `crates/loom-tree/src/encode.rs`: the new members. Inferred.
- `crates/loom-protocols/src/lib.rs`, read only, cited: `ProtocolEntry` at `:63-68`, the digest
  at `:273-278`.
- `crates/loom-runlog/src/lib.rs`: the `Stopped` encoder, `event_members` and decoder, and
  `closed_at` at `:586`. Cited.
- `crates/loom-runlog/tests/run_log.rs`. Cited.
- `ess/domains/tree.yaml`, cited: `TreeStop` at `:29-31`, `TreeProtocol` at `:133-147`,
  `CaseStatus` at `:252-290`, `TreeStatus` at `:290-309`.
- `ess/domains/runlog.yaml`: `Stopped` at `:149-156`. Cited.
- `generated/`, `docs/contracts/run-log-v1.json`, `website/docs/reference/ess/`: regenerated.
  Inferred.
- `crates/loom-cli/tests/tree_run.rs`: the status tests at `:639-810`. Cited.
- `CHANGELOG.md`: `[Unreleased]` at `:13`. Cited.

Edges:

- depends_on `story:tree-run-command`, `story:open-children-from-arguments-from` and
  `story:child-record-in-the-run-log`.
- Shares `ess/domains/runlog.yaml` and `crates/loom-runlog` with
  `story:child-record-in-the-run-log`, and starts after it.
