---
format: aep.planning-md/3
id: approval-record:loom-d409-fix-release
kind: approval-record
status: approved
title: 'Loom 0.55.1 fix release accepted under the orchestrator grant (D409): one forced call, refused effects observed, budget stop named'
tags:
- non-interactive
relations:
- decides: story:forced-call-asks-for-one-call
- decides: story:refused-effect-goes-back-to-the-model
- decides: story:turn-budget-stop-names-the-arithmetic
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-03T06:23:11Z", actor: "human:timo", revision: 2}
---
Accepted under the orchestrator's grant, item D409, 2026-11-03, as a fix release alone under D169
(a consumer is blocked today: a downstream adopter's first live agent passed 0 of 19 runs on
Loom 0.55.0). Its words:

- "deliver a loom fix release (next patch after 0.55.0, expected 0.55.1) for three defects a
  downstream adopter hit on loom 0.55.0."
- A: "the model called `action_arguments` more than once" ... "Candidate fix (inferred, not run):
  parallel_tool_calls=Some(false) on the forced select_action and action_arguments requests.
  Verify the mechanism in code first. Decide yourself whether answer() should also take the first
  of several identical calls; the contract must stay safe."
- B: "Fix: a refused read or effect argument goes back to the model as an observation it can
  correct within the budget. If the run must stop, the reason names the refusal. A step that needs
  approval stays gated as before."
- C: "Loom adds a clear error, or a validation guard at spec or run start that names the
  arithmetic. Pick the smaller safe change."
- "ESS pin: ... move loom's ESS crates and pin to 0.84.0 in this change if the move is mechanical
  (builds, targeted tests green). If it needs more than that, skip it and say so."
- "Do not merge, tag or release before I send the go."

Decisions taken inside the item:

- A: `answer()` takes calls that carry the same arguments as one answer; calls that differ stay
  unusable. Identical calls name one choice, so nothing is guessed.
- C: the clear error, no spec guard: the adopter's budgets (8 steps, 16 calls) are consistent, so a
  guard would refuse nothing in the case reported.

Delivery: one branch `fix/loom-0.55.1` from `origin/main` (00dc7fe8), the store commit, the unit
commits, the release-prepare commit (0.55.1), one bot PR to main. Merge, tag and release wait for
the orchestrator's go.
