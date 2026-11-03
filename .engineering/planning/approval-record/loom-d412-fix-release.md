---
format: aep.planning-md/3
id: approval-record:loom-d412-fix-release
kind: approval-record
status: approved
title: 'Loom 0.55.2 fix release accepted under the orchestrator grant (D412): per-step record, the answer before the budget ends, a transport failure retried then observed'
tags:
- non-interactive
relations:
- decides: story:turn-records-each-step
- decides: story:turn-answers-before-its-budget-ends
- decides: story:http-read-transport-failure-goes-back
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-03T09:24:07Z", actor: "human:timo", revision: 2}
---
Accepted under the orchestrator's grant, item D412, 2026-11-03, as a fix release alone under D169
(a consumer is blocked: a downstream adopter's scheduled runs on Loom 0.55.1 passed 0 of 2). Its
words:

- "loom 0.55.2 fix release. It goes alone under D169 because the adopter is blocked: its scheduled
  runs on 0.55.1 passed 0 of 2."
- "Scope, three stories, accepted through one approval-record citing D412. Carry branch
  plan/d411-consumer-report-drafts into this PR."
- 1: "the run state records each step with its action, effect outcome, refusal or error, and the
  budget left. If a log line per step is cheap, emit that too."
- 2: "when the remaining steps or model calls only cover the answer, the turn asks for the answer.
  Otherwise it ends naming that it ran out before answering. Reproduce first; if the per-step
  record shows the refused-argument retries of fix B use up steps, cover that."
- 3: "a read whose transport fails (connect, send or timeout; not an HTTP status the binding maps)
  is retried once. If it still fails, the failure goes back to the model as an observation within
  the step budget, as a refused argument does. Only a final failure ends the run, and the run
  names the endpoint and the error. Effects that are not idempotent reads are not retried."
- "when the ESS version moves in a workflow, its ESS_SHA256 must move too. Add a check, or put a
  note in loom's AGENTS.md release steps, whichever is smaller."
- "Do not merge before my go."

Delivery: one branch, `fix/loom-0.55.2`, from `origin/main` (b2b2a9f1) through
`plan/d411-consumer-report-drafts`. It carries the store commit, the unit commits and the
release-prepare commit (0.55.2), and goes to main as one bot PR. Merge, tag and release wait for
the orchestrator's go.
