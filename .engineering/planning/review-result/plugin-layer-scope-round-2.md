---
format: aep.planning-md/3
id: review-result:plugin-layer-scope-round-2
kind: review-result
status: active
title: Scope critic, plugin-layer, round 2
relations:
- reviews: story:connectors-cli-reads
- reviews: story:slack-plugin
- reviews: architecture-decision-record:plugin-hooks
revision: 1
---
needs-revision

- `story:connectors-cli-reads` — the ADR decides "Data sources are read through Connectors, behind Commission's `ConnectorInvoker`", and no item builds, binds or uses a `ConnectorInvoker`. The title still says "behind ConnectorInvoker", but the body (revision 3) is a plain CLI client and `story:plugin-host`'s `DataSourceEffects` calls `ConnectorsCli::read` directly. The promise is narrowed with nothing recording the decision, so the story should either claim the invoker for the turn's reads or say it deviates from the ADR and why. The title should change either way. — `.engineering/planning/architecture-decision-record/plugin-hooks.md:22`, `.engineering/planning/story/connectors-cli-reads.md:6` and `:34`
- `story:slack-plugin` — config `min_age_minutes` and `seed` (with the seeded channel order) trace to no sentence in the epic, the ADR or the operator's words. The epic asks only for "the Slack channels its bot belongs to" and the `objectives` hook gives "weights for the next poll". Round 1 removed the allow and deny lists for the same reason, and these two were in the same config line. Remove them, or quote where the operator asked. — `.engineering/planning/story/slack-plugin.md:46-47`, `:55-56`, `:61`

What I read: 7 artifacts shown whole (the epic, the ADR and the five stories), plus `review-result:plugin-layer-scope-round-1` and the design round-1 review for context. Commands: `aep plan artifact show` on each, `aep plan artifact graph`, `aep plan artifact kinds`, and `git`/`ls` on the w1 wave trees to check the deferred idea stories. I extracted 24 promises (8 epic outcome, 7 epic acceptance, 9 ADR decision and consequences) and traced 22 to an item. One is deferred and named (the YAML last step). One is untraced (`ConnectorInvoker`). All five operator asks are claimed: read-only first, the pipeline, the data-source verbs (`sources()` plus `ReadKind` list, search, get), "see how much code", and objectives (the `objectives` hook, and `mention_comes_first` for "respond when tagged").

Round-1 findings:
- **YAML-only plugin gap:** resolved. The epic now has a § Deferred (`.engineering/planning/epic/plugin-layer.md:36-40`), and the measurement story no longer claims `story:yaml-only-plugin`.
- **Measurement story:** its acceptance line on `story:yaml-only-plugin` is gone.
- **Allow and deny lists:** resolved, they are gone from the config.

What I could not establish:
- `story:yaml-only-plugin` and `story:reference-enrichment` do not exist in this tree. They exist only on `wave/2026-10-09-w1` (commit 546dd4e, not an ancestor of this branch). The epic names them with that wave, so I treat the deferral as honest. The names will not resolve here until w1 merges.
- The conductor `charters/loom.md` the stories cite is not on disk at `~/beyond10x/conductor/charters`, so I could not check the operator's approved plan.
- Unease, not a finding: `story:plugin-host`'s task path (a second model call that picks a protocol from `ProtocolCatalog::bundled()`) is in neither the epic nor the ADR. The epic's "3 proposals" and "one record line per item" are the only grounding, so I left it, as round 1 did.
- Out of my lane: nothing checks that "learn about dev stack" and "help people with cheap lookups" change any behaviour beyond the weights. That is acceptance or design.

```findings
[
  {"file": ".engineering/planning/architecture-decision-record/plugin-hooks.md", "line": 22, "category": "scope", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "story:connectors-cli-reads — the ADR promise that data sources are read 'behind Commission's ConnectorInvoker' is claimed by no item (the stories use a plain ConnectorsCli and a plugin-host DataSourceEffects) while the story title still says 'behind ConnectorInvoker'; claim the invoker for the turn's reads or record the deviation from the ADR in the body and fix the title (.engineering/planning/story/connectors-cli-reads.md:6, :34)"},
  {"file": ".engineering/planning/story/slack-plugin.md", "line": 46, "category": "scope", "severity": "warning", "verdict": "needs-revision", "origin": "pre-existing", "message": "story:slack-plugin — config `min_age_minutes` and `seed` (and the seeded channel order and `young_message_is_not_an_item`, lines 55-56 and 61) trace to no sentence in epic:plugin-layer, the ADR or the operator's words, which ask only for the channels the bot belongs to and objective weights for the poll; remove them or cite the operator"}
]
```
