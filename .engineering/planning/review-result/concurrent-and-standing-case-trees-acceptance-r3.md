---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-acceptance-r3
kind: review-result
status: active
title: Plan critic acceptance, round 3, on epic:concurrent-and-standing-case-trees
relations:
- reviews: epic:concurrent-and-standing-case-trees
- reviews: story:flow-limits-recorded-as-events
- reviews: story:capacity-given-to-an-admission-program
- reviews: story:concurrent-cases-in-the-tree-daemon
- reviews: story:child-case-own-workspace
- reviews: story:run-caps-per-arrival
- reviews: story:standing-tree-rests-quiescent
- reviews: story:standing-tree-wakes-at-a-host-instant
- reviews: story:tree-usage-and-model-budget
revision: 1
---
needs-revision

All 11 of my round-2 fixes hold in the stored bodies. The revision leaves 8 findings: 2 blockers and 6 warnings. Line numbers below are lines of the files under `.engineering/planning/`.

- `story:standing-tree-wakes-at-a-host-instant` — The two bullets meant to show a wake stopped by the kept `TreeBudget` cannot pass, and the bullets that can pass would also pass with no budget kept at all. The failing bullets are `--max-agent-calls 1` and cost `1500`, each saying "the first wake stops `BudgetSpent` … before `reply.write`". On `agentpulse/1` the opening `tree run` already takes `ack.write`, then stops before `reply.write`. So the tree never rests `Quiescent`, and a wake of a tree that is not `Quiescent` "is answered with its state and changes nothing" (`:38`). The passing bullets (agent calls 2, cost 3000, steps 2) never stop. The kept budget (design change 3) therefore has no working check — `.engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:189-194` against `:38-39`, `:60-62`
- `story:standing-tree-wakes-at-a-host-instant` — The bullet "under a host clock fixed at 1970 the reports and lines of the `area/1` bullets above are byte for byte the same" cannot pass. It names nothing to compare against. The `woken` line carries `since_open` with measured `wall_millis_sum`, and `action finished` lines carry step wall times, so two runs never match byte for byte. It also sits under the library test `wake_budgets.rs` while it refers to the CLI bullets run through the daemon — `.engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:195-196` against `:30-31`, `:183`
- `story:standing-tree-wakes-at-a-host-instant` — The bounded `--max-steps 2` bullet ("its second `tree run` takes no step and stops `BudgetSpent` naming `max_steps`") only passes if a served bounded tree counts its steps from `tree open`. No story says so:
  - The Outcome calls this "as before" (`:64-66`).
  - `story:tree-daemon` counts only wall and CPU from `tree open` (`tree-daemon.md:52-54`).
  - `--max-steps` stops "the run" per invocation (`tree-run-command.md:155`).
  - The bullet also does not say whether the second run restates the flag.

  As written, the second run takes two steps and stops `Stuck` — `.engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:180-181`
- `story:concurrent-cases-in-the-tree-daemon` — The bullet "With `hub/1` and only `b`: the tree stops `Quiescent`, exit 7" expects a state the Outcome's rule does not give. The `hub/1` root has an open child, so it is `HoldsChildren`. The rule gives `Quiescent` only "when every case left is `Resting`". Read literally, it gives no stop at all for a root that holds only resting children — `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:199` against `:47-52`
- `story:concurrent-cases-in-the-tree-daemon` — In the wake bullet, "`b` shows `Queued` or `Running`" is a passing state that no gate holds. `bin/read-feed` returns at once and the wake answers only after `b` rests. So a test sees the state on some runs and not on others — `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:195`
- `story:capacity-given-to-an-admission-program` — The daemon bullet needs trees `A`, `B` and `C` opened on three different `--workspace` directories, and the body does not say so. Under story 6's rule of one writer per workspace across served trees, a `B` that shares `A`'s directory queues behind `A`'s held gate "even with a slot free". `B`'s `intake.prep` never starts, and the expected `running: 2, queued: 1` is never reached — `.engineering/planning/story/capacity-given-to-an-admission-program.md:164-170` against `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:30-34`
- `story:flow-limits-recorded-as-events` — The bullet "An admission stub … sees … the root's two open children with their opening instants" cannot pass. The `admit-open` root "first opens two `item/1` children that wait on `bin/gate`", and a parent "takes its next step only once it has no open child". So the root's admission never runs while those children are open — `.engineering/planning/story/flow-limits-recorded-as-events.md:103-105,114-115` against `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:27-28`
- `story:flow-limits-recorded-as-events` — The daemon bullet "a change made while the admission stub is held by `bin/gate` … present in the next admission's" has two gaps:
  - No binding holds the admission stub at a gate: `admit-gated.bindings.yaml` gates `intake.prep`, not `wave.admit`.
  - No input makes a next admission runnable: `wave.admit` runs once per revision of `intake`.

  It is the same fixture gap round 2 found elsewhere — `.engineering/planning/story/flow-limits-recorded-as-events.md:116-118` against `.engineering/planning/story/capacity-given-to-an-admission-program.md:134-143`

**What I read:** 9 of 9 ids (the epic and eight stories), each body in full from its stored file under `.engineering/planning/`. For the two stories outside the set I read only the `git diff`. Both changes hold. The new cost bullet in `budget-and-limit-stops-raise-decision-requests` works out: with `--extend-by 1` the budget goes from 3000 to 6000, so the third session (4500) runs. Also read:
- the brief, `e4r2-changes.md`, my round-2 verdict and `e4r1` of the wakes story;
- `aep plan artifact kinds` and `lifecycle story`;
- `crates/loom-executor/src/selection.rs:270-330`, `ess/domains/bindings.yaml:100-135`, `crates/loom-bindings/src/decode.rs:667-696`;
- `story:tree-daemon`, `story:action-limits-enforced` and `story:tree-run-command`, on slots, budgets and workspaces.

**Round-2 fixes confirmed (my 11):**
- CPU and wall checks now use separate engine files, and `bin/spin` does not sleep.
- `pulse/1` gives exactly two steps per wake.
- The bullets now assert the `WakeReport` members and `since_open`.
- In run-caps, the order and the count of 11 follow from `selection.rs:285-319`, the end state is `accepted` with exit 0, and the agent check is a library test.
- The capacity timing fix is in place (the case keeps its slot), and so is the input for the standing wake.
- `area/1` and `area-stuck` are defined.
- `hub/1` and the X, A, B bullet are present.
- `Absent`, `dir: 7` and the containing-directory case are present.

**Slot kept across steps (design change 1), my lane only:** this change makes `story:tree-daemon`'s "third's first `action started` follows the terminal line" and the epic's "third starts after one gave its slot back" hold reliably. I found no slot-counting bullet that it breaks.

**What I could not establish:**
- How a library test (`tree_usage.rs`, `wake_budgets.rs`) observes "exit 5" and `tree status`. The `crates/loom-tree` crate does not exist at `61fd500b`.
- Whether `tree approve` of the person-bound `feed.answer` produces `feed_answered`. This decides whether the second half of the Waiting-over-Stuck bullet can be reached.
- Whether `engine-daemon-cpu.yaml` carries `adapter: daemon`. Unlike the wall file, its line does not say so, and a CPU ceiling needs that adapter.

**Out of my lane, not setting the verdict:**
- Fairness: a long runnable case holds its slot until it waits or ends, so queued cases wait behind it (design).
- The parent-steps-only-with-no-open-child rule leaves `flow`'s open children always empty for an admission on the parent. Orchestration § 2.4a has `wave.admit` count the open story children (scope and design).
- Story 4's "its steps take `INSTANT` as their instant" contradicts the step instants from the host clock under the daemon in `e4r1` § Open 8 (design).
- `story:capacity-given-to-an-admission-program` already has a `## Scope`.

```findings
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 189
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the bullets 'under --max-agent-calls 1 the first wake stops BudgetSpent' and 'under 1500 the first wake stops' cannot pass, because the opening tree run of agentpulse/1 already stops before reply.write and a wake of a tree that is not Quiescent changes nothing, and the passing bullets never stop, so the kept TreeBudget has no working check"
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 195
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet 'under a host clock fixed at 1970 the reports and lines of the area/1 bullets above are byte for byte the same' names nothing to compare against, the woken and action finished lines carry measured wall times that differ run to run, and it sits under the library test while it refers to the daemon-driven CLI bullets"
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 180
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bounded --max-steps 2 bullet expects the second tree run to stop at once on max_steps, which needs a served bounded tree to count steps from tree open, but tree-daemon counts only wall and CPU from tree open and --max-steps stops per run, the Outcome calls it 'as before', and the bullet does not say whether the second run restates the flag"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 199
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet 'With hub/1 and only b: the tree stops Quiescent, exit 7' expects a state the Outcome does not give: the hub root is HoldsChildren and the Outcome gives Quiescent only when every case left is Resting"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 195
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'b shows Queued or Running' is a passing state no gate holds, since bin/read-feed returns at once and the wake answers after b rests, so a test sees it on some runs and not others"
- file: .engineering/planning/story/capacity-given-to-an-admission-program.md
  line: 164
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the daemon bullet does not say trees A, B and C are opened on three different --workspace directories, and under story 6's rule of one writer per workspace across served trees a B that shares A's directory queues behind A's held gate, so running 2 and queued 1 are never reached"
- file: .engineering/planning/story/flow-limits-recorded-as-events.md
  line: 114
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the bullet 'an admission stub sees the root's two open children with their opening instants' cannot pass, because the admit-open root first opens two gated item/1 children and a parent takes its next step only once it has no open child, so the root's admission never runs while they are open"
- file: .engineering/planning/story/flow-limits-recorded-as-events.md
  line: 116
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the daemon bullet 'a change made while the admission stub is held by bin/gate is present in the next admission's' names no binding that gates wave.admit (admit-gated gates intake.prep) and no input that makes a next admission runnable (wave.admit runs once per intake revision)"
```
