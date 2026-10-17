---
format: aep.planning-md/3
id: review-result:model-routes-acceptance-round-1
kind: review-result
status: active
title: Plan critic acceptance, epic:model-routes, round 1
relations:
- reviews: epic:model-routes
- reviews: story:llm-routes-pin
- reviews: story:sdk-models-from-config
- reviews: story:cli-run-config
- reviews: story:docs-model-routes
revision: 1
---
needs-revision

story:llm-routes-pin — the acceptance "`b10x-llm-models` is built with its `environment` and `file` features where `loom-sdk` depends on it" is vacuously true at this story, because `crates/loom-sdk/Cargo.toml` depends only on `b10x-llm-core` (line 21) and this story adds no `models` dependency; the bullet belongs to `story:sdk-models-from-config` or must name the dependency this story adds — .engineering/planning/story/llm-routes-pin.md:50
story:llm-routes-pin — the acceptance never observes that `loom.models.ModelsConfig`, `RoleModel` or `ModelRole` exist: `ess specify validate` exits 0 on an empty `models.yaml`, and no check says `generated/rust/loom` holds the types the Outcome promises and `story:sdk-models-from-config` depends on — .engineering/planning/story/llm-routes-pin.md:52
story:cli-run-config — `a_role_neither_configured_nor_given_is_refused_before_any_model_call` is a bare test name with no stated observable (exit status, error text naming the role, or zero requests at the stub), so "refused" cannot be checked the same way twice — .engineering/planning/story/cli-run-config.md:43
story:docs-model-routes — the acceptance checks only that the page exists, builds and has parseable files; nothing observes the content the Outcome lists (gateway bearer and anonymous, Anthropic direct, OpenRouter direct, the `--config` line, overrides, both SDK calls), so a page with one trivial route passes — .engineering/planning/story/docs-model-routes.md:30

What I read: 5 ids given (`epic:model-routes` and the 4 stories), 5 read in full with `aep plan artifact show <id>`, plus `aep plan artifact kinds`. I checked the tree with `grep` and `ls` on the `Cargo.toml` manifests, `ess/domains`, `crates/loom-docs`, `model_catalog.rs:111` and `consumers/pinned`.

What I could not establish:
- Whether `llm_core::Model` exposes an "identity" that `a_config_names_the_model_of_each_role` can compare. llm 0.6.0 is not in the tree, so I did not judge `story:sdk-models-from-config`'s first bullet.
- I did not run `aep plan artifact lifecycle` and did not run the CLI validator.
- Out of my lane: `story:docs-model-routes` has `crates/loom-docs` and a `Cargo.toml` dev-dependency in scope, and `story:llm-routes-pin` edits every `Cargo.toml`. That is a shared-surface question for parallel-safety, and it does not set my verdict.

```findings
- file: .engineering/planning/story/llm-routes-pin.md
  line: 50
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance 'b10x-llm-models is built with its environment and file features where loom-sdk depends on it' is vacuously true at this story, because loom-sdk depends only on b10x-llm-core and this story adds no models dependency; the bullet belongs to story:sdk-models-from-config or must name the dependency this story adds"
- file: .engineering/planning/story/llm-routes-pin.md
  line: 52
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance never observes that loom.models.ModelsConfig, RoleModel or ModelRole exist: validate exits 0 on an empty models.yaml and no check says generated/rust/loom holds the types the Outcome promises"
- file: .engineering/planning/story/cli-run-config.md
  line: 43
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "a_role_neither_configured_nor_given_is_refused_before_any_model_call is a bare test name with no stated observable (exit status, error text naming the role, or zero requests at the stub)"
- file: .engineering/planning/story/docs-model-routes.md
  line: 30
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance checks only that the page exists, builds and has parseable files; nothing observes the content the Outcome lists (gateway bearer and anonymous, Anthropic direct, OpenRouter direct, the --config line, overrides, both SDK calls)"
```
