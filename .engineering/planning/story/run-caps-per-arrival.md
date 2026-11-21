---
format: aep.planning-md/3
id: story:run-caps-per-arrival
kind: story
status: proposed
title: A writer's max_runs can count from its case's latest outside revision, an observation takes no run cap, and a used-up writer stops the run naming its last failure
summary: Writer and observation told apart by the Canon effect class; budget.runs_per case or arrival for writers; max_runs or runs_per on an observation refused at load RunsCapOnObservation; an arrived run-log line; TreeStop::RunsSpent exit 4, never a wait on a person
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
scope:
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/tests/adversary_w20261121w1_u2.rs
- confidence: cited
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-bindings/tests/runs_per.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area
- confidence: cited
  path: crates/loom-cli/tests/tree_runs_per_arrival.rs
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: crates/loom-tree/tests/runs_per_agent.rs
- confidence: cited
  path: docs/contracts/loom-action-selection.md
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: website/docs/guides/bindings.md
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:29:07Z", actor: "agent:sdc", revision: 13, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

Loom tells a writer from an observation by the Canon action's effect class, as the delivered
selector and `story:bound-artifact-revisions` already do: an observation is an action whose effect
class is not `write` and that may produce evidence (a non-empty `may_produce`, its binding's
evidence entries read from `exit` or a report), whose records are about a subject at its revision;
every other action is a writer for run caps. That is an action whose effect class is `write` (its
step may move an artifact's revision), and also an action whose effect class is not `write` and
whose `may_produce` is empty: the selector finds such an action useful at every case revision it
has not run at (`crates/loom-executor/src/selection.rs:300-305`), and it records nothing a check
could read, so only a run cap bounds it. The rule is session `sdc`'s (2026-11-21,
`.engineering/drafts/e4-sdc-answers.md`, change 3; `.engineering/drafts/e4-sdc-answers-2.md`).
This story lands it for every executor; `story:bound-program-result-table`, which it depends on,
counts `max_runs` for every program action, checks it with a writer, and leaves the rule to this
story (its § Out of scope).

- **Writers.** `max_runs` (default 8) counts per case, as today, or, with `budget.runs_per:
  arrival`, since the latest arrival in the case. A writer whose own writes move its artifact does
  not reset its count, so a writer and a check that keeps failing loop at most `max_runs` times
  between two arrivals.
- **Observations.** `max_runs` does not bind them: the effect port never refuses an observation
  `RunsSpent`. An observation runs at most once per case revision unless the host reports it due,
  the delivered selector rule (accepted by session `sdc` on 2026-11-21: "an extra run after an
  unrelated artifact moved is harmless"), and is bounded only by the tree's step budget. A binding
  that states `budget.max_runs` or `budget.runs_per` on an observation is refused at load
  `RunsCapOnObservation`, naming the action and the member (session `sdc`: "A setting that is
  accepted and not read is how today's trial defect stayed hidden"). The loader can tell, because
  it already finds the protocol's declared action (`crates/loom-bindings/src/load.rs:92-100`). To
  tell a stated `max_runs` from the default, `ActionBudget.max_runs` becomes optional in the
  decoded document (absent when not stated); the default 8 is applied to writers by the effect
  port, and `DEFAULT_MAX_RUNS` stays 8.
- **Decoding `runs_per`.** `budget` gains the member `runs_per`, a string, `case` or `arrival`. The
  decoder's rule that every `budget` member is a whole number from 1
  (`crates/loom-bindings/src/decode.rs:679-686`) changes: it holds for the numeric members only,
  and any other `runs_per` value (`revision`, a number) is refused `InvalidBudget` naming
  `budget.runs_per`. `runs_per` enters the decoder's `BUDGET` allow-list (`decode.rs:42`).
- **A used-up writer stops the run.** When the selector picks a writer the effect refuses
  `RunsSpent`, `tree run` stops that case with the new stop `TreeStop::RunsSpent`: the case, the
  action, the runs counted, the scope (`case` or `arrival`) and its last failure, the latest record
  about the writer's output whose result its `retry_when` lists (kind, result, producer, subject
  revision), absent when there is none. Exit 4, the status of a case that cannot go on (accepted by
  session `sdc` on 2026-11-21); it is never a wait on a person (exit 3) and prints no `tree
  approve` command.

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
interview ... over Timo's answers". SDC removes the flat cap from its check bindings when it
converts them to `loom.bindings/1` (`e4-sdc-answers-2.md`), so the refusal breaks nothing of SDC's.

Loom counts the same way today: `story:bound-program-result-table` § Acceptance, "An action that
has already run its `budget.max_runs` times in this case is not performed and is reported
`RunsSpent`", for every program action. The effect class is the Canon action's `effect`
(`ActionFacts.effect`, `crates/loom-governor/src/lib.rs:1184-1196`); the selector orders
non-`write` before `write` by it (the sort key at `crates/loom-executor/src/selection.rs:331`, the
least `(writes, id)` at `:336-338`), and `story:bound-artifact-revisions` § Outcome says "An
action whose effect class is not `write` that changed an artifact records no evidence for that
step". The selector's rule is per case revision (`selection.rs:285-305`: `due`, then
`performed_at(action, case_revision)`, then useful when `may_produce` is empty or a kind of it is
open). A writer is sent back by `retry_when` (`story:tree-run-command` § Acceptance, "a writer
whose check failed with a `retry_when` result is taken again"), which the loader checks against
the protocol's kinds (`crates/loom-bindings/src/load.rs:118-123`). `ActionBudget.max_runs` is a
required integer today (`ess/domains/bindings.yaml:115-121`), and three texts say so:
`AGENTS.md` ("an action with no `budget.max_runs` holds 8"), `website/docs/guides/bindings.md`
("`budget` holds `max_runs` (8 when absent)") and `docs/contracts/loom-action-selection.md`
("`max_runs` stays a ceiling in the effect").

## Waits on

- `story:standing-tree-rests-quiescent`: the story before this one on `ess/domains/tree.yaml`
  (`TreeStop`), `crates/loom-tree`, `crates/loom-cli/src/lib.rs` and
  `crates/loom-cli/tests/fixtures/area/`, which this story extends.
- `story:tree-run-command`: `crates/loom-tree`, the run that writes the log lines and stops, and
  `StepHistory`.
- `story:bound-program-result-table` (`RunsSpent`, the program count, a test that drives a
  writer), `story:bound-agent-node` (the agent count, `retry_records`),
  `story:bound-artifact-revisions` (the revision computation before each evaluation).

Comes before, on shared files: `story:tree-usage-and-model-budget` (`ess/domains/runlog.yaml`,
`ess/domains/tree.yaml`, `crates/loom-tree`, the agent port), and through it every later story of
the epic; `story:child-case-own-workspace` also shares `ess/domains/bindings.yaml`,
`crates/loom-bindings/src/decode.rs` and `load.rs` with this story, and
`story:capacity-given-to-an-admission-program` shares those and the program port; both are ordered
after this story through the chain.

Unordered with this story on shared files, under the store's rule that the second to land rebases
(the epic's § Order lists them): `story:tree-waits-open-decision-requests` and
`story:non-independent-review-becomes-decision-request` (`ess/domains/tree.yaml`,
`crates/loom-tree`), `story:authority-records-bound-and-grounded-in-canon`
(`ess/domains/tree.yaml`), `story:bindings-child-path-refusal-names-the-bindings-directory`,
`story:autonomy-grant-table` and `story:agent-session-through-substrate`
(`ess/domains/bindings.yaml`, `load.rs`). No edge is added for them.

## ESS first

`ess/domains/bindings.yaml`: `RunsScope` (`Case`, `Arrival`); `ActionBudget.max_runs` becomes
`Optional<Integer>` and `ActionBudget` gains optional `runs_per`; `BindingsRefusalReason` gains
`RunsCapOnObservation` (action, member). `ess/domains/runlog.yaml`: the event `arrived` (moved
artifacts, each with old and new revision). `ess/domains/tree.yaml`: `TreeStop` gains `RunsSpent`
(case, action, runs, scope, optional last failure). On that commit `task drift` fails and
`crates/loom-cli/tests/tree_runs_per_arrival.rs` does not compile.

## Acceptance

Files added to `crates/loom-cli/tests/fixtures/area/`:

- `draft.protocol.yaml`, `draft/1`: placed artifacts `brief` and `draft`; evidence kinds
  `draft_check` (about `draft`, results `valid`, `invalid`) and `draft_written` (about `draft`);
  the observation `draft.check` (may produce `draft_check`) and the writer `draft.write` (effect
  `write`, may produce `draft_written`); one outcome, `accepted`, legitimate when a `valid`
  `draft_check` about the current `draft` is held.
- `draft.bindings.yaml`: `draft.check` bound to `bin/draft-check` (exit 0, `valid`, when `draft`
  has at least the number of lines written in `brief`, else exit 1, `invalid`); `draft.write` bound
  to `bin/draft-write` (appends one line to `draft`, exit 0, `draft_written` `written` about the
  moved `draft`) with `retry_when: {draft_check: [invalid]}`, `budget: {max_runs: 2, runs_per:
  arrival}`.
- `draft-case.bindings.yaml`: the same with `runs_per: case`; `draft-12.bindings.yaml`: the same
  with `max_runs: 12`; `draft-agent.bindings.yaml`: `draft.write` bound to the agent adapter `stub`
  with the same budget.

The order the selector gives (`selection.rs:285-305`, `:331`, `:336-338`): at the start both
actions are useful (each has an open kind); `draft.check` is non-`write`, so it goes first. After
an `invalid` check the check is not useful (performed at that case revision, its kind counted) and
the writer is: on the absent `draft` because `draft_written` is open (`retry_records` gives nothing
while the revision is `none`, `story:bound-agent-node` § Outcome), later because it is due (its
`retry_when` matches the `invalid` record about the current `draft`). The write moves `draft`,
which opens `draft_check` again and counts `draft_written`, so the check goes next. So a run
alternates check, write, check, ... and ends when the check is `valid` (the outcome is legitimate
and nothing is useful: terminated `accepted`, exit 0) or when the writer is spent.

`crates/loom-cli/tests/tree_runs_per_arrival.rs` drives `b10x-loom tree run`, no model:

- `draft.bindings.yaml`, `brief` holding `5`: the log holds check (`invalid`), write, check
  (`invalid`), write, check (`invalid`), and the run stops `RunsSpent`, exit 4, naming the case,
  `draft.write`, 2 runs, scope `arrival` and last failure `draft_check` `invalid` by the check's
  producer; the output holds no `tree approve` command, and no `arrived` line was written.
- The test then rewrites `brief` to `3`: the next `tree run` writes one `arrived` line naming
  `brief` with both revisions before its first `evaluated` line, takes write (the third line) and
  check (`valid`), terminates the case through `accepted` and exits 0.
- `draft-case.bindings.yaml`, the same two runs: after the rewrite the next `tree run` writes the
  `arrived` line, takes no step, and stops `RunsSpent` again with scope `case`, exit 4.
- `draft-12.bindings.yaml`, `brief` holding `10`: check, then ten times write and check; the check
  runs 11 times, is never refused, and has exactly one `action finished` line per case revision;
  the case terminates `accepted`, exit 0.
- The count after the store is reopened equals the count before it: the first bullet's store,
  reopened, stops `RunsSpent` at once with 2 runs.

`crates/loom-tree/tests/runs_per_agent.rs` drives the tree library with `draft-agent.bindings.yaml`
and a stub agent adapter registered by the test that appends one line to `draft` (no model):
`brief` holding `5` gives the same sequence and returns the same `TreeStop::RunsSpent` as the
first bullet.

`crates/loom-bindings/tests/runs_per.rs` loads bindings against `draft/1`:

- `budget: {max_runs: 3}` on `draft.check` is refused `RunsCapOnObservation` naming the action and
  `budget.max_runs`; `budget: {runs_per: arrival}` on it, naming `budget.runs_per`;
  `draft.bindings.yaml` loads, and a writer with no `budget` decodes with `max_runs` absent.
- `runs_per: revision` and `runs_per: 2` on `draft.write` are refused `InvalidBudget` naming
  `budget.runs_per`; `max_runs: 0` is still refused `InvalidBudget` naming `budget.max_runs`.
- An action of a fixture protocol whose effect class is not `write` and whose `may_produce` is
  empty loads with `budget: {max_runs: 2, runs_per: arrival}`: it is a writer for run caps.
- `crates/loom-bindings/tests/adversary_w20261121w1_u2.rs`
  (`defaults_apply_to_the_model_and_max_runs_only`) asserts `max_runs` absent where it asserted 8,
  and `crates/loom-bindings/tests/bindings_document.rs` asserts the stated values as `Some`.
- `CHANGELOG.md` has a `**Breaking:**` bullet: a `loom.bindings/1` document that states
  `budget.max_runs` on an observation is refused, an observation is never refused `RunsSpent`, and
  `ActionBudget.max_runs` is optional.
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/runlog.yaml`, `ess/domains/tree.yaml`, generated
`generated/rust/loom/src/{bindings,runlog,tree}.rs`; `crates/loom-bindings/src/decode.rs`
(`BUDGET` gains `runs_per`; the whole-number rule holds for the numeric members only),
`crates/loom-bindings/src/load.rs` (`RunsCapOnObservation`), `crates/loom-bindings/src/lib.rs`
(`DEFAULT_MAX_RUNS` applied by the port), the program and agent ports' run count (in
`crates/loom-bindings/src`; neither `story:bound-program-result-table` nor `story:bound-agent-node`
names the module); `crates/loom-bindings/tests/bindings_document.rs` and
`crates/loom-bindings/tests/adversary_w20261121w1_u2.rs` (their `max_runs` asserts follow the
optional member); `crates/loom-tree` (the `arrived` line, the `RunsSpent` stop);
`crates/loom-cli/src/lib.rs` (the stop's output); `crates/loom-cli/tests/tree_runs_per_arrival.rs`,
`crates/loom-tree/tests/runs_per_agent.rs`, `crates/loom-bindings/tests/runs_per.rs` (new);
`crates/loom-cli/tests/fixtures/area/` (adds `draft.protocol.yaml`, `draft.bindings.yaml`,
`draft-case.bindings.yaml`, `draft-12.bindings.yaml`, `draft-agent.bindings.yaml`,
`bin/draft-write`, `bin/draft-check`); `AGENTS.md` (the sentence "an action with no
`budget.max_runs` holds 8" becomes "a writer with no `budget.max_runs` is capped at 8; an
observation states none"); `website/docs/guides/bindings.md` (the `budget` member: `runs_per`,
writers only, the default for writers); `docs/contracts/loom-action-selection.md` (the sentence
"`max_runs` stays a ceiling in the effect" becomes "`max_runs` stays a ceiling in the effect for
writers; an observation has none"); `CHANGELOG.md`.

## Out of scope

Counting per revision of one subject artifact (Canon names no subject per action). Not counting a
program that could not start (exit 126 or 127, trial 2 R3). Tree budgets at a wake
(`story:standing-tree-wakes-at-a-host-instant`). Telling one case's write from a person's in a
shared workspace: both are arrivals for the other case.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree` (the `arrived` line, the count, the `RunsSpent` stop) together with `crates/loom-bindings` (decode, load, `ActionBudget`). Neither dominates by itself. Today only the bindings half exists in the tree. — cited (story Files and Outcome)
- **Files, existing in the tree:** `ess/domains/bindings.yaml:115-121` (`loom.bindings.ActionBudget`, `max_runs: Integer`); `crates/loom-bindings/src/decode.rs:43` (`BUDGET` allow-list) and `:667-695` (`budget()`); `crates/loom-bindings/src/load.rs:118-123` (the `retry_when` check, where the observation refusal fits); `crates/loom-bindings/src/lib.rs:30` (`DEFAULT_MAX_RUNS`); `crates/loom-bindings/tests/bindings_document.rs:105,173-179,701,781`; `crates/loom-cli/src/lib.rs` — cited
- **Files, created by earlier stories:** `ess/domains/runlog.yaml`, `ess/domains/tree.yaml`, `crates/loom-tree`, `crates/loom-cli/tests/fixtures/area/`. None exists at `61fd500b`. Edge-wise they come from `story:tree-run-command` and `story:standing-tree-rests-quiescent`, which this story depends on. — cited (story Waits on)
- **Files, new:** `crates/loom-cli/tests/tree_runs_per_arrival.rs`, `crates/loom-tree/tests/runs_per_agent.rs`, `crates/loom-bindings/tests/runs_per.rs`, and `draft.protocol.yaml`, `draft.bindings.yaml`, `draft-case.bindings.yaml`, `draft-12.bindings.yaml`, `draft-agent.bindings.yaml`, `bin/draft-write`, `bin/draft-check` under the `area/` fixtures. — cited
- **Symbols:** `ActionBudget.max_runs` (to `Optional`), `ActionBudget.runs_per` (new), `RunsScope`, the refusal reason `RunsCapOnObservation`, the run-log event `arrived`, `TreeStop::RunsSpent`, `DEFAULT_MAX_RUNS`, `ActionFacts.effect` (`crates/loom-governor/src/lib.rs:1183-1196`), `WRITE` (`crates/loom-executor/src/selection.rs:246`) — cited
- **Read, not changed:** `crates/loom-executor/src/selection.rs:285-306` (`useful`) and `:331` (`known.effect.as_deref() == Some(WRITE)`); `crates/loom-governor/src/lib.rs:1183-1196`. — cited
- **Also likely:** the program and agent ports' run-count modules. The story says "not read", and both source stories leave them unnamed, so I cannot give paths. — inferred
- **Also likely:** `crates/loom-bindings/tests/adversary_w20261121w1_u2.rs:370` asserts `budget.max_runs == 8`. It stops compiling once `max_runs` is `Option`. — inferred (grep of the tree)
- **Documents:** `docs/contracts/loom-action-selection.md:168` says "`max_runs` stays a ceiling in the effect", which is no longer true for observations. `website/docs/guides/bindings.md:164` says "`max_runs` (8 when absent)" and has no `runs_per`. Neither is in the story's Files. — inferred
- **Confidence:** medium. The bindings half is high: the lines match the tree and the loader already has the protocol's declared action at `load.rs:92-100`. The `loom-tree`, runlog, tree and fixture half is unread because those files do not exist yet. The port files are unnamed.
- **Would collide with** (`depends_on` order from `aep plan artifact graph --format json`):
  - `story:standing-tree-rests-quiescent` on `tree.yaml`, `crates/loom-tree`, `crates/loom-cli/src/lib.rs` and the `area/` fixtures: ordered, this story depends on it.
  - `story:tree-run-command` on `crates/loom-tree` and `crates/loom-cli/src/lib.rs`: ordered, a direct dependency.
  - `story:bound-program-result-table` on `bindings.yaml`, the program port and `RunsSpent`: ordered, a dependency.
  - `story:bound-agent-node` on `bindings.yaml` and the agent port: ordered, a dependency.
  - `story:bound-artifact-revisions` on `bindings.yaml` and the revision computation: ordered, a dependency.
  - `story:agent-node-run-records` on `runlog.yaml`, `tree.yaml` and `crates/loom-tree`: ordered, it is an ancestor.
  - `story:child-case-runtime`, `story:record-copy-between-cases`, `story:named-egress-per-action`, `story:confinement-level-required`, `story:engine-configuration-document`, `story:action-limits-enforced`, `story:limit-hits-in-run-log`, `story:tree-daemon`, `story:hash-chained-run-log` on `bindings.yaml`, `load.rs`, `lib.rs` or the tree files: ordered, all ancestors.
  - `story:tree-usage-and-model-budget` on `runlog.yaml`, `tree.yaml`, `crates/loom-tree`: ordered, depends on this story.
  - `story:standing-tree-wakes-at-a-host-instant` (`runlog.yaml`, `tree.yaml`, `crates/loom-tree`, `area/`): ordered, depends on this story.
  - `story:flow-limits-recorded-as-events`: ordered, depends on this story.
  - `story:child-case-own-workspace` (`bindings.yaml`, `decode.rs`, `load.rs`, `tree.yaml`, `area/`): ordered, after this story through `standing-tree-wakes-at-a-host-instant`.
  - `story:capacity-given-to-an-admission-program` (same files plus the program port): ordered, after this story through `standing-tree-wakes-at-a-host-instant`.
  - `story:concurrent-cases-in-the-tree-daemon` (`bindings.yaml`, `runlog.yaml`, `tree.yaml`, `crates/loom-tree`, `area/`): ordered, after this story through `child-case-own-workspace`.
  - `story:budget-and-limit-stops-raise-decision-requests`: ordered, after this story through `tree-usage-and-model-budget`.
  - `story:run-log-references-exec-file-changes` (`runlog.yaml`, `crates/loom-tree`): ordered, after this story.
  - **No `depends_on` path in either direction:**
    - `story:tree-waits-open-decision-requests` (`tree.yaml`, `runlog.yaml` via its neighbour, `crates/loom-tree`, `crates/loom-cli/src/lib.rs`). Its own body names the shared `tree.yaml` and `crates/loom-cli/src/lib.rs`.
    - `story:non-independent-review-becomes-decision-request` (`runlog.yaml`, `tree.yaml`, `crates/loom-tree`).
    - `story:authority-records-bound-and-grounded-in-canon` (`tree.yaml`, `CaseApproval`).
    - `story:bindings-child-path-refusal-names-the-bindings-directory` (`bindings.yaml`, `load.rs`).
    - `story:autonomy-grant-table` and `story:agent-session-through-substrate` (`bindings.yaml`, which they extend).
- **Contradicted by the code:** see Story defects below. — cited

- **Safety fact:** the refusal can be placed in the loader because `load.rs:92-100` already finds the protocol's declared action, and the governor reads `declared.effect` from the same value (`crates/loom-governor/src/lib.rs:1185-1188`). Making `max_runs` optional breaks every reader of the generated `ActionBudget.max_runs: i64`. Today those are `bindings_document.rs` and `adversary_w20261121w1_u2.rs`; the port consumers do not exist yet. — step 2 (pointed at file:line), unproven
