---
format: aep.planning-md/3
id: story:agent-http-binding
kind: story
status: active
title: An agent spec's http binding reads an allow-listed path by GET, bounded, and its answer reaches the model as data
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-serve
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: crates/loom-agent
- confidence: inferred
  path: crates/loom-agent/Cargo.toml
- confidence: cited
  path: crates/loom-plugin/src/effects.rs
- confidence: inferred
  path: crates/loom-sdk/src/agent.rs
- confidence: cited
  path: ess/domains/agent.yaml
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T15:17:33Z", actor: "human:timo", revision: 11}
- {from: "proposed", to: "active", at: "2026-10-25T15:17:34Z", actor: "human:timo", revision: 12}
---
## Outcome

An agent spec's `http` binding reads an in-cluster endpoint: GET only, inside `allow_paths`, no
redirect followed, the response bounded by `max_bytes` and marked when cut, a bearer read from the
variable `bearer_env` names. What it returns reaches the model as data, never as instructions. The
`connectors` kind reads the same sources through Connectors' loki and prometheus adapters.
Milestone M2 of the Kubernetes design.

## Why

D7: logs and metrics are read-only tool bindings. The HTTP client is llm's `b10x-llm-http`, as the
Laya selector uses it (AGENTS.md § Laya selector); Loom adds no hand-written HTTP client.

## Acceptance

- `a_post_on_an_http_binding_is_refused` (and `agent check` refuses the method).
- `a_path_outside_allow_paths_is_refused`, before any request; a path with `..` or an encoded
  separator is refused the same way.
- `a_response_over_max_bytes_is_truncated_and_marked`.
- `a_log_line_is_data_not_instructions`: a response carrying `</earlier-steps>` stays inside the
  data block (`<`, `>`, `&` as `\u` escapes).
- A redirect is not followed and is reported as a refusal naming the location.
- Tests run against a loopback listener; no network.

## ESS first

The first commit adds the `http` binding scenarios to `ess/domains/agent.yaml` only; they are red
until the binding lands.

## Not in scope

Writes through `http`; credentials other than one bearer variable.

## Scope

## Scope

Derived 2026-10-25 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-agent` (the `http` read, GET-only, `allow_paths`, `max_bytes`, `bearer_env`) — cited (story § Scope); not in the tree at 0.40.0, `story:agent-check` creates it
- **Files:** `crates/loom-plugin/src/effects.rs:166` (`fn read`) and `:385-392` (`declared`, which resolves a read only through `loom.datasource` `list`/`search`/`get`; dispatch by binding kind lands here) — cited (story § Scope)
- **Files:** `ess/domains/agent.yaml:257-282` (`HttpBinding` fields `allow_paths`, `method`, `bearer_env`, `max_bytes`; `Binding` union tagged `kind`) — cited
- **Files:** `crates/loom-agent/Cargo.toml` (`b10x-llm-http` at the llm tag every crate pins, 0.8.1 today, e.g. `crates/loom-sdk/Cargo.toml:25`) and `Cargo.lock` — inferred; the story writes "`Cargo.toml`", but the workspace root has no `[workspace.dependencies]` (`Cargo.toml:1-10`)
- **Symbols:** `HttpClient::post_json` is the only `b10x-llm-http` use today (`AGENTS.md:713`, `crates/loom-selector-laya`); the GET path needs its own call — cited
- **Also likely:** `crates/loom-sdk/src/agent.rs:12-13` — inferred, the `bearer_env` name check already lives there; no change expected
- **Also likely:** `crates/loom-plugin/src/effects.rs:144` (`transcript`) and the data-block escaping `AGENTS.md:636-642` — inferred, `a_log_line_is_data_not_instructions` passes without change if the http answer flows through the same `SourceRead`/transcript path
- **Also likely:** `crates/loom-agent` tests with a loopback listener — inferred
- **Documents:** `AGENTS.md` (§ Agent spec :598-620), `CHANGELOG.md` — cited
- **Confidence:** medium — the story names the crate, but whether the kind dispatch is a branch in `effects.rs` or a second effect port in `crates/loom-agent` is a design choice the story leaves open, and the crate does not exist yet
- **Would collide with:** any unit touching `crates/loom-plugin/src/effects.rs` (the read path) or `crates/loom-agent`
- **Safety fact:** `declared` (`effects.rs:385-392`) matches only `DataSource.{list,search,get}` from `loom.datasource`, so an `HttpBinding` not threaded into `DataSourceEffects.sources` is unreachable from every existing turn; adding a kind branch cannot change a `connectors` read — step 2, unproven
- **Not established:** the Outcome's "`connectors` kind reads the same sources through Connectors' loki and prometheus adapters" names no Loom file (`git grep -i 'loki\|prometheus' crates/` matched nothing); whether `b10x-llm-http` exposes a GET with redirect-off and a byte bound, or Loom must wrap it

## Ready

After `story:agent-serve`.
