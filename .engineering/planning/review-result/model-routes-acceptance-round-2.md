---
format: aep.planning-md/3
id: review-result:model-routes-acceptance-round-2
kind: review-result
status: active
title: Plan critic acceptance, epic:model-routes, round 2
relations:
- reviews: epic:model-routes
- reviews: story:llm-routes-pin
- reviews: story:sdk-models-from-config
- reviews: story:cli-run-config
- reviews: story:docs-model-routes
revision: 1
---
approve

What I read: 6 ids (`epic:model-routes`, the 4 stories, `review-result:model-routes-acceptance-round-1`). I read all 6 in full with `aep plan artifact show <id>`. I did not run `aep plan artifact lifecycle`.

I checked the tree with `sed`, `grep` and `ls` on `crates/loom-cli/src/{lib,main,model_catalog}.rs`, `crates/loom-docs` and the llm 0.6.0 checkout. That checkout is at `~/.cargo/git/checkouts/llm-91be42478a73cba6/74921f7`.

All four round-1 findings are fixed.
- **`story:llm-routes-pin`:**
  - The `environment` and `file` features bullet now sits on `story:sdk-models-from-config`, where `loom-sdk` gains the `models` dependency.
  - The three declared types (`ModelsConfig`, `RoleModel`, `ModelRole`) are now observed by `grep` over `generated/rust/loom/src` and by `ess specify compile`.
  - `Breaking: none.` is stated for the pin.
- **`story:cli-run-config`:** the refusal test now states its observables: exit 1, stderr naming `classifier` and `--classifier-model`, and zero requests at the stub.
- **`story:docs-model-routes`:** `the_documented_model_routes_parse` asserts the endpoint kinds, the two URLs and the required strings.

What I could not establish:
- **Exit-status wording:** the `story:cli-run-config` parenthetical "(the exit `run --help` gives an unusable model)" is loosely worded. `run --help` says only `1  the run failed` (`crates/loom-cli/src/lib.rs:33`), and the "models cannot be used" wording is `plugin run`'s (line 292). The checkable observable, exit 1, is right. `execute` returns `Err` for an unresolvable model before any model call (`main.rs:524-535`), so I left it as wording and not a finding.
- **Model identity:** in llm 0.6.0, `Model::provenance()` exists (`crates/llm-core/src/port.rs:44`). I take that as what `a_config_names_the_model_of_each_role` compares, but I did not confirm provenance carries the route name.
- **Out of my lane (design):** `the_documented_model_routes_parse` parses `loom.yaml` "with the parser `loom_sdk::models` uses", but the SDK stability bullet lists only `from_config` and `from_routes`. Whether that parser is public is a design question.
- **Out of my lane (parallel-safety):** `Cargo.lock` and several `Cargo.toml` files are shared across all four stories.

```findings
[]
```
