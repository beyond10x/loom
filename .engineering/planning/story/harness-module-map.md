---
format: aep.planning-md/3
id: story:harness-module-map
kind: story
status: implemented
title: Map Harness crates to Loom responsibilities
refs:
- provider: taskboard
  reference: L-001
relations:
- decomposes: epic:loom-native-harness
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: crates/loom/tests/harness_map.rs
- confidence: cited
  path: docs/design/harness-map.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-04T01:17:41Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-04T01:17:41Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-04T01:42:22Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
## Outcome

A reviewed map from every Harness crate to the Loom responsibility it serves, with one disposition
per crate, so each porting story knows whether it depends on a pinned Harness revision, ports
source, or leaves a crate where it is. Loom owns the model/tool loop (Atlas ADR 0071); Harness keeps
serving agentide, agent-platform, metaharness and uilab until each moves (ADR 0071, operator
decision 2026-10-04). `story:harness-loop-port` and `story:session-transcript-streaming` act on its
`port` and `depend` rows; `story:frontier-projection` takes the projection seam from it.

## Scope

- One table in `docs/design/harness-map.md`: crate, what it owns today (Harness README § Layout at
  `798325f0`, release 0.13.3), the `docs/design/loom-design.md` § Owns responsibility it serves or
  `none`, one disposition — `depend` (git dependency at a pinned revision, as the current consumers
  already do), `port` (source carried into Loom), or `not carried` (naming the repository that owns
  it instead) — and, for a `port` row, the Loom crate or module it lands in.
- A section "New in Loom" listing every § Owns responsibility no row names; the projected
  catalogue, action selection and argument generation have no Harness counterpart.
- Name the seams a port reuses: `ModelPort` and `ToolPort` (`harness-wire/src/port.rs`),
  `TurnEnvironmentProvider` (`harness-loop/src/environment.rs`), the approval checkpoint
  (`harness-loop/src/approval.rs`).
- A `port` row names the licence the source carries today (`LicenseRef-B10x-Proprietary`) and the
  licence it is carried under in Loom (Apache-2.0, operator 2026-10-04).
- Files: `docs/design/harness-map.md` (new), `crates/loom/tests/harness_map.rs` (new).

## Shared surface

None. This story is off the `ess/` chain: it edits no specification, no generated model and no
existing source file. `story:frontier-projection` and `story:harness-loop-port` depend on it.

## ESS

No new noun: the map is a design document and `ess/` is unchanged.

## Constraints

Read-only on `beyond10x/harness`: no commit, tag or release there, and no consumer pin moves.

## Acceptance

The test `harness_map_covers_every_crate` in `crates/loom/tests/harness_map.rs` passes. It reads
`docs/design/harness-map.md` and `docs/design/loom-design.md` and checks:

1. The table has exactly one row for each of the 14 crates under `beyond10x/harness/crates` at
   `798325f0` — harness-app-server, harness-cli, harness-credential, harness-flow, harness-http,
   harness-loop, harness-mcp, harness-messages, harness-responses, harness-substrate,
   harness-toolchain, harness-tools, harness-wire, harness-xtask — held as a constant in the test,
   and no other row.
2. Every row's disposition is exactly one of `depend`, `port` or `not carried`.
3. Every row names a § Owns responsibility of `loom-design.md` or `none`.
4. Every § Owns responsibility of `loom-design.md` is named by at least one row or listed under
   "New in Loom".
5. Every `port` row names a Loom target and both licences; every `not carried` row names a
   repository.

## Source

TASKBOARD L-001; Atlas ADR 0071 and its operator decision of 2026-10-04; `docs/design/loom-design.md`
§ Owns; `AGENTS.md` § Boundary; `beyond10x/harness` README § Layout and AGENTS.md at `798325f0`.
