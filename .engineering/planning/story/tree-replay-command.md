---
format: aep.planning-md/3
id: story:tree-replay-command
kind: story
status: active
title: b10x-loom tree replay decides every case again and says whether state and decision equal the last evaluated line; the suite step replay runs it
summary: Runbook replay-sibling waits for it; read-only, no lock; per case state_matches_log and decision_matches_log against the last Evaluated line; exit 0 or 3; suite step Available; ESS TreeReplay and CaseReplay
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-suite-format
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-suite/suite.yaml
- confidence: inferred
  path: crates/loom-cli/tests/fixtures/tree-two-level
- confidence: cited
  path: crates/loom-cli/tests/main.rs
- confidence: inferred
  path: crates/loom-cli/tests/tree_replay
- confidence: cited
  path: crates/loom-cli/tests/tree_test/mod.rs
- confidence: cited
  path: crates/loom-tree/src/approve.rs
- confidence: cited
  path: crates/loom-tree/src/encode.rs
- confidence: cited
  path: crates/loom-tree/src/lib.rs
- confidence: inferred
  path: crates/loom-tree/src/replay.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: crates/loom-tree/src/suite/availability.rs
- confidence: cited
  path: crates/loom-tree/src/suite/decode.rs
- confidence: cited
  path: crates/loom-tree/src/suite/run.rs
- confidence: cited
  path: crates/loom-tree/tests/main.rs
- confidence: inferred
  path: crates/loom-tree/tests/replay
- confidence: cited
  path: crates/loom-tree/tests/suite_format/mod.rs
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/run-a-case-tree.md
- confidence: cited
  path: website/docs/reference/cli.md
revision: 33
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T20:59:40Z", actor: "agent:sdc", revision: 32, executor: "agent:loom", correlation: "runbook-wave16-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T20:59:41Z", actor: "agent:sdc", revision: 33, executor: "agent:loom", correlation: "runbook-wave16-2026-11-22"}
---
## Outcome

`b10x-loom tree replay --store <store>` decides every case of a tree again from what the store
holds. For each case it says whether the case's state and Canon's decision equal what the case's
last `evaluated` line in its run log recorded. It writes nothing. The suite step `replay` runs
it, so Runbook's cycle `replay-sibling` (`cycles/tree-suite.yaml:265-274` in Runbook, pending
today) no longer waits for this story. Cites are at `afa46eea`.

**What waits today.**

- `SUITE_AVAILABILITY` marks the step `replay` `Waits("story:tree-replay-command")`
  (`crates/loom-tree/src/suite/availability.rs:56`, `:77`).
- The suite runner turns down a `replay` step as a step kind it does not run
  (`suite/run.rs:1425-1433`).
- The decoder already takes the step, with the members `case` and `principal`
  (`suite/decode.rs:696`, `:734`). `CycleView.steps` already keeps a named `replay` step's JSON
  output for `{step: <name>}` (`suite/select.rs:47`).
- `story:tree-suite-format` names `replay` among the steps that give JSON output.

**What Runbook's cycle needs** (read only, Runbook at `17c00ab`):

- `steps: [{do: open}, {do: run, exit: 3}, {do: replay, name: replayed}]`. The replay has no
  member of its own and its default exit is 0.
- The check `{check: all, select: [{step: replayed}, {path: "cases[*]"}], where:
  {state_matches_log: true, decision_matches_log: true}}`. So the output is an object whose
  `cases` list holds one entry per case, with those two booleans.
- Runbook's own driver did the same in its `replay` command (`driver/src/main.rs:2053-2075`):
  - it decides each case again at its last `evaluated` event;
  - it compares the state and the decision digests with the logged ones;
  - it lists a case with no `evaluated` event as `evaluated: false`;
  - it exits 0 when all match and 1 otherwise, and takes no lock.

**What Loom already has.**

- Each case's run log holds `Evaluated { case_revision, frontier, decision_digest }`
  (`generated/rust/loom/src/runlog.rs:90-99`). `tree run` appends it in `evaluated`
  (`crates/loom-tree/src/run.rs:875-911`), when the revision or the decision changed:
  - `decision_digest` is the SHA-256 of `plain(document)` (`:1615`, `hex` `:1655`);
  - the document comes from `CanonGovernor::frontier` and `decision_document`
    (`crates/loom-governor/src/lib.rs:1317-1329`);
  - the frontier id is derived from the case id, its revision and the decision (`:2848-2864`),
    so it is the same in a fresh process.
- `tree status` opens the same governor over the store without the lock and decides each case
  (`crates/loom-tree/src/status.rs:68-110`). The run's governor takes no evaluation time either
  (`run.rs:208-215`), so a decision made again needs no instant.

**A closed case is never decided again by the run.** `tree run` takes the deepest case not yet
terminated (`run.rs:261-273`, `terminated` at `:864-871`) and appends `evaluated` at the start
of that case's step (`:313`).

- So a case the run closed keeps, as its last `Evaluated` line, the one written before its
  terminating step. That step's records, revision moves and the termination come after it.
- `terminate` sets the termination and does not move the revision
  (`crates/loom-governor/src/children.rs:697-713`). The step's revision moves do (`moved`,
  `lib.rs:1857-1880`, which `record_step`, `:1780`, calls). Its records add evidence
  (`receive`, `:1630-1655`), which does not move the revision.
- A replay over the store as it is now would find every closed case different. Runbook's
  driver decided as of the logged event. D5 says how Loom does.

**After.**

- **The library.** `b10x_loom_tree::replay_tree(store, catalog, case) -> TreeReplay`
  (inferred name, new `crates/loom-tree/src/replay.rs`). `case` is `None` for the whole tree, or
  one case, which narrows the report to it and its descendants (D1). The narrowing happens in
  `replay_tree`. It opens the governor as `tree_status` does and takes the cases in
  `tree status` order.
  - It reads the case's last `Evaluated` line. `evaluated` writes one only when the case
    revision or the decision digest changed since the last (`run.rs:900-904`). So the state as of
    that line is also the state at every later step start until the next change.
  - **As of that line (D5 a),** it rebuilds the case as it was at that line. It starts from the
    held `CaseState` (`crates/loom-governor/src/lib.rs:222-248`) and undoes each line of the
    case's own log after it, newest first. The exact inverse per line kind:
    - **`ActionFinished`:**
      - every record named in `evidence` is taken out of `evidence`;
      - every move in `revisions` is moved back, the artifact from `to` to `from`, and the
        revision goes back one for each move (D6 says which moves the line names).
      - The run writes `evidence` as the ids the step added (`run.rs:406`, `:463`, `:1806`),
        leaving out the `case_outcome` records (`evidence_ids`, `run.rs:1152-1165`). The opening
        records of a `case_open` step (`children.rs:627-630`) are in it; the `case_outcome`
        records of its children are not.
    - **`ChildOpened` (child X):** the link to X is taken out of `children`, and the record with
      X's id is taken out of `evidence` (`derived_uuid("case_outcome\n<parent>\n<X>")`,
      `children.rs:548-551`; the open pushed both, `:616-623`).
    - **`ChildClosed` (child X):** nothing of its own. The undo of X's `ChildOpened` line removes
      the link and the record, in whatever form `terminate` left it (`children.rs:765-775`
      replaces it in place under the same id).
      - The invariant that makes this exact: a `ChildClosed` line after a case's last
        `Evaluated` line always has its `ChildOpened` line after that `Evaluated` line too. Only
        `tree run` writes `Evaluated` (`run.rs:313`), at the step start of the case it picks,
        and it picks the deepest case not terminated (`run.rs:260-264`). While X is open, X or a
        case below it is picked before the parent, so the parent is not evaluated between X's
        open and X's close.
      - So the open-time form of the record, whose `observed_at` the log does not hold, is never
        needed. The run-log contract does not change.
      - A log that breaks the invariant (written by hand, or by a Loom with another order) gets
        D7.
    - **`Terminated`:** `termination` is set to none. `terminate` moves no revision
      (`children.rs:697-713`).
    - **`AuthorityRecorded`:** nothing. `tree approve` writes the approval to Loom's approvals
      file and this line (`crates/loom-tree/src/approve.rs:238-258`, `record_approval`), never
      to `CaseState`, and the governor's decision does not read that file.
    - **Every other kind** (`Stopped`, `StopRecorded`, `StopLifted`, `Discarded`,
      `HandoffChecked`, `ActionStarted`, `CaseOpened`, `Evaluated`): nothing. They write no
      `CaseState` field (inferred for `Discarded` and `HandoffChecked`; the unit confirms each by
      the site that appends it).
    - **A line whose inverse does not apply** is the case's `unmatched_line` (its `seq`), and
      the undo stops there. It does not apply when:
      - a record id to take out is not held;
      - a move's `to` is not the artifact's held revision;
      - a link to take out is not held;
      - a `ChildClosed` has no `ChildOpened` of the same child after the `Evaluated` line (D7).
  - A change the store holds and the log does not record stays in the rebuilt case and shows as
    a difference. Finding such changes is what the replay is for.
  - It decides the rebuilt case through the governor path `evaluated` uses (D4 a): `frontier`
    and `decision_document` over a `MemoryCaseStore` (`crates/loom-governor/src/lib.rs:489`)
    holding that one case. It digests the document with the same `plain` and `hex` as
    `evaluated`; the digest moves into one function both call.
  - Per case it reports:
    - `case`, `evaluated`, and the `seq` and `at` of the line;
    - `case_revision` (rebuilt) and `logged_case_revision`;
    - `unmatched_line`: the `seq` of the first line whose inverse does not apply, absent when
      every line's does;
    - `state_matches_log`: the revisions are equal and there is no `unmatched_line`;
    - `decision_digest` and `logged_decision_digest`;
    - `decision_matches_log`: the digests are equal.
  - A case without an `Evaluated` line is listed under D3.
  - It writes nothing and takes no lock, as `tree status`.
- **The command.** `TreeCommand::Replay` (`crates/loom-cli/src/lib.rs:266-300`) with `--store`
  and an optional `--case`, printing `replay_json` as one JSON object `{"cases": [...]}`.
  - Exit 0 when every compared entry matches, `TREE_REPLAY_DIFFERS` (D2) when one does not, and
    `TREE_REFUSED` (1, `:586`) on a refusal: `NotATree` or `StoreUnreadable`, as `tree status`
    refuses, and `CaseNotInTree` for a `--case` the tree does not hold, as `tree approve`
    refuses one (`crates/loom-tree/src/approve.rs:200-206`; `ess/domains/tree.yaml:1120`,
    `:1162`).
  - On a difference, standard error names the first case that differs and which comparison
    failed, `state` or `decision`, as `tree verify` names its failure
    (`crates/loom-cli/src/tree.rs:686-703`).
- **The suite step.** `replay` becomes `Available`.
  - `suite/run.rs` runs it as it runs `verify` (`:1389-1396`): the exit as above, and the output
    `replay_json`. The step's `case` is resolved as an `approve` step's case.
  - Its members are as D1 says. `decode.rs:734` gives `allow` and `replay` one list today; it
    is split, so that `allow` keeps `principal`.

## Decisions

- **D1. The members of the `replay` step.** Default **a**.
  - **a.** `case` stays, optional: it narrows the report to that case and its descendants.
    `principal` is dropped from the step's members, because a replay records nothing. A suite
    that gives it is refused `MemberUndeclared`. No suite in Loom or Runbook gives it.
  - **b.** Both stay, and `principal` is read and ignored.
  - **c.** Neither: the step has the common members only.
- **D2. The exit when a case differs.** Default **a**.
  - **a.** 3, as `tree verify` exits when a log fails (`TREE_LOG_FAILED`, `lib.rs:589`), under a
    name of its own, `TREE_REPLAY_DIFFERS`.
  - **b.** 1, as Runbook's driver did. That is the exit of a refusal in Loom (`TREE_REFUSED`),
    so a difference and a refusal could not be told apart.
- **D3. A case with no `Evaluated` line** (opened, not yet decided by `tree run`). Default **a**.
  - **a.** Listed as `{case, evaluated: false}`, without the two booleans, and not counted as a
    difference, as Runbook's driver did. A suite's `all` check over `cases[*]` then fails on
    it; that is the cycle's to say.
  - **b.** Listed with both booleans false, counted as a difference.
- **D4. Which path decides again.** Default **a**.
  - **a.** The governor path `evaluated` uses, over an in-memory store holding the rebuilt case,
    so the same inputs give the same digest.
  - **b.** The pure `evaluate` (`crates/loom-governor/src/lib.rs:2654`) over the rebuilt
    snapshot, records and decisions: an independent second path. It refuses inputs the governor
    sets aside (`:2640-2646`), so a case the governor decided can be refused here.
- **D5. Which state a case is compared in.** Default **a**.
  - **a.** As of its last `Evaluated` line, rebuilt by undoing what the log records after it, as
    above. A closed case compares like an open one, and Runbook's `replay-sibling`, whose tree
    holds a closed child, can pass its `all` check.
  - **b.** A terminated case is listed as `{case, terminated: true, outcome}` and not compared.
    Simpler, but Runbook's `all` check over `cases[*]` then fails on the closed child of
    `replay-sibling`, so the cycle cannot pass.
- **D6. Where the revision moves come from.** The log never records them today: every
  `ActionFinished` is written with `revisions: Vec::new()` (`run.rs:407`, `:464`, `:1807`;
  `approve.rs:384`; `history.rs:142` is a test). A step moves revisions through `record_step`
  (governor `lib.rs:1780`, `moved` `:1857-1880`, one case revision per move). The moves of the
  refresh before a step (`run.rs:308`) come before the step's `Evaluated` line (`:313`) and
  need no undo.
  - A program step writes its moves at every end after its run, also the ends that return no
    report: a cancel, a runner refusal, a blob failure, a signal, an error exit,
    `ReportUnreadable` and `ResultNotInTable` (`crates/loom-bindings/src/program.rs:30`, `:71`,
    `:341`, `:348`, `:355`, `:361`, `:368`, `:444`, `:454`). Those ends are Commission's
    `EffectOutcomeRefused {reason, ends_run}` (`ess/commission/domains/responsibility.yaml:783`),
    which has no member for moves, and `effect_summary` reads none (`run.rs:1195-1227`).
  - Default **a**.
  - **a.** Each `ActionFinished` names the difference of the case's artifact revisions before
    and after its step, whatever the step's end.
    - The run reads `governor.revisions` (`lib.rs:1185`) where it reads `evidence_ids` before
      the step (`run.rs:317`, `:1785`) and after it (`:399`, `:456`, `:1799`). Each artifact
      whose revision differs is one move `{artifact, from, to}`, in artifact order.
    - `tree approve` reads it before `one_step` (`approve.rs:542`), so the line of a person
      program that did not end well (`approve.rs:378-384`, `:545-550`) names its moves too. Its
      `evidence` stays empty: it records nothing (`approve.rs:142-148`).
    - Exact, because a step writes its moves in one `record_step` call and `moved` gives at most
      one move per artifact per call.
      - `drive` performs one step (`run.rs:1659-1664`).
      - Every end of a program step after its run writes once and returns (the lines above,
        and `:372`, `:395`, `:497`).
      - A person step writes once. Without a program it refreshes (`person.rs:146`), and its
        write (`:254`) is given no targets (`:150`). With one, the write is `:254`, or `:319` or
        `:323` when the program did not end well.
      - Test 11 checks it: the rebuilt revision equals the logged one only when every move of
        the steps after the line was logged once.
    - The moves are not read from the effect's report, so a refused or cancelled step needs no
      member Commission's type does not have.
    - Not Breaking for a reader. `revisions` has been declared since the run log was
      (`ess/domains/runlog.yaml:84`, `:109-110`, `81a81886`), and `b10x-loom-runlog` encodes and
      decodes it (`crates/loom-runlog/src/lib.rs:518-634`). An older Loom with that decoder
      reads a line with moves; the chain digest covers it like any member.
    - What the log holds changes: a line that held `[]` now names the moves. That gets its own
      acceptance (test 10) and a `CHANGELOG.md` line.
  - **b.** The moves of a performed step only, from its report (`step.moves`,
    `program.rs:497-506`, `step_json` `:1095-1108`). The limit is stated: a refused or cancelled
    step that moved an artifact logs no move. A case whose last line after its `Evaluated` line
    is such a step shows a state difference that is not one. Test 11 would then assert that
    difference.
  - **c.** Derive the moves another way, from the workspace's placed files or the program's
    observations. The workspace has moved on since, so the moves are not reliably known.
  - **d.** Compare the revision only for a case with no `ActionFinished` line after its last
    `Evaluated` line. A closed case whose last step wrote a placed artifact is listed with
    `state_matches_log` absent. That case is in `replay-sibling` (inferred), so its check fails.
- **D7. A log that breaks the order of D5 a** (a `ChildClosed` after the last `Evaluated` line
  whose `ChildOpened` is before it). `tree run` never writes one (D5 a). Default **a**.
  - **a.** A difference naming the line: the case's `unmatched_line` is that `ChildClosed`
    line's `seq`, `state_matches_log` is false, and the command exits `TREE_REPLAY_DIFFERS`.
    Standard error names the case, `state` and the line. The other cases are replayed and
    reported as usual.
  - **b.** A named refusal, `LogNotReplayable` (a new `loom.tree.TreeRefusalReason`), exit 1,
    and no report for any case. One case's log would then hide the others' results.

## ESS first

New nouns, declared first in `ess/domains/tree.yaml` beside `loom.tree.TreeVerify` (`:375`):

- `loom.tree.TreeReplay` (`cases: List<loom.tree.CaseReplay>`);
- `loom.tree.CaseReplay`:
  - `case`, `evaluated: Boolean`;
  - `seq`, `at`, `case_revision`, `logged_case_revision`, `unmatched_line: Integer`,
    `state_matches_log`;
  - `decision_digest`, `logged_decision_digest`, `decision_matches_log`;
  - every field after `evaluated` is optional, absent when `evaluated` is false;
    `unmatched_line` is also absent when every line's inverse applies.

`loom.tree.StepKind` already holds `replay` (`:507-509`). `generated/rust/loom` is generated
again; the reference pages are made by `task docs-generate`.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs and
fixtures are POSIX sh, never Python. No test here adds a start of `b10x-loom` to the `tree_run`
binary (`story:tree-run-binary-fits-its-start-bound`). A slot job ends within 10 minutes, or
names its time with `slot --timeout` (at most 30 minutes).

**`crates/loom-tree/tests/main.rs`, a new module `replay`** (inferred), in process. Fixture: the
two-level tree of `crates/loom-cli/tests/fixtures/tree-two-level/`, opened and run through the
library. Each test copies it to a directory of its own.

1. `a_tree_with_closed_cases_replays_unchanged`.
   - Two states of the tree.
     - **At a wait where one child is closed and its sibling is open** (inferred: the wait
       after the first leg's close).
     - **Run to its end**, every case closed.
   - At the wait the test asserts the fixture's premise: after the parent's last `Evaluated`
     line its log holds a `ChildOpened` line for each child and a `ChildClosed` line for the
     closed one (D5 a's invariant, which `tree run` keeps).
   - At the wait it checks the removal: the rebuilt parent, as `b10x_loom_tree::rebuild_case`
     returns it (inferred name; public and read-only, `replay_tree` calls it), holds neither
     child's link and neither child's `case_outcome` record, the closed child's included, and
     its entry has no `unmatched_line`.
   - In both states every case is `evaluated: true` with both booleans true, and the exit value
     the command would take is 0.
   - The store's bytes are the same before and after.
   - Failing first: there is no `replay_tree`. Without D5 a, every closed case would differ.
2. `a_revision_moved_outside_the_log_is_a_state_difference`.
   - At the wait, one open case's artifact is moved through the governor's `update_revision`
     (`crates/loom-governor/src/lib.rs:1096-1121`), with no log line.
   - That case is `state_matches_log: false`; every other case matches.
   - Failing first: as test 1.
3. `a_record_received_outside_the_log_is_a_decision_difference`.
   - At the wait, one `canon-evidence/1` record is received into one open case through the
     governor's `receive` (`:1630-1655`), with no log line. The record is one the case's open
     obligation reads, so Canon's decision changes (inferred: the unit picks it from the
     fixture's protocol). `receive` does not move the revision.
   - That case is `state_matches_log: true` and `decision_matches_log: false`.
   - Failing first: as test 1.
4. `a_case_not_yet_decided_is_listed_and_not_a_difference` (D3 a).
   - Fixture: the tree opened and not run (inferred: `tree open` writes no `Evaluated` line;
     if it writes one, a child opened by the run and not yet decided).
   - That case's entry is exactly `{case, evaluated: false}`, with no booleans, and no
     difference is counted.
   - Failing first: as test 1.
5. `a_replay_of_one_case_lists_it_and_its_descendants` (D1 a).
   - On the ended tree, `replay_tree(.., Some(<the first leg>))` lists that case and its
     children, in `tree status` order, and no other case.
   - Failing first: as test 1.

**`crates/loom-tree/tests/suite_format/mod.rs`** (D1 a).

6. `a_replay_step_takes_case_and_refuses_principal`.
   - `{do: replay, case: <case>}` decodes.
   - `{do: replay, principal: person:owner}` is refused `MemberUndeclared`, naming `principal`.
   - `{do: allow, case: <case>, principal: person:owner}` still decodes.
   - Failing first: today `replay` takes `principal` (`suite/decode.rs:734`).

**`crates/loom-cli/tests/main.rs`, a new module `tree_replay`** (inferred). It starts the binary,
because the printed object and the exit are what it tests.

7. `tree_replay_prints_the_cases_and_exits_by_what_it_found`.
   - On test 1's ended tree: exit 0, and standard output is `replay_json` of the library's
     value.
   - On test 3's tree: exit 3, and standard error names that case and `decision`.
   - On test 2's tree: exit 3, and standard error names that case and `state`.
   - On test 4's tree: exit 0.
   - On an empty directory: exit 1 with `NotATree`.
   - On test 1's ended tree with `--case` naming a case the tree does not hold: exit 1 with
     `CaseNotInTree`, and nothing on standard output.
   - On test 12's tree: exit 3, and standard error names the parent, `state` and the `seq` of
     the appended `ChildClosed` line.
   - Failing first: there is no `tree replay`.

**The suite.**

8. `crates/loom-tree/tests/suite_format/mod.rs`. `replay` is `Available`, and a cycle `open`,
   `run` (exit 3), `replay` with Runbook's check passes. Failing first: the cycle is pending and
   waits for this story.
   - The tests that used `replay` as their example of a waiting step move to another waiting
     step, `allow`: `:1152-1192`, `:1273`.
   - So do `crates/loom-cli/tests/tree_test/mod.rs:226-287` and
     `crates/loom-cli/tests/fixtures/tree-suite/suite.yaml:46-47`.
   - The copy of Runbook's suite in `fixtures/tree-suite/runbook/cycles.suite.yaml:133-141`
     (`replay-unchanged`, a tree run to its end) now runs, and passes.
9. Runbook, read only. `b10x-loom tree test` over Runbook's `cycles/tree-suite.yaml` reports
   `replay-sibling` as passing, then `move to expected`. Runbook drops `pending: true` itself.

**The moves in the log (D6 a), `crates/loom-tree/tests/main.rs`, module `replay`.** Fixture: as
tests 1-5. A leg's `route.write` (`tour-leg.bindings.yaml`, `stubs/route-write.sh`) writes the
placed artifact `route`. The run is stopped by `max_steps` (`run.rs:99`, `:244`) right after
that step, before the leg's next step writes `Evaluated` (inferred: the step count the unit
finds for the first leg's `route.write`).

10. `a_step_that_moves_an_artifact_logs_the_move`.
    - That `ActionFinished` line's `revisions` is one move `{artifact: route, from, to}`, `to`
      being the revision the leg holds.
    - `b10x_loom_runlog`'s decoder reads the line back with the move; the case's log verifies,
      as `tree verify` checks it.
    - **A refused step names its move too.** The test gives the leg a bindings variant
      (`tour-leg-refused-write.bindings.yaml`, new, inferred name). Its `route.write` keeps
      `stubs/route-write.sh`, which exits 0, and gains an entry `from: exit` whose `results`
      list only `"7"`, with no `"*"`. The program writes `route`, and 0 is not in the table, so
      the step is refused `ResultNotInTable` after its moves are written (`program.rs:453-454`).
      Inferred: the entry's kind is one `route.write` may record; the unit picks it from
      `tour-leg.yaml`. That line's `revisions` is the move of `route` and its `refusal` names
      `ResultNotInTable`.
    - A `tree approve` whose person program exits non-zero after moving a placed artifact
      (inferred: a stub of the fixture's `extra/` that writes and exits 1) logs its move and no
      evidence.
    - Failing first: `revisions` is `[]` (`run.rs:407`).
11. `the_undo_of_a_logged_move_matches_and_a_foreign_one_differs`.
    - At that stop the leg's last `ActionFinished` comes after its last `Evaluated` line and names
      the move. The replay moves `route` back and reports the leg `state_matches_log: true` and
      `decision_matches_log: true`.
    - The same with the refused `route.write` of test 10, stopped by `max_steps` right after
      it: the leg matches. This is the case D6 b would report as a false difference.
    - Then `route` is moved again through the governor's `update_revision`, with no log line.
      The logged `to` is no longer held: the leg is `state_matches_log: false`, and its
      `unmatched_line` is the `seq` of the `ActionFinished` line.
    - Failing first: without D6 a the move is not logged, the rebuilt revision is one above the
      logged one, and the leg differs at the first point.

**A log that breaks D5 a's order (D7 a), module `replay`.**

12. `a_child_closed_whose_open_precedes_the_last_evaluation_is_named`.
    - Fixture: test 1's wait. Through `b10x_loom_runlog::RunLog::open(<store>)` and `append`
      (`crates/loom-runlog/src/lib.rs:215`, `:251`), the test appends to the parent's log:
      - a copy of the event of its last `Evaluated` line;
      - then a copy of the event of its `ChildClosed` line.
    - The log verifies (the `append` checks it). Its last `Evaluated` line is now after the
      closed child's `ChildOpened` line, and one `ChildClosed` line follows it.
    - The parent's entry has `state_matches_log: false` and `unmatched_line` equal to the
      appended `ChildClosed` line's `seq`. Every other case's entry is as in test 1, and the
      exit value the command would take is 3.
    - Failing first: there is no `replay_tree`. An undo that removed the link and the record
      for every `ChildClosed` line, whatever the order, would name no line.

**Documents.**

- `website/docs/guides/run-a-case-tree.md:471-473`, `:649`: the waiting example changes, and
  `replay` leaves the table of waiting parts.
- `website/docs/reference/cli.md` is generated.
- `CHANGELOG.md` `[Unreleased]` names the command.

**Gate.** `task check` exits 0.

## Out of scope

- A replay at another instant (`--at`), or of a line other than a case's last `Evaluated`.
- Runbook's re-drive class (`--recorded`, `--inject`, `--on-divergence`; Runbook's
  `docs/cycles-suite.md:239-255`), which replays model calls, not Canon's decision.
- A lock: like `tree status`, the replay reads while a writer may write. A torn read shows as a
  difference, never as a write.

## Scope

Derived 2026-11-22 from the plan tree at `3cd1de48` (the cited files are unchanged since
`afa46eea`); Runbook was read at `17c00ab`, read only.
**Cited** means read at the line named; **inferred** means a reading that could be wrong.

- `ess/domains/tree.yaml` (`:363-381`, `:507-509`, `:519`) and `generated/rust/loom` — cited.
- `crates/loom-tree/src/replay.rs` (new) — inferred.
- `crates/loom-tree/src/lib.rs` (`:47-91`), `encode.rs` (`:203`) and `status.rs` (`:68-110`) —
  cited.
- `crates/loom-tree/src/run.rs` (`:99`, `:208-215`, `:244`, `:260-273`, `:308-317`, `:396-407`,
  `:456-464`, `:680-731`, `:828-847`, `:864-911`, `:1152-1165`, `:1171-1191`, `:1195-1227`,
  `:1615`, `:1655`, `:1659-1664`, `:1785-1807`) — cited.
- `crates/loom-tree/src/approve.rs` (`:142-148`, `:200-206`, `:238-258`, `:362-384`,
  `:542-550`), changed by D6 a — cited. `person.rs` (`:146-150`, `:254`, `:319-323`), read and
  not changed — cited.
- `crates/loom-bindings/src/program.rs` (`:30`, `:71`, `:341-497`, `:1095-1108`),
  `ess/commission/domains/responsibility.yaml` (`:783-789`), `ess/domains/runlog.yaml` (`:84`,
  `:109-110`) and `crates/loom-runlog/src/lib.rs` (`:215`, `:251`, `:518-634`), read and not
  changed — cited.
- `crates/loom-cli/tests/fixtures/tree-two-level/` (`tour-leg.bindings.yaml`,
  `stubs/route-write.sh`) — cited. The variant `tour-leg-refused-write.bindings.yaml` and a
  failing person stub under `extra/` (new) — inferred.
- `crates/loom-tree/src/suite/availability.rs` (`:56`, `:77`), `decode.rs` (`:696`, `:734`),
  `run.rs` (`:1389-1396`, `:1425-1433`) and `select.rs` (`:47`) — cited.
- `crates/loom-cli/src/lib.rs` (`:266-300`, `:586-589`) and `crates/loom-cli/src/tree.rs`
  (`:39-50`, `:686-703`) — cited.
- `crates/loom-governor/src/lib.rs` (`:222-248`, `:489`, `:1096-1121`, `:1185`, `:1317-1329`,
  `:1630-1655`, `:1686-1695`, `:1780`, `:1857-1880`, `:2640-2654`, `:2848-2864`) and
  `children.rs` (`:266-297`, `:548-551`, `:608-631`, `:697-713`, `:765-775`), read and not
  changed — cited.
- `crates/loom-tree/tests/main.rs` and `tests/suite_format/mod.rs` (`:1152-1192`, `:1273`) —
  cited; the module `replay` is inferred.
- `crates/loom-cli/tests/main.rs`, `tests/tree_test/mod.rs` (`:226-287`),
  `tests/fixtures/tree-suite/suite.yaml` (`:46-47`) and
  `tests/fixtures/tree-suite/runbook/cycles.suite.yaml` (`:133-141`) — cited; the module
  `tree_replay` is inferred.
- `website/docs/guides/run-a-case-tree.md` (`:471-473`, `:649`), `website/docs/reference/cli.md`
  (generated), `CHANGELOG.md` and `AGENTS.md` — cited, except `AGENTS.md`, inferred.

## Edges

- `depends_on story:tree-suite-format` (active, merged on this branch): the step `replay`, its
  decoder and the availability table are its.
- `AGENTS.md`, `CHANGELOG.md` and the generated files (`generated/rust/loom`, the reference
  pages) are shared with most stories. They are textual merges, regenerated on the merge, and
  are not listed below.
- **Would collide with**, no edge. Each entry names the shared file and the merge order:
  - `crates/loom-tree/src/suite/availability.rs`, `run.rs` and the waiting-step tests
    (`suite_format/mod.rs`, `tree_test/mod.rs`, the guide): `tree-test-decide-and-requests`
    (proposed) makes `decide` available. Under this story the waiting example moves to `allow`,
    not `decide`, so the two do not take the same example. This story merges first. Textual
    merge.
  - The same files: `tree-suite-approve-result-and-reasons`, `suite-cycle-writes-a-workspace-file`
    and `tree-suite-stop-resume-reopen-steps` (active, merged on this branch) and
    `tree-test-command`, `suite-runs-cycles-in-parallel` and
    `suite-runs-cycles-under-one-cgroup-root` (active). Other steps and members. Textual merge,
    no order.
  - `ess/domains/tree.yaml`: `tree-run-command`, `tree-test-command` and
    `program-reports-its-cost` (active; `loom.tree.StatusRun`, `:228`), and the
    proposed tree stories (`tree-waits-open-decision-requests`, `tree-usage-and-model-budget`,
    `standing-tree-rests-quiescent`, `standing-tree-wakes-at-a-host-instant`). New types beside
    `TreeVerify`; others add theirs elsewhere. Textual merge, no order.
  - `crates/loom-cli/src/lib.rs` and `tree.rs` (`TreeCommand`, the dispatch):
    `tree-run-command`, `tree-run-takes-a-delegated-cgroup`, `person-action-runs-a-program`,
    `person-action-records-a-named-result` and `every-printed-decision-command-runs` (active);
    `decision-inbox-commands`, `daemon-continues-a-decided-request`,
    `question-and-outcome-decisions-hold-runs` and `tree-waits-open-decision-requests`
    (proposed);
    `secrets-subcommand` and `mantle-placement` (draft). One variant each. Textual merge, no
    order.
  - `crates/loom-tree/src/run.rs`: the digest function moves out of `evaluated`
    (`:875-911`). A story that edits `evaluated` merges after this one. The rest of `run.rs` is a
    textual merge, no order, with:
    - active: `cancel-a-running-bound-program`, `cancelled-step-may-run-again`,
      `every-printed-decision-command-runs`, `handoff-check-holds-the-cancel`,
      `handoff-when-a-program-decides`, `next-decision-names-the-principal`,
      `outcome-when-reads-the-run-count`, `person-action-runs-a-program`,
      `program-reports-its-cost`, `tree-library-exports-what-an-engine-needs`,
      `tree-run-command`, `tree-suite-format`, `tree-suite-stop-resume-reopen-steps` and
      `tree-test-command`;
    - draft: `requests-from-canon-pending-decisions`;
    - the proposed stories that scope `crates/loom-tree` as a whole (below).
  - `crates/loom-tree/src/approve.rs` (D6 a: the moves of a person step):
    `person-action-runs-a-program`, `person-action-records-a-named-result`,
    `every-printed-decision-command-runs` and `next-decision-names-the-principal` (active, merged
    on this branch: `0d0f9fed`, `5af3f9c2`, `0198a5d1`). This story edits their code as
    merged. Textual merge, no order.
  - `crates/loom-cli/tests/fixtures/tree-two-level/` (tests 10 and 11 add a bindings variant
    and a person stub; no existing file changes): `person-action-runs-a-program` (active,
    merged on this branch), `tree-waits-open-decision-requests`, `readme-logo-and-badges`
    (proposed), `mantle-placement` (draft) and `one-test-binary-per-crate` (active). New files
    only. Textual merge, no order.
  - `crates/loom-cli/tests/fixtures/tree-suite/suite.yaml`:
    `question-and-outcome-decisions-hold-runs` (proposed) scopes `crates/loom-cli/tests/fixtures`.
    Other cycles. Textual merge, no order.
  - `one-test-binary-per-crate` (active) moves `crates/loom-cli/tests` and
    `crates/loom-tree/tests` into one binary each. The new modules go where it puts them.
    Textual merge, no order.
  - The proposed stories that scope `crates/loom-tree` as a whole (for example
    `agent-node-run-records`, `run-caps-per-arrival` and `child-case-own-workspace`). Textual
    merge, no order.
