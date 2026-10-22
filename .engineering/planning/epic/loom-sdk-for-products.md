---
format: aep.planning-md/3
id: epic:loom-sdk-for-products
kind: epic
status: draft
title: An application defines an agent as data and runs it on Loom through one tagged crate
relations:
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

An application defines an agent as data, runs it under Loom with the tool policy, budget and
confinement it declares, and observes the run, through one tagged crate, `b10x-loom-sdk`.

## Why

Agency (`~/beyond10x/agency`, ADR 0105), takt and loom-coder each build agents on Loom, and each
now wires the runtime by hand: seven arguments to `run_until_blocked`
(`crates/loom-commission/src/runtime.rs:867`), the tool list as the constants `TOOLS` and
`CODEX_TOOLS` (`crates/loom-session-executor/src/lib.rs:90`, `:94`), one egress aperture per child
and no event stream. ADR 0099: what the SDK lacks becomes a Loom story; consumers do not work
around it. The design is `loom-sdk.md` (design 2026-10-22, outside the repository), whose claims
were checked against the code at `c5228ee0`; the corrections are in the stories.

Serves ADR 0096 point 6, ADR 0099, ADR 0104 and ADR 0105 (Atlas, drafts where marked there).

## Acceptance

- An `AgentSpec` (ESS domain `loom.agent`) builds a `SessionExecutor`, and `consumers/pinned`
  defines one and runs it through `loom_sdk` alone (`story:pinned-consumers-agency-shape`).
- A host receives every `RuntimeEvent` of a run through an `EventSink`, and reads each tool
  decision of a session from its `trace-ir/1` record; Loom decides no tool call.
- A confined child gets every field of a `ConfinementSpec` the pinned Substrate honours, and a
  field it cannot honour is refused naming the field.
- `sdk-stability.md` states a deprecation window and a tag rule.

## Decomposition

| Story | Depends on |
|---|---|
| `story:substrate-0-8-pin` | none |
| `story:several-egress-apertures` | `substrate-0-8-pin`, a Substrate release with several apertures per run |
| `story:child-workdir` | `substrate-0-8-pin`, a Substrate release with a working directory outside `/workspace` |
| `story:writable-host-root` | `decision-blocker:sdk-writable-host-root` (Q2), a Substrate release |
| `story:confinement-spec` | `several-egress-apertures`, `child-workdir` |
| `story:agent-domain` | none |
| `story:session-executor-from-spec` | `agent-domain` |
| `story:tool-decisions-read-back` | none (metaharness 0.13.0 is pinned since wave 2026-10-22-w2) |
| `story:runtime-events` | none |
| `story:runtime-wrapper` | none |
| `story:loop-context-split` | none |
| `story:session-lineage` | `agent-domain` |
| `story:stability-version-rule` | none |
| `story:pinned-consumers-agency-shape` | `agent-domain`, `runtime-wrapper` |

The design's fifteenth story, `host-stop-after-checks`, shipped as
`story:host-stop-when-a-step-follows` (implemented, wave 2026-10-22-w2), and its
`metaharness-pin-0-13` is done by `story:run-record-names-agent-sessions` (metaharness `0.13.0`);
neither is filed again.

## Decisions taken (approval-record:loom-sdk-for-products-filing)

Q1 A, Q3 A, Q4 A, Q5 B, Q6 A, Q7 A. Q2 is `decision-blocker:sdk-writable-host-root`.

## Not in scope

- A swarm or multi-session executor (Q5 B: after Agency exists).
- Registry publishing (Q6 A: git tags).
- Product nouns above agent and tool policy (team, workforce, work item): Agency's.
- Changes in a consumer's repository: takt `takt-uses-loom-session-executor`, loom-coder's tag pin,
  Agency's agents as `AgentSpec` plus product fields.
- Deciding a tool call, seeing one while it happens, holding a harness loop, wire or session.
