---
format: aep.planning-md/3
id: story:operator-constraints-in-ess
kind: story
status: proposed
title: The LoomAgent controller's admission and rendering rules and agent stage-files are declared in ESS
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:20:19Z", actor: "human:timo", revision: 2}
---
## Outcome

The rules the `LoomAgent` controller and `b10x-loom agent stage-files` enforce are declared in ESS
and held by conformance scenarios, not only by Rust tests: admission's refusals (allow-listed Secret
and claim names, `MountPathInvalid`, the credential walk over `spec.command`, a serve variant
without `runtime.connectors`), what rendering derives from a resource (variant, init container,
mounts, security context), and staging (directory 0700, files 0600 owned by the pod user, stale keys
removed, a non-directory destination refused).

## Why

The orchestrator, 2026-10-26 (dispatch-queue #13): the trial agent "reported the same for rendering,
stage-files and operator admission" as `story:agent-spec-constraints-in-ess` found for the decoder:
constraints held by Rust tests and not by the specification.

## Acceptance

- Each admission refusal reason is an outcome of a `loom.resource` command with an authored
  scenario; `task conform` runs them against `b10x-loom-operator`'s admission.
- `stage-files` is a stateless decision declared in ESS whose refusals and file modes a scenario
  requires.
- `task check` exits 0; the CRD changes only if a status member is added, and then by regeneration.

## Not now

The orchestrator's item of 2026-10-26 forbids touching `crates/loom-operator`, the CRD and the chart
while the kind trial runs; the trial may file further stories against the same files.

## Scope

Derived 2026-10-26 by the coordinator; the trial agent's own list was not available to it.
**cited** = read in the tree, **inferred** = could be wrong.

- **Primary surface:** `ess/domains/resource.yaml` (`ApplyLoomAgent`, `RecordCheck`) — cited
- **Files:** `crates/loom-operator/src/admission.rs`, `crates/loom-operator/src/render.rs` — cited
- **Files:** `crates/loom-cli/src/stage.rs` — cited
- **Files:** `crates/loom-conformance/src/resource.rs` — inferred
- **Generated:** `generated/rust/loom/`, `website/docs/reference/ess/`; `deploy/crd/` only if status changes — inferred
- **Documents:** AGENTS.md § Resource, `docs/design/loom-agent-crd-deployment.md` § 9a, CHANGELOG.md — cited
- **Confidence:** low (which rules the trial agent meant is not established)

## Carried from wave 2026-10-26-q13

- The u2 adversary (`review-result:agent-spec-constraints-adversary-q13`) wrote an operator test, `adv_q13_the_serve_variant_answers_the_new_reason_at_the_member`, green at `96f0e6ae`: an `AgentServe` resource answers `PortOutOfRange at spec.agent.expose.a2a.port`, `NotAnInstant at spec.agent.session.budget.deadline` and `NotAVariableName at spec.agent.bindings[2].bearer_env`, while a `Command` resource answers `Malformed at spec.agent`. It was not committed because the operator was frozen for the cluster trial; write it again here, and make the `Command` case answer the named reason.
- `crates/loom-operator/src/admission.rs:5`: the module doc lists the serve variant's reasons without the six new ones.
