---
format: aep.planning-md/3
id: approval-record:sdc-requests-accepted-by-default-2026-11-21
kind: approval-record
status: approved
title: Epics and stories drafted from a request of session sdc are accepted once reviewed and scoped, from 2026-11-21
summary: 'Session sdc, as orchestrator: an sdc-requested artifact moves to proposed by itself after its critic rounds and scoping; wave order, other requesters and releases are not covered'
tags:
- sdc
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-11-21T02:28:32Z", actor: "agent:sdc", revision: 3, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## What was decided

Session `sdc`, orchestrator of session `loom`
(`approval-record:sdc-orchestrates-loom-2026-11-21`), decided on 2026-11-21: every epic and story
session `loom` drafts from a request of session `sdc` is accepted by default, without a separate
proposal to `sdc` or to the operator.

## The words

- The operator to session `sdc`, 2026-11-21, as `sdc` relayed it (not heard by session `loom`
  directly): "Just note it down as a standing rule that all the stories you're basically creating
  here are accepted, and all the ones you're ad hoc sending to the Loom or the Canon session, they
  should be also automatically accepted by default, because we don't want to wait on this at all."
- Session `sdc` to session `loom`, the same day: "every epic and story you draft from a request of
  the sdc session is accepted by default, without a separate proposal to me or to him. Your own
  rules still bind, as you said they bind my instructions: a story becomes `proposed` once it has
  passed your critic panel and is scoped, and at that point it moves by itself; you tell me what
  moved in your next status message."

## How session `loom` applies it

- An artifact tagged `sdc` moves from `draft` to `proposed` when its critic rounds are recorded
  with their outcomes and its Scope is written (typed scope included). Session `loom` makes the
  move and names it in its next status message to `sdc`.
- The move rests on `sdc`'s authority to accept stories under the orchestrator record. Session
  `loom` did not hear the operator's sentence itself; if the operator says otherwise in session
  `loom`, his word there replaces this record.
- Applied on 2026-11-21 to: `epic:decision-requests-in-the-runtime`,
  `epic:concurrent-and-standing-case-trees`, the observation-collection epic once drafted,
  `story:run-log-references-exec-file-changes`, `story:selector-stop-names-set-aside-record`,
  `story:budget-and-limit-stops-raise-decision-requests`,
  `story:non-independent-review-becomes-decision-request`.

## Not covered, in `sdc`'s words

"the order of waves when two epics compete for capacity (ask me, or follow the order I gave),
stories from other requesters (ai-agent-platform's boundary epic keeps its own path), releases,
tags, pushes, version bumps."

- A wave still starts on `sdc`'s decision, and its commits are the ones that decision lists.
- An artifact with an open blocker, or whose acceptance needs a decision that is the operator's
  (security, anything outward-facing or hard to reverse), stays `draft` and is put to him.

## Until

Session `sdc` or the operator says otherwise.
