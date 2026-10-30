---
format: aep.planning-md/3
id: story:command-resources-declared
kind: story
status: implemented
title: A Command LoomAgent declares its resources or the controller refuses it ResourcesRequired
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:O1
scope:
- confidence: cited
  path: charts/loom-agents/templates/configmap.yaml
- confidence: cited
  path: charts/loom-agents/values.yaml
- confidence: inferred
  path: crates/loom-operator/src/admission.rs
- confidence: cited
  path: crates/loom-operator/src/cli.rs
- confidence: cited
  path: crates/loom-operator/src/config.rs
- confidence: cited
  path: crates/loom-operator/src/render.rs
- confidence: cited
  path: crates/loom-operator/tests/chart.rs
- confidence: cited
  path: crates/loom-operator/tests/golden/takt-delivery.yaml
- confidence: cited
  path: website/docs/reference/operator-cli.md
revision: 15
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 12}
- {from: "proposed", to: "active", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 13}
- {from: "active", to: "implemented", at: "2026-10-30T17:37:26Z", actor: "human:timo", revision: 15, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
## Outcome

A `Command` `LoomAgent` runs with the CPU and memory its `spec.agent.deploy.kubernetes.resources`
declares, or with the controller configuration's new optional `defaultCommandResources`; with
neither, the controller refuses it `ResourcesRequired` at `spec.agent.deploy.kubernetes.resources`
before it renders anything, the detail saying that Loom cannot size a program it does not know and
naming the member to set. `defaultResources` (250m CPU, 256Mi) stays the size of an `AgentServe`
pod, whose program is Loom's own. The `loom-agents` chart ships `defaultCommandResources:
{cpu: "4", memory: 8Gi}`, sized for a coding agent (the takt example, `examples/loom-agent/`
in takt, declares 8Gi; the G7 runs, which build several crates, set 8 CPU / 16Gi), so an install
through the chart keeps admitting a `Command` resource that declares nothing. `ResourcesRequired`
fires only when neither the resource nor the controller configuration sets resources: the built-in
configuration (`render` without `--config`) and a hand-written `config.yaml` without the member.

## Why

takt's G7 pod run `run-takt-g7-llm-120509` (2026-10-30) was OOM-killed (exit 137 at 12:04:33Z,
27 s after its case line, before step 1 printed) under Loom's default `memory: 256Mi`
(`crates/loom-operator/src/config.rs:207`, `charts/loom-agents/values.yaml:27`). The Claude program
alone peaked at 250 MiB in a takt probe (`p-claude-nocred`), and the run's first step ran `cargo test`
(inferred from the log ending at the case line). The G7 manifests now set 8 CPU / 16Gi by hand.
Source: the trial analysts' report 5, item 7 (K7 in the trial summary), and the orchestrator's item
D283.

The orchestrator offered two fixes: a default that fits a coding agent, or an explicit value with a
clear admission error. This story takes the second, for these reasons:

- A `Command` resource runs the image's own program (takt, a coding agent, a smoke container); its
  memory need ranges from a few MiB to 16Gi, so no one default fits, and every default that is
  wrong fails silently: too small is an OOM kill mid-step, too large is a pod that never schedules.
- The controller renders requests equal to limits (`render.rs` `fn resources`), so a coding-agent
  default (8Gi or more) would reserve that much for every `Command` pod, a smoke container
  included.
- A refusal names the member before the pod exists, where an OOM kill names nothing (exit 137).
- An operator who wants one size for all `Command` resources sets `defaultCommandResources` once;
  the chart does so for every chart install (orchestrator's amendment, D285).

**Breaking:** a `Command` resource that declares no resources, on a controller whose configuration
sets no `defaultCommandResources`, is refused where it was admitted. The CHANGELOG line names the
migration for a configuration not written by the chart: add `defaultCommandResources: {cpu, memory}`
to `config.yaml`, or declare `spec.agent.deploy.kubernetes.resources` on each resource.

## Acceptance

`crates/loom-operator/tests/command_resources.rs` (names may be refined; each is a red run first):

- `a_command_resource_without_resources_is_refused_resources_required`: under `Config::builtin()`,
  a `Command` resource with no `deploy.kubernetes.resources` (with a `spec.agent` that has no
  `deploy`, and with no `spec.agent`) is refused `ResourcesRequired` at
  `spec.agent.deploy.kubernetes.resources`; `render` exits 3 with that line; nothing is rendered.
- `a_command_resource_gets_the_configured_command_default`: with `defaultCommandResources: {cpu: "4",
  memory: 8Gi}`, the agent container and the init containers carry `4` / `8Gi` as requests and limits.
- `a_declared_size_wins_over_every_default`: a `Command` resource declaring `8` / `16Gi` gets them.
- `an_agent_serve_resource_keeps_the_default_resources`: an `AgentServe` resource with no
  resources still gets `defaultResources` (250m / 256Mi built in).
- `defaultCommandResources` is optional in `config.yaml`; a misspelt member is still refused.
- The chart's values set `defaultCommandResources: {cpu: "4", memory: 8Gi}` and render it into
  the ConfigMap; a values override with it unset renders no member
  (`crates/loom-operator/tests/chart.rs`). A `Command` resource with no resources rendered under
  the chart's configuration gets `4` / `8Gi`.
- Unchanged: the boundary sidecar's own fixed size.

## ESS first

None. Controller refusals are the controller's, not the `loom.resource` domain's (AGENTS.md
§ Resource, as for `BoundaryEgressHost`); the controller configuration is not declared in `ess/`.
The red run is the new test file on a commit that adds only it.

## Scope

Confirmed by the wave 2026-10-30-w3 diff (`git diff a5f30124 impl/command-resources-declared`, head 6879b251):

- `crates/loom-operator/src/config.rs`: `defaultCommandResources`, and the quantity check on every `Resources` field in `Config::parse`
- `crates/loom-operator/src/admission.rs`: refusal `ResourcesRequired` (step 17), `pod_resources`, `Admitted.resources`
- `crates/loom-operator/src/render.rs`: renders the admitted size; `crates/loom-operator/src/cli.rs`: `RENDER_HELP`
- `charts/loom-agents/values.yaml`, `charts/loom-agents/templates/configmap.yaml`: the chart default `{cpu: "4", memory: 8Gi}`
- tests: `command_resources.rs` (new), `adversary_w20261030w3_a.rs` (new), `chart.rs`, `render.rs`, `command_confinement_profiles.rs`, `adv_cmdv_u4_config_files.rs`, `adversary_d243_u1.rs`, `security_d243_u1.rs` (test configs gained the member)
- `website/docs/reference/operator-cli.md` (generated), `README.md`, `AGENTS.md`; CHANGELOG.md by the coordinator
- Not touched, noted: `docs/design/loom-agent-crd-deployment.md:201` still describes the old default
