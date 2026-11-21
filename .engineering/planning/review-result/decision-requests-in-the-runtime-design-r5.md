---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-design-r5
kind: review-result
status: active
title: Plan critic design, round 5, on story:decisions-resume-the-held-run and story:daemon-continues-a-decided-request of epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:decisions-resume-the-held-run
- reviews: story:daemon-continues-a-decided-request
revision: 1
---
needs-revision

story:daemon-continues-a-decided-request — the graph orders this story after story:concurrent-cases-in-the-tree-daemon, but the body is still written against tree-level states: § ESS first says `Decide` "moves a `Waiting` tree to `Queued`", § Waits on lists concurrent-cases as "Shared with no edge", and § Out of scope excludes the per-case states. The body needs to state `Decide` and `Tick` per case (the decided case leaves `Waiting` and queues for a slot, or takes a free one) and record the edge it now has — .engineering/planning/story/daemon-continues-a-decided-request.md:111-113, :102-103, :162; .engineering/planning/story/concurrent-cases-in-the-tree-daemon.md:94-119; aep plan artifact graph --format json

story:decisions-resume-the-held-run — `tick --resume` resumes only the requests "this tick" took on silence, and that leaves two gaps. First, its error path ("first resumes the requests it took before the error") needs a list of those requests, but the tick's error carries none and `TickReport` is declared unchanged. Second, the body never says how a run whose tick resume failed or was refused gets resumed later: a repeat tick takes nothing, and the retry rule covers only `decide`, `take` and `record`, which needs a person. The fix is to have `tick --resume` also resume requests that are Decided by `silence` and still held, which closes both gaps — .engineering/planning/story/decisions-resume-the-held-run.md:132-134, :140-141, :123-124, :143; .engineering/planning/story/silence-and-expiry-at-a-host-tick.md:55-56 (stored body)

epic:decision-requests-in-the-runtime — the Order paragraph says story 22 "depends on all three" (stories 16 and 21 and story:tree-daemon). It has a fourth `depends_on` edge, to story:concurrent-cases-in-the-tree-daemon, which the paragraph does not name — .engineering/planning/epic/decision-requests-in-the-runtime.md:181-182; aep plan artifact graph --format json

What I read: the 2 stories in full, the diffs of story 16, the silence story and the epic, story:tree-daemon, story:concurrent-cases-in-the-tree-daemon, story 14's resume section, sdc answers 7 and 8, and ledgers e3r7 and e3r9. Commands run: `aep plan artifact relations`, `aep plan artifact graph --format json` (350 `depends_on` edges walked store-wide, outside the set too, with a cycle search: no cycle), `aep plan artifact validate` (`valid`, nothing on these two stories). Code citations checked: lib.rs:26-35, :90-182, resume.rs:291, :190, intent.rs:274-286, :654, :695-700, runtime.rs:2374, run.rs:1300-1310, main.rs:616-685, loom-sdk lib.rs:81, adversary_slice_run.rs:564-566, `HoldSuspension`. Acceptance reads traced to a producer: 14 (story 21: the fixtures of stories 13 and 14, `decisions record`, `complete_decision`, `tick`, `TickReport`, the inbox SDK, the `Decision` hold, routing with default on silence; story 22: story 16's writers and fixtures, `tree-run-command`'s fixture, tree-daemon's socket, `Daemon` runner and `StoreServed`, story 21's `--resume` refusal). An edge or chain orders all 14; the only exception is the tick error list above, which nothing produces.

Checks with no defect found:
- Exit statuses 30 to 35 do not clash with the run table, `resume` 10-22, `approve` 19 and 23, the inbox's 10, 11, 12 and 20, or any status another epic story numbers.
- `crates/loom-cli/src/decisions.rs` is owned in this order: 11, 12, 13, 14, 21, 16, 22, each pair joined by a `depends_on` edge. Stories 17 and 20 name only other `*_decisions.rs` files.
- Each of `sdc`'s three conditions has a bullet that can fail: story 21's first tick bullet; story 16's `continue:` bullet and story 22's served-store bullet; story 22's bullets 2, 5 and 11.
- Neither story makes a model call in a test, and both put their declarations under ESS first.

What I could not establish:
- Whether a commission on a clock-compatible protocol can be rebuilt from its resume context outside `resume_with_state`, which refuses any protocol other than `software-change@1` (intent.rs:695-700). Story 21 says both paths rebuild "as `b10x-loom resume` does", yet its outcome bullets use such a protocol. This belongs to the acceptance and scope lanes, not mine.
- Whether a decider can reply "strong A" while the request still waits on a person with a default on silence, which the tick fixture needs (story 21 lines 298-301). Acceptance lane.
- Whether the domain checks `NotEligible` before `DecisionStateConflict` on a second `decide`, which decides whether the retry rule works for a principal who is not eligible. Acceptance lane.

```findings
- file: .engineering/planning/story/daemon-continues-a-decided-request.md
  line: 111
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the graph orders this story after story:concurrent-cases-in-the-tree-daemon, but ESS first still moves a Waiting tree to Queued, Waits on calls concurrent-cases shared with no edge, and Out of scope excludes per-case states; the body must state Decide and Tick per case and name the edge"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 140
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "tick --resume resumes only what this tick took, so its error-path sentence needs a taken-before list the tick error does not carry (TickReport is unchanged), and no path resumes a run whose tick resume failed without a person; tick --resume should also resume requests Decided by silence and still held"
- file: .engineering/planning/epic/decision-requests-in-the-runtime.md
  line: 181
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Order paragraph says story 22 depends on all three of stories 16, 21 and story:tree-daemon, but the graph gives it a fourth depends_on edge to story:concurrent-cases-in-the-tree-daemon that the paragraph does not name"
```
