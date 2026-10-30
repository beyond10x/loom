---
format: aep.planning-md/3
id: story:boundary-container-projected-token
kind: story
status: implemented
title: spec.boundary renders a boundary container with a projected token, and refuses secrets beside it
relations:
- decomposes: epic:credential-free-agent-pod
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: charts/loom-agents
- confidence: cited
  path: crates/loom-operator/src/admission.rs
- confidence: cited
  path: crates/loom-operator/src/config.rs
- confidence: cited
  path: crates/loom-operator/src/render.rs
- confidence: cited
  path: ess/domains/resource.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T00:48:53Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-30T00:48:53Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-30T02:59:52Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
## Outcome

A `LoomAgent` whose spec declares `spec.boundary` (any of `model`, `connectors`, `git`, each with
an `upstream` URL and an `audience`; `connectors` with its `operations`, `git` with its
`repositories`) renders a second container, `boundary`, running `b10x-loom agent boundary` as user
10002 from the controller's default image. A projected volume holds one service-account token per
declared upstream, its audience the declared one and its expiry 600 s, and is mounted into the
boundary container alone. The agent container gets `LOOM_BOUNDARY_MODEL_URL`,
`LOOM_BOUNDARY_CONNECTORS_URL` and `LOOM_BOUNDARY_GIT_URL` (for each declared upstream:
`http://127.0.0.1:8401/v1`, `http://127.0.0.1:8402`, `http://127.0.0.1:8403`). A resource that
declares `spec.boundary` beside `spec.secretRef` or `spec.secretFiles` is refused before any child
exists, and so is an upstream the controller's configuration does not allow.

## Why

Design D243 § 2 and § 4, decision A1 (operator direction 2026-10-30, epic
`epic:credential-free-agent-pod`).

## Acceptance

- ESS first: `loom.resource.LoomAgentSpec` gains `boundary: Optional<loom.resource.Boundary>`;
  `task drift` and `task crd-drift` fail on the spec-only commit and pass after regeneration.
- `a_boundary_renders_a_second_container_holding_the_only_token` (render test: the token volume is
  mounted only into `boundary`, user 10002, no capability, read-only root, no workspace volume).
- `the_agent_container_gets_the_boundary_urls_and_no_token`.
- `a_boundary_beside_a_secret_is_refused` (`secretRef` and `secretFiles`, each).
- `a_boundary_upstream_the_controller_does_not_allow_is_refused`.
- Golden render file for a credential-free Command resource; `task crd-drift` exits 0.

## ESS first

`ess/domains/resource.yaml`: the types `Boundary`, `BoundaryUpstream`, `ConnectorsBoundary`,
`GitBoundary` and the member `LoomAgentSpec.boundary`. The refusals are the controller's (as for
`spec.env`, wave decision D5), not the domain's. Red: `task drift`/`task crd-drift` on the
spec-only commit.

## Scope

- `ess/domains/resource.yaml`, `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/crds/`
- `crates/loom-operator/src/{resource,admission,render,config}.rs`, `tests/render.rs`, golden files
- `charts/loom-agents/` (values, config), AGENTS.md § Resource and § Operator, README.md,
  CHANGELOG.md, `website/` operator pages
