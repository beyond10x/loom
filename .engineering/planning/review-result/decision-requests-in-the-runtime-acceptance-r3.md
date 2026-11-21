---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-acceptance-r3
kind: review-result
status: active
title: Plan critic acceptance, round 3, on epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:approvals-supplied-to-canon-evaluation
- reviews: story:decided-record-written-then-recorded
- reviews: story:approval-decisions-resume-held-runs
- reviews: story:decision-inbox-commands
- reviews: story:decision-request-file-store
- reviews: story:decision-suspension-reason
- reviews: story:decision-request-entity
- reviews: story:escalate-when-conditions
- reviews: story:grounds-resolve-against-held-cases
- reviews: story:decision-routing-by-holder
- reviews: story:non-independent-review-becomes-decision-request
- reviews: story:llm-decider
- reviews: story:question-and-outcome-decisions-hold-runs
- reviews: story:authority-records-bound-and-grounded-in-canon
- reviews: story:requests-from-canon-pending-decisions
- reviews: story:autonomy-grant-table
- reviews: story:silence-and-expiry-at-a-host-tick
- reviews: story:decider-port-and-grounding-check
- reviews: story:decided-records-carry-grounds-to-canon
- reviews: story:tree-waits-open-decision-requests
revision: 1
---
needs-revision

Round 2's four findings all hold as fixed in the stored bodies. The round-3 revision left three acceptance defects, all introduced by it.

Findings:

- `story:approval-decisions-resume-held-runs` — two command-line bullets cannot be run as written. "At `A0` with the classes document giving the merge class `expires_after: 3600` and the run's instant fixed at 12:00, the request's `expires_at` is 13:00" and the `A1` silence bullet that follows it both need `run --state` to take a fixed instant. `b10x-loom run` reads `&HostClock` at `crates/loom-cli/src/main.rs:1200` and `:1204`. The decision flags the story lists include no `--at`, and no `--at` exists on `run` in `crates/loom-cli`. The `expires_at` of a real run is therefore now plus 3600, never 13:00, and `decisions tick --at 12:59` and `--at 13:00` cannot be pinned to it. The body either adds a way to fix the run's instant, or the bullets read the printed `expires_at` and tick relative to it. — `.engineering/planning/story/approval-decisions-resume-held-runs.md:155`
- `story:non-independent-review-becomes-decision-request` — the bullet "At `A3` with the decider's kind set so that its principal equals `llm-node:impl`" describes a state the story's own naming rule cannot produce. Condition 1 and `story:llm-decider` fix a decider's principal as `decider:<kind>`, so no kind makes it equal the producer `llm-node:impl`. As written, the `decider not independent: principal` refusal has no reachable test. The fixture needs a set-aside record whose producer is `decider:<kind>`. — `.engineering/planning/story/non-independent-review-becomes-decision-request.md:131`
- `story:non-independent-review-becomes-decision-request` — three parts of the three independence conditions have no bullet, so each can be dropped with every bullet green.
  - The run-log branch of condition 1: a decider whose principal equals a producer of a subject revision recorded in the case's run log.
  - The run-log branch of condition 2: "no run-log line of the case records a revision of the subject by it".
  - `ModelDecider` reporting its route's model id and adapter. Bullet 3 uses a recorded `decider:stub` with model `fixture-model`, and `story:llm-decider` has no bullet for identity either.
  
  Add one bullet per part. — `.engineering/planning/story/non-independent-review-becomes-decision-request.md:134`

What I read: 22 of 22 artifacts, namely the epic, the 20 stories and the upstream blocker. I read the epic and every story with `aep plan artifact show`, the blocker likewise, plus `e3-round-2-acceptance.md` and `e3r3-changes.md`. I also checked `runtime.rs:2400-2416`, `lib.rs:1036-1044`, `crates/loom-intake-slice/src/clock.rs`, `crates/loom-cli/src/main.rs` and `tree-run-command` for how an instant is supplied.

Round-2 check, all four hold:
- `approval-decisions-resume-held-runs`, moved ground.
  - The bullet now uses a `path_line` ground on `notes.md`, with the case revision unchanged.
  - `PathLineResolver` writes a digest into the ground, so a rewrite is detectable at re-check (`decider-port-and-grounding-check` Outcome).
  - It expects `resume` refused as not granted, not `CaseMoved`, with the request Superseded.
  - The separate `CaseMoved` bullet now says the provider is not asked and the request stays Decided, so the two bullets are distinct.
- `authority-records-bound-and-grounded-in-canon`, moved ground.
  - "Spec moves to t2, which raises the case to r4" is now its own bullet, and the evidence ground `check-1` expiring at an unchanged `r3` is separate.
  - The two ways an entry stops applying can be told apart.
- `requests-from-canon-pending-decisions`, record keys.
  - Two capabilities give two `record_key`s under one `question_key`, and one capability with two requirements gives one request.
  - Both match routing's key definitions.
- `approvals-supplied-to-canon-evaluation`, grounds check.
  - A `path_line` bullet shows the record withheld, Canon reporting approval-required, and the request Superseded.
  - The same check holds for the tree in `tree-waits-open-decision-requests`.

What I could not establish:
- The Canon-dependent bullets of stories 17 to 19, including whether Canon excludes an evidence ground as `expired` at an unchanged case revision (ledger § Open 1). This is upstream by design and did not set the verdict.
- `story:silence-and-expiry-at-a-host-tick` and `story:decision-inbox-commands` use "a test passes a fixed clock", which I took as an SDK-level test. I did not check that a CLI test can do this.
- `story:selector-stop-names-set-aside-record` and `story:tree-run-command` of the other epic, beyond the `--at` flag.

Out of my lane, and they did not set the verdict:
- Design: `story:decision-routing-by-holder` defines options and a `record_key` for approval, outcome and question stops only. Story 20 routes a review stop through it, and its "second `tree run` opens no other request" bullet needs a review key that neither story states.
- Design: routing's preamble says a decider failure under `rule_and_decider*` "is a veto" (`decision-routing-by-holder.md:75`). Its `rule_and_decider_final` row and bullets decide a failure on the denying option with route `Decided`, and a veto ends the run (`RunEnds`) as refused. The bullets are explicit, so the check is still determinate.
- Design: `tree-waits-open-decision-requests` says an outcome decided "leave it untaken" opens no new request at that revision. Routing reuses only an Open or Escalated request per record key, so the two read differently.

```findings
- file: .engineering/planning/story/approval-decisions-resume-held-runs.md
  line: 155
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the two command-line expiry bullets fix the run's instant at 12:00 and expect expires_at 13:00 and ticks at 12:59 and 13:00, but b10x-loom run reads HostClock (crates/loom-cli/src/main.rs:1200) and neither run nor the decision flags take an instant, so expires_at is the wall-clock time plus 3600 and the bullets cannot be built; add a way to fix the instant or make the bullets tick relative to the printed expires_at"
- file: .engineering/planning/story/non-independent-review-becomes-decision-request.md
  line: 131
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet sets the decider's kind so that its principal equals llm-node:impl, but a decider's principal is always decider:<kind> (condition 1 and story:llm-decider), so the principal refusal has no reachable test; the fixture needs a set-aside record whose producer is a decider:<kind> principal"
- file: .engineering/planning/story/non-independent-review-becomes-decision-request.md
  line: 134
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet covers the run-log branch of condition 1 (a decider that produced a recorded revision of the subject), the run-log branch of condition 2, or ModelDecider reporting its route's model and adapter, so each can be dropped with every bullet green"
```
