---
format: aep.planning-md/3
id: story:command-variant-runs-once
kind: story
status: implemented
title: A Command LoomAgent runs its program once to completion as a Job, not a restarting Deployment
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: charts/loom-agents
- confidence: inferred
  path: crates/loom-conformance
- confidence: cited
  path: crates/loom-operator/src/reconcile.rs
- confidence: cited
  path: crates/loom-operator/src/render.rs
- confidence: inferred
  path: deploy/crd
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: inferred
  path: generated/rust/loom
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T15:49:07Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-26T15:49:08Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-26T20:55:51Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
## Outcome

A `Command` `LoomAgent` runs its program to completion once, as a Kubernetes `Job` (or a run-once
mode of the resource), instead of a `Deployment` that restarts it. A finished run is not started
again by the controller; a new run is a new generation of the resource (or a new resource).

## Why

Trial S5/S6, 2026-10-26: the `Command` pod is a Deployment, so a takt run that ends is restarted;
the trial wrapped takt in a script ending in `sleep infinity` to keep one run. Gap: "Command variant
runs to completion as a Job (or run-once mode) instead of a restarting Deployment".

## Acceptance

- `a_command_resource_renders_a_job_not_a_deployment`: render of a `Command` resource yields one
  `Job` (`backoffLimit: 0`, `restartPolicy: Never`) with the same pod template, Secrets, workspace
  and NetworkPolicy as today; an `AgentServe` resource still renders a Deployment.
- `a_finished_job_is_not_restarted`: reconcile over a completed Job of the current generation
  creates nothing.
- `a_new_generation_replaces_the_job`: a changed spec deletes the old Job and creates one for the
  new generation.
- ESS: `loom.resource` declares the run-once lifecycle (states for a completed and a failed run)
  before the controller changes; scenarios red first.

## Scope

Derived 2026-10-26 by the coordinator from `b6e35835`. **cited** = read in the tree, **inferred** =
could be wrong.

- `crates/loom-operator/src/render.rs`, `reconcile.rs` — cited
- `ess/domains/resource.yaml` (`LoomAgent` states, `ObserveDeployment`) — cited
- `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/crds/`, `charts/loom-agents/templates/role.yaml` (RBAC for `batch/jobs`) — inferred
- `crates/loom-conformance/src/resource.rs` — inferred
- AGENTS.md § Resource, `docs/design/loom-agent-crd-deployment.md` § 9a, CHANGELOG.md — cited
- Would collide with: every story of `epic:loom-agent-command-variant` (render.rs, resource.yaml)
