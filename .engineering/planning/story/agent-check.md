---
format: aep.planning-md/3
id: story:agent-check
kind: story
status: implemented
title: b10x-loom agent check refuses a loom.agent/1 spec that cannot be served, and operator-reply@1 is in the plugin catalog
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-domain
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-agent
- confidence: cited
  path: crates/loom-cli/src/agent.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-conformance/src
- confidence: inferred
  path: crates/loom-conformance/tests/conform.rs
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: inferred
  path: crates/loom-protocols/tests/operator_reply.rs
- confidence: cited
  path: ess/domains/agent.yaml
- confidence: inferred
  path: ess/ess-inputs.yaml
- confidence: inferred
  path: ess/scenarios/agent-check
- confidence: inferred
  path: generated/rust/loom
- confidence: cited
  path: protocols/operator-reply/1.yaml
- confidence: inferred
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference/cli.md
- confidence: inferred
  path: website/docs/reference/ess
revision: 26
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T15:14:05Z", actor: "human:timo", revision: 23}
- {from: "proposed", to: "active", at: "2026-10-25T15:14:06Z", actor: "human:timo", revision: 24}
- {from: "active", to: "implemented", at: "2026-10-25T16:48:20Z", actor: "human:timo", revision: 26, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
## Outcome

`b10x-loom agent check --spec <FILE>` reads a `loom.agent/1` document and refuses, before anything
runs, a spec that cannot be served; the bundled protocol `operator-reply@1` is in the plugin
catalog, so a spec's inner loop can name it. Milestone M0 of the Kubernetes design
(`loom-kubernetes.md` § 7).

## Why

The design's D1 and D4. `inbound-answer@1` sends nothing by design
(`protocols/inbound-answer/1.yaml:7-8`), so a reply that is sent needs its own protocol. The check
mirrors `workflow::load`, which refuses an action its protocol lacks
(`crates/loom-executor/src/workflow.rs:220`).

## Acceptance

Conformance scenarios in `ess/scenarios/` (domain `loom.agent`), and the binary:

- `a_valid_spec_checks`: `examples/agents/operator.yaml` checks, exit 0.
- `a_state_naming_an_unknown_action_is_refused`, naming the state and the action.
- `a_source_without_a_binding_is_refused`, naming the source.
- `a_secret_literal_is_refused`: only `env:` and `file:` references, as llm's routes.
- Also refused, each naming the field: `reply.send` in `policy.capabilities` with `writes.mode:
  none`; an empty `channel.allow`; an `http` binding whose method is not GET; a binding naming a
  program other than `runtime.connectors.program` while `runtime.confinement` is `none` (Q6 A).
- `operator_reply_send_needs_a_proposal`: under Canon, `reply.send` is not admissible before
  `reply.proposed` holds.
- `operator-reply@1` (`protocols/operator-reply/1.yaml`): `source.read`, `reply.propose`,
  `reply.send` (effect write, requires `reply.send`), `reply.decline`; outcomes `answered` and
  `declined`. `ProtocolCatalog::plugins()` holds it beside `inbound-answer@1`; `bundled()` does not
  (`crates/loom-protocols/tests/inbound_answer.rs` extended).
- `website/docs/reference/cli.md` regenerated; `task check` exits 0.

## ESS first

The first commit adds the `loom.agent` check scenarios and the refusal type to `ess/` only; the
scenario `a_state_naming_an_unknown_action_is_refused` is red until the check lands.

## Not in scope

Serving the spec (`story:agent-serve`); `http` bindings at run time (`story:agent-http-binding`).

## Scope

Confirmed on the wave head (the merge of `impl/agent-check`, `git diff --stat e77a247c..de91c818`):

| path | |
|---|---|
| `crates/loom-agent/` (`b10x-loom-agent`: `lib.rs`, `credentials.rs`, `tests/check.rs`, `tests/adv_check.rs`) | added |
| `protocols/operator-reply/1.yaml`, `crates/loom-protocols/src/lib.rs`, `crates/loom-protocols/tests/{operator_reply,inbound_answer}.rs` | confirmed |
| `crates/loom-cli/src/{lib,main,agent}.rs`, `crates/loom-cli/tests/{agent_check,adv_agent_check}.rs`, `crates/loom-cli/Cargo.toml` | confirmed |
| `crates/loom-conformance/src/{lib,agent}.rs`, `tests/conform.rs`, `tests/adversary2_w2_conformance_status.rs`, `Cargo.toml` | confirmed |
| `ess/domains/agent.yaml`, `ess/ess-inputs.yaml`, `ess/scenarios/agent-check/` (nine scenarios, eight fixtures) | confirmed |
| `generated/rust/loom/` (regenerated), `crates/loom-sdk/src/agent.rs` (re-exports) | confirmed |
| `crates/loom-executor/tests/{crate_names,ess_gate,adversary_ess_gate}.rs` | confirmed |
| `website/docs/reference/{cli,crates}.md`, `website/docs/reference/ess/` (regenerated), `website/data/status.json` | confirmed |
| `README.md`, `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml` (workspace member), `Cargo.lock` | confirmed |
| `website/docs/reference/sdk-stability.md`, `examples/agents/operator.yaml` | not touched |

Not declared, per D3: "a binding naming a program other than `runtime.connectors.program`" (no binding of `loom.agent` names a program).

## Ready

After `story:agent-domain`.
