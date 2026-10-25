---
format: aep.planning-md/3
id: story:loom-agent-resource-domain
kind: story
status: active
title: 'The LoomAgent resource: ESS domain loom.resource and the CRD schema projected from it and loom.agent'
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-conformance
- confidence: cited
  path: crates/loom-resource
- confidence: cited
  path: crates/loom-xtask
- confidence: cited
  path: deploy/crd
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/data/ess
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference/crates.md
- confidence: cited
  path: website/docs/reference/ess
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T15:15:30Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-25T15:15:30Z", actor: "human:timo", revision: 3}
---
## Outcome

The `LoomAgent` Kubernetes custom resource has a typed home and a lifecycle in Loom's
specification, and its CRD schema is projected from that specification, never written by hand:

- ESS domain `loom.resource` (`ess/domains/resource.yaml`) declares `LoomAgentSpec` (the
  resource's `spec`: `agent` is `loom.agent.AgentSpec` verbatim, design D1, no second agent schema;
  `routes`, the llm routes document, untyped; `connectorsConfig`, `secretRef`, `image`), the entity
  `LoomAgent` (identity `name`; `observedGeneration`; `check`, what `b10x-loom agent check` said;
  lifecycle `Pending`, `Running`, `Refused`, `Degraded`) and the commands the controller records
  with: `ApplyLoomAgent`, `RecordCheck`, `ObserveDeployment`, `DeleteLoomAgent`, with their events,
  errors and the view `LoomAgents`.
- `deploy/crd/loomagents.loom.beyond10x.dev.yaml` is the `CustomResourceDefinition` (group
  `loom.beyond10x.dev`, version `v1alpha1`, kind `LoomAgent`, namespaced, status subresource) whose
  `openAPIV3Schema` is projected by `loom-xtask crd-generate` from `ess generate --kind schema`:
  `spec` from `loom.resource.LoomAgentSpec`, `status` from the entity `loom.resource.LoomAgent`
  (its fields other than `name`), every `$ref` inlined into a structural schema, a tagged union
  flattened to the wire form Loom's decoder reads (the tag beside the variant's fields), `const`
  spelled as a one-value `enum`, `Json` as `x-kubernetes-preserve-unknown-fields: true`, and
  nothing ESS-specific left in. `loom-xtask crd-drift` fails when the committed file differs.
- `b10x-loom-resource` (`crates/loom-resource`) holds the generated behaviour of `loom.resource`
  over an in-memory record (`LoomAgentRecord`), the storage port the generated behaviour needs,
  and the CRD projection (`crd::manifest`) the xtask calls; the controller (step 6 of the design,
  not this story) reconciles through it.
- The conformance target (`crates/loom-conformance`) answers the four `loom.resource` commands
  and the `LoomAgents` view through `b10x-loom-resource`, so `task conform` executes every
  `loom.resource` scenario.

## Why

Design `docs/design/loom-agent-crd-deployment.md` (PR beyond10x/loom#127) D8: "Spec first: the
CRD's `openAPIV3Schema` is generated from Loom's `loom.agent` domain, and Loom's `ess/domains/`
gains a domain for the `LoomAgent` resource lifecycle (beside `loom.agent`) before the controller
is written." Its § 9 step 5 is this story; step 6 (`crates/loom-operator`) follows it. Decisions
taken before this story (§ 10): row 1 A (the pod runs Loom's CLI image), row 2 A (group
`loom.beyond10x.dev`, `v1alpha1`), row 6 A (slice A: http reads and A2A, no credentials).
Approval: `approval-record:loom-agent-resource-domain-filing`.

## Design

- The record holds the generation it applied and the check's result, not the spec: the spec stays
  in the cluster and the controller re-reads it on every reconcile. A stored `AgentSpec` holds a
  `Decimal` (`loops.inner.classify_threshold`) that no view can observe exactly on ESS 0.69.0
  (`ESS-SYNTH-001`, "Decimal and Binary64 replay results are unsupported"), which would leave every
  refusal and every preserving outcome without a scenario. Reported as an ESS gap.
- `respecify` is a transition from every state, Pending included, to Pending, so one `moves:`
  branch answers every newer generation and the generated behaviour needs no hand-written
  obligation.
- `ObserveDeployment` takes `availability` as an enum (`Available`, `Unavailable`): a Boolean input
  beside `when_subject_state:` is refused as `non_exhaustive_branches`.
- The CRD schema carries no `additionalProperties: false` (Kubernetes refuses it); unknown fields
  are pruned by the API server, and `b10x-loom agent check` in the pod refuses an undeclared
  member of the agent document. `x-kubernetes-validations` are not projected.
- `ess/system.yaml` moves to `format: ess/25` for the create-or-update `unknown_instance: true`
  branch beside `when_subject:` branches; the other domains are unchanged by it.

## ESS first

The first commit changes only `ess/` (`ess/domains/resource.yaml`, `ess/system.yaml`,
`ess/ess-inputs.yaml`). On it `task drift` fails, naming the files `generated/rust/loom/` lacks
(`src/resource.rs`) and the ones a fresh synthesis changes; the run is recorded in the wave
record. Later commits make it pass without changing `ess/`.

## Acceptance

Named conformance scenarios of the suite `ess verify conform synthesize --path ess --suite-format
5` writes, each executed and passed by `task conform` against `b10x-loom-resource` through the
conformance target:

- `loom.resource.ApplyLoomAgent/outcome/created`, `/stale`, `/unchanged`, `/respecified`;
- `loom.resource.RecordCheck/outcome/running`, `/refused`, `/stale`, `/unknown`, and the
  `loom.resource.LoomAgent/state/<Running|Refused|Degraded>/refuses/loom.resource.RecordCheck`
  scenarios;
- `loom.resource.ObserveDeployment/outcome/degraded`, `/recovered`, `/still-running`,
  `/still-degraded`, `/no-deployment`, `/unknown`;
- `loom.resource.DeleteLoomAgent/outcome/deleted`, `/unknown`.

And:

- `ess specify validate --path ess --strict-requires` exits 0; `ess verify conform synthesize`
  reports 0 refusals and `completeness.unanswered` stays at the base's 6 (all `loom.run`).
- `task ess-gate`, `task drift`, `task no-hand-model`, `task conform` and the new `task crd-drift`
  exit 0; `task check` exits 0.
- `crates/loom-resource/tests/crd.rs`: the committed CRD is a fixed point of `crd::manifest`; the
  projected schema holds no `$ref`, `$defs`, `const`, `additionalProperties`, `title` or
  `x-ess-*` key; `spec.agent` is required and `spec.agent.bindings.items` carries `kind` with
  `enum: [connectors, http]` and the fields of both variants beside it; `spec.routes` carries
  `x-kubernetes-preserve-unknown-fields: true`; `status.state` enumerates exactly the lifecycle's
  states; `examples/agents/operator.yaml` wrapped as a `LoomAgent` validates against the projected
  `spec` schema (a JSON Schema check in the test, no cluster).
- `website/docs/reference/ess/` gains the `loom.resource` page by `task docs-generate`;
  `task docs-check` exits 0.

## Not in scope

The controller `crates/loom-operator` and its image (design step 6); the deployment repository's
chart and resources (steps 7-8); `kube`/`k8s-openapi` dependencies; admission of secret-looking
literals (the controller's, step 6); `loom_sdk` re-exports of `loom.resource` types; a release.

## Scope

- `ess/domains/resource.yaml` (new), `ess/system.yaml`, `ess/ess-inputs.yaml`
- `generated/rust/loom/` (regenerated)
- `crates/loom-resource/` (new): `Cargo.toml`, `src/lib.rs`, `src/crd.rs`, `tests/crd.rs`,
  `tests/record.rs`
- `crates/loom-conformance/src/lib.rs`, `crates/loom-conformance/src/resource.rs` (new),
  `crates/loom-conformance/Cargo.toml`
- `crates/loom-xtask/src/main.rs` (`crd-generate`, `crd-drift`), `crates/loom-xtask/Cargo.toml`
- `deploy/crd/loomagents.loom.beyond10x.dev.yaml` (generated)
- `Taskfile.yml` (`crd-generate`, `crd-drift`, `check`), `Cargo.lock`
- `website/docs/reference/ess/`, `website/data/ess/`, `website/docs/reference/crates.md`
  (regenerated), `website/data/status.json`
- `AGENTS.md` (§ ESS, § Generated files, a § Resource section), `README.md`, `CHANGELOG.md`

## Ready

Ready now: `story:agent-domain` (`loom.agent`) is implemented, ESS 0.69.0 is pinned.
