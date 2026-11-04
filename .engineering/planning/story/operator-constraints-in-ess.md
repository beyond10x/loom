---
format: aep.planning-md/3
id: story:operator-constraints-in-ess
kind: story
status: active
title: The LoomAgent controller's admission and rendering rules and agent stage-files are declared in ESS
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: crates/loom-cli/src/stage.rs
- confidence: inferred
  path: crates/loom-conformance/Cargo.toml
- confidence: inferred
  path: crates/loom-conformance/src/admission.rs
- confidence: cited
  path: crates/loom-conformance/src/lib.rs
- confidence: cited
  path: crates/loom-operator/src/admission.rs
- confidence: inferred
  path: crates/loom-operator/tests/admission_reasons.rs
- confidence: cited
  path: docs/design/loom-agent-crd-deployment.md
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: inferred
  path: ess/scenarios
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/data/ess/loom-resource.domain-graph.json
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference/ess/loom-resource.md
revision: 21
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:20:19Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-04T07:33:04Z", actor: "human:timo", revision: 4}
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

Derived 2026-11-04 by `story-scoper` at `9bd79bcd`; replaces the 2026-10-26 section. **cited** = read in the
tree, **inferred** = could be wrong.

- **Primary surface:** `ess/domains/resource.yaml`: a new admission decision (reason enum, error, one outcome per
  reason) beside `RecordCheck` (:646), and the `stage-files` decision, declared in `loom.resource` so this story does
  not edit `ess/domains/agent.yaml` — cited (the file), decided by the coordinator (the domain)
- **Files:** `crates/loom-operator/src/admission.rs` (`admit` :1474, the one admission entry point; module doc
  :1-158; the carried item: a `Command` spec answers the decoder's named reason, not `Malformed`, :1517-1528) — cited
- **Files:** `crates/loom-cli/src/stage.rs` (`stage` :33; link :37, not a directory :40, 0700 :42/:48, non-UTF-8
  :54, outside the volume :68, non-regular skipped :70-75, 0600 :82-89, stale directory :110, stale file :112) — cited
- **Files:** `crates/loom-conformance/src/lib.rs` (dispatch :225-243), `crates/loom-conformance/Cargo.toml`
  (gains `b10x-loom-operator`, so kube and k8s-openapi join the `conform` build), `Cargo.lock` — cited (dispatch,
  missing dependency), inferred (the dependency is added)
- **Files:** a new `crates/loom-conformance/src/admission.rs` adapter over `admit` with resource fixtures, and a
  pure stage decision function that conformance can call without `b10x-loom-cli` — inferred
- **Files:** `ess/scenarios/` for admission and stage-files, with resource fixtures — inferred
- **Files:** an operator test for the carried adversary case, e.g. `crates/loom-operator/tests/admission_reasons.rs`
  — inferred
- **Generated:** `generated/rust/loom/` (`task generate`), `website/docs/reference/ess/loom-resource.md`,
  `website/data/ess/loom-resource.domain-graph.json` (`task docs-generate`); `deploy/crd/` only if
  `LoomAgentSpec` or its status changes — cited (generator table), inferred (which files)
- **Documents:** AGENTS.md § Resource and § Operator, `docs/design/loom-agent-crd-deployment.md` § 9a (:327),
  CHANGELOG.md, `website/data/status.json` — cited
- **Out of this story:** what rendering derives (`render.rs`: variant, init containers, mounts, security context)
  has no acceptance line, so it is not declared here; `ChildNotOwned` (`reconcile.rs:776`) is decided at
  reconcile time from the cluster, not from the spec, and is left out — decided by the coordinator
- **Shared with story:a2a-record-and-streaming:** the regenerated `generated/rust/loom/src/behaviour.rs`,
  `plan.json`, `PLAN.md`, CHANGELOG.md, AGENTS.md (other sections), possibly `crates/loom-conformance/src/lib.rs`
  and `website/data/status.json`; merged one after the other and regenerated after the second merge
- **Confidence:** medium (every refusal and stage check is located at a line; the declaration shape is the
  implementor's)

## Carried from wave 2026-10-26-q13

- The u2 adversary (`review-result:agent-spec-constraints-adversary-q13`) wrote an operator test, `adv_q13_the_serve_variant_answers_the_new_reason_at_the_member`, green at `96f0e6ae`: an `AgentServe` resource answers `PortOutOfRange at spec.agent.expose.a2a.port`, `NotAnInstant at spec.agent.session.budget.deadline` and `NotAVariableName at spec.agent.bindings[2].bearer_env`, while a `Command` resource answers `Malformed at spec.agent`. It was not committed because the operator was frozen for the cluster trial; write it again here, and make the `Command` case answer the named reason.
- `crates/loom-operator/src/admission.rs:5`: the module doc lists the serve variant's reasons without the six new ones.
