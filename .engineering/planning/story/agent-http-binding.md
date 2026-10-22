---
format: aep.planning-md/3
id: story:agent-http-binding
kind: story
status: draft
title: An agent spec's http binding reads an allow-listed path by GET, bounded, and its answer reaches the model as data
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-serve
revision: 1
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

`crates/loom-agent` (the `http` read), `crates/loom-plugin/src/effects.rs` (dispatch by binding
kind), `ess/domains/agent.yaml`, `Cargo.toml` (`b10x-llm-http` for `loom-agent`), AGENTS.md,
CHANGELOG.md.

## Ready

After `story:agent-serve`.
