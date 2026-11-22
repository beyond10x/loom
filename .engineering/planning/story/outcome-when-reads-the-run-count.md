---
format: aep.planning-md/3
id: story:outcome-when-reads-the-run-count
kind: story
status: draft
title: An action's spent run allowance becomes an evidence record a protocol claim reads in an outcome's requires
summary: 'binding evidence entry from: runs (Program only) writes result spent with the run that reaches budget.max_runs; the protocol names the kind and claim; run.rs step_inputs keeps the kind from making the writer useful; works on Canon 0.10.0; when is split out (D131)'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:person-action-runs-a-program
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: cited
  path: crates/loom-executor/src/selection.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: crates/loom-tree/tests/main.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: generated/rust/loom
revision: 19
---
## Outcome

A protocol can state that an action's allowance of runs is spent, and Canon reads it in an
outcome's `requires`. This works on Canon 0.10.0, the pin today
(`crates/loom-governor/Cargo.toml:18`).

Loom surfaces the run count as an evidence record; the protocol declares the record's kind and a
claim over it. The claim is the readable form, and a record is the only way a claim gets a value:
Canon's `Claim` is `true_when: Predicate`, "a proposition whose value is established from
evidence" (Canon `crates/canon/src/model/mod.rs:211-218` at `0.11.0`; the same at `0.10.0`).

Reading the same claim in a decision requirement's `when` (Canon 0.11.0) is
`story:outcome-decision-when-reads-the-run-count` (decision D131).

**Runbook's need.** Runbook's bugfix protocol says its outcomes `not_reproduced` and `not_fixed`
end the case "once the writers are spent" (Runbook `protocol/sdc-bugfix.protocol.yaml:251-273`,
branch `impl/runbook-test-suite`). Nothing in a case says so today, so two cycles end at the
first outcome that holds (Runbook `docs/cycles-suite.md:307-308`):

- `bugfix-not-red` ends `not_reproduced` before `test.reproduce` is sent back;
- `bugfix-edited-test` ends `not_fixed` before `change.fix` is sent back.

With the claim in `requires`, neither outcome holds until the writer is spent. The writers'
allowance is `budget.max_runs: 3` (Runbook `bindings/sdc-bugfix.yaml:39`, `:65`).

### What Loom counts today

- `ProgramEffects::runs` (`crates/loom-bindings/src/program.rs:198-220`) counts the observations
  of source `loom/bound-program` (`PROGRAM_SOURCE`, `:111`) that name the case and the action. It
  leaves out those with a `class` (`exits`). A cancelled run counts
  (`ess/domains/bindings.yaml:321-326`).
- A step whose count has reached `budget.max_runs` is refused `RunsSpent` with nothing started
  (`program.rs:265-275`). `budget.max_runs` is 8 when absent (`bindings.yaml:124-136`).
- The count is in no record, claim or case input. Case inputs are fixed when the case opens
  (Canon's `case` section), so an input cannot carry a count that changes.

### The change (under D1 a, D2 a, D3 a)

- **A new value source.** A Program binding's evidence entry may read `from: runs`
  (`ResultSource` gains `Runs`, `ess/domains/bindings.yaml:71-82`). Its value is `spent` on the
  run that brings the action's count to `budget.max_runs`, and on no other run.
  - `from: runs` belongs to a Program executor only. The other four executors refuse it at load
    with `MemberNotForExecutor`, naming `evidence[<i>].from`:
    - Person and Open, by the rule that holds today, since they read `Effect` alone
      (`crates/loom-bindings/src/load.rs:136-141`);
    - Agent and Copy, by a new arm, since today they accept any source (`:140`).
  - The entry's table must list `spent` and nothing else. Any other value is refused at load
    with `InvalidEvidence`, naming `evidence[<i>].results`.
  - The entry writes its record only on that run: `kind`, `subject`, `producer` and `upstream`
    as for any entry, and the result is the table's value for `spent`.
  - The entry's `kind` must be one the action may produce, as every entry's must
    (`bindings.yaml:94-97`), so Canon sees a kind the action produces.
- **When and at which revision.** The record is written in the same step write as the run's
  other records (`attribute`, `program.rs:648-701`), after them.
  - It is about the subject at the revision the run left it at, which is what the run's other
    records name.
  - It is attributed at the step's case revision, and the next evaluation sees it.
- **A run that writes no records writes none.** That is a cancelled run (no records,
  `bindings.yaml:323-326`) or a run in an `exits` class (not counted). See D2 for what follows.
- **Usefulness.** The surface is `crates/loom-tree/src/run.rs`, in `step_inputs`
  (`:840-891`), where the tree builds the selector's history, its due actions and its idle set
  (`:880-889`).
  - An action that is not due, and whose open evidence holds only kinds its binding writes
    `from: runs`, is put in the idle set for that step. The open evidence is read from the
    governor's `action_facts` (`crates/loom-governor/src/lib.rs:1216`).
  - Without that, `DeterministicSelector::useful` (`crates/loom-executor/src/selection.rs:285-306`)
    finds the open `spent` kind once the writer is not performed at the case revision. The
    writer would then look useful until it is spent, and run again with nothing to do.
- **What Runbook writes** (its side, not this story's):
  - The writer's binding gets
    `{kind: repro_spent, subject: repro, producer: loom, from: runs, results: {spent: spent}}`.
  - The protocol gets the kind, `test.reproduce` `may_produce: [repro_written, repro_spent]`,
    and a claim `repro.writer_spent: {true_when: {evidence: {kind: repro_spent, subject: repro,
    result: spent}}}`.
  - `not_reproduced` then requires that claim beside its `any`. The same goes for `change.fix`
    and `not_fixed`.

## Decisions

- **D1. The claim's name and shape.** Default **a**.
  - **a.** The protocol names the kind and the claim, and the binding ties the kind to the count
    with `from: runs`. Loom reserves no name. Each writer gets a kind of its own, and a protocol
    without such an entry is unchanged.
  - **b.** A fixed kind, `loom_runs_spent`, that Loom writes for every action with a budget,
    when the protocol declares it. The subject is the action's first evidence entry's. There is
    no binding change, but a protocol must take Loom's name.
  - **c.** A case input. Rejected: an input is fixed at open, and the count changes during the
    case.
  - **d.** The count itself as a result on every run (`"1"`, `"2"`, …). Rejected: the protocol
    would hold the bindings' number, and every run writes one more record.
- **D2. When the record is written.** Default **a**, kept with the consequence below.
  - **a.** With the records of the run that reaches the budget.
    - **Consequence.** A cancelled last run spends the allowance (a cancelled run counts,
      `bindings.yaml:323-326`) but writes no `spent` record. An outcome that requires the spent
      claim can then never hold, and the case stops `Stuck` at the next `RunsSpent`.
    - What a cancelled run counts for is `story:cancelled-step-may-run-again`'s decision (its D2
      keeps the cancelled run in `max_runs`). If that changes, this consequence changes with it.
  - **b.** Also write the record on the `RunsSpent` refusal after a cancelled last run. That is
    a refusal that writes a record, against "nothing is written on a refusal"
    (`program.rs:265-275`).
- **D3. The writer's usefulness.** Default **a**.
  - **a.** In `run.rs` `step_inputs`, an action whose only open evidence is its own `from: runs`
    kinds is idle for the step.
  - **b.** No change, and the protocol is to keep the spent claim out of what makes the writer
    useful. Loom cannot check that.

## ESS first

`ess/domains/bindings.yaml` comes first:

- `loom.bindings.ResultSource` (`:80-82`) gains `Runs`.
- The comment at `:71-79` gains: "`Runs` Loom's count of the action's runs: `spent` on the run
  that reaches `budget.max_runs`; a Program executor's entry only".

On that commit `task drift` fails until the generated `loom::bindings` is regenerated.

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step ends within 120 s.
- No test makes a network request or a model call, and no test reads a credential file.
- Fixtures are POSIX sh, never Python.
- A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30
  minutes).

**`crates/loom-bindings/tests/bound_program.rs`** (an existing binary; this adds no new one):

1. `a_runs_entry_writes_spent_on_the_last_run`.
   - The binding has `budget: {max_runs: 2}` and a `from: runs` entry; the stub exits 0 at once.
   - The first `perform` writes no record of the entry's kind. The second writes exactly one,
     with result `spent`, the subject at the revision the run left it at, and the same
     observation id as the run.
   - Failing first: `from: runs` does not decode.
2. `a_runs_entry_belongs_to_a_program`.
   - One binding each for a Person, an Open, an Agent and a Copy executor, with a `from: runs`
     entry. Each is refused at load with `MemberNotForExecutor`, naming `evidence[0].from`.
   - A Program binding whose `from: runs` table lists `"0"` is refused with `InvalidEvidence`,
     naming `evidence[0].results`.
   - Failing first: the decoder refuses `runs` with `InvalidEvidence` ("it is exit, effect or a
     JSON pointer", `decode.rs:611-620`), before the executor rule is reached.
   - The Agent and Copy cases also catch the planted defect of leaving the
     `Executor::Agent | Executor::Copy => true` arm (`load.rs:140`) as it is.
3. `a_cancelled_last_run_writes_no_spent_record`.
   - With `max_runs: 1`, the one run is cancelled (`cancel_when`, `:1783`). It writes no record,
     and the next step is refused `RunsSpent`.
   - Failing first: it does not decode. This is a guard of D2 a and of its consequence.

**`crates/loom-tree/tests/main.rs`**

These tests are in a new module `runs_spent`, with its own `halt@1`-shaped fixture held as
`const` texts.

4. `an_outcome_waits_for_the_spent_claim`.
   - `work.do` has `max_runs: 2` and `retry_when: {done: [failed]}`, and its `do.sh` exits 1.
   - The protocol declares the kind `spent` (in `work.do`'s `may_produce`), a claim `work.spent`
     over it, and the outcomes `finished: {requires: {claim: work.done}}` and
     `given_up: {requires: {claim: work.spent}}`.
   - One `run_tree` takes `work.do` twice and ends `Terminal` with `given_up`.
   - Under the planted defect of no `from: runs` entry in the binding, the run ends after the
     second `work.do` with `Stuck`.
   - Failing first: it does not decode.
5. `the_spent_kind_does_not_make_the_writer_useful`.
   - The fixture is built so the selector reaches `open_evidence`. `useful` returns false for an
     action performed at the case revision before it reads `open_evidence`
     (`selection.rs:298-304`), so another action must move the case revision after the writer.
     A reader cannot do it: a non-`write` action that changes a placed artifact is `Discarded`
     (`crates/loom-bindings/src/program.rs:56-58`). A second write on `work` would reopen
     `done`. So the fixture has a second artifact.
   - Artifacts: `work` (`work.txt`) and `note` (`note.txt`).
   - `work.do` (`effect: write`, `max_runs: 3`, `do.sh` exits 0) writes `done` about `work`.
   - `note.write` (`effect: write`, precondition `{claim: work.done}`, `may_produce: [noted]`)
     runs `note.sh`, which appends a line to `note.txt` and exits 0. That moves `note`'s
     revision, and so the case revision, after `work.do`.
   - `finished` must stay unmet after `note.write`. If it held, Commission would end the run
     `Completed` on `governor.completion` (`crates/loom-commission/src/runtime.rs:1727-1733`)
     before the selector runs, and D3 b could not show. So `finished` requires `work.done`, a
     claim `note.written` over `noted`, and a claim `note.checked` over a kind `checked` that no
     action of the fixture produces. Canon accepts a declared kind that no action produces; it
     refuses only an undeclared one (`UndeclaredEvidenceKind`, Canon
     `crates/canon/src/validate/mod.rs:189-192` at `0.10.0`).
   - The kind `spent` and the claim `work.spent` are declared as in test 4, with `given_up`
     requiring `work.spent`; one run of three spends nothing, so `given_up` is unmet too.
   - Under D3 a, one `run_tree` takes `work.do` once and `note.write` once. At the revision
     `note.write` left, `note.write` has no open evidence and `work.do` is idle, so no step is
     taken. No outcome is legitimate and nothing waits on a person, an approval or a handoff, so
     the run ends `Stuck` (`crates/loom-tree/src/run.rs:454-480`, `waits` at `:1294-1295`).
   - The test asserts the stop `Stuck`, one `action started` line for `work.do` and one for
     `note.write`.
   - Planted defect, D3 b: at that revision `work.do` is not performed, is not due, and has
     `spent` open (`selection.rs:285-306`), so the selector takes it a second time and the
     `work.do` count is 2. The run stays bounded: `max_runs: 3` refuses a fourth start
     (`RunsSpent`, `program.rs:265-275`).
   - The implementer runs this test once with the D3 a change left out (D3 b) and shows it red
     in the wave report, then green with the change.
   - Failing first: it does not decode.

**Gate.** `task check` exits 0.

## Out of scope

- Reading the claim in a decision requirement's `when`
  (`story:outcome-decision-when-reads-the-run-count`).
- A count for Agent, Copy, Open or Person actions.
- Runbook's protocol and bindings changes, and turning `bugfix-not-red` and
  `bugfix-edited-test` from pending.

## Scope

Derived 2026-11-22 from the plan tree at `c260bdd1`; Canon was read at tag `0.11.0`, read-only.
**Cited** means read at the line named; **inferred** means a reading that could be wrong.

- `ess/domains/bindings.yaml` (`:71-82`, `:94-114`, `:124-136`, `:321-326`) — cited.
- `generated/rust/loom` (`loom::bindings`) — cited.
- `crates/loom-bindings/src/decode.rs` (`:606-620`, the `from` values) — cited.
- `crates/loom-bindings/src/load.rs` (`:134-149`, which executor reads which source) — cited.
- `crates/loom-bindings/src/program.rs`: `runs` (`:198-220`), the `RunsSpent` check
  (`:265-275`), `value_of` (`:769-801`) and `attribute` (`:648-701`) — cited.
- `crates/loom-tree/src/run.rs`, `step_inputs` (`:840-891`), the idle set (`:880-889`), for D3
  — cited. The read of `action_facts` there is inferred.
- `crates/loom-executor/src/selection.rs` (`:285-306`) and `crates/loom-commission/src/runtime.rs`
  (`:1727-1733`), read and not changed — cited.
- `crates/loom-bindings/tests/bound_program.rs` and `crates/loom-tree/tests/main.rs` — cited.
- `website/docs/reference/bindings.md` or the bindings guide, and `CHANGELOG.md` — inferred.

## Edges

- `depends_on story:person-action-runs-a-program` (active). It splits `perform` into prepare and
  start and moves the run count with them (its lines 196-221). This story writes into that path.
- **Order:** last of the four: after `story:cancelled-step-may-run-again`,
  `story:handoff-check-holds-the-cancel` and `story:tree-suite-stop-resume-reopen-steps`.
  `story:outcome-decision-when-reads-the-run-count` follows it. The order is stated in prose,
  with no depends_on.
- **Would collide with**, no edge:
  - `story:person-action-runs-a-program` on `program.rs` and `crates/loom-tree/src/run.rs`. It
    merges first.
  - `story:cancelled-step-may-run-again` on `crates/loom-tree/src/run.rs` (`step_inputs`, the
    due and history lines `:852-889`) and `crates/loom-tree/tests/main.rs`. It also decides what
    a cancelled run counts for (D2 above).
  - `story:handoff-check-holds-the-cancel` on `crates/loom-tree/src/run.rs` and
    `crates/loom-tree/tests/main.rs`.
  - `story:tree-suite-stop-resume-reopen-steps` on `crates/loom-tree/src/run.rs` (the refusal
    after the lock).
