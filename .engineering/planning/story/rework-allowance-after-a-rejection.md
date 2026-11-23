---
format: aep.planning-md/3
id: story:rework-allowance-after-a-rejection
kind: story
status: active
title: 'A rejection gives a bound action a fresh max_runs allowance (budget.runs_per: rejection), budget.rework_limit bounds the rejections, and RunsSpent names the bound that ended it'
summary: Runbook rework-fresh-allowance; a rejection is any case record its retry_when lists, whatever its principal (person, decider:<kind>, rule:<name>, a check), of a kind and result budget.rework_on lists (else retry_when; a listed failed check outside rework_on makes the action due and counts inside the window), never the action's own run; one rejection per interval between runs; window placed by the cited observation; rework_limit default 2; spent once, on the first run at a rework count equal to rework_limit, never on a full window alone; unplaced records count by the same one-per-interval rule; runs_per case unchanged; ESS RunsScope {Case, Rejection} owned here, run-caps-per-arrival adds Arrival; no new run-log kind
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
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: inferred
  path: crates/loom-bindings/src/retry.rs
- confidence: cited
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_retry.rs
- confidence: cited
  path: crates/loom-tree/tests/main.rs
- confidence: inferred
  path: crates/loom-tree/tests/rework
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
revision: 26
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T02:43:09Z", actor: "agent:sdc", revision: 25, executor: "agent:loom", correlation: "runbook-rework-allowance-2026-11-23"}
- {from: "proposed", to: "active", at: "2026-11-23T02:43:10Z", actor: "agent:sdc", revision: 26, executor: "agent:loom", correlation: "runbook-rework-allowance-2026-11-23"}
---
## Outcome

A bound action may count its `budget.max_runs` from the latest rejection in its case instead of
from the case's start: with `budget.runs_per: rejection`, a rejection gives it a fresh
allowance of `max_runs` runs, and `budget.rework_limit` (a whole number from 1, default 2)
bounds how many rejections it may answer. A rejection is a record of a kind and result that
`budget.rework_on` lists (the same shape as `retry_when`, each pair also in `retry_when`), or,
with `rework_on` absent, that `retry_when` lists (D16). A record `retry_when` lists and
`rework_on` does not, such as a failed check, still makes the action due and counts nothing but
the run it causes, inside the current window. A rejection counts whoever produced it: a person,
a decider (`decider:<kind>`), a rule (`rule:<name>`) or a program check, but never a record of
the action's own run (D12). When either bound ends the action, the step is
refused `RunsSpent`, and the refusal names the bound that ended it. `runs_per` absent or `case`
counts as today. Under `rejection` a `from: runs` entry writes `spent` once, on the first run
made at a rework count equal to `rework_limit` (no rejection can renew it), never on a full
window alone (D10). No run-log kind is added. Runbook asked for this (its
`rework-fresh-allowance` case: a fourth run after three rejections). Cites are at `3cdab7cb`.

**Before.** `runs()` counts every `PROGRAM_SOURCE` observation naming the case and the action,
but those of a run the binding's `exits` classes, over the case's whole life
(`crates/loom-bindings/src/program.rs:1000-1023`). `prepare` refuses `RunsSpent` once that count
reaches `max_runs` (`:776`, `:783-793`), and the `from: runs` entry writes `spent` on the run
that reaches it (`:420-424`, `:439`). A writer sent back by a reviewer three times with
`max_runs: 3` is refused on the fourth run, though each review asked for new work. Runbook's
`change.implement` holds `max_runs: 3` (Runbook `bindings/sdc-story.yaml:27`).

**After.** With `budget: {max_runs: 1, runs_per: rejection, rework_limit: 3}` the writer runs
once, is rejected, runs once more, and so on: after the third rejection it runs a fourth time.
The fourth rejection is one more than `rework_limit`, and the next step is refused `RunsSpent`
naming `budget.rework_limit`; the run then stops `Stuck`. With `max_runs: 2` the writer may run
twice between two rejections; a third run without a new rejection is refused `RunsSpent` naming
`budget.max_runs` and the rejection that opened the allowance.

### Decided (Runbook, 2026-11-23, relayed by the coordinator)

- **Member names.** `budget.runs_per: case | rejection`, default `case`; there is no
  `rejection_is_arrival` boolean. `budget.rework_limit`, a whole number from 1, default 2, read
  only under `runs_per: rejection`.
- **A rejection is a record, whatever its principal.** In `runbook run` a decider's rejection is
  Loom's `tree approve` with `result: rejected` (Runbook `decide.rs:34`; the daemon path
  `engine/calls.rs:200`), its principal `decider:<kind>` or `rule:<name>`, never a person's name.
  So a rejection is a case record whose kind and result the action lists (in `rework_on`, else
  in `retry_when`, D16; the match `retry_records` uses,
  `crates/loom-bindings/src/retry.rs:197-199`), whatever its producer. Loom does not filter on a
  human principal. Acceptance tests 1-3 hold one principal each.
- **The refusal names its bound.** `RunsSpent`, with `budget.max_runs` or `budget.rework_limit`
  in its detail (D5).
- **No new run-log kind.** The refused step's `action finished` line carries the refusal text as
  today (`program.rs:1258-1268`). The history reads every `action finished` line alike
  (`LogHistory::new`, `crates/loom-tree/src/history.rs:44-70`) and treats only the `Cancelled:`
  prefix apart (`:23-32`), so the replay and the history read the line unchanged.

### Decided (the coordinator's defaults after the critics of revision 14)

- **D9. One rejection per interval.** The rework count counts at most one rejection in each
  interval between two runs of the action: before its first run, between two runs, after its
  last run. Two records between the same two runs count once, placed or not (D2 a; tests 8, 9
  and 14).
- **D10. When `from: runs` writes `spent`.** Under `runs_per: rejection` a run writes `spent`
  only when the rework count equals `rework_limit`: a further run then needs a new rejection,
  which is refused. Only the first run made at that count writes it: the run whose preceding
  interval holds the rejection that brought the count to `rework_limit`; later runs at the same
  count write none, so the case holds one `spent` record (decided on revision 18; test 15).
  This needs no read of earlier records: it follows from the intervals of D9. A full window
  alone writes no `spent`: a later rejection opens a new window, so `spent` there would end a
  `given_up` protocol with allowance still to come (decided by the coordinator on revision 16,
  the window-full condition dropped). This replaces the earlier D4.
  Stated limits, read in code:
  - the run that writes `spent` is the last one a protocol whose outcome requires the `spent`
    claim takes: that outcome ends the case after it, before a review of that run, as today
    under `case` (`crates/loom-tree/tests/runs_spent/mod.rs:259-271`,
    `an_outcome_waits_for_the_spent_claim`, ends `Terminal` `given_up` right after the run that
    reached `max_runs`);
  - a writer whose window is full before the rework count reaches `rework_limit`, and that is
    due again with no new rejection, is refused `RunsSpent` naming `budget.max_runs` with no
    `spent` record, and the run stops `Stuck`, as a cancelled last run does today
    (`AGENTS.md:1480-1481`).
- **D11. This story owns `RunsScope` and `runs_per`.** It declares
  `loom.bindings.RunsScope` with `Case` and `Rejection` and the member `runs_per`.
  `story:run-caps-per-arrival` adds `Arrival`, and its body (revision 13, which still declares
  `RunsScope {Case, Arrival}`, `runs_per` and an optional `max_runs`) must say "adds `Arrival`"
  when it is built; this story does not rewrite it. The edge `story:run-caps-per-arrival
  depends_on story:rework-allowance-after-a-rejection` is recorded for the shared files
  `ess/domains/bindings.yaml`, `crates/loom-bindings/src/decode.rs`,
  `crates/loom-bindings/src/program.rs` and the generated files. Test 18's refusal of
  `runs_per: arrival` is reversed by run-caps. This replaces the earlier D7.
- **D12. A record of the action's own run is no rejection.** A record that cites a
  `PROGRAM_SOURCE` observation naming this case and this action (its own run; the lookup
  `retry_records` does at `retry.rs:208-216`) neither counts nor places. Without this a writer
  whose own table writes a result its `retry_when` lists (as the `runs_spent` fixture's
  `work.do` does with `retry_when: {done: [failed]}`) would open a window on every run.
  Stated limit (decided on revision 18, D12 b): `runs_per: rejection` with a `retry_when` whose
  kinds only the action's own run writes decodes and loads without error, opens no window and
  never writes `spent` (the rework count stays 0, below every `rework_limit`). The action ends
  after `max_runs` runs, refused `RunsSpent` naming `budget.max_runs`, and the run stops `Stuck`;
  a `given_up` outcome over `spent` is not reached. No load refusal is added; test 5 pins it.
- **D13. Only `program` and `person` take the members.** `runs_per`, `rework_limit` and
  `rework_on` (D16) on an `agent`, `open` or `copy` binding are refused `MemberNotForExecutor`
  at `budget.<member>`, as the agent-only budget members are refused on other executors
  (`crates/loom-bindings/src/load.rs:318-345`). `prepare` serves a program and a person's
  program; nothing reads the members elsewhere.
- **D14. Each `runs_per` value states its own counting basis.** `Case` and `Rejection` count the
  `PROGRAM_SOURCE` observations `runs()` counts. `Arrival` keeps run-caps' basis (its `action
  finished` lines), not this story's. The ESS comment of `RunsScope` names the basis per value.
- **D15. A record about no revision counts.** A listed record whose `subject_revision` is `none`
  (a person's record about an artifact with no revision yet,
  `crates/loom-tree/src/person.rs:190-193`) counts and places like any listed record. The
  rule and `retry_records` agree on what is listed; they do not agree on revisions:
  `retry_records` skips `none` and superseded revisions (`retry.rs:200-204`), the rule does not
  (D3 a).
- **D16. `budget.rework_on` names the rejections** (decided by the coordinator on revision 20,
  from Runbook's requirement). Runbook's `change.implement` lists four `retry_when` entries
  (Runbook `bindings/sdc-story.yaml:29-33`): `implementation_change: [coverage_refused]` (its
  own record, D12), `conformance_run: [failed]` (a failed check), `code_review: [rejected]`
  (written by `change.review`'s program) and `merge_approval: [rejected]` (through `approve`).
  A window must open on the last two only. So:
  - `rework_on` is a map of kind to a list of results, the shape of `retry_when`, decoded by the
    same decoder (`decode.rs:863-882`, `retry_when`; inferred: it takes the member's path for
    its refusals) into `List<loom.bindings.RetryCondition>`;
  - that decoder accepts an empty map (`{}`) and an empty result list (`{kind: []}`); in
    `rework_on` either would pass the subset check vacuously and name no rejection, so the
    action would open no window and never write `spent`. Both are refused `InvalidBudget` at
    `budget.rework_on` (an empty list at `budget.rework_on.<kind>`), decided on revision 22.
    `retry_when` itself keeps accepting both, unchanged;
  - absent: every pair `retry_when` lists is a rejection (the rule of revision 20);
  - present: only its pairs open a window and count toward `rework_limit`. A record
    `retry_when` lists and `rework_on` does not still makes the action due (`retry_records` is
    unchanged) and the run it causes counts inside the current window against `max_runs`;
  - each pair must appear in `retry_when`: otherwise refused `InvalidBudget` naming
    `budget.rework_on` (a pair outside `retry_when` would name a rejection that never makes the
    action due);
  - `rework_on` without `runs_per: rejection` (absent or `case`) is refused `InvalidBudget`
    naming `budget.rework_on`, as `rework_limit` is; on an `agent`, `open` or `copy` binding it
    is refused `MemberNotForExecutor` (D13).
  Runbook sets `rework_on: {code_review: [rejected], merge_approval: [rejected]}` beside
  `rework_limit: 3`. Tests 6, 16, 17 and 18.

### The rule

- **Rejections.** The case records (`governor.evidence(case)`, as `retry.rs:180`) whose `kind`
  member equals the record's kind and whose kind and result `rework_on` lists, or `retry_when`
  when `rework_on` is absent (D16), at any subject revision, `none` included (D3, D15), any
  producer, but a record of the action's own run (D12).
- **Placing a rejection.** A record is placed at the latest position, in `try_observations`
  order ("in the order received", `crates/loom-governor/src/lib.rs:1218-1223`), of an
  observation it cites that names the case. A tree approval cites its `PERSON_SOURCE`
  observation (`crates/loom-tree/src/person.rs:170`, `:178`, producer the principal at `:236`); a
  program check cites its run's `PROGRAM_SOURCE` observation (D1).
- **The window.** Under `runs_per: rejection` the runs counted are those `runs()` counts today
  whose observation comes after the latest placed rejection; with no placed rejection, all of
  them.
- **The rework count.** The intervals between this action's counted runs (before the first,
  between two, after the last) that hold at least one rejection, placed or not (D9, D2 a).
  A placed rejection lies in the interval its observation's position gives. An unplaced one
  lies in the interval its receipt order gives among the records of the action's runs (the
  records citing a counted run's `PROGRAM_SOURCE` observation, in `governor.evidence(case)`
  order, "in the order received", `crates/loom-governor/src/lib.rs:1208-1216`). A counted run
  whose step writes no record anchors nothing, so for unplaced records the intervals on its two
  sides are one (stated limit: fewer counts, never more).
- **The check**, in `prepare` (`program.rs:783-793`), in this order:
  1. rework count > `rework_limit`: refused `RunsSpent`, detail `"{action} was rejected {k}
     times in case {case}, the latest {evidence_id} by {producer}; its budget.rework_limit is
     {n}"`;
  2. window count ≥ `max_runs`: refused `RunsSpent`, detail `"{action} has run {runs} times in
     case {case} since the rejection {evidence_id} by {producer}, its budget.max_runs is {n}"`
     (with no rejection yet: `"... {runs} times in case {case}, its budget.max_runs is {n}"`,
     today's text).
  Under `runs_per: case` the text is today's, byte for byte.
- **`from: runs`** (`program.rs:420-424`): as D10.
- **One helper.** The count is one function in `program.rs` beside `runs()`, used by `prepare`
  and by the `spent` check, answering the window count, the rework count and the latest
  rejection (evidence id, producer). The "listed" match is shared with `retry_records`
  (inferred: a helper in `retry.rs` over a list of `RetryCondition`), applied to `rework_on`
  when present, else to `retry_when`; with `rework_on` absent the two agree on which kinds and
  results are listed; they differ on revisions (D15) and on the action's own run (D12).
- **Decoding.** `budget` (`crates/loom-bindings/src/decode.rs:831-860`) reads `runs_per` as a
  string, `rework_limit` as a whole number from 1 and `rework_on` as a map of lists (D16); the
  three enter `BUDGET` (`:68-73`). Refused `InvalidBudget`:
  - `runs_per` other than `case` or `rejection` (`arrival`, `revision`, a number), naming
    `budget.runs_per`;
  - `rework_limit` not a whole number from 1, naming `budget.rework_limit`;
  - `rework_limit` stated while `runs_per` is absent or `case`, naming `budget.rework_limit`
    ("read only under runs_per: rejection");
  - `runs_per: rejection` on an action whose `retry_when` is empty, naming `budget.runs_per`
    (no record could open an allowance). This check runs after `retry_when` is decoded
    (`decode.rs:316-318`; inferred site).
  - `rework_on` with a pair `retry_when` does not list, or with `runs_per` absent or `case`,
    naming `budget.rework_on` (the same site);
  - `rework_on: {}`, naming `budget.rework_on`, and a kind with an empty list
    (`rework_on: {code_review: []}`), naming `budget.rework_on.code_review` (D16). `retry_when`
    keeps accepting `{}` and `{kind: []}` as today.
  At load, any of the three members on an `agent`, `open` or `copy` binding is refused
  `MemberNotForExecutor` (D13, `load.rs:318-345`).
- **The person path.** `tree approve` prepares a person program through the same `prepare`
  (`crates/loom-tree/src/approve.rs:307`, `:341`), so the rule holds there with no change.
- **Not changed.** `tree status` lists runs as today (`crates/loom-tree/src/status.rs:283`); the
  idle set of spent writers (`crates/loom-tree/src/run.rs:1042`) reads the open kinds, not the
  count. `DEFAULT_MAX_RUNS` stays 8 (`crates/loom-bindings/src/lib.rs:84`).
- **Documents.** `AGENTS.md` (`:1476-1488`), the bindings guide
  (`website/docs/guides/bindings.md:166-170`, `:197-203`, `:210-211`) and `CHANGELOG.md` name
  both members, the rule, the two refusal texts and D10.
- **Runbook's setting.** Its `rework-fresh-allowance` case (a fourth run after three rejections)
  needs `rework_limit: 3` on `change.implement`; the default 2 refuses after the third
  rejection. It also needs `rework_on: {code_review: [rejected], merge_approval: [rejected]}`
  (D16), so a failed conformance run does not count as a rejection. Runbook sets both in its
  bindings; Loom does not.

## Decisions

- **D1. How a rejection is placed among the runs.** Default **a**.
  - a. By the observations it cites, in `try_observations` order. Every approval and every
    program check cites one.
  - b. By its `observed_at` against the runs' `observed_at`. Under a fixed `--at` instant two
    events share a time, so the order is lost.
- **D2. A listed record that cites no observation of the case.** Default **a**.
  - a. It counts toward `rework_limit` by the interval rule of D9 (all rejections between two
    runs count once in total, placed or not; decided on revision 18) and opens no window: it
    never grants a run, and it cannot carry the count past `rework_limit` with no run between
    (test 14).
  - b. It is not a rejection at all.
- **D3. Rejections about an earlier revision of their subject.** Default **a**.
  - a. They count: a rejection is an event of the case, placed by order, not by revision. A late
    review of an old revision still opens a window, and the selector decides whether the writer
    is due.
  - b. Only records about the current revision, as `retry_records` reads them (`retry.rs:200-204`).
- **D4.** Replaced by D10.
- **D5. Where the bound is named.** Default **a**.
  - a. In the refusal's detail text. A `ProgramRefusal` reaches the log only as the text
    `"{reason:?}: action {action}: {detail}"` (`program.rs:1258-1268`).
  - b. A typed member `bound` on `loom.bindings.ProgramRefusal`, which the text would then also
    render.
- **D6. What `rework_limit` counts.** Default **a**.
  - a. The rejections of this action: the records its own `retry_when` lists.
  - b. The rejections of every action of the case.
- **D7.** Replaced by D11.
- **D8. Arrival and rejection together.** Default **a**.
  - a. Exclusive: `runs_per` holds one value. An action counts per case, per arrival or per
    rejection.
  - b. A list (`runs_per: [arrival, rejection]`), each event opening a window.

## ESS first

`ess/domains/bindings.yaml`, before the code:

- a new enum `loom.bindings.RunsScope`, variants `Case` and `Rejection` (D11), with a comment
  naming the rule and each value's counting basis (D14);
- `loom.bindings.ActionBudget` (`:129-139`) gains `runs_per: Optional<loom.bindings.RunsScope>`
  (absent reads as `Case`), `rework_limit: Optional<Integer>` (absent reads as 2 under
  `Rejection`) and `rework_on: Optional<List<loom.bindings.RetryCondition>>` (absent: every
  `retry_when` pair is a rejection; D16; `RetryCondition` at `:143`, the type of `retry_when` at
  `:231-232`), and its comment names the three;
- `loom.bindings.ProgramRefusal` (`:464-472`) and `ProgramRefusalReason` (`:450-459`, `RunsSpent`)
  are unchanged (D5 a); `BindingsRefusalReason` (`:276`) is unchanged: `InvalidBudget` (`:286`)
  and `MemberNotForExecutor` cover every new refusal.

Then `generated/rust/loom/` is rewritten with `task generate` (`Taskfile.yml:101-105`; ess pin
0.97.4, `ess/ess-inputs.yaml:2`), never by hand: `ActionBudget` (`bindings.rs:13`) gains the three
fields. `generated/ess/loom/` is rewritten with `task suite-generate` (`Taskfile.yml:73-78`,
`SLOT_TIMEOUT: 30m`), and `task slow` checks it byte for byte. On the ESS commit alone `task
drift` passes and the decoder's literal of `ActionBudget` (`decode.rs:832-837`) does not compile.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are
POSIX sh, never Python. No test starts `b10x-loom` or lives in the `tree_run` test binary. No
test checks its own wall time. A slot job ends within 10 minutes, or names its time with `slot
--timeout` (at most 30 minutes). No test binary is added: each test is a function or a module of
a binary that exists.

**`crates/loom-tree/tests/main.rs`, a new module `rework`** (inferred name; `tests/rework/mod.rs`,
as `runs_spent/` is, `tests/main.rs:6-10`), in process through `run_tree` and `approve`, as
`replay/fixture.rs:249-262` calls it. Fixture: a one-case tree the test writes, under
Substrate's `local` adapter with `UNCONFINED_FLAG_REASON`, a fixed `TreeTime::At`, `max_steps:
Some(10)`. Its protocol (inferred shape; the unit writes it):

- the artifact `change` (`change.txt`);
- `change.implement`, a writer bound to `program` (`implement.sh` appends one line to
  `change.txt`, exit 0), recording `implemented: done`, with `retry_when: {review: [rejected]}`
  and `budget: {max_runs: 1, runs_per: rejection, rework_limit: 3}`;
- `change.review`, bound to `person` with `principal: <P>`, one `from: effect` entry of kind
  `review` on `change` listing `accepted` and `rejected`; a claim reads `review: accepted`.

One helper takes `<P>`. Each round: `run_tree` until it stops `AwaitingPerson` with an
`ApproveAction` decision for `change.review`; then `approve` with that principal,
`result: Some("rejected")`. Why the rounds alternate, read in code: a rejection at the current
revision counts the `review` kind, so the review is not useful until `change` moves
(`crates/loom-governor/src/lib.rs:1250-1252`, `:1281-1285`), and the open retry record makes
the writer due.

1. `a_decider_rejection_gives_a_fresh_allowance` (red first), `<P>` = `decider:x`.
   - After three rejections the log holds four `action finished` lines for `change.implement`,
     each with exit 0 and no refusal, and the round after the fourth run stops `AwaitingPerson`
     for `change.review`.
   - After a fourth rejection the next `run_tree` writes one `action finished` line for
     `change.implement` whose refusal starts `RunsSpent: action change.implement:` and holds
     `budget.rework_limit is 3` and `decider:x`, and stops `Stuck`. Why `Stuck`: the refused
     writer is not due again before the next `evaluated` line, the review's kind is counted at
     this revision, nothing is useful and no handoff applies; it is the stop a `RunsSpent`
     gives today (`AGENTS.md:1480-1481`, `crates/loom-tree/tests/runs_spent/mod.rs:276-292`).
   - Failing first: the decoder refuses `budget.runs_per` `UndeclaredMember` (`decode.rs:842`,
     `known`), so the tree does not open.
2. `a_rule_rejection_gives_a_fresh_allowance`, `<P>` = `rule:y`: the assertions of test 1,
   `Stuck` included.
3. `a_person_rejection_gives_a_fresh_allowance`, `<P>` = `ada`: the assertions of test 1,
   `Stuck` included.
4. `runbooks_shape_writes_spent_on_the_last_renewal` (D10). Runbook's shape: `budget: {max_runs:
   3, runs_per: rejection, rework_limit: 3}`, `<P>` = `decider:x`, and `change.implement` also
   has `{kind: implement_spent, subject: change, producer: loom, from: runs, results: {spent:
   spent}}` (the protocol adds `implement_spent` to its `may_produce`). At each point:
   - runs 1, 2 and 3 (after rejections 0, 1 and 2; rework count below 3): no `implement_spent`
     record; each round stops `AwaitingPerson` for `change.review`;
   - run 4 (after the third rejection; window count 1, rework count 3 = `rework_limit`): its
     step writes `implement_spent: spent`; the round stops `AwaitingPerson` for
     `change.review` (no outcome reads the kind, and the spent kind never makes the writer
     useful, `run.rs:1042`);
   - after the fourth rejection: the step is refused `RunsSpent` naming `budget.rework_limit is
     3`, no second `implement_spent` record, stop `Stuck`;
   - **variant:** the protocol's outcome `given_up` requires a claim over `implement_spent`.
     Run 4 writes the record and the round stops `Terminal`, detail `given_up`, with no fourth
     review asked (D10, first stated limit).
   - Failing first: as test 1.
5. `own_records_alone_open_no_window` (D12, stated limit). The `runs_spent` give-up tree
   (`crates/loom-tree/tests/runs_spent/mod.rs:28-70`: `work.do` with `retry_when: {done:
   [failed]}`, `do.sh` exits 1, a `from: runs` entry, `given_up` requires `work.spent`), its
   budget `{max_runs: 2, runs_per: rejection}`. The bindings load. The log holds two `work.do`
   lines with exit 1 and no refusal, then one refused `RunsSpent` naming `budget.max_runs is
   2`; no `spent` record; stop `Stuck`, not `Terminal` `given_up`. Each `done: failed` record
   cites `work.do`'s own run, so none opens a window and the rework count stays 0, below every
   `rework_limit`. Failing first: as test 1.
6. `runbooks_merge_approval_opens_a_window` (D16; Runbook's exact shape). The fixture of test 1
   with Runbook's kinds: the person action is `change.merge` (principal `decider:x`, kind
   `merge_approval`), the protocol also declares `implementation_change`, `conformance_run` and
   `code_review`, and `change.implement` records `implementation_change` with `results: {"0":
   null, "8": coverage_refused}`, as Runbook's does (`bindings/sdc-story.yaml:34-39`). Its
   members are Runbook's four `retry_when` entries (`bindings/sdc-story.yaml:29-33`) and
   `budget: {max_runs: 3, runs_per: rejection, rework_limit: 3, rework_on: {code_review:
   [rejected], merge_approval: [rejected]}}`.
   - After three `merge_approval: rejected` approvals by `decider:x`, four `change.implement`
     lines with exit 0 and no refusal (with `max_runs: 3` the fourth needs the window a
     rejection opened).
   - After a fourth, one refused line naming `budget.rework_limit is 3` and `decider:x`, stop
     `Stuck`.
   - Failing first: as test 1.

**`crates/loom-bindings/tests/bound_retry.rs`**, in process through the port, on its fixture
(`draft.write` with `retry_when: {draft_check: [invalid]}`, `bound_retry.rs:7-15`, `:208-227`).
`hold_citing` (`:640-686`) gains the producer as a parameter (it is `checker` today); a record
it holds cites an observation of the case received after the runs before it.

7. `max_runs_spent_between_rejections_names_max_runs` (red first). Members `budget: {max_runs:
   2, runs_per: rejection}`.
   - Two runs perform; a `draft_check: invalid` record by `checker` is held; two more runs
     perform; the fifth step is refused `RunsSpent` naming `budget.max_runs is 2`, the held
     record's evidence id and `checker`.
   - With producers `decider:x` and `rule:y` in place of `checker`, the same.
   - Failing first: the decoder refuses the member.
8. `the_rework_limit_ends_the_allowance`. `budget: {max_runs: 1, runs_per: rejection}` (default
   `rework_limit` 2): run, hold, run, hold, run, hold; the next step is refused `RunsSpent` naming
   `budget.rework_limit is 2` and the third record's evidence id. Failing first: as test 7.
9. `two_rejections_between_two_runs_count_once` (D9). `budget: {max_runs: 1, runs_per:
   rejection}`: run, hold, hold, run (performs; rework count 1), hold, run (performs; rework
   count 2), hold; the next step is refused naming `budget.rework_limit is 2` (rework count 3).
   Failing first: as test 7; with every record counted, the third run would be refused (three
   records, more than 2).
10. `records_that_open_no_window`. Each with `budget: {max_runs: 1, runs_per: rejection}`
   (`rework_limit` 2), after one run:
   - a `draft_check: valid` record (a result `retry_when` does not list), and a `draft_lint:
     invalid` record (a kind it does not list): the next step is refused naming
     `budget.max_runs is 1`;
   - listed records each citing an observation that names another case (D2 a): after the first,
     the next step is refused naming `budget.max_runs is 1` (rework count 1, no window opened);
     after the second and the third, the same: the three lie in one interval and count once;
   - a listed record written by `draft.write`'s own run (D12): `document`'s writer gains an entry
     `{kind: draft_check, from: exit, results: {"*": invalid}}` (the protocol lets it produce
     `draft_check`, inferred), so its run writes a listed record citing its own observation;
     the next step is refused naming `budget.max_runs is 1`, and the rework count stays 0;
   - a listed record about an earlier revision of `draft` (D3 a), and one about `none` (D15):
     each opens a window, and the next step performs.
   Failing first: as test 7.
11. `runs_per_case_counts_as_before`. `budget: {max_runs: 2}` and then `{max_runs: 2, runs_per:
    case}`: two runs, a listed record held, the third step refused with today's text, byte for
    byte (`"draft.write has run 2 times in case <c>, its budget.max_runs is 2"`). Failing first:
    none for the first document (the guard); the second fails on the unknown member.
12. `an_older_store_counts_its_rejections`. Runs and listed records written under a document with
    no `runs_per`; the same governor is then driven with a document stating `runs_per:
    rejection`: the window starts at the latest record written before the change, and the
    rejections written before it count toward `rework_limit`. Failing first: as test 7.
13. `spent_is_written_when_the_rework_limit_is_reached` (D10). The writer also has a
    `from: runs` entry (`{kind: draft_spent, from: runs, results: {spent: spent}}`; the protocol
    fixture declares the kind, inferred), `budget: {max_runs: 2, runs_per: rejection,
    rework_limit: 1}`: runs 1 and 2 write no `draft_spent` record (run 2 fills the window, rework
    count 0); a third step with no rejection is refused naming `budget.max_runs is 2` and writes
    nothing; after one rejection run 3 writes `draft_spent: spent` (rework count 1 =
    `rework_limit`); after a second rejection the step is refused naming `budget.rework_limit is
    1` and writes nothing. Failing first: as test 7. Planted defect: `spent` written on a full
    window alone (the condition D10 dropped): run 2 writes the record and the test fails.
14. `unplaced_records_between_two_runs_count_once` (D2 a). `budget: {max_runs: 2, runs_per:
    rejection}` (`rework_limit` 2): run 1, a placed listed record (rework count 1 = limit - 1),
    run 2, then two listed records citing an observation of another case. The next step
    performs (rework count 2 = `rework_limit`, not 3; window count 1 below 2) and, with the
    `draft_spent` entry of test 13, writes `draft_spent: spent` (D10). Failing first: as test
    7; with one count per unplaced record the step would be refused naming
    `budget.rework_limit is 2`.
15. `spent_is_written_once_per_rework_count` (D10, first run only). `budget: {max_runs: 3,
    runs_per: rejection, rework_limit: 1}` with test 13's `draft_spent` entry: run 1 writes no
    `draft_spent` record; a listed record is held (rework count 1); run 2 writes one; runs 3
    and 4, at the same rework count, write none, so the case holds one `draft_spent` record; the
    fifth step is refused naming `budget.max_runs is 3`. Failing first: as test 7. Planted
    defect: `spent` on every run at that count: runs 3 and 4 write two more records.
16. `rework_on_tells_a_failed_check_from_a_rejection` (D16; Runbook's shape). The fixture
    protocol also declares `conformance_run`, `code_review` and `merge_approval` (`PROTOCOL`,
    `bound_retry.rs:59-83`; inferred: what Canon needs to accept the held records, such as
    letting `draft.check` produce the first two and `draft.lint` the third). The writer's
    members: `retry_when: {draft_check: [invalid], conformance_run: [failed], code_review:
    [rejected], merge_approval: [rejected]}` and `budget: {max_runs: 3, runs_per: rejection,
    rework_limit: 3, rework_on: {code_review: [rejected], merge_approval: [rejected]}}`
    (`draft_check` stands for Runbook's own `implementation_change`, held by D12 in test 10).
    - Run 1; a `conformance_run: failed` record by `ess`: `retry_records` returns it (the writer
      is due, unchanged); run 2 performs; a second such record; run 3 performs; the fourth step
      is refused with the no-rejection text `"draft.write has run 3 times in case <c>, its
      budget.max_runs is 3"`: the records opened no window, and the rework count is 0.
    - Then a `code_review: rejected` record by `checker`, citing a `PROGRAM_SOURCE` observation
      of `draft.check`'s run in the case (a program check): the next step performs (window
      count 1, rework count 1).
    - Then a `merge_approval: rejected` record by `decider:x`, citing a `PERSON_SOURCE`
      observation of the case (as an approval writes it): the next step performs (rework count
      2).
    - With `rework_on` left out, the first `conformance_run: failed` record opens a window and
      the fourth step of the first bullet performs: the rule of revision 20, kept as the default.
    Failing first: as test 7.

**`crates/loom-bindings/tests/bindings_document.rs`**, decode and load only:

17. `runs_per_and_rework_limit_decode` (red first): absent reads `runs_per: None`, `rework_limit:
    None`, `rework_on: None`; `rejection` with `rework_limit: 3` reads both; `runs_per: case`
    reads `Case`; Runbook's `change.implement` members (`retry_when` with its four entries,
    `budget: {max_runs: 3, runs_per: rejection, rework_limit: 3, rework_on: {code_review:
    [rejected], merge_approval: [rejected]}}`) read `rework_on` as two conditions in document
    order; the three members on a `person` binding load.
18. `runs_per_and_rework_limit_refusals`: `InvalidBudget` at `budget.runs_per` for `arrival`,
    `revision` and `3`, and for `rejection` on an action with no `retry_when`; at
    `budget.rework_limit` for `0`, `"2"`, and for `rework_limit: 2` with `runs_per` absent and
    with `runs_per: case`; at `budget.rework_on` for a pair `retry_when` does not list
    (`rework_on: {code_review: [accepted]}` beside `retry_when: {code_review: [rejected]}`, and a
    kind `retry_when` does not name), for a value that is not a map of lists, for `rework_on: {}`,
    at `budget.rework_on.code_review` for `rework_on: {code_review: []}` (each beside a
    `retry_when` that lists `code_review: [rejected]`, which loads alone), and for
    `rework_on` with `runs_per` absent and with `runs_per: case` (D16). `MemberNotForExecutor`
    at `budget.runs_per`, `budget.rework_limit` and `budget.rework_on` on an `agent`, an `open`
    and a `copy` binding (D13). Guard: `retry_when: {}` and a `retry_when` kind with an empty list
    (a kind the fixture's protocol declares; load checks only that, `load.rs:125-130`), with no
    `budget` member of this story, still load (unchanged).
    `every_refusal_reason_is_reachable_from_a_document` is unchanged. `story:run-caps-per-arrival`
    reverses the `arrival` case (D11).

**Gate.** `task check` exits 0; `task slow` passes after the ESS change.

## Out of scope

- `runs_per: arrival`, the `arrived` run-log line, `RunsCapOnObservation` and
  `TreeStop::RunsSpent` (`story:run-caps-per-arrival`).
- Rewriting `story:run-caps-per-arrival`'s body (D11: it changes to "adds `Arrival`" when built).
- Changing what `runs()` counts within a window: cancelled and classed runs count as today
  (`story:cancelled-step-may-run-again`, its § Out of scope, line 152).
- `tree status` showing the window or the rework count.
- An agent executor's run count (`story:bound-agent-node`).
- Setting `rework_limit` in Runbook's bindings.

## Scope

Derived 2026-11-23 from the plan tree at `3cdab7cb`. **Cited** means read at the line named;
**inferred** means a reading that could be wrong.

- `ess/domains/bindings.yaml` (`:129-139`, `:143`, `:231-232`) — cited; `RunsScope` is new.
- `generated/rust/loom/src/bindings.rs` (`:13`) and `generated/ess/loom/` — cited; regenerated.
- `crates/loom-bindings/src/decode.rs` (`:68-73`, `:304`, `:316-318`, `:831-860`, `:863`) — cited;
  the site of the `retry_when` and `rework_on` checks and the decoder's path parameter are
  inferred.
- `crates/loom-bindings/src/load.rs` (`:318-345`) — cited (D13).
- `crates/loom-bindings/src/program.rs` (`:420-424`, `:439`, `:776`, `:783-793`, `:1000-1023`)
  — cited.
- `crates/loom-bindings/src/retry.rs` (`:197-199`, `:208-216`) — cited; the shared match is
  inferred.
- `crates/loom-bindings/tests/bound_retry.rs` (`:208-227`, `:640-686`) and
  `crates/loom-bindings/tests/bindings_document.rs` — cited.
- `crates/loom-tree/tests/main.rs` (`:6-10`) — cited; the module `rework` is inferred.
- `AGENTS.md` (`:1476-1488`), `website/docs/guides/bindings.md` (`:166-170`, `:197-203`,
  `:210-211`) — cited; `CHANGELOG.md` — inferred.
- Read and not changed — cited:
  - `crates/loom-bindings/src/program.rs` (`:142`, `:1258-1268`), `src/lib.rs` (`:84`),
    `src/load.rs` (`:156-168`), `src/retry.rs` (`:44`, `:169-241`, `:200-204`);
  - `crates/loom-tree/src/person.rs` (`:170`, `:178`, `:190-193`, `:236`), `src/approve.rs`
    (`:47-49`, `:307`, `:341`), `src/status.rs` (`:283`), `src/run.rs` (`:1042`),
    `src/history.rs` (`:23-32`, `:44-70`);
  - `crates/loom-governor/src/lib.rs` (`:1208-1216`, `:1218-1223`, `:1250-1252`, `:1281-1285`);
  - `crates/loom-executor/src/selection.rs` (`:285-305`);
  - `ess/domains/bindings.yaml` (`:276`, `:286`, `:450-459`, `:464-472`);
  - `crates/loom-tree/tests/replay/fixture.rs` (`:249-262`), `tests/runs_spent/mod.rs`
    (`:28-70`, `:259-292`);
  - `Taskfile.yml` (`:73-78`, `:101-105`), `ess/ess-inputs.yaml` (`:2`).

## Edges

- No `depends_on` of this story. `story:bound-program-retry-records` (`retry_when`,
  `retry_records`) and `story:outcome-when-reads-the-run-count` (`from: runs`) are on this
  branch.
- **Recorded edge (D11):** `story:run-caps-per-arrival depends_on
  story:rework-allowance-after-a-rejection`. Shared files: `ess/domains/bindings.yaml`
  (`ActionBudget`, `RunsScope`), `crates/loom-bindings/src/decode.rs` (`budget`, `BUDGET`),
  `crates/loom-bindings/src/program.rs` (the count), `generated/rust/loom/`,
  `generated/ess/loom/`; also `lib.rs`, `bindings_document.rs` and the bindings guide. When
  run-caps is built it adds `Arrival` with its own basis (D14), makes `max_runs` optional,
  refuses `rework_limit` and `rework_on` with `RunsCapOnObservation` on an observation as it
  does `max_runs` and `runs_per`, reverses test 18's `arrival` case and changes the `Stuck` stop
  tests 1-6 pin. Its cites of `decode.rs:679-686` and `:42` are stale at this head (`:831-860`,
  `:68-73`).
- **ESS regeneration.** Every story that changes `ess/` conflicts on the `inputs:` line of
  `generated/ess/loom/synthesis.txt` (`:5`) and on `generated/rust/loom/`. They merge one at a
  time: the second regenerates (`task generate`, `task suite-generate`) after the first lands.
- `AGENTS.md` and `CHANGELOG.md` are shared with most stories. They are textual merges and are
  not listed below.
- **Would collide with**, no edge. Each entry names the shared surface and the merge order:
  - `story:program-reports-its-cost` (active, not merged): `crates/loom-bindings/src/decode.rs`,
    `program.rs` (another region: the run's observation and `ActionFinished`),
    `ess/domains/bindings.yaml`, `generated/rust/loom/`, `generated/ess/loom/`. Order: this
    story first; it regenerates; that story regenerates after.
  - `story:writer-not-called-leaves-the-due-set` (active): `crates/loom-tree/tests/main.rs`
    (one `mod` line each). A not-called run is not counted by either story; this story adds no
    count for it. Textual merge, either order.
  - `story:cancelled-step-may-run-again`: merged at `3cdab7cb` (`e16671b3`; the store lists it
    active). It keeps what `max_runs` counts (its line 152); the window counts the same runs.
    No order.
  - `story:open-children-from-arguments-from`: its code is on this branch (`78469efc`; the store
    lists it active), and `arguments` now lives in `crates/loom-bindings/src/arguments.rs`, so
    its cite `program.rs:456-519` is stale. Shared: `ess/domains/bindings.yaml`,
    `website/docs/guides/bindings.md`. No order.
  - `story:tree-test-decide-and-requests` (proposed): no shared file. Its scripted decide entry
    decides a tree decision request in process; a suite cycle that rejects through it opens a
    window by the same rule, since the rule reads records, not who wrote them. No order.
  - `story:one-test-binary-per-crate` (active, not merged): it turns `bound_retry.rs` and
    `bindings_document.rs` into modules of `tests/main.rs`. Tests 7-18 move with their files.
    Either order.
  - The 18 other proposed stories whose scope lists `crates/loom-bindings/src/decode.rs` or
    `ess/domains/bindings.yaml` (among them `story:child-case-own-workspace`,
    `story:bound-agent-node`, `story:tree-usage-and-model-budget`) order among themselves by
    their own edges; this story goes first among them, as the nearest to Runbook's gap, and each
    regenerates after it.
