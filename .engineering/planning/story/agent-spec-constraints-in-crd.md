---
format: aep.planning-md/3
id: story:agent-spec-constraints-in-crd
kind: story
status: proposed
title: The loom.agent types carry the decoder's value constraints, so the LoomAgent CRD refuses what the pod refuses
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-spec-constraints-in-ess
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:20:19Z", actor: "human:timo", revision: 2}
---
## Outcome

The `loom.agent` types carry the decoder's value constraints themselves, so every projection of the
specification refuses what the pod's `agent check` refuses, the `LoomAgent` CRD included: a `Port`
newtype (`invariants: [value >= 1, value <= 65535]`, projected as `minimum`/`maximum`), a
variable-name newtype for `bearer_env` (`alphabet:` plus a first character that is no digit), the
`format` value, non-blank text where ESS can say it, and `deadline` as `format: date-time`. Every
`List<…>` the document declares is optional or defaulted, so `crates/loom-resource/src/crd.rs` drops
its `ABSENT_LIST_IS_EMPTY` exemption.

## Why

The original outcome of `story:agent-spec-constraints-in-ess` (adversary findings J3 and J5 of
review-result:adversary-w20261025-resource-pass-1). That story landed the constraints as named
refusals of `CheckAgentSpec`; this one moves them onto the types.

## Acceptance

- `deploy/crd/loomagents.loom.beyond10x.dev.yaml` and `charts/loom-agents/crds/` regenerated with the
  constraints; `task crd-drift` exits 0.
- A `LoomAgent` with `port: 0`, `bearer_env: "x y"` or `deadline: 2026-12-31` fails CRD schema
  validation (`crates/loom-resource/tests/crd.rs`).
- `ess verify conform synthesize` reports 0 refusals with the new types.

## Not now

1. The orchestrator's item of 2026-10-26 forbids touching the CRD and the chart while a `LoomAgent`
   trial runs on a kind cluster.
2. ESS 0.69.2 refuses a value type's invariants that no view publishes: a probe making
   `EgressAperture.port` a `Port` newtype gave `refusal[ESS-SYNTH-013]: type loom.agent.Port has no
   scenario`, and the `loom.agent` domain is stateless with no view. Reported to the ess session as an
   ESS gap; the fix or a view that publishes the types comes first.

## Scope

Derived 2026-10-26 by the coordinator. **cited** = read in the tree, **inferred** = could be wrong.

- **Primary surface:** `ess/domains/agent.yaml` (`EgressAperture.port` :66, `A2aExposure.port` :336, `KubernetesEgress`/`KubernetesIngress` ports :394, :405, `HttpBinding.bearer_env`, `RunBudget.deadline` :118) — cited
- **Generated:** `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/crds/` (`task crd-generate` copies it), `website/docs/reference/ess/` — cited
- **Files:** `crates/loom-resource/src/crd.rs` (`ABSENT_LIST_IS_EMPTY`), `crates/loom-resource/tests/crd.rs` — cited
- **Files:** `crates/loom-sdk/src/agent.rs` (the decoder reads the newtypes), `crates/loom-operator` (field types of the generated structs) — inferred
- **Documents:** AGENTS.md § Agent spec and § Resource, CHANGELOG.md — cited
- **Confidence:** medium (the newtype change ripples into every crate reading these fields)
