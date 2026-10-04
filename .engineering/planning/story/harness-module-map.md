---
format: aep.planning-md/3
id: story:harness-module-map
kind: story
status: draft
title: Map Harness crates to Loom responsibilities
refs:
- provider: taskboard
  reference: L-001
relations:
- decomposes: epic:loom-native-harness
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A reviewed map from every Harness crate to the Loom responsibility it serves, with one disposition
per crate, so each porting story knows whether it depends on a pinned Harness revision, ports
source, or leaves a crate where it is. Loom owns the model/tool loop (Atlas ADR 0071); Harness keeps
serving agentide, agent-platform, metaharness and uilab until each moves (ADR 0071, operator
decision 2026-10-04).

## Scope

- One table in `docs/design/harness-map.md`: crate, what it owns today (Harness README § Layout at
  `798325f0`, release 0.13.3), the `docs/design/loom-design.md` § Owns responsibility it serves or
  `none`, and one disposition: `depend` (git dependency at a pinned revision, as the current
  consumers already do), `port` (source carried into a Loom crate), or `not carried` (naming the
  repository that owns it instead).
- Every § Owns responsibility is named by at least one row or listed as new in Loom; the projected
  catalogue, action selection and argument generation have no Harness counterpart.
- Name the seams a port reuses: `ModelPort` and `ToolPort` (`harness-wire/src/port.rs`),
  `TurnEnvironmentProvider` (`harness-loop/src/environment.rs`), the approval checkpoint
  (`harness-loop/src/approval.rs`).
- A `port` row names the licence the source carries today (Harness is
  `LicenseRef-B10x-Proprietary`, Loom is Apache-2.0).

## ESS

No new noun: the map is a design document and `ess/` is unchanged.

## Constraints

Read-only on `beyond10x/harness`: no commit, tag or release there, and no consumer pin moves.

## Acceptance

`docs/design/harness-map.md` has exactly one row for each of the 14 crates under
`beyond10x/harness/crates` at revision `798325f0` (harness-app-server, harness-cli,
harness-credential, harness-flow, harness-http, harness-loop, harness-mcp, harness-messages,
harness-responses, harness-substrate, harness-toolchain, harness-tools, harness-wire,
harness-xtask), each naming the `loom-design.md` § Owns responsibility it serves or `none` and one
disposition of `depend`, `port` or `not carried`, and every § Owns responsibility is named by at
least one row or listed as new in Loom.

## Source

TASKBOARD L-001; Atlas ADR 0071 and its operator decision of 2026-10-04; `docs/design/loom-design.md`
§ Owns; `beyond10x/harness` README § Layout and AGENTS.md at `798325f0`.
