---
format: aep.planning-md/3
id: story:run-caps-per-arrival
kind: story
status: draft
title: A writer's max_runs can count from its case's latest outside revision, an observation has no run cap, and a used-up writer stops the run naming its last failure
summary: Writer and observation told apart by the Canon effect class; budget.runs_per case or arrival for writers; an arrived run-log line; TreeStop::RunsSpent exit 4, never a wait on a person
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:bound-program-result-table
- depends_on: story:bound-agent-node
- depends_on: story:bound-artifact-revisions
- depends_on: story:tree-run-command
- depends_on: story:standing-tree-rests-quiescent
revision: 4
---
## Outcome

Loom tells a writer from an observation by the Canon action's effect class, as the delivered
selector and `story:bound-artifact-revisions` already do: a writer is an action whose effect class
is `write` (its step may move an artifact's revision); an observation is any other action that may
produce evidence (a non-empty `may_produce`, its binding's evidence entries read from `exit` or a
report), whose records are about a subject at its revision. The rule is session `sdc`'s
(2026-11-21, `.engineering/drafts/e4-sdc-answers.md`, change 3).

- **Writers.** `max_runs` (default 8) counts per case, as today, or, with `budget.runs_per:
  arrival`, since the latest arrival in the case. A writer whose own writes move its artifact does
  not reset its count, so a writer and a check that keeps failing loop at most `max_runs` times
  between two arrivals.
- **Observations.** `max_runs` does not bind them: the effect port never refuses an observation
  `RunsSpent`. An observation runs at most once per case revision unless the host reports it due,
  which is the delivered selector rule, and is bounded only by the tree's step budget. A binding
  that states `runs_per` on an observation is refused at load `RunsScopeOnObservation`, naming the
  action.
- **A used-up writer stops the run.** When the selector picks a writer the effect refuses
  `RunsSpent`, `tree run` stops that case with the new stop `TreeStop::RunsSpent`: the case, the
  action, the runs counted, the scope (`case` or `arrival`) and its last failure, the latest record
  about the writer's output whose result its `retry_when` lists (kind, result, producer, subject
  revision), absent when there is none. Exit 4, the status of a case that cannot go on; it is never
  a wait on a person (exit 3) and prints no `tree approve` command.

An arrival is a revision move of a placed artifact that no step of that case made: before each
evaluation Loom computes the placed artifacts' revisions (`story:bound-artifact-revisions`), and
when one moved since the case's last evaluation or step, the case's run log gets one `arrived`
line naming each moved artifact with its old and new revision, before the `evaluated` line. A move
a step of the case made is in that step's `action finished` line and is no arrival. A file the host
writes (an `intake` entry), a person's edit, and another case's write into a shared workspace are
arrivals. This is the one detection of an outside move in the epic;
`story:standing-tree-wakes-at-a-host-instant` takes its moved artifacts from it.

The count of a writer is the number of `action finished` lines of that action in the case's run log
after its latest `arrived` line (after `case opened` when there is none) under `arrival`, and over
the whole log under `case`, for `program` and `agent` executors alike.

## Why

"Per action `max_runs` (default 8) counts every `action_finished` of the case's whole life ...: a
case that lives for months would exhaust it" (`~/beyond10x/sdc/docs/orchestration.md` § 1.2);
"per-action `max_runs` counted per subject revision, not per case life" (§ 3). Session `sdc`
narrowed it on 2026-11-21: "Run caps per arrival: right for writers ..., wrong for observations
... an observation has no per-arrival cap; it runs at most once per subject revision unless
reported due ..., bounded only by the step budget. ... A used-up writer must stop the run with a
reason that names the action and its last failure, never as a wait on a person." Its evidence is
trial 2, run note R4 (`~/beyond10x/sdc/docs/trial-2.md`, "R4: stuck again after the f1 decision"):
"the default `max_runs` 8 also bound the observations. `questions.check` had run 8 times ... so
neither could check the amended revision", after which a writer was admitted and "wrote a fresh
interview ... over Timo's answers".

Loom counts the same way today: `story:bound-program-result-table` § Acceptance, "An action that
has already run its `budget.max_runs` times in this case is not performed and is reported
`RunsSpent`", for every program action. The effect class is the Canon action's `effect`
(`ActionFacts.effect`, `crates/loom-governor/src/lib.rs:1184-1196`); the selector orders
non-`write` before `write` by it (`crates/loom-executor/src/selection.rs:319`), and
`story:bound-artifact-revisions` § Outcome says "An action whose effect class is not `write` that
changed an artifact records no evidence for that step". The selector's rule is per case revision
(`selection.rs:285-305`: `due`, then `performed_at(action, case_revision)`); the requester's
"subject revision" is read as that rule (work order § Open). A writer is sent back by `retry_when`
(`story:tree-run-command` § Acceptance, "a writer whose check failed with a `retry_when` result is
taken again"), which the loader checks against the protocol's kinds
(`crates/loom-bindings/src/load.rs:118-123`).

## Waits on

- `story:standing-tree-rests-quiescent`: the story before this one on `ess/domains/tree.yaml`
  (`TreeStop`), `crates/loom-tree`, `crates/loom-cli/src/lib.rs` and
  `crates/loom-cli/tests/fixtures/area/`, which this story extends.
- `story:tree-run-command`: `crates/loom-tree`, the run that writes the log lines and stops, and
  `StepHistory`.
- `story:bound-program-result-table` (`RunsSpent`, the program count), `story:bound-agent-node`
  (the agent count, `retry_records`), `story:bound-artifact-revisions` (the revision computation
  before each evaluation).

Comes before, on shared files: `story:tree-usage-and-model-budget` (`ess/domains/runlog.yaml`,
`ess/domains/tree.yaml`, `crates/loom-tree`), and through it every later story of the epic;
`story:child-case-own-workspace` also shares `ess/domains/bindings.yaml` and
`crates/loom-bindings/src/load.rs` with this story, and `story:capacity-given-to-an-admission-program`
shares those and `crates/loom-bindings/src/decode.rs` and the program port; both are ordered after
this story through the chain.

## ESS first

`ess/domains/bindings.yaml`: `RunsScope` (`Case`, `Arrival`), `ActionBudget` gains optional
`runs_per`, and the refusal `RunsScopeOnObservation` (action). `ess/domains/runlog.yaml`: the event
`arrived` (moved artifacts, each with old and new revision). `ess/domains/tree.yaml`: `TreeStop`
gains `RunsSpent` (case, action, runs, scope, optional last failure). On that commit `task drift`
fails and `crates/loom-cli/tests/tree_runs_per_arrival.rs` does not compile.

## Acceptance

The tests drive `b10x-loom tree run` (the layer that writes `arrived` and stops), in
`crates/loom-cli/tests/tree_runs_per_arrival.rs`, with files added to
`crates/loom-cli/tests/fixtures/area/`: `draft.protocol.yaml` (`draft/1`: an input artifact
`brief`, an artifact `draft`, a writer `draft.write` with effect `write`, an observation
`draft.check` that may produce `draft_check` about `draft`), `draft.bindings.yaml`
(`draft.write` with `retry_when: {draft_check: [invalid]}`) and stub programs: `draft.write`
appends one line to `draft`; `draft.check` exits 0 (`valid`) once `draft` has the number of lines
the test writes into `brief`, else 1 (`invalid`). No model.

- `draft.write` with `max_runs: 2` and `runs_per: arrival`, `brief` asking for 5 lines: the run
  takes write, check, write, check, then stops `RunsSpent` exit 4 naming the case, `draft.write`,
  2 runs, scope `arrival` and last failure `draft_check` `invalid` by the check's producer; the
  output holds no `tree approve` command, and no `arrived` line was written for the writer's moves.
- The test then rewrites `brief` (asking for 3 lines): the next `tree run` writes one `arrived`
  line naming `brief` with both revisions before its first `evaluated` line, takes `draft.write`
  once more, `draft.check` records `valid`, and the case goes on.
- The same fixture with `runs_per: case` (and with no `runs_per`): after the rewrite of `brief` the
  next `tree run` stops `RunsSpent` again with scope `case` and takes no step.
- `draft.write` with `max_runs: 12` and `brief` asking for 10 lines: `draft.check` runs 10 times
  and is never refused, although no `max_runs` above 8 is stated for it; the log holds exactly one
  `action finished` line of `draft.check` per case revision.
- An agent writer (a stub adapter, no model) with `runs_per: arrival` is counted and stopped the
  same way.
- The count after the store is reopened equals the count before it.
- A bindings file with `runs_per: arrival` on `draft.check` is refused at load
  `RunsScopeOnObservation` naming the action, and one with `runs_per: revision` naming the member
  (`crates/loom-bindings/tests/runs_per.rs`).
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/runlog.yaml`, `ess/domains/tree.yaml`, generated
`generated/rust/loom/src/{bindings,runlog,tree}.rs`; `crates/loom-bindings/src/decode.rs`,
`crates/loom-bindings/src/load.rs`, the program and agent ports' run count (modules named by
`story:bound-program-result-table` and `story:bound-agent-node`, not read); `crates/loom-tree` (the
`arrived` line, the `RunsSpent` stop); `crates/loom-cli/src/lib.rs` (the stop's output);
`crates/loom-cli/tests/tree_runs_per_arrival.rs`, `crates/loom-bindings/tests/runs_per.rs` (new);
`crates/loom-cli/tests/fixtures/area/` (adds `draft.protocol.yaml`, `draft.bindings.yaml`,
`bin/draft-write`, `bin/draft-check`); `CHANGELOG.md` (`**Breaking:**` observations are no longer
refused `RunsSpent`).

## Out of scope

Counting per revision of one subject artifact (Canon names no subject per action). Not counting a
program that could not start (exit 126 or 127, trial 2 R3). Tree budgets at a wake
(`story:standing-tree-wakes-at-a-host-instant`). Telling one case's write from a person's in a
shared workspace: both are arrivals for the other case.
