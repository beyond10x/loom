---
format: aep.planning-md/3
id: story:cancelled-step-may-run-again
kind: story
status: proposed
title: A step a cancel ended is not performed for the step history, so the next tree run takes it again
summary: LogHistory and the refused-since filter skip an action finished line refused Cancelled; the run count still counts the cancelled run; tests on the Halt fixture, red first
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:cancel-a-running-bound-program
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-tree/src/history.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: crates/loom-tree/tests/main.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T13:08:59Z", actor: "agent:loom", revision: 7}
---
## Outcome

A step that a cancel ended does not count as performed in the step history. The next `tree run`
at the same case revision takes the action again, instead of reporting `Stuck` or passing it by.
The run count (`budget.max_runs`) still counts the cancelled run, as
`story:cancel-a-running-bound-program` decided (`ess/domains/bindings.yaml:323-326`).

### What happens today

This was established by reading the code at `c260bdd1`; no probe was run (see Acceptance).

- A cancelled step writes its `action finished` line, refused
  `Cancelled: action <a>: <detail>`, with no exit and no evidence. The run then stops
  `Cancelled` (`crates/loom-tree/src/run.rs:373-397`, `:431-448`; the refusal text comes from
  `crates/loom-bindings/src/program.rs:874-888`). Every port's cancel ends in that one function:
  the open port reaches it at `crates/loom-bindings/src/open.rs:696`.
- `LogHistory::new` (`crates/loom-tree/src/history.rs:29-61`) counts every `ActionFinished` line
  as performed, whatever its refusal. It counts at the revision of the last `evaluated` line
  (`:42-48`) and at the revision the step left the case at: the next `evaluated` line's, else
  the current one (`:36-41`, `:52-54`).
- A cancelled step moves nothing. The next run's `evaluated` line is written only when the
  revision or the decision digest changed (`run.rs:769-780`). So the cancelled action counts as
  performed at the case's current revision.
- `DeterministicSelector::useful` (`crates/loom-executor/src/selection.rs:285-306`) answers "not
  useful" for an action performed at the revision unless the action is due.
- The action is not due either. `step_inputs` drops from `due` every action whose step was
  refused since the case was last evaluated (`run.rs:852-865`), and a cancel is such a refusal.
- **The result.** The next `tree run` passes the cancelled action by. When nothing else is useful
  it stops `Stuck` (exit 4, `crates/loom-cli/src/lib.rs:456`), or it waits or takes another
  action. The action is taken again only once the case revision moves.

## Decisions

- **D1. How a line is known as cancelled.** Default **a**.
  - **a.** By its `refusal`, which starts with `Cancelled: `. `run.rs:947-950` (`cancelled_step`)
    already reads an outcome this way, and `program.rs:874-888` is the only writer of that
    prefix. One function in `history.rs`, `cancelled(&ActionFinished) -> bool`, holds the test,
    and `run.rs` uses it for the line too. There is no format change.
  - **b.** `ActionFinished` gains `cancelled: Optional<Boolean>` in `ess/domains/runlog.yaml`
    (`:94-108`). That is typed, but it changes the run log contract
    (`docs/contracts/run-log-v1.json`), and old logs would have no member.
- **D2. What a cancelled line still counts for.** Default **a**.
  - **a.** It counts for nothing in the history: not performed at any revision, and not a refusal
    that drops the action from `due`. `performed()` (`history.rs:70-73`), the number of finished
    lines, still counts it, as a reader of the log sees the line.
  - **b.** It drops the action from `due` as other refusals do, and is performed at no revision.
    A writer that a retry record made due would then not be taken again after a cancel.

## ESS first

No new noun under D1 a. Under D1 b, `loom.runlog.ActionFinished` gains `cancelled` first.

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step ends within 120 s.
- No test makes a network request or a model call, and no test reads a credential file.
- Fixtures are POSIX sh, never Python.
- A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30
  minutes).

All tests go in the `cancel` module of `crates/loom-tree/tests/main.rs` (`:236-529`), on its
`Halt` fixture. That fixture is a `halt@1` root with `work.look` and `work.do`; `do.sh` sleeps
30 s and leaves a process behind (`:248-391`). A test changes only its own scratch copy: the
bindings file and the workspace's `do.sh`. Unless a test says otherwise, it rewrites `do.sh` to
`exit 0` before its second run. Each test asserts that it ended within 60 s.

1. `a_cancelled_step_is_taken_again_at_the_same_revision`.
   - `cancelled_after(Duration::ZERO)` ends the first run `Cancelled` with `next: [work.do]`.
   - A second `run_tree` with a flag that is never set writes an `action started` line for
     `work.do` and ends `Terminal` with the outcome `finished`.
   - Failing first: today the second run takes no step and stops `Stuck`; `work.do` is performed
     at the revision, and nothing else is useful.
2. `a_cancelled_due_writer_stays_due`.
   - The fixture's `work.do` gets `retry_when: {done: [failed]}`.
   - `do.sh` counts its calls in `target/n`. The first call exits 1; the second sleeps as the
     fixture's `DO` does; the third exits 0.
   - The first run takes `work.do`, gets a `done` record with result `failed`, and so takes it
     again as due. That second call is cancelled.
   - A second run takes `work.do` a third time and ends `Terminal`.
   - Failing first: a cancel is a refusal since the last `evaluated` line, so `run.rs:852-865`
     drops `work.do` from `due`, the history counts it as performed, and the second run stops
     `Stuck`.
   - This test is red under D2 b by design. It is the test that tells D2 a from D2 b.
3. `a_refused_step_that_is_no_cancel_still_counts`.
   - `work.do` gets `retry_when: {done: [failed]}` and `budget: {max_runs: 1}`, and `do.sh` is
     `exit 1`.
   - The first run takes `work.do` once. Its second, due step is refused `RunsSpent`, and the
     run stops `Stuck`.
   - A second run at the same revision writes no `action finished` line and stops `Stuck`.
   - This is a guard: it passes today. It goes red under the planted defect of skipping every
     refused line instead of only cancelled ones. The second run would then take `work.do`
     again and write another `RunsSpent` line.
4. `a_cancelled_run_still_spends_the_budget`.
   - `work.do` has `budget: {max_runs: 1}`. The first run is cancelled; `do.sh` is then
     rewritten to `touch target/ran`.
   - The second run selects `work.do`. Its step is refused `RunsSpent`, naming `has run 1
     times`, and `target/ran` does not exist.
   - Failing first: today the second run selects nothing and stops `Stuck`.
   - It also catches the planted defect of leaving cancelled runs out of `ProgramEffects::runs`
     (`crates/loom-bindings/src/program.rs:198-220`). The program would then run, and
     `target/ran` would exist.
5. `history_knows_a_cancelled_line`. This is a unit test in `crates/loom-tree/src/history.rs`.
   - A log of `evaluated` at revision 3, then `action finished` for `a` refused
     `Cancelled: action a: …`, gives `performed_at("a", 3) == false`.
   - The same log with refusal `RunsSpent: …` gives `true`.
   - For the cancelled log, `performed("a") == 1`: the count of finished lines still holds the
     cancelled line (D2 a). This catches the mutant that skips cancelled lines in the count map
     too.
   - Failing first: the first assertion fails.

The suite's time: tests 1, 2 and 4 each wait about 1 s for their cancel (the `Halt` program is
killed within 100 ms under `local`). Tests 3 and 5 take no program time beyond `exit 1`.

No probe was run while drafting this: one build of the `b10x-loom-tree` test binary on this host
was judged too costly for a claim the reading settles.

## Out of scope

- A cancelled `tree approve` person program (`story:person-action-runs-a-program`):
  `tree approve` passes no cancel.
- The `handoff checked` line of a cancelled check (`story:handoff-check-holds-the-cancel`). That
  line is not an `action finished` line, and the history does not read it.
- Changing what `budget.max_runs` counts.

## Scope

Derived 2026-11-22 from the plan tree at `c260bdd1`. **Cited** means read at the line named.

- `crates/loom-tree/src/history.rs` (`:29-61`, `:70-73`) — cited.
- `crates/loom-tree/src/run.rs` (`:852-865`, `:947-950`; read: `:373-397`, `:431-448`,
  `:769-780`, `:889`) — cited.
- `crates/loom-executor/src/selection.rs` (`:285-306`), read and not changed — cited.
- `crates/loom-bindings/src/program.rs` (`:874-888`, `:198-220`), read and not changed — cited.
- `crates/loom-tree/tests/main.rs` (`mod cancel`, `:236-529`) — cited.
- `ess/domains/runlog.yaml` (`:94-108`) and `docs/contracts/run-log-v1.json`, only under D1 b
  — cited.
- `CHANGELOG.md` — inferred.

## Edges

- `depends_on story:cancel-a-running-bound-program`: the cancelled `action finished` line and the
  run count it keeps.
- **Order:** this story merges before `story:tree-suite-stop-resume-reopen-steps`. That story's
  `resume` step runs `tree run` again; after a cancel it should take the cancelled action, as
  this story makes it. The order is stated in prose, with no depends_on.
- **Would collide with**, no edge:
  - `story:person-action-runs-a-program` (active) on `crates/loom-tree/src/run.rs` and on the
    meaning of a refused line. That story keeps "a failed `ActionFinished` line counts as
    performed" (its lines 326-330), which test 3 holds. It also moves the run count's code in
    `program.rs`, which test 4 reads. It merges first, because it is active; this story rebases
    on it.
  - `story:tree-suite-stop-resume-reopen-steps` on `ess/domains/runlog.yaml`, only under D1 b.
  - `story:handoff-check-holds-the-cancel` on `crates/loom-tree/src/run.rs` and on the `cancel`
    module of `crates/loom-tree/tests/main.rs`. Either may merge first.
