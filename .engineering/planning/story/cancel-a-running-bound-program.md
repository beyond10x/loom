---
format: aep.planning-md/3
id: story:cancel-a-running-bound-program
kind: story
status: active
title: A cancel handle ends a running bound program before its timeout, and the step ends cancelled with no evidence record
summary: Cancel type in the intake slice, ChildCommand::with_cancel, ProgramEffects::with_cancel; Kill at the next read step; Cancelled refusal; no result-table row, no retry
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-run-command
- depends_on: story:concurrent-confined-runs
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_retry.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement/engine.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/engine_confinement.rs
- confidence: inferred
  path: crates/loom-tree
- confidence: inferred
  path: crates/loom-tree/src/status.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: inferred
  path: ess/domains/tree.yaml
- confidence: cited
  path: ess/intake/domains/confinement.yaml
- confidence: inferred
  path: generated/rust/intake
- confidence: inferred
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
- confidence: cited
  path: website/docs/guides/embed-the-runtime.md
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:06:16Z", actor: "agent:loom", revision: 6}
- {from: "proposed", to: "active", at: "2026-11-22T10:25:38Z", actor: "agent:sdc", revision: 9, executor: "agent:loom", correlation: "runbook-cancel-2026-11-22"}
---
## Outcome

A caller can end a running bound program before its timeout. The run's cancel flag is the one
`run_tree` takes today: `cancel: Option<&AtomicBool>` (`crates/loom-tree/src/run.rs:126-132`),
read only before each step (`:166-167`). It now also reaches every program the run starts. Once
it is set, the program is killed, the step ends as a cancelled step, and no process of the
program is left. Cites are at `b9ec2657`.

**The handle.** One small type, `Cancel`, in `b10x_loom_intake_slice::confinement`, the lowest
crate that starts a child. It is a cloneable shared flag over an `Arc<AtomicBool>`:
`Cancel::new()`, `cancel(&self)`, `is_cancelled(&self) -> bool`. Setting it cannot be undone.

**`run_tree` keeps its signature.** `cancel` stays `Option<&AtomicBool>`, so Runbook's call does
not change.

- `run_tree` builds its own `Cancel` from that flag. A watcher in a `std::thread::scope` reads the
  caller's `AtomicBool` every 100 ms, and sets the run's `Cancel` when the flag is set. The scope
  ends when `run_tree` returns, so no thread outlives the run.
- The unit may choose another bridge that keeps the signature and the bounds below, and names
  it. One example is a `Cancel` that borrows the flag.
- This story adds `Cancel` to the intake slice only. Its re-export through the `embed` module of
  `b10x-loom-tree` belongs to `story:tree-library-exports-what-an-engine-needs`, which owns that
  module and depends on this story. This story does not edit `crates/loom-tree/src/lib.rs`.

**How it reaches the child.** `ChildCommand::with_cancel(cancel)` puts the handle on the child,
so `TestRunner::run_child` (`crates/loom-intake-slice/src/confinement.rs:96-106`) keeps its
signature and every runner gets the handle with the child. `ProgramEffects::with_cancel(self,
cancel) -> Self` stores the handle on the port. `ProgramEffects::new`
(`crates/loom-bindings/src/program.rs:143-161`) keeps its parameters. The port gives the handle
to the bound program's child and to its `arguments_from` child (`run_arguments_from`,
`crates/loom-bindings/src/arguments.rs:67`). `OpenEffects` (`crates/loom-bindings/src/open.rs:136`)
takes the handle the same way for the `arguments_from` program it runs. `run_tree` gives its
`Cancel` to every port it builds.

**The kill.** The cancel uses the timeout's kill: `Signal::Kill` with no grace.
- Under `linux`, the read loop of the pipe session (`confinement.rs:752-767`) already wakes
  every `READ_STEP` (1 s, `:831`). It now ends when the handle is set. The loop then sends the
  same `driver.signal(id, Signal::Kill, grace_ms 0)` it sends at the drain deadline (`:769-783`),
  and the whole cgroup ends.
- Under `local`, `finish` (`crates/loom-intake-slice/src/confinement/engine.rs:572-585`) waits
  on the run, the deadline and the handle (polled every 100 ms). On a cancel it calls the same
  `kill` (`:588-590`, `:716-734`), which kills the process group and waits at most the
  runner's `KillBounds::kill_settle` (`KILL_SETTLE`, 5 s, on a host's runner, `:685-705`).

The kill is sent at the first read step after the cancel, not after another drain margin.
Waiting `DRAIN_MARGIN` (30 s, `:813-815`) would put the bound at 31 s. **The bounds:**

- `run_child` returns within 7 s of the cancel under either adapter with a host's `KillBounds`.
- `run_tree` returns to its caller within 10 s of the caller setting its flag during a step:
  - 100 ms for the watcher;
  - the 7 s above;
  - writing the step's `action finished` line and the case's `stopped` line.
A child that has a cancel but no standard input runs as a child with empty standard input, so
every cancelled child goes through that loop. A bound program always has standard input
(`crates/loom-intake-slice/src/bindings.rs:438`).

**What `run_child` answers.** A cancel set before the start is refused
`ConfinementRefusal::Cancelled` before any check or driver open, and nothing starts. A child
killed by its cancel is refused `Cancelled` with the detail `the child was killed by its cancel
after <seconds> s`. A cancel set after the child ended changes nothing: the result is the one
it would be without the cancel.

**The cancelled step.** `ProgramEffects::perform` (`program.rs:215-509`) reads the handle in
two places. The first is after the run count (`:238-248`) and before the `arguments_from`
program and the bound program start (`:288`). A handle set there refuses the step `Cancelled`,
starts nothing and keeps no observation. A `Cancelled` answer of `run_child` for either child
follows the runner-refusal path (`:337`, `:370-377`):

- the run's observation is kept, and it counts against `budget.max_runs`, as every call of
  `run_child` does (`:309-311`). `ProgramRun` gains `cancelled: true`, and its
  `runner_refusal` names `Cancelled`;
- the placed artifacts' moves are written alone;
- no evidence record is written, no result table is read, and no blob is kept, since the answer
  holds no output.

The answer is `EffectOutcome::Refused` with the reason `Cancelled: action <action>: <detail>`
(`ProgramRefusalReason::Cancelled`, the form of `refusal`, `:807-817`) and `ends_run: true`. The
Commission run then ends `EffectRefused` and invokes the action no second time. In a tree run
the step's `action finished` line holds `refusal` `Cancelled: …`, no `exit` and no `evidence`
(`loom.runlog.ActionFinished`, `ess/domains/runlog.yaml:93-108`). `run_tree` then asks the
flag and stops as a cancelled run stops before a step.

**The cancelled stop.** That stop is `Cancelled`, a new member of `loom.tree.TreeStop`
(`ess/domains/tree.yaml:29-32`), and not `BudgetSpent`.

- This also changes the stop before a step. Today a flag set between steps writes `stop`
  `BudgetSpent` with `next` empty and returns `TreeStop::BudgetSpent` with the detail "the run
  was cancelled" (`run.rs:166-167`, `:192-208`). After this story both are `Cancelled`. An
  embedder that matched `BudgetSpent` for a cancel sees `Cancelled`, and `CHANGELOG.md` names
  it under `**Breaking:**`.
- The case's `stopped` line holds `stop` `Cancelled` and `next` the action whose step was
  cancelled (for a cancel before a step, the action that was to be taken next).
- `tree status` therefore shows, for that case, `stop` `Cancelled` and `stop_next` `[<action>]`.
  The case's `runs` entry for the cancelled run (`loom.tree.StatusRun`, `tree.yaml:209-222`)
  gains `cancelled: true`, read from `ProgramRun.cancelled` and absent on every other run.
- `b10x-loom tree run` still exits 5.

**Retries.** A cancelled step writes no evidence. So `retry_records`
(`crates/loom-bindings/src/retry.rs:169-241`), which reads evidence only, gets nothing from it.
The next run of the action gets no `LOOM_RETRY_FEEDBACK` because of the cancelled step, and its
`retry_of` does not name it.

**The timeout path is unchanged.** Substrate still ends a child at its timeout, Loom still kills
it after `DRAIN_MARGIN` under `linux` and at the deadline under `local`, and `timeout` is still
the key a timed-out run reads.

## Why

Runbook's daemon runs many trees in one process and must be able to stop one of them now: an
operator's stop, a run that is superseded, a daemon shutting down. A tree's cancel is asked only
between steps (`story:tree-run-command`). A bound program may run up to 3600 s
(`crates/loom-bindings/src/lib.rs:77`), and nothing can end it before its timeout: `run_child`
blocks until the end (`confinement.rs:599-633`), only `spawn_child` returns a child that can be
signalled (`:649-675`, `confinement/child.rs:198`), and the port uses `run_child` alone
(`program.rs:309`, `arguments.rs:67`). Timo: runs "in an isolated, secure way"; an operator who
cannot stop a run does not control it.

## ESS first

`ess/` changes, with one new member in each of three declared types:

- `ess/intake/domains/confinement.yaml:12-14`: `intake.confinement.ConfinementRefusal` gains
  `Cancelled`;
- `ess/domains/bindings.yaml:417-427`: `loom.bindings.ProgramRefusalReason` gains `Cancelled`;
- `ess/domains/bindings.yaml:315-334`: `loom.bindings.ProgramRun` gains `cancelled: Boolean`.
- `ess/domains/tree.yaml`: `loom.tree.TreeStop` (`:29-32`) gains `Cancelled`, and
  `loom.tree.StatusRun` (`:209-222`), the per-case run entry of `tree status`, gains
  `cancelled: Optional<Boolean>`.

On that commit `slot task drift` and `slot task intake-drift` (`Taskfile.yml:103`, `:61`) both
fail.
The new tests in `crates/loom-bindings/tests/bound_program.rs` and
`crates/loom-intake-slice/tests/engine_confinement.rs` do not compile, because `Cancel` and
`with_cancel` do not exist yet.

## Acceptance

The fixtures are those of `crates/loom-bindings/tests/bound_program.rs`: a protocol with a
`program` action bound to a shell stub, a `local` runner opened with `UNCONFINED_FLAG_REASON`
(`:236`), no model and no network. The stub that is cancelled is
`sh -c 'sleep 3 && touch target/late & sleep 30'`, with `timeout_seconds: 60`. Each `linux` test
uses the delegated harness of `crates/loom-intake-slice/tests/support/mod.rs:37-126`, skips
naming the missing prerequisite where there is no delegated cgroup v2, and is `#[ignore]` off
Linux. Each test ends within 60 s, and the test step stays within 120 s. The new tests of
`crates/loom-bindings` and `crates/loom-intake-slice` go into existing files, which become
modules of `tests/main.rs` under `story:one-test-binary-per-crate`. The tests "in
`crates/loom-tree`" go into `crates/loom-tree/tests/main.rs`, which is that crate's one test
binary today.

- A bound program cancelled 1 s after it started ends, and `perform` returns within 7 s of the
  cancel under `local`. Test: `a_cancelled_program_ends_within_seven_seconds`.
- The same holds under `linux`. Test: `a_cancelled_confined_program_ends_within_seven_seconds`.
- 4 s after the step ended, `target/late` does not exist under either adapter, so no process
  of the program outlived the cancel. Tests: `a_cancelled_program_leaves_no_process` and
  `a_cancelled_confined_program_leaves_no_process`.
- A handle set after a stub that exits 0 at once ended leaves the outcome, records, observation
  and report equal to those of the same step with no handle. Test:
  `a_cancel_after_the_program_ended_changes_nothing`.
- A handle set before `perform` refuses the step `Cancelled` and `ends_run`. A recording runner
  receives no child, and the governor holds no new observation. Test:
  `a_cancel_before_the_start_refuses_the_step_and_starts_nothing`.
- A handle set while the `arguments_from` program runs ends that program, and the bound program
  never starts. Test: `a_cancel_during_the_arguments_program_starts_no_bound_program`.
- A cancelled step records no evidence, even with a binding whose results list `*`, and its
  report has no `evidence`. Test: `a_cancelled_step_records_no_evidence`.
- The cancelled run's observation holds `cancelled: true`, no `exit`, and a `runner_refusal`
  that starts `Cancelled`. With `budget.max_runs: 1` the next step is refused `RunsSpent`. Test:
  `a_cancelled_run_is_observed_and_counted`.
- A Commission run whose step was cancelled ends `EffectRefused`, and the effect port was invoked
  once. Test: `a_cancelled_step_ends_the_run_and_is_not_invoked_again`.
- With `retry_when` naming a check kind, a step cancelled after one recorded check, and then a
  second run of the action: the second program's `LOOM_RETRY_FEEDBACK` document and its
  `retry_of` name only the check record, and nothing of the cancelled run. Test:
  `a_cancelled_step_is_not_retried`.
- Two runs in one process, each with its own handle: cancelling the first leaves the second's
  program to exit 0. Test: `a_cancel_of_one_run_leaves_the_other_running`.
- `run_tree` with its handle set by the observer at `action started` writes one
  `action finished` line with `refusal` starting `Cancelled`, no `exit` and empty `evidence`,
  then stops as a run cancelled before a step does. Test (in `crates/loom-tree`):
  `a_tree_run_cancelled_during_a_step_stops_after_it`.
- An embedder's cancel. `run_tree` blocks on one thread, with its `&AtomicBool` shared through a
  `std::thread::scope`. A second thread sets the flag 1 s after the step's `action started`
  line. `run_tree` returns within 10 s of the set, with `TreeRun.stop` `Cancelled`, and no
  process of the program is left. Test (in `crates/loom-tree`):
  `a_tree_run_cancelled_from_another_thread_returns_within_ten_seconds`.
- A flag set between steps gives `TreeRun.stop` `Cancelled` and a `stopped` line with `stop`
  `Cancelled` and `next` the action that was to be taken next, no longer `BudgetSpent`. Test (in
  `crates/loom-tree`): `a_cancel_before_a_step_stops_cancelled`.
- After that run, `tree_status` gives the cancelled case `stop` `Cancelled` and `stop_next`
  `[<the cancelled action>]`, and the run's `TreeRun.stop` is `Cancelled`. A run whose
  wall-time budget is spent still gives `BudgetSpent`. Test (in `crates/loom-tree`):
  `a_status_names_a_cancelled_stop_and_its_action`.
- In the same status, the case's `runs` entry for the cancelled run holds `cancelled: true`
  and no `exit`, and the case's other runs hold no `cancelled`. Test (in
  `crates/loom-tree`): `a_status_marks_the_cancelled_run`.
- `run_child` under either adapter, given a handle that is already set, refuses `Cancelled`, and
  no marker file is written. Test: `a_child_whose_cancel_is_set_is_refused_before_launch`.
- The timeout path is unchanged. These tests keep every line and pass:
  `a_local_child_past_its_timeout_is_killed_with_its_group` and
  `a_local_child_with_stdin_past_its_timeout_is_killed_with_its_group`
  (`engine_confinement.rs:627`, `:646`), `a_timed_out_child_is_a_result_with_no_exit_status`
  (`confined_child.rs:260`), `a_timeout_records_the_result_of_the_timeout_key`,
  `a_timeout_without_a_timeout_key_falls_to_star` and
  `a_timeout_with_neither_key_is_refused_result_not_in_table` (`bound_program.rs:513-538`).
- The generated code matches the specification. Command: `slot task drift` and
  `slot task intake-drift` exit 0 after `task generate` and `task intake-generate`. On the ESS
  first commit they fail.
- `slot task check` exits 0.

## Out of scope

Changes to Substrate, to Canon, and to the CLI's default. The CLI wires no signal to a cancel
handle; its interrupt handling (`crates/loom-cli/src/main.rs:191`, `:760`) stays as it is. A
graceful stop (`Signal::Term` with a grace period) before the kill. Cancelling an agent node or a
model call. `ConfinedChild::signal`, which a live child already has. The fixed text
`UNCONFINED_FLAG_REASON` stays; Runbook decided no story for it. The tree daemon's `tree stop`
(`story:tree-daemon`).

Depends on `story:tree-run-command` (the handle `run_tree` takes) and
`story:concurrent-confined-runs`, and is built after both.

## Scope

Refreshed 2026-11-22 from the plan tree at `b9ec2657` (`wave/2026-11-21-w7`). **Cited** means
read at the `file:line` named. **Inferred** means a reading that could be wrong.

- **Primary surface:** `crates/loom-intake-slice/src/confinement.rs`: `Cancel` (new);
  `ChildCommand::with_cancel` beside `with_timeout` and `with_stdin` (`:1438`, `:1449`);
  `SubstrateRunner::run_child` (`:599-633`); the read loop and kill of `launch_with_stdin`
  (`:718-810`); `DRAIN_MARGIN` and `READ_STEP` (`:813-831`, unchanged) — cited
- **Files (source):** `crates/loom-intake-slice/src/confinement/engine.rs`: the `local`
  `launch` (`:492-548`), `finish` (`:572-585`), `kill` (`:588-590`, `:716-734`), `KillBounds`
  (`:685-705`) — cited
- **Files (source):** `crates/loom-bindings/src/program.rs`: the port's fields (`:113-120`),
  `with_cancel` beside `new` (`:143-161`), `perform` (`:215-509`), `arguments` (`:541-578`),
  `refusal` (`:807-817`), `run_json` (`:863-877`) — cited
- **Files (source):** the `OpenEffects` port (`crates/loom-bindings/src/open.rs:136`) and
  `run_arguments_from` (`crates/loom-bindings/src/arguments.rs:67`) — cited
- **Files (source):** `crates/loom-tree/src/run.rs`: `run_tree` keeps its signature
  (`:126-132`), builds its `Cancel` from the flag and gives it to each port it builds; the stop
  before a step (`:166-167`, `:192-208`) becomes `Cancelled` — cited for the lines, inferred for
  the ports' construction sites
- **Not this story's:** `crates/loom-tree/src/lib.rs`. The `Cancel` re-export through `embed`
  belongs to `story:tree-library-exports-what-an-engine-needs`, which depends on this story
- **Files (ESS source):** `ess/intake/domains/confinement.yaml:12-14`,
  `ess/domains/bindings.yaml:315-334`, `:417-427`, `ess/domains/tree.yaml:29-32`, `:209-222` —
  cited
- **Files (generated):** `generated/rust/intake/**`, `generated/rust/loom/**`,
  `website/docs/reference/ess/intake-confinement.md`, `website/docs/reference/ess/loom-bindings.md`
  — cited for the pages, inferred for the paths of the regenerated Rust
- **Not changed:** `crates/loom-bindings/src/retry.rs` (it reads evidence only, `:1-16`),
  `crates/loom-intake-slice/src/confinement/child.rs`, `ess/domains/runlog.yaml` (`refusal` of
  `ActionFinished` is a string, `:93-108`) — cited
- **Files (tests):** `crates/loom-bindings/tests/bound_program.rs`,
  `crates/loom-bindings/tests/bound_retry.rs`,
  `crates/loom-intake-slice/tests/engine_confinement.rs` (`local` timeouts `:627-701`),
  `crates/loom-intake-slice/tests/confined_child.rs` (the delegated cases),
  `crates/loom-tree/tests/main.rs` — cited
- **Files (docs):** `website/docs/guides/bindings.md` § Running a program binding (`:229`);
  `website/docs/guides/embed-the-runtime.md`, section "Run a case tree" added by
  `story:tree-library-exports-what-an-engine-needs`: the cancel flag of `run_tree`, set from
  another thread, ends a running program within the bounds above;
  `website/docs/guides/run-a-case-tree.md` § Run it until it waits (`:51`): the stop `Cancelled`.
  `AGENTS.md` § Bindings document (`:1348`) and the pipe-session paragraph (`:2674-2690`).
  `CHANGELOG.md` one `Added` bullet and one `**Breaking:**` bullet (the stop before a step)
  under `[Unreleased]` (`:13-15`) — cited for the lines, inferred for the "Run a case tree"
  section, which is not on the tree yet
- **Would collide with:** `story:concurrent-confined-runs` on `confinement.rs` (that story
  first); `story:bound-program-contract` on `ProgramRun` (`ProgramRun.class`) and
  `ProgramRefusalReason` in `ess/domains/bindings.yaml` and on `program.rs`;
  `story:per-run-program-path` on `program.rs` (a second builder beside `with_cancel`); the unit
  that merges second adds its member to the type as merged — cited from those stories' Scope
  sections, inferred for the order. `story:tree-library-exports-what-an-engine-needs`, which
  depends on this story, on `website/docs/guides/embed-the-runtime.md` (`:52-69`) and on the
  tests of `crates/loom-tree/tests/main.rs`; it re-exports `Cancel` in its own `embed` module.
  The later unit merges the earlier head
- **Exhaustive matches:** a new `ConfinementRefusal` member breaks every `match` over it that
  has no wildcard (exit-status and label maps in `crates/loom-cli`) — inferred
