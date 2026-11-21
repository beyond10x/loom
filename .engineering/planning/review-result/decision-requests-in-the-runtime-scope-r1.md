---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-scope-r1
kind: review-result
status: active
title: Plan critic scope, round 1, on epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:approval-decisions-resume-held-runs
- reviews: story:approvals-supplied-to-canon-evaluation
- reviews: story:authority-records-bound-and-grounded-in-canon
- reviews: story:autonomy-grant-table
- reviews: story:decided-records-carry-grounds-to-canon
- reviews: story:decided-record-written-then-recorded
- reviews: story:decider-port-and-grounding-check
- reviews: story:decision-inbox-commands
- reviews: story:decision-request-entity
- reviews: story:decision-request-file-store
- reviews: story:decision-routing-by-holder
- reviews: story:decision-suspension-reason
- reviews: story:escalate-when-conditions
- reviews: story:grounds-resolve-against-held-cases
- reviews: story:llm-decider
- reviews: story:question-and-outcome-decisions-hold-runs
- reviews: story:requests-from-canon-pending-decisions
- reviews: story:silence-and-expiry-at-a-host-tick
- reviews: story:tree-waits-open-decision-requests
revision: 1
---
needs-revision

- `epic:decision-requests-in-the-runtime` — "The run's `escalate_when` conditions send a decider's result to a person, or end the run under a final holder" is claimed only up to the router, which answers `RunEnds`, and none of the host stories (13, 14, 15) ends a run on it. `story:approval-decisions-resume-held-runs` is the most natural owner, with a line each in 14 and 15. — `.engineering/planning/epic/decision-requests-in-the-runtime.md:22-23`, `.engineering/planning/story/decision-routing-by-holder.md:78`
  - Quote: "end the run under a final holder" (the same words also appear in the epic's `escalate_when` paragraph).
  - `RunEnds` leaves the request Open with `escalation_suppressed` recorded. It comes up at `decider_final` and `rule_and_decider_final` (a withheld rule verdict, or a failure with no denying option), at `rule_final`, and under any `escalate_when` condition that holds.
  - `DecisionAuthority` is specified to answer only `Allow`, `Deny` or `ApprovalRequired`. `DecisionHoldingExecutor` and `tree run` say nothing about what they do on `RunEnds`.
  - Answer 3 says a veto under `rule_and_decider_final` "ends the run as refused (exit 6 today)". No host story names that exit or end state.
  - No acceptance in stories 13 to 15 drives a run to `RunEnds`. The epic acceptance bullet "under `decider_final` it ends the run with one `escalation_suppressed` of cause `condition`" can only be met by a router fixture.
  - A request left Open with a held run and nothing to resume it is the opposite of "ends the run".
- `epic:decision-requests-in-the-runtime` — "A request that nobody answers expires at a host tick, or is decided with its default on silence" is claimed only at the tick, and no story gives a host-run request its `expires_at`. `story:approval-decisions-resume-held-runs` and `story:tree-waits-open-decision-requests` take the decision flags. `story:autonomy-grant-table` holds the classes document, with no expiry member. — `.engineering/planning/epic/decision-requests-in-the-runtime.md:23-24`, `.engineering/planning/story/decision-routing-by-holder.md:6-12`
  - The router takes "an optional expiry instant" in `DecisionStop` (`story:decision-routing-by-holder`, Outcome).
  - Story 13's flags (`--autonomy-level`, `--autonomy-presets`, `--decision-classes`, `--admitted`, `--override`, `--escalate-when`, `--principals`, `--decider`) carry no expiry setting. Story 15 reuses them, and story 14 builds the same stops.
  - So a request opened by `run` or `tree run` never gets `expires_at`, and `decisions tick` touches nothing in a real run. Only the library fixtures of stories 12 and 14 reach expiry.
  - Nothing in the epic's Not in scope or Upstream says this part was dropped.
  - The fix is to name the source: a flag, a classes-document member, or a stated decision that expiry is fixture-only for now.
- `story:tree-waits-open-decision-requests` — the `AwaitingPerson` case (an action bound to a `person` executor, turned into an approval request whose record is evidence written by `PersonActionWriter`) is a fourth kind of stop that neither the request, the answers, nor the epic Outcome asks for. Either the epic Outcome names it or the story drops it. — `.engineering/planning/story/tree-waits-open-decision-requests.md:28-30`, `.engineering/planning/epic/decision-requests-in-the-runtime.md:15-17`
  - The epic Outcome lists three stops: "an approval-required action, a handed-off question, or an outcome that needs an explicit decision". The request and both answer files name the same three kinds, plus the budget and limit classes, which are in the other epic.
  - Only the one-line Stories entry, "each wait of `tree run`", can be read as covering it.
  - A person-bound action is work, not a decision. Routing it by holder lets a decider or rule holder produce the evidence that stands for a person's work (story line 43: "A decider or rule holder decides in the same `tree run`").
  - Whether that is wanted is not for this critic, but nobody asked for it.

What I read: 21 artifacts (the epic, 19 stories, the blocker) with `aep plan artifact show` on each, plus both answer files, `~/beyond10x/sdc/docs/decisions.md`, `autonomy-levels.md` and `ess/domains/decision.yaml` (grep only, for reuse and expiry), and the critic procedure and rubric. I extracted 46 promises from the request, the epic Outcome and Acceptance, and the two answer files. 42 trace to a story. 2 are named as held in the other epic (the budget and limit requests, and the file-change story). 2 are traced only in part (the two findings above).

What I could not establish:
- Nothing else in the set claims a promise outright. Prior-decision reuse (story 4) is the reference behaviour of `decision-grounds.sh` and is described in `decisions.md:134-137`, so I did not call it reach. Carrying grounds on explicit decisions (story 17) follows from Canon's side and is named in the epic Stories list.
- Out of my lane, not counted in the verdict:
  - Story 15 prints `decisions decide … --tree <STORE>`. Neither it nor story 13 says that `decisions` accepts `--tree` or which writers it then uses. Story 13 states `--state` and `GrantRecordWriter` explicitly. Acceptance and design critics should look at this.
  - `AutonomyChanged` in story 3 has no caller in the set.
  - Answer e's "the request is superseded" is delivered by "the next routing" (story 9). I took that as a named decision, not a narrowing.
  - Story 3 is the only story that cites `escalate_when` in an Out of scope line while its "ESS first" is extended by story 8. That is a parallel-safety question.

```findings
- file: .engineering/planning/epic/decision-requests-in-the-runtime.md
  line: 23
  category: scope
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "\"end the run under a final holder\" is promised and only the router (story:decision-routing-by-holder) answers RunEnds; no host story (approval-decisions-resume-held-runs, question-and-outcome-decisions-hold-runs, tree-waits-open-decision-requests) says what the slice or the tree does on RunEnds or ends a run as refused (answer 3, exit 6), so the end-the-run half of escalate_when and the final holders is unclaimed at the host level"
- file: .engineering/planning/epic/decision-requests-in-the-runtime.md
  line: 23
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"A request that nobody answers expires at a host tick, or is decided with its default on silence\" is claimed only by the tick; no story gives a request opened by run or tree run its expires_at (the decision flags, the classes document and DecisionStop callers carry no expiry), so expiry is reachable only in fixtures and the epic does not say it was dropped"
- file: .engineering/planning/story/tree-waits-open-decision-requests.md
  line: 28
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the AwaitingPerson case (a person-executor action routed as an approval request whose record is evidence, and decidable by a decider or rule holder) is a fourth kind of stop that the request, both answer files and the epic Outcome do not name; the epic Outcome lists approval, question and outcome only, so the story should drop it or the epic should name it"
```
