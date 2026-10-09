---
format: aep.planning-md/3
id: review-result:plugin-layer-scope-round-1
kind: review-result
status: active
title: Scope critic, plugin-layer, round 1
relations:
- reviews: epic:plugin-layer
- reviews: story:plugin-measurement
- reviews: story:slack-plugin
- reviews: architecture-decision-record:plugin-hooks
revision: 1
---
needs-revision

- `epic:plugin-layer` — the ADR promises "what generalises is expressed as YAML (a protocol checked by Canon, and a YAML-only plugin, `story:yaml-only-plugin`)". The epic names it neither as covered nor as deferred, and the store has no `story:yaml-only-plugin`. Say in the epic that this part is deferred, or claim it in an item. `story:plugin-measurement` is the natural place for the note — `.engineering/planning/architecture-decision-record/plugin-hooks.md:27`
- `story:plugin-measurement` — the acceptance "`story:yaml-only-plugin` cites the page" claims a change to an artifact that is not in the store and not in this story's Scope. The Why also leans on `story:reference-enrichment`, which is absent too. Either drop the line or create the stories as deferred items. Today it cannot be met — `.engineering/planning/story/plugin-measurement.md:29`
- `story:slack-plugin` — "channel allow and deny lists" in the config have no source in the epic or the ADR. Neither asks for channel filtering beyond "the Slack channels its bot belongs to". Remove them or cite the operator's words — `.engineering/planning/story/slack-plugin.md:31`

What I read: 7 artifacts, using `aep plan artifact show` on the epic, the ADR and the five stories, plus `aep plan artifact graph`. The epic and ADR gave 18 promises. The epic has 8 in the Outcome and 4 acceptance lines; the ADR adds 6 hooks and the read-only, governed-turn, Connectors, state-outside-workspace and measure-then-YAML promises. I traced 17 to an item. The one gap is the YAML-only plugin, an ADR promise the epic never excludes. All five operator asks are claimed: read-only first, the pipeline, the five data-source verbs, "see how much code", and long-horizon objectives (the `objectives` hook in the host, weights in the Slack config).

What I could not establish:
- The drafter's report was not given, so I could not tell whether the YAML-only plugin was deliberately deferred.
- Nothing checks that the objectives ("learn about dev stack" and the others) influence polling. That is the acceptance critic's lane and does not set my verdict.
- `story:plugin-host` has the task path name "the protocol `loom-intake-router` picks", which neither the epic nor the ADR mentions. It is probably needed to produce the third proposal, so I did not raise it.
- Out of my lane, design: the epic's `plugin run slack-handler --config <file>` "without further input" omits the `--state <dir>` that `story:plugin-host` requires.

```findings
- file: .engineering/planning/architecture-decision-record/plugin-hooks.md
  line: 27
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "epic:plugin-layer — the ADR promise that what generalises is expressed as YAML through a Canon-checked protocol and `story:yaml-only-plugin` is neither claimed by an item nor recorded as deferred in the epic, and no such story exists in the store; story:plugin-measurement would most naturally record it"
- file: .engineering/planning/story/plugin-measurement.md
  line: 29
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:plugin-measurement — the acceptance '`story:yaml-only-plugin` cites the page' claims a change to an artifact that does not exist in the store and is outside the story's Scope (which lists only the measurement page); the Why also relies on the absent `story:reference-enrichment`"
- file: .engineering/planning/story/slack-plugin.md
  line: 31
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:slack-plugin — config 'channel allow and deny lists' traces to no sentence in epic:plugin-layer or architecture-decision-record:plugin-hooks, which ask only for the channels the bot belongs to"
```
