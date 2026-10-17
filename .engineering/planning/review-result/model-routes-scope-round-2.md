---
format: aep.planning-md/3
id: review-result:model-routes-scope-round-2
kind: review-result
status: active
title: Plan critic scope, epic:model-routes, round 2
relations:
- reviews: epic:model-routes
- reviews: story:llm-routes-pin
- reviews: story:sdk-models-from-config
- reviews: story:cli-run-config
- reviews: story:docs-model-routes
revision: 1
---
approve

I extracted 9 promises from the epic and traced all 9 to an item. I found no gap, no reach beyond the epic, no duplicate claim and no silent narrowing in revision 4 of the four stories.

Promises and the items that claim them:
- One config names an `llm.routes/1` document and a route per role: `story:llm-routes-pin` (the `loom.models.ModelsConfig` type) and `story:sdk-models-from-config` (the loader reads it).
- `run --config loom.yaml` runs on those routes: `story:cli-run-config`.
- Credentials resolve from `secret:` references through llm's resolvers, with Loom reading no secret: `story:sdk-models-from-config`, which the CLI story uses.
- `loom_sdk::models::from_config(path)` and `from_routes(path, name)` return `llm_core::Model`: `story:sdk-models-from-config`.
- The routes path is relative to the config file: `story:sdk-models-from-config`.
- `--model` and `--classifier-model` each override one role: `story:cli-run-config`.
- Docs show a gateway (bearer and anonymous), Anthropic direct and OpenRouter direct: `story:docs-model-routes`.
- The llm pin goes from 0.4.0 to 0.6.0: `story:llm-routes-pin`.
- The `loom.models` ESS domain: `story:llm-routes-pin`.

The epic's exclusions hold. `story:cli-run-config` leaves out `plugin run`, `resume` and removing `--catalog`. No item adds a fallback target or a credential Loom reads itself. The graph shows no other artifact claiming part of this epic.

I treated these additions as consequences of a promise, not as reach:
- The regenerated reference pages and `CHANGELOG.md` entries in the pin story follow from the ESS domain and the pin.
- The `host_ports.rs` and SDK stability-page entries follow from "the SDK offers the same loader".
- The `loom-docs` drift test in `story:docs-model-routes` follows from the docs promise.

**What you read:** 5 artifacts (the epic, the four stories at revision 4) and the round-1 record `review-result:model-routes-scope-round-1`. Commands: `aep plan artifact show` on each, `aep plan artifact graph`, and `ls` of `crates` and `website/docs/guides` to confirm `loom-docs` and the guides directory exist.

**What you could not establish:**
- The epic's "Why" lists `keychain:` among the `secret:` kinds, but the SDK story builds only the `environment` and `file` features. The epic's "The shape (decided)" section does not promise keychain, so I did not count it as a finding. This is the same call as in round 1.
- The `cli-run-config` acceptances for a missing `classifier` role and for a route that cannot take a forced tool call have no sentence in the epic. The tool-call refusal matches `--catalog` behaviour, and the missing-role refusal follows from the epic's two-role config, so I read both as parity.
- Out of my lane and not weighed: whether each acceptance is checkable, the `Ready` ordering across stories, and whether `story:docs-model-routes` and `story:cli-run-config` can run in parallel.

```findings
[]
```
