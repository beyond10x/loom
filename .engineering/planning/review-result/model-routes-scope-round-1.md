---
format: aep.planning-md/3
id: review-result:model-routes-scope-round-1
kind: review-result
status: active
title: Plan critic scope, epic:model-routes, round 1
relations:
- reviews: epic:model-routes
- reviews: story:llm-routes-pin
- reviews: story:sdk-models-from-config
- reviews: story:cli-run-config
- reviews: story:docs-model-routes
revision: 1
---
approve

I extracted 9 promises from the epic and traced all 9 to an item. I found no gap, no reach beyond the epic, no duplicate claim and no silent narrowing.

**What I read:** 5 artifacts (the epic and its four stories). Commands: `aep plan artifact show` on each, then `aep plan artifact graph`, `aep plan artifact kinds` and `aep plan artifact relations`.

Promises and the items that claim them:
- One config names an `llm.routes/1` document and a route per role: `story:llm-routes-pin` (the `loom.models.ModelsConfig` type) and `story:sdk-models-from-config` (the loader reads it).
- `run --config loom.yaml` runs on those routes: `story:cli-run-config`.
- Credentials resolve from `secret:` references through llm's resolvers: `story:sdk-models-from-config`, which `story:cli-run-config` uses.
- `loom_sdk::models` returns `llm_core::Model`, via `from_config(path)` and `from_routes(path, name)`: `story:sdk-models-from-config`.
- The routes path is relative to the config file: `story:sdk-models-from-config`.
- `--model` and `--classifier-model` each override one role: `story:cli-run-config`.
- Docs show a gateway (bearer and anonymous), Anthropic direct and OpenRouter direct: `story:docs-model-routes`.
- The llm pin goes from 0.4.0 to 0.6.0: `story:llm-routes-pin`.
- The `loom.models` ESS domain: `story:llm-routes-pin`.

The epic's exclusions are respected. `story:cli-run-config` leaves out `plugin run`, `resume` and removing `--catalog`. No item adds a second-target fallback or a credential Loom reads itself. The graph shows nothing outside the set that claims part of this epic.

**What you could not establish:**
- The epic's "Why" lists `keychain:` among the `secret:` kinds, but the pin and SDK stories build only the `environment` and `file` features. Whether keychain support is a Loom promise or only a description of llm 0.6.0 is the operator's call, so I did not count it as a finding.
- The "refused before any model call if the route cannot take a forced tool call" acceptance in `story:cli-run-config` has no sentence in the epic. It matches existing `--catalog` behaviour, so I treated it as parity and not as reach.
- Whether each acceptance can be checked, the cross-story dependency ordering, and whether `story:docs-model-routes` and `story:cli-run-config` can run in parallel belong to other critics, and I did not assess them.

```findings
[]
```
