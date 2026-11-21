---
format: aep.planning-md/3
id: story:standing-tree-wakes-at-a-host-instant
kind: story
status: draft
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
revision: 4
---
## Outcome

`b10x-loom tree wake <store> --at INSTANT` wakes a quiescent standing tree served by the daemon
(`Wake`). `--at` is required, and the daemon reads no clock to decide a wake. Without a daemon the
command exits non-zero `DaemonUnreachable`; a direct host runs `tree run --at` instead. The
daemon's `Wake` calls one function of `crates/loom-tree`, `wake(store, at)`.

On a wake, every case that is not terminated goes through the arrival detection of
`story:run-caps-per-arrival`: a placed artifact whose revision moved with no step of the case
moving it gives the case an `arrived` line. Each case is then evaluated at `INSTANT`.

- When a case has a useful action or waits on a person, authority or a decision request, the root's
  run log gets one `woken` line (the instant, `expired` when a held record stopped applying at or
  before it, the cases that had an arrival, and the tree's usage since `tree open`), and the tree
  continues as `tree run --at INSTANT` would: it queues for a run slot as `story:tree-daemon` says,
  its steps take `INSTANT` as their instant, and it rests or stops as
  `story:standing-tree-rests-quiescent` says.
- When no case has, the tree stays `Quiescent` and no `woken` line is written. The `arrived` lines
  and moved revisions of that wake stay recorded; a second wake at the same instant then finds no
  move and changes no byte of the store.
- A wake of a tree that is not `Quiescent`, or not standing, is answered with its state and changes
  nothing. A wake whose instant is before the latest instant in the tree's run logs is refused
  `InstantBeforeLast`, naming both.

Why two lines: `arrived` is per case and happens wherever a case is evaluated (a direct run, a busy
tree, a wake); it is where a writer's run count restarts. `woken` is per tree, only at a wake; it
records the instant the host passed, covers a wake by expiry in which nothing arrived, and is where
the tree's budgets restart. The moved artifacts are in the `arrived` lines only; `woken` names the
cases.

**Budgets at a wake** (session `sdc`'s rule, 2026-11-21, `.engineering/drafts/e3-sdc-answers-5.md`:
"a wake starts a new run for budget purposes with the tree's totals since `tree open` recorded
beside the per-run counters"). A wake starts a new run of a standing tree for budget purposes: the
ceilings and budgets do not change and no binding raises one; the per-run counters start at the
latest `woken` line: steps, agent calls, the sum of step wall times, CPU time, and with them the
elapsed wall time, model calls, tokens and cost of `story:tree-usage-and-model-budget`.
`TreeUsage` gains `since_open`, the same figures counted from `tree open`, which `tree status`, the
`woken` line and the root's `stopped` lines carry beside the per-run figures. A bounded tree counts
as before.

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

The budget rule is the requester's. Today a served tree's totals count from `tree open`
(`story:tree-daemon` § Outcome: "for a served tree, `tree open`, so its totals survive a restart")
and `--max-agent-calls` "counts those lines over the logs of the whole tree, across runs"
(`story:agent-node-run-records` § Outcome); a tree that lives for months would spend them, the
driver's own failure for `max_runs` ("a case that lives for months would exhaust it",
orchestration § 1.2). The totals since `tree open` stay recorded so an admission rule can bound a
standing tree's whole spend (`story:capacity-given-to-an-admission-program`).

## Waits on

- `story:standing-tree-rests-quiescent`: `Quiescent`, the horizon, `next_expiry`.
- `story:run-caps-per-arrival`: the arrival detection and the `arrived` line this story uses.
- `story:tree-usage-and-model-budget`: the story before this one on `ess/domains/tree.yaml`
  (`TreeUsage`), `ess/domains/runlog.yaml` and the budget counters in `crates/loom-tree`.
- Through them, `story:tree-daemon` (the daemon's commands, `DaemonUnreachable`).

Comes before, on shared files: `story:child-case-own-workspace` (`ess/domains/tree.yaml`,
`crates/loom-tree`), and through it every later story of the epic.

## ESS first

`ess/domains/tree.yaml`: the command `Wake` (store, instant), `WakeReport` (state, cases with an
arrival, optional next expiry), the refusal `InstantBeforeLast` (asked, latest), and `TreeUsage`
gains `since_open`. `ess/domains/runlog.yaml`: the event `woken` (instant, `expired`, cases,
`since_open`). On that commit `task drift` fails, the new scenario for `Wake` is red, and
`crates/loom-cli/tests/tree_wake.rs` does not compile.

## Acceptance

With `crates/loom-cli/tests/fixtures/area/` (`area/1` of `story:standing-tree-rests-quiescent`),
to which this story adds `engine-daemon-budget.yaml` (one run slot, a per-run wall ceiling of 3 s
and a per-run CPU ceiling of 2000 ms) and `bin/spin` (a stub that burns 800 ms of CPU and sleeps
1 s); a managed Substrate daemon; no model. The tree rested at `T` with next expiry
`E = T + 3601 s`. `crates/loom-cli/tests/tree_wake.rs`:

- `tree wake --at T+60s` with no file changed answers `Quiescent`, writes no line, and the store's
  bytes are unchanged; a second identical wake also.
- After the test appends a line to the `intake` file, `tree wake --at T+60s` writes one `arrived`
  line naming `intake` in the root's log and one `woken` line naming the root, triage runs once with
  instant `T+60s`, and the tree rests `Quiescent` again.
- After the test changes a placed file no action of `area/1` reads (`notes`), `tree wake --at
  T+70s` writes one `arrived` line, no `woken` line, answers `Quiescent`; a second `tree wake --at
  T+70s` changes no byte of the store.
- `tree wake --at E` writes one `woken` line with `expired: true` and no `arrived` line, the health
  observation runs once, and the tree rests with next expiry `E + 3601 s`.
- `tree wake --at T-1s` exits non-zero `InstantBeforeLast` naming both instants.
- `tree wake` without `--at` exits 2 before it connects; with no daemon it exits non-zero
  `DaemonUnreachable`.
- `tree wake` of a bounded tree, and of a tree that is running, answers its state and writes
  nothing.
- Wall: under `engine-daemon-budget.yaml`, three wakes that each run one 1 s step pass the 3 s
  ceiling since `tree open` and never stop `BudgetSpent`; `tree status` shows the last run's
  `wall_millis_sum` below 3000 beside `since_open.wall_millis_sum` above 3000. A bounded tree of four such steps
  stops `BudgetSpent` naming `wall_seconds`.
- CPU: the same with `bin/spin` and the 2000 ms ceiling: three wakes run, `since_open.cpu_millis`
  exceeds 2000; a bounded tree of three such steps stops `BudgetSpent` naming `cpu_millis`.
- Steps: a standing tree served with `--max-steps 2` whose wakes each take two steps runs at every
  wake; a third step within one wake stops `BudgetSpent` naming the steps.

`crates/loom-tree/tests/wake_budgets.rs` drives `wake` and the run of `crates/loom-tree` directly,
with a stub agent adapter registered by the test (agent steps are not served by the daemon until
`story:agent-session-through-substrate`) and the files `agents.protocol.yaml` and
`agents.bindings.yaml` of `story:tree-usage-and-model-budget`:

- Agent calls: under `--max-agent-calls 1`, a standing tree that takes one agent step per wake runs
  that step at each of three wakes; a second agent step within one wake stops `BudgetSpent` exit 5.
  `since_open.agent_calls` is 3.
- Cost: under `--max-model-cost-micro-usd 2000` with 1500 per session, the same: one session per
  wake, `since_open.cost_micro_usd` 4500.
- The daemon reads no clock to wake: under a host clock fixed at 1970 the reports and lines depend
  only on `--at`.
- `website/docs/reference/cli.md` is regenerated.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, generated
`generated/rust/loom/src/{tree,runlog}.rs`, `ess/scenarios/` (tree domain); `crates/loom-tree`
(`wake`, the counters' starting line, `since_open`); `crates/loom-cli/src/lib.rs` (`tree wake`);
`crates/loom-cli/tests/tree_wake.rs`, `crates/loom-tree/tests/wake_budgets.rs` (new);
`crates/loom-cli/tests/fixtures/area/` (adds `engine-daemon-budget.yaml`, `bin/spin`);
`website/docs/reference/cli.md`
(regenerated); `CHANGELOG.md`.

## Out of scope

A file watch or a timer in the daemon. Calling `decisions tick` at a wake. Writers' run counts
(`story:run-caps-per-arrival`). Waking a tree served by no daemon. Per-case scheduling of a woken
tree (`story:concurrent-cases-in-the-tree-daemon`, which lands later and applies to it).
