---
format: aep.planning-md/3
id: story:harness-loop-port
kind: story
status: draft
title: Port Harness's model client, provider adapters and tool loop into Loom
summary: Ported per the module map, relicensed Apache-2.0, Harness and its consumers unchanged.
refs:
- provider: taskboard
  reference: L-002
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:agent-executor
- depends_on: story:harness-module-map
- depends_on: story:selection-revalidation
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom/Cargo.toml
- confidence: inferred
  path: crates/loom/src/harness/
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: inferred
  path: crates/loom/tests/fixtures/provider-wires/
- confidence: inferred
  path: crates/loom/tests/harness_loop_port.rs
- confidence: inferred
  path: ess/domains/run.yaml
- confidence: inferred
  path: generated/rust/loom/
revision: 4
---
## Outcome

Loom carries Harness's loop: the model client, the provider adapters and the tool loop are ported
from `beyond10x/harness` at `798325f0` into Loom, crate by crate as `story:harness-module-map`
disposes them (`port` rows). The candidates, from Harness README § Layout: `harness-wire` (neutral
values, `ModelPort`, `ToolPort`), `harness-http` (transport, SSE framing, retry), `harness-responses`
and `harness-messages` (the two provider projections) and `harness-loop` (turn assembly, tool round
trips, approvals, budgets, cancellation). The map decides each; a row the map marks `depend` is not
ported here.

The ported loop calls Loom's own pieces at Harness's seams: the catalogue from
`story:frontier-projection` is what each request's tool list carries (through the
`TurnEnvironmentProvider` seam, `harness-loop/src/environment.rs:47`), and a model's tool call goes
through selection, argument generation and revalidation (`story:action-selector`,
`story:argument-generator`, `story:selection-revalidation`) before it becomes a `ProposedAction`.

- **Licence.** Ported source is relicensed Apache-2.0 (operator, 2026-10-04; `AGENTS.md` § Boundary).
  Harness keeps `LicenseRef-B10x-Proprietary`.
- **Harness's consumers do not break.** `beyond10x/harness` is not changed: no commit, tag or
  release there, and no consumer pin moves. agentide, agent-platform, metaharness and uilab keep
  depending on Harness until each moves (ADR 0071, operator decision 2026-10-04).
- **Placement.** Each `port` row of `docs/design/harness-map.md` names the Loom crate or module it
  lands in. Where a row leaves that open, the port lands as modules of `b10x-loom` under
  `crates/loom/src/harness/`.
- **Contracts.** The provider-wire contracts the ported adapters are held to are copied from Harness
  `contracts/provider-wires/` (`anthropic-messages`, `openai-responses`) into
  `crates/loom/tests/fixtures/provider-wires/`, with the Harness revision they came from.

## Shared surface

Link 6 of the `epic:loom-native-harness` chain. It depends on `story:selection-revalidation`, and
`story:session-transcript-streaming` depends on it; the whole order is in `story:agent-executor`
§ Shared surface. Its chain surface is `crates/loom/src/lib.rs`, `crates/loom/Cargo.toml` and
`Cargo.lock`; it edits `ess/domains/run.yaml` and regenerates `generated/rust/loom/` only if the
port meets a domain noun (below). It also depends on `story:agent-executor` and
`story:harness-module-map` directly.

## ESS

No new noun is expected: requests, stream events, items and tool calls are wire values of the
ported code, not domain entities, and `loom.run.Turn` already types a turn. If the port finds a
domain noun the ported code needs, it goes into `ess/domains/run.yaml` first, `ess specify validate
--path ess` passes, and `task generate` regenerates.

## Scope

- `crates/loom/src/harness/` (new, unless the map names other targets)
- `crates/loom/src/lib.rs`, `crates/loom/Cargo.toml`, `Cargo.lock`
- `crates/loom/tests/harness_loop_port.rs` (new)
- `crates/loom/tests/fixtures/provider-wires/` (new)
- `ess/domains/run.yaml`, `generated/rust/loom/` (chain surface; only if a noun is needed)

## Constraints

Read-only on `beyond10x/harness`. Port, do not rewrite from zero (`AGENTS.md` § Boundary).

## Acceptance

The test `ported_loop_round_trip` in `crates/loom/tests/harness_loop_port.rs` passes and checks
items 1 to 4 over a provider-emulated endpoint, with the Commission fake governor serving the
software-change frontier. The test `ported_source_licence` in the same file passes and checks
item 5. Item 6 is observed outside the tests.

1. `crates/loom/tests/fixtures/provider-wires/` holds exactly two wires, `anthropic-messages` and
   `openai-responses`, and no other; for each of the two, the request body Loom sends for a fixed
   one-turn input equals that wire's pinned request fixture.
2. The tool list of every request equals the catalogue projected from the frontier current when
   the request was assembled.
3. A scripted model reply that calls a catalogue action makes Loom return a `ProposedAction` for
   that action, and the model's tool call reached the selector and the argument generator exactly
   once each.
4. A scripted model reply that calls a name outside the catalogue is answered to the model as a
   refusal naming that name, and no `ProposedAction` is returned.
5. No file under `crates/` contains the string `LicenseRef-B10x-Proprietary`; each ported file
   states Apache-2.0, in its own header (`SPDX-License-Identifier: Apache-2.0`) or through the
   manifest of the crate that holds it; and `cargo tree -p b10x-loom -e normal --prefix none` names
   no crate whose row in `docs/design/harness-map.md` has the disposition `port`.
6. No `beyond10x/harness` pin moved and no commit was made in `beyond10x/harness`. The story's
   report records `git -C ../harness log -1 --format=%H` before the port starts and again at
   delivery, and the two are the same sha. `Cargo.lock` and
   `cargo tree -p b10x-loom -e normal --prefix none` name no `beyond10x/harness` source at a
   revision other than `798325f0`.

## Source

TASKBOARD L-002 (the executor carries Harness's loop); `epic:loom-native-harness` § Outcome
("carries over Harness's loop"); Atlas ADR 0071 and its operator decision of 2026-10-04;
`AGENTS.md` § Boundary; Harness README § Layout at `798325f0`; finding 2 of
`review-result:loom-native-harness-scope-r1`.
