---
format: aep.planning-md/3
id: story:standing-tree-wakes-at-a-host-instant
kind: story
status: proposed
title: tree wake --at wakes a quiescent standing tree on an arrival or an expired record, and the wake starts a new run for budget purposes
summary: Wake command, woken run-log line beside the arrived lines, idempotent at one instant, InstantBeforeLast; per-run counters from the wake with the totals since tree open beside them
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:standing-tree-rests-quiescent
- depends_on: story:run-caps-per-arrival
- depends_on: story:tree-usage-and-model-budget
scope:
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area
- confidence: cited
  path: crates/loom-cli/tests/tree_wake.rs
- confidence: inferred
  path: crates/loom-conformance/src/tree.rs
- confidence: inferred
  path: crates/loom-runlog
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: crates/loom-tree/tests/wake_budgets.rs
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: inferred
  path: ess/scenarios/tree-run
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:29:08Z", actor: "agent:sdc", revision: 12, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`b10x-loom tree wake <store> --at INSTANT` wakes a quiescent standing tree served by the daemon
(`Wake`). `--at` is required, and the daemon reads no clock to decide a wake. Without a daemon the
command exits 1 `DaemonUnreachable`; a direct host runs `tree run --at` instead. The daemon's
`Wake` calls one function of `crates/loom-tree`, `wake(store, at)`.

On a wake, every case that is not terminated goes through the arrival detection of
`story:run-caps-per-arrival`: a placed artifact whose revision moved with no step of the case
moving it gives the case an `arrived` line. Each case is then evaluated at `INSTANT`.

- When a case has a useful action or waits on a person, authority or a decision request, the root's
  run log gets one `woken` line (the instant, `expired` when a held record stopped applying at or
  before it, the cases that had an arrival, `new_run`, whether the wake starts a new run for budget
  purposes, and the tree's usage since `tree open` at the wake), and the tree continues as `tree
  run --at INSTANT` would: it queues for a run slot as `story:tree-daemon` says, its steps take
  `INSTANT` as their instant, and it rests or stops as `story:standing-tree-rests-quiescent` says.
  Under this story every wake that acts is a wake of a `Quiescent` tree, so its `woken` line holds
  `new_run: true`.
- When no case has, the tree stays `Quiescent` and no `woken` line is written. The `arrived` lines
  and moved revisions of that wake stay recorded; a second wake at the same instant then finds no
  move and changes no byte of the store.
- A wake of a tree that is not `Quiescent`, or not standing, is answered with its state and changes
  nothing. A wake whose instant is before the latest instant in the tree's run logs is refused
  `InstantBeforeLast`, naming both, exit 1.

The condition "`Quiescent`" is the tree's one state, as `story:tree-daemon` and
`story:standing-tree-rests-quiescent` keep it when this story lands.
`story:concurrent-cases-in-the-tree-daemon`, which lands later on this chain, gives each case its
own state and changes the condition to: a wake acts on the tree's `Resting` and `HoldsChildren`
cases, and a tree with a `Waiting` case is not refused a wake. That story states and checks the new
condition; this story's checks use trees whose one case is the root.

Why two lines: `arrived` is per case and happens wherever a case is evaluated (a direct run, a busy
tree, a wake); it is where a writer's run count restarts. `woken` is per tree, only at a wake; it
records the instant the host passed and covers a wake by expiry in which nothing arrived, and a
`woken` line holding `new_run: true` is where the tree's budgets restart. The moved artifacts are
in the `arrived` lines only; `woken` names the cases.

**Budgets at a wake** (session `sdc`'s rule, 2026-11-21, `.engineering/drafts/e3-sdc-answers-5.md`:
"a wake starts a new run for budget purposes with the tree's totals since `tree open` recorded
beside the per-run counters"). Only a wake of a tree that was `Quiescent` starts a new run of a
standing tree for budget purposes (session `sdc`, `.engineering/drafts/e4-sdc-answers-3.md`:
"only a wake of a Quiescent tree starts a new budget run"); under this story no other wake acts,
and `story:concurrent-cases-in-the-tree-daemon` keeps the rule for the wakes it adds. At such a
wake the ceilings and budgets do not change and no binding raises one; the counters of both
windows of `story:tree-usage-and-model-budget` start at the latest `woken` line that starts a run
(`new_run: true`): steps, the sum of step wall times, CPU time and the elapsed wall time, and agent
calls, model calls, tokens and cost. A daemon restarted after the wake rebuilds them from the run
log from that same line. `TreeUsage` gains `since_open`, the same figures counted from `tree
open`, which `tree status`, the `woken` line and the root's `stopped` lines carry beside the
per-run figures. A bounded served tree counts as `story:tree-usage-and-model-budget` says: steps
per `tree run` invocation, wall and CPU from `tree open` (`story:tree-daemon` § Outcome: "for a
served tree, `tree open`, so its totals survive a restart"), agent calls and cost over the whole
tree. An extension a decision request granted (`story:budget-and-limit-stops-raise-decision-requests`,
`budget extended`) does not carry over a wake: the new run starts at the tree's budgets (accepted
by session `sdc` on 2026-11-21, `.engineering/drafts/e4-sdc-answers-2.md`, point 4).

**The kept budget.** `story:tree-daemon` keeps no budget for a submitted tree, so this story adds
it: the tree's store keeps the `TreeBudget` of its latest `tree run` beside the root's case file
(`TreeRun.budget`). Each `tree run` of the tree replaces it with exactly the budget flags that run
gives; a member the run does not give is absent, so a `tree run` with no budget flag leaves the
tree with no budget. A wake runs under the kept budget. `--max-steps` is therefore stated again by
every `tree run` that wants it, and counts that run's steps or, for a standing tree, the steps
since the latest `woken` line that starts a run.

`tree wake` answers with a `WakeReport`: the tree's state after the wake, the cases with an
arrival, and the next expiry. The host decides when to call it: after writing a placed file (for
the SDC, an `intake` entry), and at the next expiry `tree status` prints (accepted by session
`sdc` on 2026-11-21: no file watch and no timer in the daemon).

## Why

"It wakes on an artifact revision or an expiry" and "a wake-up source (file watch on `intake`,
timer at the earliest `max_age` expiry) (*inferred*)" (`~/beyond10x/sdc/docs/orchestration.md`
§ 3); "Canon's evaluator takes an explicit instant, so expiry is deterministic and replayable"
(§ 3, rules); decision 2: "A standing case notices change through two channels: the `intake` queue
(every arrival a new revision) and a `health` claim with `max_age`" (§ 7). The request narrows the
source to "an expiry instant passed in by the host" (`.engineering/drafts/e4-request.md`, item 3),
the pattern of `story:silence-and-expiry-at-a-host-tick` § Outcome: "nothing in it reads a clock".

The budget rule is the requester's, and session `sdc` confirmed on 2026-11-21 that only a wake of
a `Quiescent` tree starts a new budget run (`.engineering/drafts/e4-sdc-answers-3.md`). Today a
served tree's totals count from `tree open` (`story:tree-daemon` § Outcome, quoted above) and
`--max-agent-calls` "counts those lines over the logs of the whole tree, across runs"
(`story:agent-node-run-records` § Outcome); a tree that lives for months would spend them, the
driver's own failure for `max_runs` ("a case that lives for months would exhaust it",
orchestration § 1.2). The totals since `tree open` stay recorded so an admission rule can bound a
standing tree's whole spend (`story:capacity-given-to-an-admission-program`). `--max-steps` is
mapped per invocation (`story:tree-run-command` § Acceptance: "`--max-steps` and
`--max-wall-seconds` each stop the run with exit 5 before the step that ..."), which this story
keeps for a bounded tree.

## Waits on

- `story:standing-tree-rests-quiescent`: `Quiescent`, the horizon, `next_expiry`, the `area/1`
  fixture.
- `story:run-caps-per-arrival`: the arrival detection and the `arrived` line this story uses.
- `story:tree-usage-and-model-budget`: the story before this one on `ess/domains/tree.yaml`
  (`TreeUsage`, `TreeBudget`), `ess/domains/runlog.yaml` and the budget counters in
  `crates/loom-tree`.
- Through them, `story:tree-daemon` (the daemon's commands, `DaemonUnreachable`).

Comes before, on shared files: `story:child-case-own-workspace` (`ess/domains/tree.yaml`,
`crates/loom-tree`), and through it every later story of the epic, among them
`story:concurrent-cases-in-the-tree-daemon`, which changes the wake condition (§ Outcome).

Unordered with this story on shared files, under the store's rule that the second to land rebases
(the epic's § Order lists them): `story:budget-and-limit-stops-raise-decision-requests`
(`TreeBudget`, `ess/domains/runlog.yaml`, `crates/loom-tree`; it lands after 3, no path to this
story), `story:tree-waits-open-decision-requests`,
`story:non-independent-review-becomes-decision-request`,
`story:run-log-references-exec-file-changes` and the other stories the epic names. No edge is
added for them.

## ESS first

`ess/domains/tree.yaml`: the command `Wake` (store, instant), `WakeReport` (state, cases with an
arrival, optional next expiry), the refusal `InstantBeforeLast` (asked, latest), `TreeUsage`
gains `since_open`, and `TreeRun` gains `budget` (the kept `TreeBudget`).
`ess/domains/runlog.yaml`: the event `woken` (instant, `expired`, cases, `new_run`, `since_open`).
On that commit `task drift` fails, the new scenario for `Wake` is red, and
`crates/loom-cli/tests/tree_wake.rs` does not compile.

## Acceptance

Files added to `crates/loom-cli/tests/fixtures/area/` (beside `area/1` of
`story:standing-tree-rests-quiescent`):

- `pulse.protocol.yaml`, `pulse/1`: one placed artifact `inbox`; two observations, `inbox.read`
  (may produce `inbox_read`) and `inbox.sort` (may produce `inbox_sorted`), both kinds about
  `inbox`; no obligation, no outcome. Both are non-`write`, so the selector takes `inbox.read`
  then `inbox.sort` (the sort key at `crates/loom-executor/src/selection.rs:331`, the least
  `(writes, id)` at `:336-338`); after both ran at a case revision neither is useful until `inbox`
  moves (`selection.rs:285-305`), and the tree rests `Quiescent`. Each wake after an append to
  `inbox` therefore takes exactly two steps.
- `pulse.bindings.yaml`: both bound to `bin/sleep1` (sleeps 1 s, exit 0, `results: {"0": valid}`).
  `pulse-spin.bindings.yaml`: both bound to `bin/spin` (a busy loop until its own CPU clock reads
  800 ms, then exit 0; no sleep). `pulse-gated.bindings.yaml`: `inbox.read` bound to `bin/gate`
  (writes its start instant, then waits until the test writes a release file named by its case,
  exit 0), `inbox.sort` to `bin/sleep1`. `bin/gate` is created here;
  `story:concurrent-cases-in-the-tree-daemon` uses it.
- `engine-daemon-wall.yaml`: adapter `daemon`, one run slot, a per-run wall ceiling of 3 s and no
  CPU ceiling. `engine-daemon-cpu.yaml`: adapter `daemon`, one run slot, a per-run CPU ceiling of
  2000 ms and no wall ceiling. Each ceiling has its own file, so neither check can stop on the
  other ceiling.
- `agentpulse.protocol.yaml`, `agentpulse/1`: placed `inbox`; artifacts `ack` and `reply`; two
  writers, `ack.write` (writes `ack`, may produce `inbox_acked` about `inbox`) and `reply.write`
  (writes `reply`, may produce `inbox_answered` about `inbox`); no obligation, no outcome.
  `agentpulse.bindings.yaml` binds both to the agent adapter `stub` with an evidence entry
  `from: effect`. Both are writers, so the selector takes `ack.write` then `reply.write`
  (`selection.rs:331`, `:336-338`); each wake after an append to `inbox` takes those two agent
  steps.

`crates/loom-cli/tests/tree_wake.rs` drives `b10x-loom tree` with a managed Substrate daemon under
`engine-daemon-1.yaml` unless a bullet names another engine file; stub programs, no model. Every
instant is in 2099, so only `--at` can produce an instant a line or a report carries. `area/1`
with `area.bindings.yaml` was opened `--standing` and rested at `T = 2099-01-01T00:00:00Z` after
`health.observe`, `intake.triage` and `retire.check` (3 steps), next expiry
`E = 2099-01-01T01:00:01Z` (`T + 3601 s`):

- `tree wake --at T+60s` with no file changed answers `Quiescent` with no case with an arrival and
  next expiry `E`, writes no line, and the store's bytes are unchanged; a second identical wake
  also.
- After the test appends a line to the `intake` file, `tree wake --at T+60s` writes one `arrived`
  line naming `intake` in the root's log and one `woken` line naming the root, with instant
  `2099-01-01T00:01:00Z`, `new_run: true` and `since_open.steps` 3; `intake.triage` runs once and
  its `action started` line carries `2099-01-01T00:01:00Z`; nothing else runs; the report answers
  `Quiescent`, names the root as the case with an arrival and gives next expiry `E`; `tree status`
  then shows `usage.steps` 1 and `since_open.steps` 4.
- After the test changes `notes`, which no action of `area/1` reads, `tree wake --at T+70s` writes
  one `arrived` line, no `woken` line, and answers `Quiescent` naming the root as the case with an
  arrival; a second `tree wake --at T+70s` changes no byte of the store.
- `tree wake --at E` writes one `woken` line with `expired: true` and no `arrived` line,
  `health.observe` runs once, and the report answers `Quiescent` with next expiry
  `2099-01-01T02:00:02Z` (`E + 3601 s`).
- `tree wake --at 2098-12-31T23:59:59Z` exits 1 `InstantBeforeLast` naming that instant and the
  latest instant.
- `tree wake` without `--at` exits 2 before it connects; with no daemon it exits 1
  `DaemonUnreachable`.
- `tree wake` of a bounded tree of `area/1` that stopped `Stuck` answers `Stuck` and writes
  nothing. A standing tree of `pulse/1` with `pulse-gated.bindings.yaml`, submitted with `tree run
  --detach`: while `bin/gate` is held, `tree wake` answers `Running` and writes nothing; after the
  release the tree rests `Quiescent`.
- Wall, under `engine-daemon-wall.yaml` with `pulse/1` and `pulse.bindings.yaml`: a standing tree
  rests after its two steps (about 2 s); three times the test appends to `inbox` and calls `tree
  wake` at a later instant; each wake takes two steps and none stops `BudgetSpent`; `tree status`
  then shows `usage.wall_millis_sum` below 3000 and `since_open.wall_millis_sum` at least 6000. A
  bounded tree of the same files: its first `tree run` takes two steps and stops `Stuck`; after an
  append its second `tree run` takes `inbox.read` (the sum since `tree open` now at least 3000) and
  stops `BudgetSpent` naming `wall_seconds` before `inbox.sort`, exit 5.
- CPU, under `engine-daemon-cpu.yaml` with `pulse-spin.bindings.yaml`, the same sequence: each wake
  uses about 1600 ms of CPU and none stops; `since_open.cpu_millis` is at least 6400 after three
  wakes; the bounded tree's second `tree run` stops `BudgetSpent` naming `cpu_millis` before
  `inbox.sort`, exit 5.
- Steps, standing, `pulse.bindings.yaml`: a tree whose `tree run` gave `--max-steps 2` takes two
  steps at each of three wakes and never stops `BudgetSpent`; `since_open.steps` is 8. Then a
  `tree run --max-steps 1` of the quiescent tree takes no step and exits 7; after the next append,
  `tree wake` takes `inbox.read` and the tree stops `BudgetSpent` naming `max_steps` before
  `inbox.sort`: the wake ran under the kept budget, replaced by the latest `tree run`.
- Steps, bounded, `pulse.bindings.yaml`: `tree run --max-steps 1` takes `inbox.read` and stops
  `BudgetSpent` naming `max_steps` before `inbox.sort`, exit 5; a second `tree run --max-steps 1`
  takes `inbox.sort` and stops `Stuck`, exit 4, as steps count per invocation.

`crates/loom-tree/tests/wake_budgets.rs` drives `wake` and the run of `crates/loom-tree` directly
and reads the returned `TreeStop` (the exit status in brackets), with a stub agent adapter
registered by the test that writes its artifact and returns a started session stating 1500
micro-USD (agent steps are not served by the daemon until `story:agent-session-through-substrate`),
on `agentpulse/1` opened `--standing`; each wake follows an append to `inbox`:

- Agent calls, counters: under `--max-agent-calls 2`, the first run and three wakes each take
  `ack.write` and `reply.write`, none stops, and `since_open.agent_calls` is 8.
- Agent calls, the kept budget: the first run under `--max-agent-calls 2` takes both and returns
  `Quiescent` (exit 7); a run of the quiescent tree under `--max-agent-calls 1` takes no step and
  returns `Quiescent`; the next wake takes `ack.write` and returns `BudgetSpent` with `limit`
  `max_agent_calls` before `reply.write` (exit 5). Repeated with a run that gives no budget flag
  in place of the second run, the wake takes both and returns `Quiescent`.
- Cost, counters: under `--max-model-cost-micro-usd 3000`, the first run and three wakes each take
  both sessions and none stops; `since_open.cost_micro_usd` is 12000.
- Cost, the kept budget: the first run under `--max-model-cost-micro-usd 3000` takes both and
  returns `Quiescent`; a run under `1500` takes no step; the next wake takes `ack.write` and
  returns `BudgetSpent` with `limit` `max_model_cost_micro_usd` before `reply.write` (exit 5).

- `website/docs/reference/cli.md` is regenerated.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, generated
`generated/rust/loom/src/{tree,runlog}.rs`, `ess/scenarios/` (tree domain); `crates/loom-tree`
(`wake`, the counters' starting line, `since_open`, the kept budget); `crates/loom-cli/src/lib.rs`
(`tree wake`); `crates/loom-cli/tests/tree_wake.rs`, `crates/loom-tree/tests/wake_budgets.rs`
(new); `crates/loom-cli/tests/fixtures/area/` (adds `pulse.protocol.yaml`, `pulse.bindings.yaml`,
`pulse-spin.bindings.yaml`, `pulse-gated.bindings.yaml`, `agentpulse.protocol.yaml`,
`agentpulse.bindings.yaml`, `engine-daemon-wall.yaml`, `engine-daemon-cpu.yaml`, `bin/sleep1`,
`bin/spin`, `bin/gate`); `website/docs/reference/cli.md` (regenerated); `CHANGELOG.md`.

## Out of scope

A file watch or a timer in the daemon. Calling `decisions tick` at a wake. Writers' run counts
(`story:run-caps-per-arrival`). Waking a tree served by no daemon. Per-case states of a woken tree
and the wake condition over them (`story:concurrent-cases-in-the-tree-daemon`, which lands later
and changes the condition). A check that a `budget extended` line does not carry over a wake: the
line exists only once `story:budget-and-limit-stops-raise-decision-requests` lands, and no path
orders the two stories (the work order's § Open).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree` (`wake(store, at)`, the budget counters' starting line, `since_open`, the kept `TreeBudget`) with the ESS domains `ess/domains/tree.yaml` and `ess/domains/runlog.yaml` — cited, story § Files and § ESS first; the crate does not exist at `61fd500b` (created by `story:tree-run-command`) — cited, `ls crates`
- **Files:** `crates/loom-cli/src/lib.rs` (`tree wake`) — cited, story § Files; file exists
- **Files:** `crates/loom-cli/tests/tree_wake.rs`, `crates/loom-tree/tests/wake_budgets.rs` (new) — cited, story § Files
- **Files:** `crates/loom-cli/tests/fixtures/area/` adds `pulse.protocol.yaml`, `pulse.bindings.yaml`, `pulse-spin.bindings.yaml`, `pulse-gated.bindings.yaml`, `agentpulse.protocol.yaml`, `agentpulse.bindings.yaml`, `engine-daemon-wall.yaml`, `engine-daemon-cpu.yaml`, `bin/sleep1`, `bin/spin`, `bin/gate` — cited, story § Files; the directory is created by `story:standing-tree-rests-quiescent` — cited
- **Files:** `ess/scenarios/` tree domain, `ess/scenarios/tree-run/` — cited as `ess/scenarios/` in story § Files; the sub-directory name is inferred from `story:tree-run-command` scope
- **Symbols:** `Wake`, `WakeReport`, `InstantBeforeLast`, `TreeUsage.since_open`, `TreeRun` (keeps last `TreeBudget`), the event `woken` — cited, story § ESS first
- **Symbols:** `DeterministicSelector::select`, `useful` read for the fixtures' step order (`selection.rs:285-305`, ordering comment at `:319`, `.min()` at `:338`) — cited, read in tree; read only, not changed
- **Also likely:** `crates/loom-conformance/src/tree.rs` — inferred, `tree.yaml` gains the command `Wake`, and `story:tree-run-command` scope says `crates/loom-conformance/tests/conform.rs:8` needs one scenario per declared command
- **Also likely:** `crates/loom-cli/src/tree.rs` — inferred, `story:tree-run-command` scope plans the tree output module (story does not name it)
- **Also likely:** `crates/loom-runlog` — inferred, the hash-chained log writer; a new `RunLogEvent` variant `woken` may need a match or codec arm (`story:hash-chained-run-log`)
- **Also likely:** `crates/loom-governor/src/lib.rs` — inferred, `next_expiry` is read for `WakeReport`; read-only use, no change expected
- **Documents:** none beyond the house-rule files (`cli.md` regenerated) — cited, story § Files
- **Confidence:** medium — the story names every file and symbol (high for those), but the code half does not exist at `61fd500b`, so `loom-tree` module paths, the daemon's `Wake` home and the kept `TreeBudget` cannot be read; the story says so itself
- **Would collide with `story:tree-waits-open-decision-requests`:** `crates/loom-tree`, `ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, `crates/loom-cli/src/lib.rs` — cited, that story's Files; no `depends_on` path either way (it is before `story:concurrent-cases-in-the-tree-daemon`, not before this story)
- **Would collide with `story:budget-and-limit-stops-raise-decision-requests`:** `crates/loom-tree`, `tree.yaml` (`TreeBudget`), `runlog.yaml` — cited; no path orders the two (the story's own § Out of scope says so); it depends on `story:tree-usage-and-model-budget`, which is before this story, and `story:tree-waits-open-decision-requests`
- **Would collide with `story:non-independent-review-becomes-decision-request`:** `crates/loom-tree`, `tree.yaml`, `runlog.yaml` — cited; no path
- **Would collide with `story:run-log-references-exec-file-changes`:** `runlog.yaml`, `crates/loom-tree` — cited; no path (its `limit-hits-in-run-log` ancestor is before this story, the story itself is not)
- **Would collide with `story:authority-records-bound-and-grounded-in-canon`:** `tree.yaml` (`CaseApproval`) — cited; no path
- **Would collide with `story:requests-from-canon-pending-decisions`, `story:ingress-routes-per-action`, `story:confinement-level-on-oci-and-pod`:** `crates/loom-tree` only (new test files, modules) — cited; no path; a light collision (new files)
- **Would collide with `loom-cli/src/lib.rs` editors with no path:** `story:decision-inbox-commands`, `story:silence-and-expiry-at-a-host-tick`, `story:approval-decisions-resume-held-runs`, `story:question-and-outcome-decisions-hold-runs`, `story:decision-suspension-reason` — cited, their Files; distinct regions of one file, no path
- **Would collide with, ordered by a `depends_on` path, after this story:** `story:child-case-own-workspace` (direct), `story:concurrent-cases-in-the-tree-daemon`, `story:capacity-given-to-an-admission-program`, `story:flow-limits-recorded-as-events` — cited; `pulse/1`, `bin/gate`, `bin/sleep1` and `area/` fixtures are shared with them; all after this story
- **Would collide with, ordered before this story:** `story:tree-daemon`, `story:tree-run-command`, `story:standing-tree-rests-quiescent`, `story:run-caps-per-arrival`, `story:tree-usage-and-model-budget`, `story:action-limits-enforced`, `story:agent-node-run-records`, `story:limit-hits-in-run-log`, `story:hash-chained-run-log` — cited; every one reaches this story's node through `depends_on`
- **Contradicted by the code:** nothing in the tree contradicts the story; `selection.rs:285-305` and `:319` read as the story says (`:319` is the ordering comment, the sort key is pushed at `:331`) — cited
- **Safety fact:** the wake changes the selector's input only through the case revision, and `useful` returns false once an action ran at that revision (`selection.rs:300-304`), with `open_evidence` reopened only when the subject moves (`crates/loom-governor/src/lib.rs:1189`), so one append gives exactly the story's steps — step 2 (`file:line`), unproven
