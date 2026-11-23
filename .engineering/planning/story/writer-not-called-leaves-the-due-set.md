---
format: aep.planning-md/3
id: story:writer-not-called-leaves-the-due-set
kind: story
status: draft
title: A writer whose program exits not-called leaves the due set until the case is evaluated again
summary: step_inputs treats an action finished line whose exit the binding lists in exits.not_called like a refusal for the due set until the next evaluated line (its own moves excepted); the run goes on to the next useful action or the handoff check; one start per evaluation; log line, max_runs and replay unchanged
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-tree/src/history.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: crates/loom-tree/tests/main.rs
- confidence: inferred
  path: crates/loom-tree/tests/not_called
- confidence: cited
  path: crates/loom-tree/tests/suite_format/mod.rs
revision: 11
---
## Outcome

After a bound program exits a code its binding lists in `exits.not_called`, `tree run` does not
take that action again until the case is evaluated again: the action leaves the due set until the
next `evaluated` line, as a refused step does today. The engine goes on to the next thing due at
this revision (another due action, or the handoff check that asks the decision for the waiting
finding). The run is not counted against `budget.max_runs`, no new log kind is written, and the
`action finished` line is unchanged, so the replay and the history read it as before. Runbook
asked for this first in the next wave; it does not depend on the `exits.error` workaround. Cites
are at `81372f6a`.

**Before.**

- **The exit is classed and the step is performed.**
  - `class_of` classes an exit the binding's `exits.not_called` lists as `NotCalled`
    (`crates/loom-bindings/src/program.rs:1273-1286`). A run killed at its timeout or ended by a
    signal is in no class.
  - The `NotCalled` end writes the moves alone, records no evidence and returns `Performed` with
    no refusal (`:371-388`, `performed` `:1078-1092`).
  - `max_runs` does not count a classed run (`:1015-1020`).
  - `tree run` writes `action finished` with the exit (for example 79) and no refusal
    (`crates/loom-tree/src/run.rs:398-423`; `effect_summary` `:1251-1252`).
- **The writer stays due.**
  - `due_actions` (`run.rs:956-970`) holds every writer with an open retry record. A retry record
    lives while its subject revision is the case's (`crates/loom-bindings/src/retry.rs:197-204`).
  - `step_inputs` takes out only the actions refused since the last `evaluated` line
    (`run.rs:994-1007`). A not-called line has no refusal, so the writer stays due.
  - `DeterministicSelector::useful` answers "useful" when `due` is true (`selection.rs:291-297`),
    before it reads `performed_at` (`:298-304`). So the writer is selected at once again.
- **Seen in Runbook's suite cycles:** about 67 starts of `spec.author` in 60 s in one cycle, and
  97 of `requirements.normalise` in another, until the cycle's 60 s bound. Each was a model node
  exiting 79 because a finding waits for a person.
- **What the contract says:**
  - `story:bound-program-contract` (body lines 114-115): a not-called run records nothing, the
    step is performed, the run is not counted;
  - the same story leaves "the due rule for a not-called writer" to `story:writer-not-called-stop`
    (lines 306-307), which is not in the store. This story takes that rule; the `WriterNotCalled`
    stop and `tree open` calling `resolve_programs` stay out of scope;
  - `AGENTS.md:1615-1616`: repeated not-called runs are bounded by the loop's step budget and
    idle bound;
  - `ess/domains/bindings.yaml:169-176`: the two exit classes.
- **The workaround** (the code under `exits.error`) ends the run with `ProgramError`, leaves the
  exit field empty and counts against `max_runs`. Runbook's runner ends on a `ProgramError`, so it
  is no fix.
- **A not-called run that moves an artifact.** The `NotCalled` end writes the placed artifacts'
  moves (`program.rs:372`, `moves: written.moves` `:386`). The next step start then writes an
  `evaluated` line, since the revision changed (`run.rs:313`, `:879-898`), and the set taken out
  of `due` stops at that line (`run.rs:994-1007`). A model node that rewrites a file on each
  not-called run would be due again after each run, even with the rule below alone (D4).
- **Where a `handoff_when` check runs.** When the picked action itself has a check, `Checked`
  runs it before the action is performed. An exit 0 hands off and the action is not performed
  (`crates/loom-tree/src/handoff.rs:151-195`). With nothing useful, `handoffs` runs the check of
  each admissible action with a `handoff` (`run.rs:1382-1420`, called at `:1532-1538`).

**After.**

- **In `step_inputs` (`run.rs:994-1007`)** the set taken out of `due` also holds every action
  whose `action finished` line is a not-called end:
  - no refusal;
  - an exit code;
  - and the action's binding (`case.bindings`) lists that code in `exits.not_called`
    (`ExitClasses`, `generated/rust/loom/src/bindings.rs:160`, `:326-330`).

  The lines read are those since the last `evaluated` line, and further back under D4 a. A
  cancelled step stays out of that set, as today.
- **Why that is enough.** Once out of `due`, the action is useful only if not performed at this
  revision (`selection.rs:298-304`). The history counts the not-called line as performed at the
  revision of its step and at the revision it left (`crates/loom-tree/src/history.rs:44-55`,
  `Interval::close` `:143-157`). So the selector passes the action over.
  - With another action useful, that one is taken.
  - With none, the run reaches `handoffs` (`run.rs:1532-1538`). An admissible action with a
    `handoff` whose check exits 0 hands off: the run stops `AwaitingPerson`. That is how Runbook's
    node asks the decision for the waiting finding.
- **When it is due again (D1 a).** At the next `evaluated` line: `evaluated` writes one when the
  case revision or the decision digest changed (`run.rs:893-898`). A moved artifact, or a record
  a person's answer adds, makes the writer due again through its retry record, if that record is
  still open.
- **Its own moves do not end the set-aside (D4 a).** An `evaluated` line does not end it when
  the interval it closes holds only not-called lines, at least one of them names a move, and
  those moves account for the line's revision: opening revision plus the interval's moves equals
  the closing one. The next `evaluated` line after that, from any other change, ends it.
- **One meaning of "accounted" (D5 a).** `Interval::close` computes that condition privately
  (`history.rs:143-146`). It moves into a `pub(crate)` function of `history.rs` (inferred name
  `accounted(opened, moves, revision)`), which `Interval::close` and `step_inputs` both call.
  That is the whole change to `history.rs`, and its behaviour stays the same.
- **Unchanged:**
  - the `action finished` line and its members (no new log kind), so `tree replay` and
    `LogHistory` read it as before;
  - `max_runs` and `class_of`;
  - an exit in `exits.error`, and an exit in neither list.
- **Documents.** The doc comment of `step_inputs` (`run.rs:972-977`) names a not-called line
  beside a refusal. `AGENTS.md:1615-1616` says a not-called writer is not due again until the
  case is evaluated again for a reason other than its own moves. `CHANGELOG.md` `[Unreleased]`
  names the change.

## Decisions

- **D1. Until when the action is set aside.** Default **a**.
  - **a.** Until the next `evaluated` line, as a refusal (`run.rs:994-1007`), with D4. A person's
    answer that adds a record changes the decision digest, so the writer runs again on the answer
    with no artifact moved.
  - **b.** Until the case revision moves. A person's answer that moves no artifact would leave
    the writer set aside, and the finding's answer would not be acted on until something wrote.
- **D2. A not-called exit at a new revision while the writer's retry record is open.** The record
  stays open while its subject revision is the case's (`retry.rs:197-204`). Default **a**.
  - **a.** The same rule: at the new revision the old line lies before the new `evaluated` line,
    so the writer is due and starts once. A second not-called exit sets it aside again. One start
    per evaluation.
  - **b.** A not-called exit closes the retry record's claim for that subject revision only, so
    the writer is not due again until the record's subject moves. That couples the due rule to the
    retry record's subject and differs from a refusal; no case asks for it.
- **D3. Whether `tree status` shows the action as set aside.** Default **a**.
  - **a.** No. The `action finished` line with its exit says it, and `tree status` is unchanged.
  - **b.** `tree status` lists the actions set aside since the last evaluation, with the reason
    (refused or not called). That is a new member of `loom.tree` status in ESS; a story of its
    own if wanted.
- **D4. A not-called run that moves an artifact.** Default **a**.
  - **a.** The set-aside survives an `evaluated` line whose interval holds only not-called lines,
    at least one naming a move, with the moves accounting for the line's revision (above). The
    limit: a record received outside the log in the same interval (no line of its own) does not
    free the writer until the next change.
  - **b.** Out of scope. A moving not-called writer is due again after each run and is bounded
    by the loop's step budget (`RunSettings.max_steps`, `run.rs:99`) and wall bound (a suite
    cycle's 60 s, `crates/loom-tree/src/suite/run.rs:401`). That is Runbook's loop for a node
    that rewrites its file.
- **D5. Where "accounted" is computed.** Default **a**.
  - **a.** One `pub(crate)` helper in `history.rs`, called by `Interval::close` and by
    `step_inputs`. The smallest change to `history.rs`, with no change of its behaviour; its
    tests (`history.rs` `mod tests`) stay green unchanged.
  - **b.** `run.rs` computes it again, and a unit test pins both to one answer over the same
    intervals. Two copies that a later change can part.

## ESS first

Exempt: no noun changes. `loom.bindings.ExitClasses` (`ess/domains/bindings.yaml:169-176`)
already declares `not_called`; the rule for the due set is run code. If D3 b is taken, its status
member is declared first in `ess/domains/tree.yaml`.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are
POSIX sh, never Python. No test starts `b10x-loom` or lives in the `tree_run` test binary. No
test checks its own wall time (the host's load makes such checks fail). A slot job ends within
10 minutes, or names its time with `slot --timeout` (at most 30 minutes).

**`crates/loom-tree/tests/main.rs`, a new module `not_called`** (inferred name; a directory
`tests/not_called/` with `mod.rs`, as `cancel_sec/` is), in process through `run_tree`
(`run.rs:148`). Fixture: a one-case tree the test writes, as `cancel_sec`'s `halt@1` tree is
(`crates/loom-tree/tests/cancel_sec/mod.rs:24-109`), under Substrate's `local` adapter,
`max_steps: Some(10)`. Its protocol (inferred shape; the unit writes it):

- artifacts `work` (`work.txt`) and `notes` (`notes.txt`);
- `work.look`, a reader whose `look.sh` exits 1, recorded `looked: failed` on `work`. A claim
  reads `looked`. As a reader it runs before every writer: the selector takes the useful action
  that does not write first, then by action id (`selection.rs:318-338`);
- `work.do`, a writer with `retry_when: {looked: [failed]}` (its retry record is open while `work`
  is unmoved) and `exits: {not_called: [79]}`. `do.sh` runs `touch target/called` (outside every
  placed artifact) and `exit 79`. It has no `handoff`;
- `work.ask`, an admissible action that may produce a kind no claim reads, so it is never useful,
  with `handoff: a person answers the finding` and `handoff_when: [sh, check.sh]`. `check.sh`
  exits 0 only when `target/called` exists, else 1.

The check of `work.ask` runs only in `handoffs`, when nothing is useful, so it tells the two
behaviours apart.

1. `a_not_called_writer_with_an_open_retry_record_hands_off_and_starts_once` (red first;
   Runbook's item a).
   - The log holds one `work.look` line, then one `action finished` line for `work.do` (exit 79,
     no refusal), then a `handoff checked` line for `work.ask` with exit 0.
   - The run stops `AwaitingPerson` with one `Handoff` decision, for `work.ask`.
   - Failing first: today `work.do` is due after each exit, so every step after `work.look` is
     `work.do` (9 of the 10 steps), `handoffs` never runs, and the run stops `BudgetSpent`.
2. `another_useful_action_is_taken_after_a_not_called_exit` (Runbook's item a, the other branch).
   - The same tree with a writer `work.tidy` (an id after `work.do`) that records `tidied: ok` on
     `notes` and moves nothing; a claim reads `tidied`, so it is useful once, and it has no retry
     record.
   - `work.tidy`'s record changes the decision digest, so an `evaluated` line follows it and
     `work.do` is due again once (D1 a).
   - Lines: `work.look`; `work.do` (79); `work.tidy` (0); `work.do` (79); `work.ask`'s
     `handoff checked` (0). Stop `AwaitingPerson`. After `work.look` (a reader, first), both
     writers are useful and `work.do` goes before `work.tidy` by id (`selection.rs:318-338`).
   - Failing first: today `work.do` takes every step it is offered, and the run stops
     `BudgetSpent`.
3. `a_not_called_writer_is_due_again_after_a_change` (D1 a, D2 a).
   - **`work` moved:** after test 1's run the test writes `work.txt` and runs again. The refresh
     moves `work` (`run.rs:306-312`), which closes `work.do`'s retry record. `work.look` is useful
     at the new revision and records `looked: failed` again, so the record opens again and
     `work.do` starts once, then `work.ask` hands off.
   - **Another artifact moved (D2 a):** the test writes `notes.txt` instead. The refresh moves
     `notes`; the record on `work` stays open, so `work.do` starts once at the new revision with
     no new `work.look` line, then `work.ask` hands off.
   - **A record received:** in place of a write, a `tidied: ok` record on `notes` is received
     through the governor's `receive` (`crates/loom-governor/src/lib.rs:1630-1655`). A claim
     reads it, so the decision digest changes, an `evaluated` line is written at the same
     revision, and `work.do` starts once more.
   - Failing first: as test 1 (the first run never reaches the handoff).
4. `a_not_called_run_that_moves_its_artifact_stays_set_aside` (D4 a).
   - `do.sh` also appends a line to `notes.txt`, a placed artifact, before `exit 79`. Its line
     names the move of `notes`, and the next step start writes an `evaluated` line at the raised
     revision.
   - `work.do` has one line, and `work.ask` hands off as in test 1.
   - **A person's answer after the moving run:** after that run the test receives a `tidied: ok`
     record on `notes` through the governor's `receive`, at the same revision, and runs again.
     The digest changes and an `evaluated` line is written. The interval it closes holds no
     line that names a move, so the set-aside ends, and `work.do` starts once on the answer.
     This variant fails if the guard "at least one names a move" is dropped: an empty interval
     would then count as the writer's own.
   - Failing first: today, and with the rule of `step_inputs` alone, `work.do` is due after each
     `evaluated` line its own move gives, and the run stops `BudgetSpent`.
5. `classed_errors_and_unclassed_exits_behave_as_before`.
   - **`exits: {error: [79]}`:** the `work.do` line has no exit and a refusal that starts
     `ProgramError: action work.do:` and names exit 79 (`program.rs:368-369`, `program_error`).
     Expected stop: `AwaitingPerson`, with one `Handoff` decision for `work.ask`. Derived by
     reading:
     - after the line, `tree run` ends early only for a cancelled step (`run.rs:413-421`); a
       refused one goes on to the next round (`:423`), whatever `ends_run` ends in Commission's
       one step;
     - the next round writes no `evaluated` line: no move (`do.sh` touches only
       `target/called`) and no record, so revision and digest are unchanged (`run.rs:893-898`);
     - `work.do` is refused since the last `evaluated` line, so not due (`run.rs:994-1007`), and
       performed at this revision, so not useful; `work.look` is performed, `work.ask` never
       useful;
     - nothing useful, so `handoffs` runs `work.ask`'s check, which exits 0 since
       `target/called` exists, and the run stops `AwaitingPerson` (`run.rs:1532-1538`).
     The line is the same before and after this story.
   - **No `exits`:** exit 79 goes through `work.do`'s table (`results: {"0": ok, "*": failed}`)
     and records `done: failed`. Each `work.do` line has exit 79, no refusal and one evidence id.
     The retry record stays open and no line is not-called, so every step after `work.look` is
     `work.do`, and the run stops `BudgetSpent`, as today.
   - Failing first: none; it is the guard that the change is limited to `not_called`.

**`crates/loom-tree/tests/suite_format/mod.rs`**, in process through `run_suite`, as
`:1345-1365` runs one.

6. `a_suite_cycle_starts_a_not_called_writer_once_per_revision` (Runbook's item b).
   - A suite of one cycle over test 1's protocol and bindings: `open`, then `run` with the exit of
     a wait (`exit: 3`, as the `replay-after-wait` cycle at `:1345` gives it), and a `count` check
     (`suite/decode.rs:958`) over a `log` selector on the root case with
     `event: [ActionFinished]` (`suite/select.rs:473-479`, `suite/decode.rs:1386-1390`), filtered
     to `action: work.do`: 1.
   - The cycle passes, and fast: one `work.look` and one `work.do` step and one check.
   - Failing first: today the cycle's `tree run` has no step bound (`max_steps: None`) and a 60 s
     wall bound (`CYCLE_WALL_SECONDS`, `suite/run.rs:401`, `:1647-1656`), so the red run takes
     about 60 s, ends `BudgetSpent`, and the count check fails. That red run is the unit's
     evidence and is not committed.

**Gate.** `task check` exits 0.

## Out of scope

- The `WriterNotCalled` stop and `tree open` calling `resolve_programs`
  (`story:bound-program-contract`, lines 306-307).
- `tree status` showing set-aside actions (D3 b).
- Changing `class_of`, `max_runs`, the log format, the replay, or how the history dates a line
  (D5 a moves one condition into a helper and keeps its behaviour).

## Scope

Derived 2026-11-23 from the plan tree at `81372f6a`. **Cited** means read at the line named;
**inferred** means a reading that could be wrong.

- `crates/loom-tree/src/run.rs` (`:893-898`, `:956-1007`, `:972-977`, `:1032`) — cited.
- `crates/loom-tree/src/history.rs` (`:100-157`; the helper of D5 a) — cited; no behaviour
  change.
- `crates/loom-tree/tests/main.rs` (`:6-10`) — cited; the module `not_called` is inferred.
- `crates/loom-tree/tests/suite_format/mod.rs` (`:1345-1365`) — cited.
- `AGENTS.md` (`:1615-1616`) — cited; `CHANGELOG.md` — inferred.
- Read and not changed — cited:
  - `crates/loom-bindings/src/program.rs` (`:368-388`, `:1015-1020`, `:1078-1092`,
    `:1273-1286`) and `crates/loom-bindings/src/retry.rs` (`:197-204`);
  - `crates/loom-executor/src/selection.rs` (`:291-304`);
  - `crates/loom-tree/src/history.rs` (`:44-55`), beyond the helper;
  - `crates/loom-tree/src/handoff.rs` (`:151-195`);
  - `crates/loom-tree/src/run.rs` (`:99`, `:148`, `:306-313`, `:398-423`, `:879-898`,
    `:1251-1252`, `:1382-1420`, `:1532-1538`);
  - `crates/loom-tree/src/suite/run.rs` (`:401`, `:1647-1656`), `suite/select.rs`
    (`:473-479`) and `suite/decode.rs` (`:958`, `:1386-1390`);
  - `crates/loom-governor/src/lib.rs` (`:1630-1655`);
  - `generated/rust/loom/src/bindings.rs` (`:160`, `:326-330`) and `ess/domains/bindings.yaml`
    (`:169-176`);
  - `crates/loom-tree/tests/cancel_sec/mod.rs` (`:24-109`).

## Edges

- No `depends_on`: `story:bound-program-contract` (active in the store) wrote `exits` and its
  classes, which are on this branch; `story:action-dated-by-revisions-its-step-left` is merged
  at `db2726ff` (the store still lists it active).
- `AGENTS.md` and `CHANGELOG.md` are shared with most stories. They are textual merges and are
  not listed below.
- **Would collide with**, no edge. Each entry names the shared file and the merge order:
  - `story:action-dated-by-revisions-its-step-left` (merged at `db2726ff`): `history.rs`. This story
    moves the condition of its `Interval::close` into one helper both use (D5 a), with no change
    of behaviour, so the two keep one meaning of "accounted". No order.
  - `crates/loom-tree/src/history.rs`: `story:cancelled-step-may-run-again` (merged) is the
    only other story whose scope names it. No order.
  - `story:program-reports-its-cost` (active, not merged): it writes `cost_micro_usd` at the
    `ActionFinished` literals (`run.rs:402-408`, `:459-469`, `:1846-1853`) and changes the
    run-log contract. This story changes `step_inputs` only, another region of `run.rs`. Textual
    merge, either order.
  - `story:run-caps-per-arrival` (proposed): `max_runs` counting, the `RunsSpent` stop and the
    selector (`selection.rs:285-305`). This story keeps "a not-called run is not counted" and
    adds no count; that story keeps the not-called exclusion when it counts. This story first.
  - `story:tree-test-decide-and-requests` (proposed): `suite/run.rs`, `suite/select.rs`,
    `suite/availability.rs`, `tree_test/mod.rs` and the tree-suite fixtures. This story adds one
    test to `crates/loom-tree/tests/suite_format/mod.rs` and changes no suite code. Textual
    merge, no order.
  - `story:one-test-binary-per-crate` (active): the new directory `tests/not_called/` is a module
    of `tests/main.rs`, the layout that story asks for. No conflict, no order.
  - Through their whole-crate `crates/loom-tree` scope, the store reports `run.rs`,
    `tests/main.rs` and `tests/suite_format/mod.rs` shared with 16 proposed stories:
    `agent-node-run-records`, `capacity-given-to-an-admission-program`,
    `child-case-own-workspace`, `concurrent-cases-in-the-tree-daemon`,
    `daemon-continues-a-decided-request`, `flow-limits-recorded-as-events`,
    `goals-entries-from-a-resolved-document`, `goals-grounds-and-free-answers`,
    `mixed-egress-launches`, `non-independent-review-becomes-decision-request`,
    `per-run-program-path`, `run-caps-per-arrival`, `standing-tree-rests-quiescent`,
    `standing-tree-wakes-at-a-host-instant`, `tree-usage-and-model-budget` and
    `tree-waits-open-decision-requests`. None has code at `step_inputs`. Textual merges; this
    story goes first.
