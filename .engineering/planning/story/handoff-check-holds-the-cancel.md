---
format: aep.planning-md/3
id: story:handoff-check-holds-the-cancel
kind: story
status: proposed
title: A handoff_when check child holds the run's Cancel and ends within 7 s of a cancel
summary: run_handoff_check takes the run's cancel; both check sites pass it; a cancelled check writes its handoff checked line refused Cancelled and the run stops Cancelled; planted defect per claim
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:cancel-a-running-bound-program
- depends_on: story:handoff-when-a-program-decides
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/handoff.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_retry.rs
- confidence: cited
  path: crates/loom-tree/src/handoff.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: crates/loom-tree/tests/main.rs
- confidence: cited
  path: docs/contracts/run-log-v1.json
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: inferred
  path: website/docs/guides/run-a-case-tree.md
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T12:49:49Z", actor: "agent:loom", revision: 11}
---
## Outcome

A `handoff_when` check child holds the same `Cancel` as the run's programs. A cancel set while
the check runs kills it, with every process it started, within the 7 s that
`story:cancel-a-running-bound-program` holds for a program (`cancel-a-running-bound-program.md:106`,
`:208-210`). The run then stops `Cancelled`, naming the action whose check was cancelled.

**Security-relevant.** This is about the lifetime of a confined child. Today a check outlives the
cancel of its run by up to its timeout (`timeout_seconds`, default 60 s, at most 3600). For that
time it holds its read-only view of the workspace, its slot and any process it started.

### What happens today

- `run_handoff_check` (`crates/loom-bindings/src/handoff.rs:79-174`) builds its child with
  `bound_case_child(...)` and `.with_read_only_workspace()` (`:115-125`), then runs it with
  `runner.run_child(&child)` (`:127`). It takes no cancel, so the child holds none. A cancel is
  read only by a child given one (`ChildCommand::with_cancel`,
  `crates/loom-intake-slice/src/confinement.rs:1600-1612`; `Cancel`, `:210-220`).
- The tree's checks hold no cancel either. `HandoffChecks` has `governor`, `logs`, `case`,
  `bindings`, `workspace`, `store` and `runner` (`crates/loom-tree/src/handoff.rs:32-41`). It is
  built in `run_tree` beside the effect port that does hold the run's `Cancel`
  (`crates/loom-tree/src/run.rs:298-320`).
- `run_tree`'s bridge sets the run's `Cancel` within 100 ms of the caller's flag
  (`CANCEL_WATCH`, `run.rs:128-165`). Nothing reaches the check, so a check set running ends at
  its own exit or at its timeout. At the timeout the run is refused `HandoffCheckFailed`
  (`handoff.rs:176-177`, `:85-96`).
- A check runs in two places:
  - before a step, in `Checked::run_with_usage` (`crates/loom-tree/src/handoff.rs:133-190`);
  - at a stuck stop, in `handoffs` (`crates/loom-tree/src/run.rs:1139-1181`).

  Both call `HandoffChecks::check` (`handoff.rs:50-84`).

### The change

- `run_handoff_check` gains a parameter `cancel: Option<&Cancel>`, and the child takes it with
  `.with_cancel(cancel.clone())` when it is given.
- `HandoffChecks` gains `cancel: &Cancel`, the run's. Both places a check runs use it.
- **A check that a cancel ended** (the runner answers `ConfinementRefusal::Cancelled`, before or
  during the run):
  - Its `handoff checked` line is written with no exit and the refusal
    `Cancelled: action <a>: the check <program> was cancelled`. That is the prefix of a cancelled
    step's `action finished` line (`crates/loom-bindings/src/program.rs:874-888`).
  - `HandoffCheck` gains `cancelled: bool`, so the tree does not read the text.
  - The run stops `Cancelled` with `next: [<a>]` (`cancelled_stop`, `run.rs:952-976`). The
    action's program does not start. `tree run` exits 5 (`crates/loom-cli/src/lib.rs:457`).
- A check that ended before the cancel was read keeps its answer, as a program does.

## Decisions

- **D1. The stop after a cancelled check.** Default **a**.
  - **a.** `Cancelled` (exit 5), as a cancelled step: the person or host asked for the stop, and
    the check did not fail.
  - **b.** `HandoffCheckFailed` (exit 1), as a check killed at its timeout. That needs no new
    path, but a cancel would read as a failure of the check, and a caller could not tell its own
    cancel from a broken check.
- **D2. How the cancel reaches `run_handoff_check`.** Default **a**.
  - **a.** A new parameter. The function is public (`crates/loom-bindings/src/lib.rs:57`); its
    callers are `crates/loom-tree/src/handoff.rs:62` and the test
    `crates/loom-bindings/tests/bound_retry.rs:1610`. Both pass it.
  - **b.** A second function, `run_handoff_check_with_cancel`, beside the first. The signature
    stays, at the cost of two entry points for one check.

## ESS first

No new noun. `loom.runlog.HandoffChecked` (`ess/domains/runlog.yaml:118-135`) keeps its members.
Its comment, "a refusal stopped the run" (`:122-123`), gains: "a cancelled check's refusal starts
`Cancelled:` and the run stopped `Cancelled`". The comment change regenerates
`docs/contracts/run-log-v1.json` if the contract carries the comment; `task drift` tells.

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step ends within 120 s.
- No test makes a network request or a model call, and no test reads a credential file.
- Fixtures are POSIX sh, never Python.
- A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30
  minutes).

All tests go in the `cancel` module of `crates/loom-tree/tests/main.rs` (`:236-529`), on its
`Halt` fixture.

- `work.do`'s binding in the test's own copy gains `handoff_when: [sh, check.sh, <marks>]` and a
  `handoff` text.
- `<marks>` is an absolute directory of the test's scratch, outside the workspace. The check's
  workspace is read-only (`handoff.rs:124`), so it marks its progress there, and the `local`
  runner lets it.
- The fixture's `check.sh` is POSIX sh. It writes `<marks>/started`, starts a process that
  writes `<marks>/late` 3 s later, and sleeps 20 s.
- The flag is set from a thread once `<marks>/started` exists, as `cancelled_after` does
  (`:368-387`).
- Today a test of this kind fails after about 20 s, the check's sleep, which stays under the
  60 s bound.

Each claim has a test, and each test has the planted defect it catches.

1. `a_cancelled_check_ends_within_seven_seconds`.
   - `run_tree` returns within 7 s of the flag.
   - 4 s after it returns, `<marks>/late` does not exist.
   - Planted defect: `HandoffChecks` passes no cancel to the check, which is today's code. The
     run then returns after about 20 s.
2. `a_cancelled_check_stops_the_run_cancelled`.
   - The run is `Ok`, with stop `Cancelled` and `next: [work.do]`.
   - The root log's `handoff checked` line for `work.do` has no exit and a refusal that starts
     with `Cancelled:`.
   - No `action started` line names `work.do`, and the workspace's `target/started` (written by
     `do.sh`) does not exist.
   - Planted defect: a cancelled check is mapped to `HandoffCheckFailed` (D1 b), so the run is
     `Err`.
3. `a_check_at_a_stuck_stop_holds_the_cancel`.
   - `check.sh` counts its calls in `<marks>/n`. The first call exits 1, and the second sleeps
     as above.
   - `do.sh` is `exit 1`. The first check lets the writer run, so `work.do` is performed and
     nothing is useful. The stuck stop runs the second check (`handoffs`, `run.rs:1166-1172`),
     which is cancelled.
   - `run_tree` returns within 7 s of the flag, with stop `Cancelled`.
   - Planted defect: the stuck-stop path checks with a `Cancel::new()` of its own instead of the
     run's. The run then returns after about 20 s.
   - Positive control, `the_stuck_stop_check_runs`: with a second call that exits 0, the same
     fixture stops `AwaitingPerson` with `work.do`'s handoff. This shows the stuck-stop path is
     reached.
4. `a_check_that_ended_keeps_its_answer`.
   - `check.sh` exits 1 at once. The flag is set once `do.sh` has started.
   - The `handoff checked` line has exit 1 and no refusal, and the run stops `Cancelled` on the
     step of `work.do`, as a cancelled step does.
   - Planted defect: every check line of a cancelled run is rewritten as cancelled. This test
     shows a cancel does not reach back into a check that already answered.

**Callers.** `crates/loom-bindings/tests/bound_retry.rs:1610` passes `None` and still passes.

**Gate.** `task check` exits 0.

## Out of scope

- The history of a cancelled step (`story:cancelled-step-may-run-again`). A `handoff checked`
  line is not an `action finished` line, and the history does not read it.
- A cancel for `tree approve` and its person program (`story:person-action-runs-a-program`).
- A check run outside `run_tree`.

## Scope

Derived 2026-11-22 from the plan tree at `c260bdd1`. **Cited** means read at the line named.

- `crates/loom-bindings/src/handoff.rs` (`HandoffCheck`, `:38-49`; `run_handoff_check`,
  `:79-174`) — cited.
- `crates/loom-bindings/src/lib.rs` (`:57`), read and not changed — cited.
- `crates/loom-bindings/tests/bound_retry.rs` (`:1610`) — cited.
- `crates/loom-tree/src/handoff.rs` (`:32-41`, `:50-96`, `:133-190`) — cited.
- `crates/loom-tree/src/run.rs` (`:298-320`, `:952-976`, `:1139-1181`) — cited.
- `crates/loom-intake-slice/src/confinement.rs` (`Cancel`, `with_cancel`), read and not changed
  — cited.
- `crates/loom-tree/tests/main.rs` (`mod cancel`) — cited.
- `ess/domains/runlog.yaml` (`:118-135`, the comment only) and `docs/contracts/run-log-v1.json`
  — cited.
- `website/docs/guides/run-a-case-tree.md` (the `handoff_when` section) and `CHANGELOG.md` —
  inferred.

## Edges

- `depends_on story:cancel-a-running-bound-program`: the `Cancel` the run's ports hold, its
  bridge, and the 7 s bound.
- `depends_on story:handoff-when-a-program-decides`: `run_handoff_check`, `HandoffChecks` and the
  `handoff checked` line.
- **Would collide with**, no edge:
  - `story:cancelled-step-may-run-again` on `crates/loom-tree/src/run.rs` and the `cancel` module
    of `crates/loom-tree/tests/main.rs`. Either may merge first; the second rebases.
  - `story:person-action-runs-a-program` (active) on `crates/loom-bindings/src/handoff.rs` and
    `crates/loom-tree/src/run.rs`, if it runs a check for a person program. It merges first
    because it is active.
