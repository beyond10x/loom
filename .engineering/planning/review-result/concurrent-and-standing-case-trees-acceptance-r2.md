---
format: aep.planning-md/3
id: review-result:concurrent-and-standing-case-trees-acceptance-r2
kind: review-result
status: active
title: Plan critic acceptance, round 2, on epic:concurrent-and-standing-case-trees
relations:
- reviews: epic:concurrent-and-standing-case-trees
- reviews: story:flow-limits-recorded-as-events
- reviews: story:capacity-given-to-an-admission-program
- reviews: story:child-case-own-workspace
- reviews: story:tree-usage-and-model-budget
- reviews: story:concurrent-cases-in-the-tree-daemon
- reviews: story:run-caps-per-arrival
- reviews: story:standing-tree-rests-quiescent
- reviews: story:standing-tree-wakes-at-a-host-instant
revision: 1
---
needs-revision

All nine round-1 acceptance fixes hold in the stored bodies. Nine findings remain or are new, all in the fixes' fixtures and bullets. Line numbers are lines of the files under `.engineering/planning/` (`show` line + 4).

- `story:standing-tree-wakes-at-a-host-instant` — the CPU bullet "a bounded tree of three such steps stops `BudgetSpent` naming `cpu_millis`" cannot pass: `bin/spin` takes about 1.8 s wall (800 ms CPU plus a 1 s sleep). The same `engine-daemon-budget.yaml` holds a 3 s per-run wall ceiling. After step 2 the wall is about 3.6 s and the CPU 1.6 s, so the tree stops on `wall_seconds`, never `cpu_millis` — `.engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:127-128` against `:103-104`
- `story:standing-tree-wakes-at-a-host-instant` — the wall bullet's "one 1 s step" per wake and the steps bullet's "wakes each take two steps" (then "a third step within one wake") name steps the `area/1` fixture cannot produce. Files and ESS add only `bin/spin`, and a wake of `area/1` runs triage once on an `intake` change. Which stub sleeps 1 s, and which input change gives two and then three steps, is not stated — `.engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:123-131`
- `story:standing-tree-wakes-at-a-host-instant` — the Outcome gives `WakeReport` members "the cases with an arrival, and the next expiry" (and the `woken` line carries `since_open`). The bullets assert only the state (`answers Quiescent`), so a report with no next expiry passes — `.engineering/planning/story/standing-tree-wakes-at-a-host-instant.md:58-59`
- `story:run-caps-per-arrival` — bullet 1 asserts the sequence "write, check, write, check, then stops `RunsSpent`", and bullet 4 asserts that the check "runs 10 times". The selector orders non-`write` before `write` (`crates/loom-executor/src/selection.rs:319`). It treats a check as useful whenever its kind is uncounted (`:305`, `counted_kinds` at `crates/loom-governor/src/lib.rs:1889`). The `draft/1` fixture does not say whether the first pick is the check or the write. So the order and the count (10 or 11) are not fixed by the body — `.engineering/planning/story/run-caps-per-arrival.md:123,131`
- `story:run-caps-per-arrival` — "the test then rewrites `brief` … `draft.check` records `valid`, and the case goes on" names no state the run ends in (exit status, stop or terminal outcome). Once the check is valid the writer is useful again at the new revision unless the fixture stops it. The bullet can therefore be met or missed by the same run — `.engineering/planning/story/run-caps-per-arrival.md:128`
- `story:run-caps-per-arrival` — the Acceptance says "The tests drive `b10x-loom tree run`", but the bullet "An agent writer (a stub adapter, no model) … is counted and stopped the same way" needs an adapter the test registers. Elsewhere the epic places that in a library test (`story:tree-usage-and-model-budget` § Acceptance, `story:standing-tree-wakes-at-a-host-instant` `wake_budgets.rs`). The layer that runs it is not named — `.engineering/planning/story/run-caps-per-arrival.md:134`
- `story:capacity-given-to-an-admission-program` — the daemon bullet expects `running: 2, queued: 1` at the admission, but the admission tree has a 1 s program step before `wave.admit`. By `story:concurrent-cases-in-the-tree-daemon` it gives its slot back between the two steps, so a queued `C` can take that slot. The body does not say when `C` is submitted, what holds it queued, or what the admission case's queue age is between steps; the figure depends on timing — `.engineering/planning/story/capacity-given-to-an-admission-program.md:103-105`
- `story:capacity-given-to-an-admission-program` — the bullet "A standing admission tree woken twice: at the second wake's admission `used` counts only that wake's steps" names no arrival or fixture that makes `wave.admit` runnable again at each wake. `admit/1` performs it once per case revision, so the wake test has no stated input — `.engineering/planning/story/capacity-given-to-an-admission-program.md:106-107`
- `story:standing-tree-rests-quiescent` — two bullets need `area/1` variants the fixture list does not give. "A standing tree whose obligation is open and nothing useful exits 4 `Stuck`" has no variant with an undischargeable obligation. "A standing root whose `retired` outcome becomes legitimate" has no input that makes `retired` legitimate. Neither can be reproduced from the body — `.engineering/planning/story/standing-tree-rests-quiescent.md:106,115`
- `story:concurrent-cases-in-the-tree-daemon` — the Outcome fixes a stop order (`Terminal`, then `Waiting`, then `Stuck`, then `Quiescent`) and "oldest first" queueing. No bullet puts a waiting case and a stuck case in one tree, or two queued cases, so a tree that reports `Stuck` over `Waiting` or runs the newest first passes — `.engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:24,42-46` against `:118-140`
- `story:child-case-own-workspace` — the Outcome refuses a value that is "absent or not a string" and one that is "containing the workspace of another case". The refusal bullet lists a relative path, a missing directory, a file, the root's directory, a directory inside another's and a symbolic link. It lists no absent or non-string value and no containing directory (`Absent` has no check) — `.engineering/planning/story/child-case-own-workspace.md:26-29` against `:101-104`

**What I read:** 9 of 9 ids (the epic and the eight stories) with `aep plan artifact show`, plus `aep plan artifact kinds`/`lifecycle` skill rules. I also read the brief, `e4r1-changes.md`, `e4-sdc-answers.md`, the last paragraph of `e3-sdc-answers-5.md` and the round-1 verdict. I opened `crates/loom-executor/src/selection.rs:285-345`, `crates/loom-governor/src/lib.rs:1170-1200,1889-1914,1936-1955` and `crates/loom-bindings/src/load.rs:116-124,284-293`. I ran no build and no test.

**Round-1 fixes confirmed (all nine of my charter):**
- The epic check carries `--at T` (`epic/concurrent-and-standing-case-trees.md:187`).
- Two epic bullets cover the wake budgets (`:178`) and runs per outside revision (`:181`).
- The no-write wake bullet is present (`wakes:109-111`).
- Separate checks cover wall, CPU, steps, agent calls and cost.
- The grandchild and agent-workspace checks are present.
- The per-case state checks use held `bin/gate` stubs.
- Exit 7 through the daemon is checked (`rests-quiescent:103`).
- `capacity` names the run-log reader, `queued: 1` and the `arguments_from` input.

Findings 4 to 6 of this round partly re-state round-1 items 3 and 5: the fix landed but its fixture is not reproducible as written.

**What I could not establish:**
- Whether `draft.check` is admissible before `draft` exists, and whether a valid check stops the writer. This decides how far the first `run-caps-per-arrival` finding goes (order only, or order and count). It depends on the catalogue rules of the not-yet-written `draft/1` protocol.
- Whether `crates/loom-tree` serves a stub agent adapter to a CLI `tree run`. This decides whether the agent bullet is a layer gap or a wording gap.

**Out of my lane, not setting the verdict:**
- `story:tree-usage-and-model-budget` (`model_calls` and `cpu_millis` unchecked there) is covered by the wakes story's bullets; I raised no finding.
- The ledger's Open 1 to 12 are scope and design decisions.
- Round-1 stale-citation findings are fixed (bodies now cite artifact, section and sentence).

```findings
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 127
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the CPU bullet says a bounded tree of three bin/spin steps stops BudgetSpent naming cpu_millis, but bin/spin takes about 1.8 s wall and engine-daemon-budget.yaml holds a 3 s per-run wall ceiling, so the tree stops on wall_seconds after step 2 with CPU at 1.6 s and the check cannot pass"
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 123
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the wall bullet's 'one 1 s step' per wake and the steps bullet's 'wakes each take two steps' and 'a third step within one wake' name steps the area/1 fixture and the added bin/spin cannot produce, and the body names no 1 s stub or input that gives two or three steps"
- file: .engineering/planning/story/standing-tree-wakes-at-a-host-instant.md
  line: 58
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome gives WakeReport the cases with an arrival and the next expiry and the woken line since_open, and no bullet asserts any of them, so a report with only the state passes"
- file: .engineering/planning/story/run-caps-per-arrival.md
  line: 123
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "bullet 1 asserts the sequence 'write, check, write, check, then stops RunsSpent' and bullet 4 'draft.check runs 10 times', but the selector orders non-write before write (selection.rs:319) and a check is useful while its kind is uncounted (selection.rs:305), and the fixture does not say whether the first pick is the check or the write, so the order and the count (10 or 11) are not fixed"
- file: .engineering/planning/story/run-caps-per-arrival.md
  line: 128
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'draft.check records valid, and the case goes on' names no end state (exit status, stop or terminal outcome), and a writer useful again at each new revision is not stopped by a valid check in the fixture"
- file: .engineering/planning/story/run-caps-per-arrival.md
  line: 134
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Acceptance says the tests drive b10x-loom tree run, yet the agent-writer bullet needs a stub adapter the test registers, which the epic places in library tests, so the layer that runs it is not named"
- file: .engineering/planning/story/capacity-given-to-an-admission-program.md
  line: 103
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the daemon bullet expects running 2 and queued 1 at the admission, but the admission tree gives its slot back between its 1 s program step and wave.admit, so a queued C can take it, and the body does not say when C is submitted, what holds it queued or the admission case's queue age, so the figures depend on timing"
- file: .engineering/planning/story/capacity-given-to-an-admission-program.md
  line: 106
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the standing admission bullet 'woken twice' names no arrival or fixture that makes wave.admit runnable again at each wake, so the check has no stated input"
- file: .engineering/planning/story/standing-tree-rests-quiescent.md
  line: 106
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullets 'a standing tree whose obligation is open and nothing useful exits 4 Stuck' and 'a standing root whose retired outcome becomes legitimate' need area/1 variants (an undischargeable obligation, an input that makes retired legitimate) that the fixture description does not give"
- file: .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md
  line: 42
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome fixes the stop order Terminal, Waiting, Stuck, Quiescent and oldest-first queueing, and no bullet puts a waiting and a stuck case in one tree or two queued cases together, so Stuck over Waiting or newest-first passes"
- file: .engineering/planning/story/child-case-own-workspace.md
  line: 101
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome refuses a value that is absent or not a string and a directory containing another case's workspace, and the refusal bullet covers neither, so the Absent reason and the containing case are unchecked"
```
