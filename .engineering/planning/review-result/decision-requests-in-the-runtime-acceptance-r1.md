---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-acceptance-r1
kind: review-result
status: active
title: Plan critic acceptance, round 1, on epic:decision-requests-in-the-runtime
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

- `story:decision-request-file-store` — the Outcome says the store keeps later-appended records such as `EscalationSuppressed` beside each request in the same file, but no Acceptance bullet writes one and reads it back across a restart or a state-conflict refusal — `.engineering/planning/story/decision-request-file-store.md:25` against `:49`
- `story:autonomy-grant-table` — the Outcome's transitions have no bullet: a second resolution with another level giving `AutonomyChanged` (from, to, by), a non-eval run with no level refused `NoLevel`, and `holder(class)` on an undeclared class refused `UnknownClass`; the only level-less bullet is the eval-run one that resolves at `A5` — `.engineering/planning/story/autonomy-grant-table.md:60` and `:110` against `:89`
- `story:decider-port-and-grounding-check` — the Outcome lists eight grounding refusals and a `Take` reply, but the Acceptance exercises five refusals. It never runs a default on silence that is not strong, a ground supporting a letter that is not an option, or `check_grounds` over a `Take` reply. A `RuleDecider` or `check_grounds` that skips them still passes — `.engineering/planning/story/decider-port-and-grounding-check.md:46` against `:77`
- `story:grounds-resolve-against-held-cases` — "gives the three outcomes of the hand-made requests" names no outcome, and the cited `decisions.md:120-128` is the grounding rule, not the table. The table of hand-made requests is at `decisions.md:145-151` and has four rows (valid, dangling, prior reuse, wrong digest), so "three" cannot be checked without guessing which one is left out — `.engineering/planning/story/grounds-resolve-against-held-cases.md:82`
- `story:decision-routing-by-holder` — the eight-row holder table is the story's core, yet no bullet drives these paths:
  - `person` recording a reusable prior decision as the recommendation;
  - `rule` deciding on a verified rule proposal, or escalating otherwise;
  - the stop's options by kind: an outcome's `A` take / `B` leave untaken, and a question's host options;
  - the `DR-<case>-<action>-<n>` numbering and `question_key` form.

  Only `budget_overrun` under `rule_final` is exercised — `.engineering/planning/story/decision-routing-by-holder.md:105`
- `story:escalate-when-conditions` — the Outcome applies conditions to four holders, and keeps the list in the grant table and `AutonomyResolved`. The bullets run `A3` (`decider`) and `A5` (`decider_final`) only. No bullet covers `rule_and_decider` (escalate to a person) or `rule_and_decider_final` (the run ends), a single-segment `*` glob, or the list appearing in `AutonomyResolved` — `.engineering/planning/story/escalate-when-conditions.md:21` and `:73`
- `story:decision-inbox-commands` — the Outcome names these, and no bullet runs them:
  - `decisions supersede`;
  - `decisions history --artifact`;
  - `decisions show` printing the `EscalationSuppressed` records;
  - the `--at` instant override.

  Only `decisions list`, `decide`, `take` and `escalate` are driven — `.engineering/planning/story/decision-inbox-commands.md:24-25` against `:62`
- `story:question-and-outcome-decisions-hold-runs` — two Outcome transitions have no bullet. One is a Superseded request refused on resume, naming the superseding request. The other is the wrapper not suspending when the route decided the request at once and its record is written, or the option produces none. Only the Expired refusal and the held cases are driven — `.engineering/planning/story/question-and-outcome-decisions-hold-runs.md:34` and `:44` against `:72`
- `story:tree-waits-open-decision-requests` — the Outcome routes three kinds of wait and lists the decision events in the run log, but the Acceptance has no bullet for these:
  - the `AwaitingPerson` approval request written by `PersonActionWriter`;
  - a ground that no longer resolves superseding the request at `tree run`;
  - the `autonomy resolved` line, once per tree in the root's log;
  - the `expired`, `superseded` and `withheld` lines.

  Only `escalation suppressed` and `record produced` lines are checked — `.engineering/planning/story/tree-waits-open-decision-requests.md:28`, `:59` against `:91`

Findings 1 to 9 share one cause: each Outcome states behaviour that no Acceptance bullet would reveal as missing. The Acceptance sections do not say how the story's checks relate to the Outcome. Each story can be closed with the listed behaviours unbuilt and every bullet still green. The fix for each is to add one observable bullet per listed behaviour, or to remove the claim from the Outcome.

What the set gets right: every story and the epic has an Acceptance section that names a fixture, a state before and after, and a refusal or exit status. Every `task check` bullet is observable. The blocked stories 16, 17 and 19 carry acceptance that cannot run today, which the brief allows. The unheld stories (1 to 15 and 18) use fixtures, stub ports and recorded models only. The fixture values of `story:autonomy-grant-table` match `autonomy-levels.md:244-257`. The test names the acceptance cites exist in the tree: `restart_durability.rs`, `crate_names.rs`, `approval_suspend_resume.rs`, `resume_after_grant.rs` and the lockfile test.

What I read: 21 artifacts of 21, the epic, the 19 stories and the upstream blocker. I ran `aep plan artifact show` on each, `aep plan artifact kinds`, `lifecycle story` and `validate` (valid), and compared fixtures with `autonomy-levels.md`, `decisions.md` and the tree.

What I could not establish:
- The brief names the files `e3-sdc-answers.md` and `e3-sdc-answers-2.md`, but I did not read them. Answer letters and point numbers were taken as the epic's Why states them.
- Whether `decisions.md:120-128` shifted when SDC corrected its files. I treated that drift as not a finding, and finding 4 stands on the "three" against four rows.
- Whether the blocked stories' acceptance will run once Canon ships.

Out of my lane, and they did not set the verdict:
- `story:decision-suspension-reason` has its acceptance bullet "a held `Decision` Suspension for it stays held" for `tick` in `story:silence-and-expiry-at-a-host-tick`. That bullet crosses the `b10x-loom-decision` and Commission crates. Whether a test in `b10x-loom-decision` can observe a Commission Suspension is a design question.
- `story:approval-decisions-resume-held-runs` states the `--mandate` refusal under Out of scope, so that is a scope question.

```findings
- file: .engineering/planning/story/decision-request-file-store.md
  line: 25
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says the store keeps later-appended records such as EscalationSuppressed beside each request in the same file, but no Acceptance bullet writes one and reads it back across a restart or a conflict"
- file: .engineering/planning/story/autonomy-grant-table.md
  line: 60
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance has no bullet for the AutonomyChanged record on a second resolution at another level, for NoLevel on a non-eval run with no level, or for UnknownClass from holder(class)"
- file: .engineering/planning/story/decider-port-and-grounding-check.md
  line: 77
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance exercises five of the eight listed grounding refusals and omits a default on silence that is not strong, a ground supporting a letter that is not an option, and check_grounds over a Take reply"
- file: .engineering/planning/story/grounds-resolve-against-held-cases.md
  line: 82
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet 'gives the three outcomes of the hand-made requests' names no outcome, and the hand-made requests table of decisions.md has four rows, so the check cannot be repeated without guessing which is left out"
- file: .engineering/planning/story/decision-routing-by-holder.md
  line: 105
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance never drives the person holder's prior-decision recommendation, the rule holder's decide-or-escalate, the stop's options for an outcome and a question, or the request_id and question_key forms, so the holder table can be closed with rows unbuilt"
- file: .engineering/planning/story/escalate-when-conditions.md
  line: 73
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance runs conditions only under decider and decider_final, never under rule_and_decider or rule_and_decider_final, never a single-segment glob, and never checks that escalate_when lands in AutonomyResolved"
- file: .engineering/planning/story/decision-inbox-commands.md
  line: 62
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance has no bullet for decisions supersede, decisions history --artifact, decisions show printing the EscalationSuppressed records, or the --at override that the Outcome names"
- file: .engineering/planning/story/question-and-outcome-decisions-hold-runs.md
  line: 72
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance has no bullet for a Superseded request refused on resume naming the superseding request, nor for the wrapper not suspending when the route decided at once with its record written or none produced"
- file: .engineering/planning/story/tree-waits-open-decision-requests.md
  line: 91
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance has no bullet for the AwaitingPerson approval request, for a moved ground superseding the request at tree run, or for the autonomy resolved, expired, superseded and withheld run-log lines"
```
