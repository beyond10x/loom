---
format: aep.planning-md/3
id: story:harness-crate-port
kind: story
status: implemented
title: Port Harness's wire, provider adapters and turn loop crates into Loom
summary: 'The porting half of the former harness-loop-port: five crates per the module map, provider-wire fixtures, relicensed Apache-2.0, Harness unchanged.'
refs:
- provider: taskboard
  reference: L-002
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:run-pipeline-skeleton
- depends_on: story:harness-module-map
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom/Cargo.toml
- confidence: cited
  path: crates/loom/src/harness/
- confidence: cited
  path: crates/loom/tests/adversary2_harness_port.rs
- confidence: cited
  path: crates/loom/tests/adversary_harness_port_boundaries.rs
- confidence: cited
  path: crates/loom/tests/fixtures/provider-wires/
- confidence: inferred
  path: crates/loom/tests/harness_port.rs
- confidence: cited
  path: crates/loom/tests/harness_port_contract.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-04T07:28:46Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-04T07:28:46Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-04T10:54:42Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":4,"verification":1}}}
---
## Outcome

The Harness crates `docs/design/harness-map.md` marks `port` from `harness-wire` to `harness-loop`
are carried from `beyond10x/harness` at `798325f0` into `b10x-loom`, relicensed Apache-2.0, at the
targets the map names: `crates/loom/src/harness/wire/`, `http/`, `responses/`, `messages/` and
`turn_loop/`. This is the porting half of the former `story:harness-loop-port`, split off on
2026-10-04 so that the port, which needs no Loom pipeline behaviour, can run beside
`story:frontier-projection`; wiring the ported loop to the frontier, the selector, the argument
generator and revalidation stays in `story:harness-loop-port`. Nothing of the port is dropped or
added by the split: acceptance items 1, 5 and 6 of the former story are this story's.

- **Licence.** Ported source is relicensed Apache-2.0 (operator, 2026-10-04; `AGENTS.md` § Boundary).
  Harness keeps `LicenseRef-B10x-Proprietary`.
- **Harness's consumers do not break.** `beyond10x/harness` is not changed: no commit, tag or
  release there, and no consumer pin moves. agentide, agent-platform, metaharness and uilab keep
  depending on Harness until each moves (ADR 0071, operator decision 2026-10-04).
- **Placement.** Each `port` row's "Loom target"; the submodule lines go into
  `crates/loom/src/harness/mod.rs`, which `story:run-pipeline-skeleton` creates.
- **Contracts.** The provider-wire contracts the ported adapters are held to are copied from Harness
  `contracts/provider-wires/` (`anthropic-messages`, `openai-responses`) into
  `crates/loom/tests/fixtures/provider-wires/`, with the Harness revision they came from.
- **Seams kept.** `ModelPort`, `ToolPort`, `TurnEnvironmentProvider` and the approval checkpoint keep
  their shape (harness-map § Seams a port reuses), so `story:harness-loop-port` can wire them and
  `story:session-transcript-streaming` can port `transcript.rs` onto them.

## Shared surface

Depends on `story:run-pipeline-skeleton` (the `harness` module and its `pub mod` line) and
`story:harness-module-map` (the dispositions). It is the only open story that edits
`crates/loom/Cargo.toml` and `Cargo.lock` before `story:loom-ess-conformance`, and it does not edit
`crates/loom/src/lib.rs`, `ess/` or `generated/`. `story:harness-loop-port` and
`story:session-transcript-streaming` depend on it.

## ESS first

- **Specification change:** none. Requests, stream events, items and tool calls are wire values of
  the ported code, not domain entities, and `loom.run.Turn` already types a turn. If the port meets a
  domain noun the ported code needs, the story stops and reports it; the noun is settled before
  `ess/` changes and is not added here.
- **Red test:** the first commit adds `crates/loom/tests/fixtures/provider-wires/` and the test
  `harness_port_wires_and_licence` in `crates/loom/tests/harness_port.rs`; it fails on that commit
  because `b10x_loom::harness` has no `wire`, `responses` or `messages` module to build a request
  body with. The port commits make it pass.

## Scope

- `crates/loom/src/harness/` (`wire/`, `http/`, `responses/`, `messages/`, `turn_loop/`, and the
  submodule lines in `mod.rs`)
- `crates/loom/Cargo.toml`, `Cargo.lock`
- `crates/loom/tests/harness_port.rs` (new)
- `crates/loom/tests/fixtures/provider-wires/` (new)

## Constraints

Read-only on `beyond10x/harness`. Port, do not rewrite from zero (`AGENTS.md` § Boundary).

## Acceptance

The test `harness_port_wires_and_licence` in `crates/loom/tests/harness_port.rs` passes and checks
items 1 and 2. Item 3 is observed outside the tests.

1. `crates/loom/tests/fixtures/provider-wires/` holds exactly two wires, `anthropic-messages` and
   `openai-responses`, and no other; for each of the two, the request body the ported adapter builds
   for a fixed one-turn input equals that wire's pinned request fixture.
2. No file under `crates/` contains the string `LicenseRef-B10x-Proprietary`; each ported file
   states Apache-2.0, in its own header (`SPDX-License-Identifier: Apache-2.0`) or through the
   manifest of the crate that holds it; and `cargo tree -p b10x-loom -e normal --prefix none` names
   no crate whose row in `docs/design/harness-map.md` has the disposition `port`.
3. No `beyond10x/harness` pin moved and no commit was made in `beyond10x/harness`. The story's
   report records `git -C ../harness log -1 --format=%H` before the port starts and again at
   delivery, and the two are the same sha. `Cargo.lock` and
   `cargo tree -p b10x-loom -e normal --prefix none` name no `beyond10x/harness` source at a
   revision other than `798325f0`.

## Source

TASKBOARD L-002; `epic:loom-native-harness` § Outcome ("carries over Harness's loop"); Atlas ADR
0071 and its operator decision of 2026-10-04; `AGENTS.md` § Boundary; `docs/design/harness-map.md`;
Harness README § Layout at `798325f0`; the former acceptance items 1, 5 and 6 of
`story:harness-loop-port` (revision 4).
