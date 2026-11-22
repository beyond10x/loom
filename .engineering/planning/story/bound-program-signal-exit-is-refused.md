---
format: aep.planning-md/3
id: story:bound-program-signal-exit-is-refused
kind: story
status: active
title: A bound program a signal ended under Substrate is refused naming its signal, never read as an exit of the result table
summary: exit 129..=192 (SIGNAL_EXITS) refused ProgramSignalled before the exit classes and the table, ends the run, counts; timeout still reads timeout; local unchanged; security-relevant
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:bound-program-contract
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/handoff.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-delegated
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: inferred
  path: crates/loom-cli/tests/tree_run/signal_exit.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: generated/rust/loom
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T15:08:34Z", actor: "agent:loom", revision: 13}
- {from: "proposed", to: "active", at: "2026-11-22T15:09:53Z", actor: "agent:sdc", revision: 14, executor: "agent:loom", correlation: "runbook-batch4-2026-11-22"}
---
## Outcome

A bound program that a signal ended is refused with its signal named. Its exit status is never
matched to a result of the author's table. Cites are at `23f1f4b3`.

**Today.**

- Substrate reports a child a signal ended as exit 128 plus the signal, 137 for SIGKILL (the
  fact: `crates/loom-intake-slice/tests/confined_child.rs:2558`,
  `a_confined_child_a_signal_ended_reports_128_and_the_signal`).
- `ProgramEffects` keeps that as the run's exit (`crates/loom-bindings/src/program.rs:388`).
- `value_of` gives a `from: exit` entry the code as text (`:784`).
- `look_up` then finds the row for `"137"`, or else the `"*"` row (`:809-815`).
- So a program the kernel killed for memory, or that a cgroup kill ended, records the author's
  "any other exit" result as if the program had answered.
- The handoff check already refuses 129..=192 (`SIGNAL_EXITS`,
  `crates/loom-bindings/src/handoff.rs:43`, `:169-176`).

**After, under D1 a.**

- **Where the check goes.** After the run's observation is kept (`program.rs:401-411`) and before
  the exit classes are read (`:453-479`), a run under these conditions refuses the step
  `ProgramSignalled` and ends the run:
  - its execution has an exit status;
  - that status is in `SIGNAL_EXITS`;
  - it did not time out.
- **The refusal text.** The detail names:
  - the program;
  - the exit status;
  - the signal (`code - 128`);
  - the blob digest of its output.

  It never names what the program printed, as `program_error` does (`:932-950`).
- **What is written.**
  - Nothing is recorded.
  - The workspace's revisions are written, as for every refused step (`self.write(&targets,
    Vec::new())`).
- **Counting.**
  - The run counts against `budget.max_runs`: it is a run, as a cancelled one is
    (`ess/domains/bindings.yaml:321-326`).
  - `class` is set before the observation is kept: `run.class = class_of(binding, execution)`
    (`program.rs:390`), and `runs()` leaves out every observation that has a class
    (`:202-222`). So today a signal exit that `exits.error` or `exits.not_called` names is
    classed, recorded with its class and not counted.
  - `class_of` (`:915-929`) returns `None` for an exit in `SIGNAL_EXITS`, before it reads
    `exits`. The observation then holds no class, the run counts, and the refusal below wins
    over the class and over a table row.
- **The order of checks.**
  - A timeout still reads the table's `timeout` row, as today (`value_of`, `:780-782`). A run
    killed at its timeout is not a signal refusal.
  - An `exits.error` or `exits.not_called` list that names a code in 129..=192 is not read for
    such a run: the signal refusal comes first.
  - The same is true for a table row keyed by such a code.
- **Exit statuses outside 129..=192** (0 to 128, 193 to 255) read the table and the classes as
  today.
- **Under `local`** a signal reaches the step without an exit status (as for the handoff check,
  `AGENTS.md:1688-1689`). That path is unchanged.

Security-relevant: yes. Today a killed program can be read as an answer. A crash or an
out-of-memory kill would then move a case through the author's "any other exit" row, with a
result the program never gave. The unit gets a security review.

## Decisions

- **D1. The refusal.** Default **a**.
  - **a.** A new `ProgramRefusalReason` variant, `ProgramSignalled`, that ends the run, as
    `ProgramError` and `Cancelled` do (`ends_run: true`, `program.rs:885-891`). A reader can tell
    a kill from the author's error class.
  - **b.** Reuse `ProgramError` and name the signal in its detail. No new variant, but
    `ProgramError` today means "a code the author's `exits.error` lists" (`bindings.yaml:428`),
    and a kill is not that.
  - **c.** A new variant that refuses the step without ending the run (`ends_run: false`, as
    `ResultNotInTable`), so a retry policy may take the action again. A kill for memory would
    then repeat until the runs are spent.
- **D2. Codes in 129..=192 in a document.** Default **a**.
  - **a.** Refused at run time only, as above. Load accepts the document as today, and the guide
    says such a row or class is never read under Substrate.
  - **b.** Also refuse at load an `exits` entry, or a `from: exit` table key, in 129..=192
    (`InvalidEvidence`, naming the member). That is stricter, but it also refuses a document
    whose program exits 130 by itself under `local`.

## ESS first

`ess/domains/bindings.yaml` comes first:

- `loom.bindings.ProgramRefusalReason` (`:434-443`) gains `ProgramSignalled`.
- The comment before it (`:419-433`) gains: "the run's exit is one Substrate gives a child a
  signal ended, 129 to 192 (`ProgramSignalled`: the step ends the run, its text names the
  signal and the blob digest; the run counts)".

On that commit `task drift` fails until the generated `loom::bindings` is regenerated.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call, no credential
file read. Fixture programs are POSIX sh, never Python. A slot job ends within 10 minutes, or
names its time with `slot --timeout` (at most 30 minutes).

**`crates/loom-bindings/tests/bound_program.rs`** (an existing binary). The tests use a fake
runner that answers a chosen exit status, as `handoff_checked` does for the check
(`crates/loom-bindings/tests/bound_retry.rs:1691-1716`).

1. `a_signal_exit_is_refused_naming_the_signal`.
   - For 129, 130, 137, 143 and 192, with the table `{"0": ok, "*": failed}`, the step is
     refused `ProgramSignalled` and ends the run.
   - The detail holds `exit <code>`, `signal <code - 128>` and the blob digest.
   - No record is written, and the kept blob holds the program's output.
   - Failing first: the `*` row records `failed`.
2. `a_table_row_or_exit_class_for_a_signal_code_is_not_read`.
   - A table `{"137": killed, "*": failed}` with exit 137 is refused `ProgramSignalled`.
   - So is `exits.error: [143]` with exit 143. Neither `killed` nor `ProgramError` appears.
   - Failing first: the `137` row records `killed`; `ProgramError` ends the 143 run.
3. `a_signalled_run_counts_even_when_exits_names_it`.
   - With `budget.max_runs: 1` and `exits.error: [137]`, a run answered 137 is refused
     `ProgramSignalled`. Its observation holds no `class`, and the next step is refused
     `RunsSpent`.
   - The same with `exits.not_called: [137]`.
   - Failing first: `class_of` classes 137 (`program.rs:390`), `runs()` leaves the observation
     out (`:202-222`), and the next step runs again instead of `RunsSpent`.
4. `the_codes_beside_the_range_read_the_table`.
   - Exits 128, 193 and 255 record the table's result.
   - A run that timed out with exit 137 reads `timeout`.
   - Failing first: none. It is a guard of the range and of the timeout's order, and it catches
     the planted defect of `128..=255`.

**`crates/loom-cli/tests/tree_run/signal_exit.rs`**, a new `#[path]` module of `tree_run.rs`:

- It runs under real Substrate, with delegation and the harness of `delegated_cgroup.rs`
  (`Scope::start`, `:66-105`).
- It skips without a delegated scope, naming what is missing, and fails instead when
  `LOOM_REQUIRE_DELEGATION=1` (`:22-23`).
- Fixture: a copy of `fixtures/tree-delegated` whose `cell.look` runs `stubs/killed.sh`, POSIX sh
  that writes `target/started` and then runs `kill -KILL $$`.

5. `under_substrate_a_killed_program_is_refused_naming_signal_9`.
   - A refused step does not end `tree run`. The loop writes its `ActionFinished` line with
     `exit` absent and `refusal` set to the refusal text (`crates/loom-tree/src/run.rs:385-411`,
     `effect_summary` `:1130-1162`), and goes on.
   - The refused action is then not due again until the case is evaluated
     (`refused_since`, `run.rs:962-973`). No outcome is legitimate, since `done` needs
     `cell.seen`, and nothing waits on a person.
   - So the run stops `Stuck` (`run.rs:1404`), and `tree run` exits 4
     (`tree_run_exit_status`, `crates/loom-cli/src/lib.rs:461-469`).
   - The test asserts:
     - exit 4 and stop `Stuck`;
     - one `ActionFinished` line for `cell.look` with no `exit`, whose `refusal` starts
       `ProgramSignalled: action cell.look:` and names exit 137 and signal 9;
     - no `cell_look` record, and the root not terminated.
   - Failing first: the `*` row records `unseen` (`fixtures/tree-delegated/cell.bindings.yaml`).
     The line has `exit: 137` and no refusal, and the run also stops `Stuck` with exit 4.
6. `under_local_a_killed_program_is_as_before`.
   - The same tree under `tree run --confinement local` (`signals.rs:55-70`).
   - Substrate's `local` adapter reports no exit status for the killed child, and Loom refuses
     that run ("Substrate's local adapter reports no exit status for the child and no timeout
     ended it", `crates/loom-intake-slice/src/confinement/engine.rs:634-645`).
   - The step is refused "... was refused by its runner: ..." (`program.rs:441-446`), not
     ending the run.
   - The test asserts:
     - exit 4 and stop `Stuck`;
     - one `ActionFinished` line for `cell.look` with no `exit`, whose `refusal` holds "was
       refused by its runner" and not `ProgramSignalled`;
     - no `cell_look` record.
   - Failing first: none. It is a guard that `local` is unchanged.

**Gate.** `task check` exits 0. The unit's report names which Substrate tests ran on this host.

## Out of scope

- 126 and 127, a launcher's failure (`LAUNCH_FAILURE_EXITS`), for a bound program.
- The handoff check, which already refuses these codes.
- D2 b.

## Scope

Derived 2026-11-22 from the plan tree at `23f1f4b3`. **Cited** means read at the line named;
**inferred** means a reading that could be wrong.

- `crates/loom-bindings/src/program.rs` (`:198-222`, `:388-390`, `:401-411`, `:453-479`,
  `:775-815`, `:885-950`) — cited.
- `crates/loom-bindings/src/handoff.rs` (`:43`, `:169-176`), read and not changed, or the
  constant moved to a shared place — cited.
- `ess/domains/bindings.yaml` (`:321-326`, `:419-443`) and `generated/rust/loom` — cited.
- `crates/loom-tree/src/run.rs` (`:385-411`, `:962-973`, `:1130-1162`, `:1404`),
  `crates/loom-cli/src/lib.rs` (`:461-469`) and
  `crates/loom-intake-slice/src/confinement/engine.rs` (`:634-645`), read and not changed —
  cited.
- `crates/loom-bindings/tests/bound_program.rs` — cited. The fake runner is inferred.
- `crates/loom-cli/tests/tree_run.rs` (the `#[path]` list, `:38-46`),
  `crates/loom-cli/tests/tree_run/signal_exit.rs` (new) and
  `crates/loom-cli/tests/fixtures/tree-delegated` — cited.
- `AGENTS.md` (the bound-program paragraph) and `CHANGELOG.md` — inferred.

## Edges

- `depends_on story:bound-program-contract` (active): its `exits` classes and `program_error`
  are what this check comes before.
- `AGENTS.md`, `CHANGELOG.md` and `generated/` are shared with most stories. They are textual
  merges and are not listed below.
- **Would collide with**, no edge. Each entry names the shared file and the merge order. The
  stories were read in the store at `23f1f4b3`; the list holds the ones whose scope or body names
  the file.
  - `crates/loom-bindings/src/program.rs`, named in the story:
    - active: `bound-program-case-id`, `bound-program-result-table`,
      `bound-program-retry-records`, `cancel-a-running-bound-program`,
      `cancelled-step-may-run-again`, `canon-0-11-0-pin`, `concurrent-confined-runs`,
      `handoff-check-holds-the-cancel`, `open-children-from-arguments-from`,
      `person-action-runs-a-program`, `program-reports-its-cost`, `protocol-argument-schema`,
      `tree-run-command`, `tree-run-tests-fit-the-budget` and `tree-status-names-what-it-read`.
      Textual merge, no order;
    - `outcome-when-reads-the-run-count` (active) adds a value source beside `value_of`
      (`:775`). Textual merge, no order;
    - proposed: `capacity-given-to-an-admission-program`, `ess-rules-of-the-runtime`,
      `gate-program-identity-bound-to-spawn`, `mixed-egress-launches`, `observe-reads-documents`
      and `per-run-program-path`. Textual merge, no order;
    - `report-formats-checked-before-the-table` (proposed) also orders checks before the table.
      It merges after this story and keeps the signal check first;
    - draft: `bound-program-run-cap-holds-across-ports`. It reads `runs()`, which this story's
      `class_of` change feeds. It merges after this story.
  - `crates/loom-bindings/src/` as a whole (the scope names the directory, which holds
    `program.rs` and `handoff.rs`):
    - active: `bound-artifact-revisions`, `child-case-runtime` and `record-copy-between-cases`;
    - proposed: `agent-node-run-records`, `bound-agent-node` and `tree-usage-and-model-budget`.

    Textual merge, no order.
  - `ess/domains/bindings.yaml`. This story adds one variant to `ProgramRefusalReason`
    (`:434-443`) and its comment:
    - every story above that adds a variant there;
    - proposed: `observe-compares-generated-trees`, `observe-executor-reads-workspace-files`,
      `observe-git-changed`, `observe-reads-documents`, `gate-run-collects-a-gates-document`,
      `run-caps-per-arrival`, `tcp-reachability-through-an-aperture`,
      `dns-resolution-through-a-resolver-aperture` and `child-case-own-workspace`.

    Textual merge, no order. `observe-git-in-a-linked-worktree` is left out: it does not name
    `ess/domains/bindings.yaml`.
  - `crates/loom-cli/tests/tree_run.rs`, the `#[path]` list:
    - `run-cgroup-root-holds-the-root` (draft), which adds a module beside this one. Textual
      merge, no order;
    - `tree-run-tests-fit-the-test-budget` (draft), which merges before this story. This module
      starts `b10x-loom`, since it tests the binary under Substrate, as that story's rule allows;
    - the other `tree_run` stories named there. Textual merge, no order.
