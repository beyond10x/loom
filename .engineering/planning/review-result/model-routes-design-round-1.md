---
format: aep.planning-md/3
id: review-result:model-routes-design-round-1
kind: review-result
status: active
title: Plan critic design, epic:model-routes, round 1
relations:
- reviews: epic:model-routes
- reviews: story:llm-routes-pin
- reviews: story:sdk-models-from-config
- reviews: story:cli-run-config
- reviews: story:docs-model-routes
revision: 1
---
needs-revision

- story:llm-routes-pin — its acceptance (line 50) requires `b10x-llm-models` with `environment` and `file` features "where `loom-sdk` depends on it". `loom-sdk` has only `b10x-llm-core` today, so the line cannot be checked until story:sdk-models-from-config adds that dependency. The reader (pin) is therefore the producer's dependent, and the edge runs the other way. The `b10x-llm-models` dependency and its features are half of the sdk loader and belong in that story, which already lists `crates/loom-sdk/Cargo.toml` in scope. Move the line to story:sdk-models-from-config and drop `crates/loom-sdk/Cargo.toml` from the pin scope except for the tag bump. — .engineering/planning/story/llm-routes-pin.md:50, crates/loom-sdk/Cargo.toml:21, .engineering/planning/story/sdk-models-from-config.md:55
- story:docs-model-routes — its outcome (lines 25-26) cannot be described without the `b10x-loom run --config loom.yaml` line and `loom_sdk::models::{from_config, from_routes}`. Those are produced by story:cli-run-config and story:sdk-models-from-config, and the story depends only on `story:llm-routes-pin`. Its Ready section declares it parallel to both. The page can land first and document a flag and functions that do not exist. Add `depends_on story:cli-run-config`, which reaches the sdk story transitively. Docs then sit beside nothing, but the set stays a tree rather than a queue. The alternative is to cut the page's outcome down to what the pin delivers (the two files and their parse test) and move the CLI and SDK sections into the cli and sdk stories. — .engineering/planning/story/docs-model-routes.md:25, .engineering/planning/story/docs-model-routes.md:41 (`aep plan artifact graph`: docs-model-routes -> llm-routes-pin only)

What I read: 5 artifacts. I ran `aep plan artifact show` on each, plus `relations`, `graph` and `validate`. I also read `crates/loom-docs/src/main.rs`, `crates/loom-docs/Cargo.toml`, `Taskfile`'s `docs-check`, and every `b10x-llm-*` line in `crates/*/Cargo.toml`.
- **Edges:** I walked 8 edges in the set: 5 `decomposes`, 3 `depends_on` and 1 `serves` out to `vision:governed-autonomy`. I checked all 759 edges in the store, and none leaves the set except `serves`. The set has no cycle and no full chain: pin → sdk → cli, and docs hangs off pin.
- **Acceptance reads:** I traced 8 acceptance reads to a producer in the set, and 6 are recorded by an edge. The two that are not are the two findings above.
- **Validate:** it reports only prose-only review-results on other artifacts, which are not findings.

What I could not establish:
- Out of my lane (scope/acceptance critics): `story:llm-routes-pin` adds an ESS domain, and `story:cli-run-config` adds `--config`. `loom-docs generate --check` regenerates `website/docs/reference/ess/*`, `website/data/ess` and `website/docs/reference/cli.md` from those, yet neither scope lists those paths. The cli story's acceptance runs `task docs-check`.
- Out of my lane: the docs test parses `loom.yaml` as `loom.models.ModelsConfig`. The docs scope names only a `b10x-llm-models` dev-dependency, not the generated `loom` crate that holds that type, and `loom-docs` does not depend on it today.
- Out of my lane (parallel-safety): CHANGELOG.md is in three scopes, and `crates/loom-sdk/Cargo.toml` and `crates/loom-cli/Cargo.toml` each appear in two.

```findings
- file: .engineering/planning/story/llm-routes-pin.md
  line: 50
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance requires b10x-llm-models with environment and file features where loom-sdk depends on it, but loom-sdk has no such dependency until story:sdk-models-from-config adds it, so the line is unverifiable here and is half of the sdk story's loader; move the line to story:sdk-models-from-config"
- file: .engineering/planning/story/docs-model-routes.md
  line: 25
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its outcome documents the run --config line and loom_sdk::models::{from_config, from_routes} that story:cli-run-config and story:sdk-models-from-config produce, yet it depends only on story:llm-routes-pin and is declared parallel to both; add depends_on story:cli-run-config, or cut the page to what the pin delivers"
```
