---
format: aep.planning-md/3
id: story:agent-domain
kind: story
status: active
title: 'An agent is data: ESS domain loom.agent declares AgentSpec and ToolPolicy'
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T08:58:29Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-22T08:58:29Z", actor: "human:timo", revision: 4}
---
## Outcome

An agent is data: ESS domain `loom.agent` (`ess/domains/agent.yaml`) declares `AgentSpec`, the
typed form of one `loom.agent/1` YAML document, and `ToolPolicy`, generated into Rust and exported
as `loom_sdk::agent`. One domain serves both designs: the SDK's agent below a product and the
Kubernetes design's spec-driven agent (`loom-kubernetes.md` D1, § 3).

`AgentSpec` holds:

- `format` (`loom.agent/1`), `agent` (`id`, `name`, `brief`);
- `model`, exactly `loom.models.ModelsConfig` (`ess/domains/models.yaml:37-43`), a role optional
  where the spec does not use it;
- `session` (kind, run budget, stop policy, confinement) for an agent that runs a harness session;
- `loops` (`outer`: poll interval, sources, `min_age_minutes`, `lookback_minutes`; `inner`:
  protocol by catalog reference, step budget, model budget, optional `classify_threshold`);
- `states` (name, `when.intent`, actions, sources), each lowering to one `loom.plugin.Projection`;
- `bindings` (`connectors`: a `loom.datasource.DataSource` plus an optional `write`; `http`: base
  URL, allowed paths, method, bearer variable, byte bound);
- `channel` (binding, allow list, bot user id), `writes` (`mode`, `max_per_hour`),
  `policy.capabilities`, `expose` (`a2a`), `runtime` (`state_dir`, `connectors` as
  `loom.datasource.ConnectorsCliConfig`, `confinement`), `deploy.kubernetes`.

`ToolPolicy` holds the allowed and denied tools, egress apertures and decision mode of a session.

Field names follow `loom.plugin.PluginConfig` and `loom.slack.SlackConfig` where the meaning is the
same, so a spec lowers to today's `PluginConfig` without translation.

## Why

Agency, loom-coder and swarm each need a typed agent below the product (ADR 0105); without one each
invents its own. Decided by the orchestrator, Q1 A (`approval-record:loom-sdk-for-products-filing`).
The Kubernetes design's `loom.agent` is the same noun: "merged with the loom-sdk Q1 AgentSpec, not
a second domain" (`approval-record:loom-kubernetes-filing`).

`commission.responsibility.Agent` already exists
(`ess/commission/domains/responsibility.yaml:864-870`): `AgentSpec` relates to it (an `AgentSpec`
defines how an assigned `Agent` runs) and does not shadow it. Run budget and stop policy reuse
Commission's declared types.

Q9 C: `loops.inner.classify_threshold` absent means no classifier request and one state; present,
the classifier keys the states.

Q6 A: `runtime.confinement` has one value today, `none`; `story:agent-check` refuses a binding that
names a program other than the `connectors` program.

## Acceptance

- `ess/domains/agent.yaml` validates, compiles and synthesizes with 0 refusals under `task ess-gate`.
- `task generate` writes `AgentSpec`, `ToolPolicy` and `DecisionMode` under `generated/rust/loom/`;
  `task drift` and `task no-hand-model` pass.
- `loom_sdk::agent` re-exports them; `crates/loom-sdk/tests/host_ports.rs` resolves the paths and
  `sdk-stability.md` lists them.
- A JSON or YAML `AgentSpec` with an unknown member is refused by the generated decoder (one unit
  test); `examples/agents/operator.yaml` (neutral values) decodes.

## ESS first

The first commit adds `ess/domains/agent.yaml` only; `task drift` is red until the generated code is
committed.

## Not in scope

`b10x-loom agent check` (`story:agent-check`); building an executor from a spec
(`story:session-executor-from-spec`); product fields (owner, team, workforce), which are Agency's.

## Scope

`ess/domains/agent.yaml`, `ess/system.yaml`, `generated/rust/loom/`, `crates/loom-sdk/src/lib.rs`,
`crates/loom-sdk/tests/host_ports.rs`, `website/docs/reference/sdk-stability.md`,
`website/docs/reference/ess/` (generated), `examples/agents/operator.yaml`, AGENTS.md, CHANGELOG.md.

## Ready

Ready now.
