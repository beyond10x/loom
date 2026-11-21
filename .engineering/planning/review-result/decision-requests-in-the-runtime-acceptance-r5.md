---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-acceptance-r5
kind: review-result
status: active
title: Plan critic acceptance, round 5, on story:decisions-resume-the-held-run and story:daemon-continues-a-decided-request of epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:decisions-resume-the-held-run
- reviews: story:daemon-continues-a-decided-request
revision: 1
---
needs-revision

story:decisions-resume-the-held-run — the outcome bullets need a state directory that `b10x-loom run --state DIR` leaves on a "clock-compatible fixture protocol whose root outcome `close` is blocked on decision `release`", but no such protocol can exist: `clock_compatible` requires a protocol identical to `protocols/system-query/1.yaml` in all but name and descriptions (outcome `answered`, no decision), so that run stops `NoLocalExecutor` at the cited `intent.rs:274-286`, and the outcome, 31, 32 and `record` bullets need a state directory the test builds through the library instead, as story 12 already does (`question-and-outcome-decisions-hold-runs.md:186-189`) — .engineering/planning/story/decisions-resume-the-held-run.md:280 (crates/loom-protocols/src/lib.rs:72-105)

story:decisions-resume-the-held-run — the tick bullet "With `DR-a`'s commission re-held for another request … `DR-a`'s `TickResume` carries `exit_status` 30" contradicts the body's own lookup: it keeps only commissions whose hold names the request, and `tick` takes no `--commission`, so `DR-a` comes out `nothing_held` with exit_status 0 ("`0` also when no run holds the request") — .engineering/planning/story/decisions-resume-the-held-run.md:337

story:decisions-resume-the-held-run — the tick setup asks for two runs with "the same `expires_at` `E`", which cannot be arranged: each run sets `expires_at` to its host-clock reading plus 3600 s, and `run` has no instant flag (`approval-decisions-resume-held-runs.md:90-98`, `:163`), so the bullet has to tick at the later of the two instants it reads — .engineering/planning/story/decisions-resume-the-held-run.md:327

story:decisions-resume-the-held-run — no command-line bullet checks that `decisions record … --state DIR` resumes a held question, or that `decide` of a held question prints the `record` command and resumes nothing; the only `record` bullet is the refused case (exit 32), and its `DR-q` is not opened by any step of the setup — .engineering/planning/story/decisions-resume-the-held-run.md:53 (bullet at :320)

story:decisions-resume-the-held-run — two `tick --resume` rules have no bullet: a request taken on silence that still awaits its record gives a `TickResume` with status 30, and a tick that stops on an error first resumes the requests it took before the error — .engineering/planning/story/decisions-resume-the-held-run.md:165-168

story:daemon-continues-a-decided-request — the restart bullet does not check that the root is still not `Terminal` when the daemon is killed "on the `decided` line"; if the daemon's next pass runs before the kill, the bullet passes without testing any restart — .engineering/planning/story/daemon-continues-a-decided-request.md:157

story:daemon-continues-a-decided-request — no bullet checks the served-tree tick output (the `TickReport`, then one `unblocked: <case> (DR-…)` line per request taken on silence) — .engineering/planning/story/daemon-continues-a-decided-request.md:89-90

story:daemon-continues-a-decided-request — ESS first says `Decide` "moves a `Waiting` tree to `Queued`", and Waits on calls concurrent-cases "Shared with no edge". The story now has a `depends_on` edge to concurrent-cases, and that story makes `Queued` a per-case state and derives the tree's state from its cases. So ESS first must name the case's move from `Waiting` to `Queued`, and the Waits-on line is wrong. — .engineering/planning/story/daemon-continues-a-decided-request.md:133 (and :123; concurrent-cases-in-the-tree-daemon.md:97, :111)

What I read: 2 of 2 ids in full (`aep plan artifact show`); the r4 and r5 briefs; ledgers e3r7 and e3r9; the story 16 diff; stories 12, 13, 14, 10, 15 (exit tables and fixtures); `story:tree-daemon`; `story:concurrent-cases-in-the-tree-daemon`; `crates/loom-cli/src/lib.rs:1-200`; the cited lines in `intent.rs`, `resume.rs`, `run.rs`, `main.rs`, `runtime.rs` and `loom-sdk/src/lib.rs`; `loom-protocols/src/lib.rs:72-105`; `protocols/system-query/1.yaml`; a cycle check over `aep plan artifact graph --format json`.

What I checked and found sound:
- Exit statuses 30 to 35 clash with nothing: `lib.rs` uses 0-4 and 10-23, and no other decision story numbers anything above 23.
- No cycle. Story 22 transitively depends on stories 12, 13 and `story:tree-run-command`.
- A decision whose resume is refused or fails is stated and tested:
  - `show` prints the request Decided after the failure.
  - A repeat `decide` names `DecisionStateConflict` and performs the merge once.
  - The tick still performs `DR-b`'s merge after `DR-a` fails, and exits 35.

What I could not establish:
- Whether the approval fixture's resume context names a model catalog file. The exit-33 bullets remove that file, and `approval_decisions.rs` does not exist yet; story 10 at `:190-191` only says "stub programs, a recorded selector".
- How a crash between `Decide` and the record is injected into a separate daemon process. Story 16's precedent (`:190`) is a direct `tree run`.

Out of my lane: story 21's summary still says "tick resumes nothing" (`decisions-resume-the-held-run.md:7`), which contradicts its body (scope/design). Story 22's Waits-on/relation mismatch is also design or parallel-safety.

```findings
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 280
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the outcome bullets need a state directory that `b10x-loom run --state DIR` leaves on a clock-compatible fixture protocol whose root outcome `close` is blocked on decision `release`, but clock_compatible requires a protocol identical to protocols/system-query/1.yaml in all but name and descriptions (outcome `answered`, no decision), so that run stops NoLocalExecutor at the cited intent.rs:274-286, and the outcome, 31, 32 and record bullets need a library-built state directory as story 12 already uses"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 337
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the tick bullet expecting exit_status 30 for DR-a's commission re-held for another request contradicts the body's lookup, which keeps only commissions whose hold names the request with no --commission on tick, so DR-a is nothing_held with exit_status 0"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 327
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the tick setup asks for two runs with the same expires_at E, which cannot be arranged because each run sets expires_at to its host-clock reading plus 3600 s and run has no instant flag, so the bullet has to tick at the later of the two instants it reads"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 53
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no command-line bullet checks that `decisions record ... --state DIR` resumes a held question or that `decide` of a held question prints the record command and resumes nothing; the only record bullet is the refused 32 case, whose DR-q is not opened by any step of the setup"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 165
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "two tick --resume rules have no bullet: a request taken on silence that still awaits its record gives a TickResume with status 30, and a tick that stops on an error first resumes the requests it took before the error"
- file: .engineering/planning/story/daemon-continues-a-decided-request.md
  line: 157
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the restart bullet does not check that the root is still not Terminal when the daemon is killed on the decided line, so if the daemon's next pass runs before the kill the bullet passes without testing any restart"
- file: .engineering/planning/story/daemon-continues-a-decided-request.md
  line: 89
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet checks the served-tree tick output (the TickReport, then one `unblocked: <case> (DR-...)` line per request taken on silence)"
- file: .engineering/planning/story/daemon-continues-a-decided-request.md
  line: 133
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "ESS first says Decide moves a Waiting tree to Queued and Waits on calls concurrent-cases shared with no edge, but the story now depends on concurrent-cases, which makes Queued a per-case state and derives the tree's state from its cases, so ESS first must name the case's move from Waiting to Queued and the Waits-on line is wrong"
```
