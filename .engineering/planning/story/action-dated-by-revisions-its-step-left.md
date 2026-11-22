---
format: aep.planning-md/3
id: story:action-dated-by-revisions-its-step-left
kind: story
status: active
title: An action is dated by the revisions its step left
summary: tree run writes ActionFinished.revisions (the step's artifact moves, before/after difference shared with tree-replay-command) and LogHistory dates each action at the revision of its step and the revision its step left, not the next evaluated line; older logs with no moves read as before
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-replay-command
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-tree/src/history.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: crates/loom-tree/tests/cancel_sec/mod.rs
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T22:20:05Z", actor: "agent:sdc", revision: 12, executor: "agent:loom", correlation: "runbook-dated-by-revisions-2026-11-22"}
- {from: "proposed", to: "active", at: "2026-11-22T22:20:06Z", actor: "agent:sdc", revision: 13, executor: "agent:loom", correlation: "runbook-dated-by-revisions-2026-11-22"}
---
## Outcome

`LogHistory` dates each action by the artifact revisions its step moved, as its `action finished`
line names them: at the revision of its step and at the revision its step left, not at the
revision of the next `evaluated` line. An action whose inputs a later step moved is useful again;
a writer is still not taken again on its own write. A log in which no line names a move (one an
older Loom wrote) is read as today. This story owns the derivation of the moves that
`story:tree-replay-command` merged, and keeps it complete for every end of a step. Runbook asked
for this first in the next wave. Cites are at `79094d48`.

**Before.**

- **The moves are written since `tree-replay-command` merged (`79094d48`).** Every
  `ActionFinished` site sets `revisions: revision_moves(before, after)`:
  - `crates/loom-tree/src/run.rs:408` and `:465-468`, in the run loop;
  - `:1850`, in `one_step`, which `tree approve` takes;
  - `crates/loom-tree/src/approve.rs:550`, for a person program that did not end well. It
    reads the revisions before `one_step`, at `:544`.
  - `before` and `after` are `artifact_revisions` (`run.rs:1181-1188`, the governor's
    `revisions`), read before the step (`:318`, `:1828`) and after it.
  - `revision_moves` (`:1190-1207`) gives one move per artifact whose revision differs, in
    artifact order. It is the difference-based derivation, so a step's moves are named whatever
    its end. Replay's merged tests show it for a refused program step and a failed person
    program (`crates/loom-tree/tests/replay/moves.rs:43-75`). No test shows it for a cancelled
    step yet.
- **The history still ignores them.** `LogHistory::new` (`crates/loom-tree/src/history.rs:43-82`,
  unchanged by the merge) counts an action as performed:
  - at the revision of the last `evaluated` line before it (`:65-67`);
  - at the revision of the next `evaluated` line, or `current` at the end (`:54-58`, `:73-75`).
- **Why that is wrong.** `evaluated` writes a line only when the case revision or the decision
  digest changed (`run.rs:893-898`). So two steps can run between two `evaluated` lines: a first
  step that moved nothing, then a second that moved artifacts.
  - The first step's action is then dated at the revision the second step left.
  - `DeterministicSelector::useful` (`crates/loom-executor/src/selection.rs:285-306`) returns
    "not useful" at `:299-303`, before `may_produce` and the open evidence are read (`:305`).
  - With nothing else useful, the run stops `Stuck` (`run.rs:1538-1539`).
- **Seen in Runbook's record `parity-loom-strong-4`, run 2** (a log written before the moves
  were):
  - `amend.upstream` ran at revision 9;
  - `requirements.normalise` ran after it, with no `evaluated` line between;
  - the next `evaluated` line was at revision 11;
  - `amend.upstream` counted as performed at 11 and was not taken again on the rejection it was
    to answer. Its `handoff_check` exited 1, and the run stopped `Stuck`.
- **The intent is written already:**
  - `AGENTS.md:1709-1710`: "performed at the revision of its step and at the revision its step
    left";
  - `docs/contracts/loom-action-selection.md:172-174`: "useful again once an artifact moves";
  - `history.rs:6`: "a writer is not taken again on its own write";
  - `story:deterministic-default-selector` (body lines 72-74).

**After.**

- **The derivation stays the difference, and this story owns it.** `revision_moves` over
  `artifact_revisions` before and after the step, at the four sites above, unchanged. The
  history below depends on the moves being complete, so the story names why they are and adds
  the missing test:
  - `drive` performs one step (`run.rs:1700-1706`).
  - A step writes its moves in one `record_step` call, and `moved` gives at most one move per
    artifact per call (`crates/loom-governor/src/lib.rs:1856-1882`).
  - Every end of a program step after its run writes once, cancelled, refused and failed ends
    included (`crates/loom-bindings/src/program.rs:341`, `:348`, `:355`, `:361`, `:368`,
    `:372`, `:395`, `:444`, `:454`, `:497`).
  - A person step writes once (`crates/loom-tree/src/person.rs:146` with no program, its
    targets empty at `:150` for the write at `:254`; with one, `:254`, `:319` or `:323`).
  - A derivation from a step's report (`step.moves`, `program.rs:1095-1108`) would name nothing
    for the cancelled, runner-refused, blob-failed, signalled and error ends, which return
    `refused(...)` or `cancelled_step(...)` with no report (`program.rs:337-368`). It is not
    used. The doc comment of `revision_moves` (`run.rs:1190-1193`) names this story beside
    replay's D6 a.
  - The run-log contract does not change: `docs/contracts/run-log-v1.json:28-45` declares
    `revisions` already. This story's diff leaves the file unchanged.
- **The history dates by the moves (D1 a).** `LogHistory::new` keeps a running revision.
  - At an `evaluated` line it becomes that line's `case_revision`. A line before the first
    `evaluated` line has no running revision and is dated as today (`history.rs:65-68`).
  - At an `action finished` line, the action counts as performed at the running revision (the
    revision of its step). The running revision then rises by the number of its moves, and the
    action counts at the new value too (the revision its step left).
  - A step its cancel ended counts at neither revision (`history.rs:11-14`,
    `story:cancelled-step-may-run-again`). Its moves still raise the running revision: a
    cancelled program step writes its moves alone (`program.rs:341`).
  - A refused or failed step counts at both revisions, as today (D2 a).
- **Older logs (D1 a).** At each `evaluated` line, and at the end against `current`, the history
  checks that the moves of the interval's lines account for the revision there.
  - When they do, every line of the interval is dated as above. A line with no moves moved
    nothing.
  - When they do not, some move of the interval was not logged. That is a log an older Loom
    wrote, or a workspace edit the refresh took at the next step start (`run.rs:306-312`). Then
    each line of the interval that names no moves is also counted, as today, at the revision of
    that `evaluated` line, else `current`.
  - A log in which no line names a move therefore reads exactly as today; Runbook's record above
    keeps its old reading. Log format v1 stays readable, and no line is rewritten.

## Decisions

- **D1. How a line that names no moves is dated.** A line written before the moves were and a
  line of a step that moved nothing look the same. Default **a**.
  - **a.** As a step that moved nothing, when the moves the interval's lines name account for
    the revision of the interval's closing `evaluated` line (or `current`). Otherwise also at
    that revision, as today.
    - Older logs read as today.
    - The limit: in a new log, a step that moved nothing, in an interval that a workspace edit
      closes, still counts at the edited revision, as today.
  - **b.** Always as a step that moved nothing. A new log is dated exactly. In an older log a
    writer is taken again once on its own write, against `history.rs:6`, and test 3 fails.
  - **c.** Tell new logs apart by a marker: a new member on `case opened` or `action finished`.
    The v1 decoder refuses a member its kind does not declare (`decode_line`,
    `crates/loom-runlog/src/lib.rs:653-655`, `closed_at` over the declared members), so an older
    Loom could not read a new log. Breaking.
- **D2. A refused or failed step.** Default **a**.
  - **a.** Dated like any step, at the revision of its step and at the one it left; its moves
    raise the running revision. Today it counts as performed (`history.rs:60-69` skips only a
    cancelled step), and `history_knows_a_cancelled_line` (`history.rs:148-163`) holds a
    refused `RunsSpent` line as performed.
  - **b.** Dated at the revision of its step only. A refused writer whose write moved an
    artifact would then be taken again on its own write.

## ESS first

Exempt: `loom.runlog.ActionFinished.revisions` and `loom.runlog.RevisionMove` are declared
(`ess/domains/runlog.yaml:82-110`) and generated; no noun or rule changes. How the history reads
the member changes; `CHANGELOG.md` names it.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Fixtures are
POSIX sh, never Python. No test here starts `b10x-loom` or lives in the `tree_run` test binary.
A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30 minutes).

Runbook's four: (1) is test 1, (2) is test 2, (3) is test 3, (4) is replay's merged
`a_step_that_moves_an_artifact_logs_the_move` (`tests/replay/moves.rs:43-75`), kept green, with
test 6 for the cancelled end.

**`crates/loom-tree/src/history.rs`, its `mod tests`**, in process. Fixture: logs built in the
test, as `log` builds one (`history.rs:118-146`).

1. `an_action_is_useful_again_when_a_later_step_moved_an_artifact` (red first).
   - Log: `evaluated` at 9; `action finished` A with no moves; `action finished` B with two
     moves; `evaluated` at 11. This is the shape of `parity-loom-strong-4` run 2, written with
     moves.
   - `performed_at(A, 9)` is true and `performed_at(A, 11)` is false; `performed_at(B, 11)` is
     true.
   - Through `DeterministicSelector` with a facts stub in which A may produce a record and its
     evidence is open, A is selected at 11 (inferred: the stub implements `ActionFactsPort`,
     `selection.rs`).
   - Variant: A moves one artifact (9 to 10) and B one (10 to 11), with no `evaluated` line
     between them. A counts at 9 and 10 and not at 11.
   - Failing first: today A counts at 11 (`history.rs:54-58`).
2. `a_writer_is_not_taken_again_on_its_own_write`.
   - Log: `evaluated` at 9; `action finished` A with one move; `evaluated` at 10.
     `performed_at(A, 10)` is true.
   - The same with the log ending after A and `current` 10.
   - Failing first: none today; it is the guard of `history.rs:6`. It fails on a history that
     dates by the step's revision alone.
3. `an_older_log_with_no_moves_is_read_as_before`.
   - Log: `evaluated` at 9; `action finished` A and B with no moves; `evaluated` at 10. A and B
     count at 9 and at 10, as `LogHistory::new` at `79094d48` counts them; the test holds that
     result as its expected value.
   - Variant: the log ends after B, and `current` is 10. The same.
   - Failing first: none today; it is the guard of D1 a. It fails on D1 b.
4. `a_cancelled_step_counts_at_no_revision_and_its_move_still_counts`.
   - Log: `evaluated` at 9; `action finished` A refused `Cancelled: action A: …` with one move;
     `action finished` B with no moves; `evaluated` at 10.
   - `performed_at(A, 9)` and `performed_at(A, 10)` are false. `performed_at(B, 10)` is true,
     and `performed_at(B, 9)` is false: B ran at the revision A's move raised.
   - Failing first: today B counts at 9 (the last `evaluated` line), not at 10 alone.
5. `a_refused_step_counts_at_both_revisions` (D2 a).
   - Log: `evaluated` at 9; `action finished` A refused `ProgramError: action A: …` with one
     move; `evaluated` at 10.
   - `performed_at(A, 9)` and `performed_at(A, 10)` are true.
   - Variant: a second step B with one move follows A before the `evaluated` line at 11. A
     counts at 9 and 10, not at 11.
   - Failing first: the variant; today A counts at 11.

**`crates/loom-tree/tests/cancel_sec/mod.rs`**, in process through `run_tree`. Fixture: its
`halt@1` tree (`Tree::open`, `:77-109`; `work: work.txt`), under Substrate's `local` adapter.

6. `a_cancelled_step_that_moved_an_artifact_names_its_move`.
   - `do.sh` is `echo more >> work.txt`, `touch target/started`, `sleep 30`. The test waits
     for `target/started` (`wait_for`, `:131-141`), sets the cancel, and the run ends
     `Cancelled`.
   - The last `action finished` line of `root` is `work.do`, refused `Cancelled: …`, and its
     `revisions` is one move of `work` whose `to` is the case's revision of `work` now.
   - About 1 s (the run's watcher and the `local` runner each take about 100 ms, `:202-203`).
   - Failing first: none at `79094d48` if the cancelled end writes as `program.rs:341` says; it
     fails on a derivation from the step's report. It is the guard of the derivation this story
     owns.

**Gate.** `task check` exits 0. `CHANGELOG.md` `[Unreleased]` names the change.

## Out of scope

- A marker that tells a new log from an older one (D1 c).
- Rewriting older logs, Runbook's record among them.
- Changing `useful` or `StepHistory` (`selection.rs:224`, `:285-306`).
- Changing the replay (`crates/loom-tree/src/replay.rs`) beyond keeping its tests green.

## Scope

Derived 2026-11-22 from the plan tree at `79094d48`. **Cited** means read at the line named;
**inferred** means a reading that could be wrong.

- `crates/loom-tree/src/history.rs` (`:1-14`, `:43-82`, `:108-164`) — cited.
- `crates/loom-tree/src/run.rs`: only the doc comment of `revision_moves` (`:1190-1193`) — cited.
- `crates/loom-tree/tests/cancel_sec/mod.rs` (`:77-141`) — cited.
- `CHANGELOG.md` — inferred.
- Read and not changed — cited:
  - `crates/loom-tree/src/run.rs` (`:148`, `:306-318`, `:400-408`, `:457-468`, `:893-898`,
    `:1032`, `:1181-1207`, `:1538-1539`, `:1700-1706`, `:1827-1850`);
  - `crates/loom-tree/src/approve.rs` (`:378-384`, `:497-554`);
  - `crates/loom-tree/tests/replay/moves.rs` (`:43-75`);
  - `crates/loom-executor/src/selection.rs` (`:224`, `:285-306`);
  - `crates/loom-governor/src/lib.rs` (`:1185`, `:1856-1882`);
  - `crates/loom-bindings/src/program.rs` (`:337-497`, `:1095-1108`);
  - `crates/loom-tree/src/person.rs` (`:146-150`, `:254`, `:319-323`);
  - `crates/loom-runlog/src/lib.rs` (`:630-655`);
  - `ess/domains/runlog.yaml` (`:82-110`) and `docs/contracts/run-log-v1.json` (`:28-45`);
  - `AGENTS.md` (`:1709-1710`) and `docs/contracts/loom-action-selection.md` (`:172-174`).

## Edges

- `depends_on story:tree-replay-command` (active, merged on this branch at `79094d48`; no
  cycle). It wrote the moves and `revision_moves`; this story reads them in the history, owns the
  derivation from here on, and keeps replay's tests green: `tests/replay/moves.rs:43-75` and
  `the_undo_of_a_logged_move_matches_and_a_foreign_one_differs` (`tests/replay/mod.rs:237-277`).
  The undo moves back one revision per logged move, so it needs the moves complete, as this
  story does.
- No other `depends_on`: `story:cancelled-step-may-run-again` and
  `story:deterministic-default-selector` are merged.
- `AGENTS.md` and `CHANGELOG.md` are shared with most stories. They are textual merges and are
  not listed below.
- **Would collide with**, no edge. Each entry names the shared file and the merge order:
  - `program-reports-its-cost` (active, not merged) adds `cost_micro_usd` to `ActionFinished`,
    writes it at the same literals (`run.rs:402-408`, `:459-469`, `:1844-1851`;
    `approve.rs:378-385`) and changes `docs/contracts/run-log-v1.json`. This story changes none
    of those literals or the contract: textual merge, either order.
  - `person-action-records-a-named-result` and `every-printed-decision-command-runs` (active in
    the store, merged on this branch at `5af3f9c2`). Their `ActionFinished` writes are in
    `take_person_action` (`approve.rs:497-554`): `:551` names its moves through `:550`, and
    `:554` through `one_step` (`run.rs:1850`). So "every site" holds at `79094d48`, and this
    story touches neither `person_action` (`approve.rs:428`) nor `take_person_action`. No
    order.
  - `story:tree-test-decide-and-requests` (proposed): its scope (`suite/availability.rs`,
    `suite/run.rs`, `suite/select.rs`, `tree_test/mod.rs`, the tree-suite fixtures, the guide)
    shares no file with this story. No order.
  - `story:tree-run-binary-fits-its-start-bound` (active): it edits `tree_run.rs`,
    `tree_run/stop.rs`, `loom-xtask` and `.config/nextest.toml`; this story touches none of
    them and adds no start to `tree_run`. Its key (its D3 a) covers `history.rs` through
    `b10x-loom`'s dep-info, so this change rebuilds its shared trees once. No order.
  - `crates/loom-tree/src/run.rs` (one doc comment): `tree-test-command` (active, not merged).
    Textual merge, no order.
  - `crates/loom-tree/tests/cancel_sec/mod.rs`: `handoff-check-holds-the-cancel` (merged). A
    new test beside its two. No order.
