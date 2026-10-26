---
format: aep.planning-md/3
id: story:agent-spec-constraints-in-ess
kind: story
status: active
title: Each value constraint of the loom.agent/1 decoder is a named refusal of CheckAgentSpec, held by ESS scenarios
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:loom-agent-resource-domain
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-agent
- confidence: inferred
  path: crates/loom-conformance
- confidence: cited
  path: crates/loom-sdk/src/agent.rs
- confidence: cited
  path: ess/domains/agent.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/scenarios/agent-check
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/reference/ess
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:20:19Z", actor: "human:timo", revision: 15}
- {from: "proposed", to: "active", at: "2026-10-26T13:20:19Z", actor: "human:timo", revision: 16}
---
## Outcome

Every value constraint the `loom.agent/1` decoder (`crates/loom-sdk/src/agent.rs`) enforces is a
named refusal of `loom.agent.CheckAgentSpec`, declared in `ess/domains/agent.yaml` and held by an
authored conformance scenario that sends a document breaking it and requires that refusal. Today
each of them answers `Malformed` with the decoder's message, so the specification says nothing about
which values are refused; the Rust tests alone hold them.

New `loom.agent.RefusalReason` variants, one `CheckAgentSpec` outcome each:

| reason | refused value | decoder today |
|---|---|---|
| `UnknownFormat` | `format` other than `loom.agent/1` | `spec`, `Malformed` |
| `BlankText` | a text member that is empty or only whitespace | `text`, `Malformed` |
| `NotAVariableName` | a `bearer_env` that is not `[A-Za-z_][A-Za-z0-9_]*` (the text never repeated) | `variable_name`, `Malformed` |
| `PortOutOfRange` | a port outside 1 to 65535 | `port`, `Malformed` |
| `NotAPlainPath` | an `allow_paths` entry that is not a `plain_http_path` | `allow_path`, `Malformed` |
| `NotAnInstant` | a `Timestamp` member (`session.budget.deadline`) that is not an RFC 3339 instant | `timestamp`, **accepted** |

The last row is a gap, not only a rename: the decoder accepts `deadline: 2026-12-31` although the
specification types it `Timestamp`.

## Why

The adversary pass on story:loom-agent-resource-domain
(review-result:adversary-w20261025-resource-pass-1, J3 and J5) found the decoder refusing documents
the specification admits. The orchestrator (dispatch-queue #13, 2026-10-26): "the `loom.agent/1`
constraints that are held by Rust tests and not by the ESS specification ... implement what fits in
one unit (ESS first: domain constraints, then regenerate and drift checks); what does not fit stays
as a follow-up story with a Scope section".

The type-level form (newtype invariants and alphabets that the JSON Schema and so the CRD project)
does not fit this unit: the same item forbids touching the CRD while a cluster trial runs, and ESS
0.69.2 refuses value invariants a stateless domain never publishes in a view (`ESS-SYNTH-013`,
"type loom.agent.Port has no scenario", probe 2026-10-26). It is `story:agent-spec-constraints-in-crd`.

## Acceptance

Authored `ess-scenario/4` scenarios under `ess/scenarios/agent-check/`, each with a fixture under
`specs/`, listed in `ess/ess-inputs.yaml`, run by `task conform` against `check_text`:

- `an-unknown-format-is-refused` (`UnknownFormat`, member `format`).
- `a-blank-text-member-is-refused` (`BlankText`, e.g. `agent.name: "  "`).
- `a-bearer-env-that-is-not-a-variable-name-is-refused` (`NotAVariableName`, detail never repeats the text).
- `a-port-outside-the-range-is-refused` (`PortOutOfRange`, `0` and `65536`).
- `an-allow-path-holding-a-dot-dot-segment-is-refused` now answers `NotAPlainPath` (was `Malformed`).
- `a-deadline-that-is-not-an-instant-is-refused` (`NotAnInstant`); a full RFC 3339 instant checks.
- `b10x-loom agent check` prints the new reason names; `task check` exits 0; `task crd-drift` exits 0
  with `deploy/crd/` unchanged.

## ESS first

The first commit changes only `ess/` (the reasons, the outcomes, the scenarios and their fixtures);
`task drift` and the new scenarios are red on it, then the decoder and `check_text` follow.

## Not in scope

The CRD and every type-level declaration (`story:agent-spec-constraints-in-crd`); the controller,
its rendering and admission and `agent stage-files` (`story:operator-constraints-in-ess`);
structural refusals (unknown member, wrong type, YAML tag, flow bound), which stay `Malformed`.

## Scope

Derived 2026-10-26 by the coordinator from the tree at `b6e35835`. **cited** = read in the tree,
**inferred** = a reading that could be wrong.

- **Primary surface:** `ess/domains/agent.yaml` (`RefusalReason` :477-481, `CheckAgentSpec` :565-616) — cited
- **Files:** `ess/scenarios/agent-check/` and `specs/`, `ess/ess-inputs.yaml` — cited
- **Files:** `crates/loom-sdk/src/agent.rs` (`text` :227, `variable_name` :237, `allow_path` :277, `port` :289, `timestamp` :331, `spec` :358) — cited
- **Files:** `crates/loom-agent/src/` (`check_text`, `spelling`), `crates/loom-agent/tests/check.rs` — cited
- **Generated:** `generated/rust/loom/` (`task generate`), `website/docs/reference/ess/`, `website/data/ess/` (`task docs-generate`) — cited
- **Also likely:** `crates/loom-conformance/src/` (the agent-check target) — inferred
- **Documents:** AGENTS.md § Agent spec, README.md, CHANGELOG.md — cited
- **Not touched:** `crates/loom-operator` (it passes `spelling(refused.reason)` through, admission.rs:556), `deploy/crd/`, `charts/` — cited
- **Confidence:** high
