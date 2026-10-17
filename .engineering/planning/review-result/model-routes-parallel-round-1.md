---
format: aep.planning-md/3
id: review-result:model-routes-parallel-round-1
kind: review-result
status: active
title: Plan critic parallel, epic:model-routes, round 1
relations:
- reviews: epic:model-routes
- reviews: story:llm-routes-pin
- reviews: story:sdk-models-from-config
- reviews: story:cli-run-config
- reviews: story:docs-model-routes
revision: 1
---
needs-revision

- story:llm-routes-pin — its second acceptance line reads a dependency story:sdk-models-from-config creates, and the edge runs the other way (the sdk story depends on the pin). `crates/loom-sdk/Cargo.toml` has no `b10x-llm-models` dependency today (only `b10x-llm-core`, line 21), so "built with `environment` and `file` where `loom-sdk` depends on it" can't be checked in the pin. The fix is to move that line into the sdk story's acceptance, or to say the pin adds the dependency. (cited) — .engineering/planning/story/llm-routes-pin.md:50, crates/loom-sdk/Cargo.toml:21
- story:cli-run-config — its acceptance requires `task docs-check`, which runs `loom-docs generate --check`. That command regenerates `website/docs/reference/cli.md` from the clap `RunArgs` this story changes, but the scope omits that file. `crates/loom-docs` depends on `b10x-loom-cli` by path. story:docs-model-routes also runs `docs-check` and has no edge to the cli story, so its acceptance reads a page the cli story rewrites. Fix: name `website/docs/reference/cli.md` in the cli scope, and either order docs after cli (an edge recording the shared `docs-check` and `cli.md` as the reason) or accept that whichever lands second must regenerate. (cited) — .engineering/planning/story/cli-run-config.md:48, crates/loom-docs/Cargo.toml (`b10x-loom-cli = { path }`), Taskfile.yml:112
- story:docs-model-routes — it claims to run "parallel to `story:sdk-models-from-config`", but both are in wave 2 with no edge and both change `Cargo.lock` without naming it. The docs story adds a dev-dependency to `crates/loom-docs/Cargo.toml` (cited). The sdk story adds `b10x-llm-models` to `crates/loom-sdk/Cargo.toml` (cited in its scope). Every gate here runs `--locked` (Taskfile.yml:32-33,112), so each must update the lockfile (inferred). Only the pin lists `Cargo.lock`. Fix: name `Cargo.lock` in both scopes and say that the two lockfile edits touch different package stanzas, or order the two stories. Both remedies are open. (inferred for the lockfile; cited for the manifests) — .engineering/planning/story/docs-model-routes.md:39,43

**What you read:** 5 artifacts (epic plus 4 stories) via `aep plan artifact show` on each and `aep plan artifact waves --status draft`. I also read `Taskfile.yml`, `crates/loom-docs/{Cargo.toml,src/main.rs}`, `crates/loom-sdk/Cargo.toml` and the CLI reference page header. Surfaces: 4 stories cited (every body has a Scope section naming paths; the store tags them "inferred"), 0 inferred-only, 0 unplaceable. Acceptance reads traced to a producer in the set: 5. Edges record 3 of them: sdk to pin, cli to sdk, docs to pin. The `cli.md` read and the sdk-dependency read have no edge.

**What I could not establish:**
- Whether the `waves` collisions on `CHANGELOG.md`, `Cargo.lock` and the `loom-cli` files involving story:secrets-subcommand matter. That story is outside the set, so I did not assess it.
- Out of my lane, for the design critic: the docs test parses its `loom.yaml` as `loom.models.ModelsConfig`, but the docs body names only a `b10x-llm-models` dev-dependency. It does not say how `loom-docs` reaches the generated `loom` crate.

```findings
- file: .engineering/planning/story/llm-routes-pin.md
  line: 50
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance reads a b10x-llm-models dependency of loom-sdk that story:sdk-models-from-config creates (crates/loom-sdk/Cargo.toml:21 has only b10x-llm-core), and the depends_on edge runs from the sdk story to the pin, not back, so the line cannot be checked when the pin lands; move it to the sdk story or state that the pin adds the dependency"
- file: .engineering/planning/story/cli-run-config.md
  line: 48
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance requires task docs-check, which regenerates website/docs/reference/cli.md from the RunArgs this story changes, but the scope omits that file; story:docs-model-routes also runs docs-check with no edge to this story and so reads a page this story rewrites; name cli.md in scope and either add an ordering edge recording docs-check and cli.md as the reason or accept that the later lander regenerates"
- file: .engineering/planning/story/docs-model-routes.md
  line: 43
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body declares itself parallel to story:sdk-models-from-config but both change Cargo.lock without naming it (this one adds a loom-docs dev-dependency, the sdk story adds b10x-llm-models to loom-sdk, and every gate runs --locked); the lockfile surface is inferred, not cited; name Cargo.lock in both scopes or add an ordering edge recording it as the reason"
```
