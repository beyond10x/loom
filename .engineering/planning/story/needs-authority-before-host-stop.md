---
format: aep.planning-md/3
id: story:needs-authority-before-host-stop
kind: story
status: draft
title: A run whose open obligations only a grant moves ends before the host stop
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:frontier-names-movers
revision: 2
---
## Outcome

A run whose open obligations are moved only by actions behind a grant ends before the host stop
is asked, even when the frontier also admits actions the port performs without one.

## Why

takt's probe on 0.34.0 (takt `545c591`): a grant-only fixture ended `HostStop` DiskLow (0 executor
calls), not `NeedsAuthority`, because the handed frontier keeps every performed admissible action
(`crates/loom-commission/src/runtime.rs:3170-3197`) and the host is asked whenever that frontier
admits one without a grant (`runtime.rs:1738-1746`, `3645-3648`). Draft until Timo wants the
documented stop order changed (AGENTS.md § Commission, the host-stop rows).

## Decision needed before acceptance

`RunOutcomeNeedsAuthority` carries a request id (`ess/commission/domains/responsibility.yaml:365-369`),
so it cannot end a run before the executor proposes.

| option | what it does | cost |
|---|---|---|
| A | skip the host stop and let the executor run | a `tests.run` proposal ends `IdleBound`, not deterministic |
| B | end before the executor with the existing `AwaitingApproval { actions }` | no new variant; a consumer reads `AwaitingApproval` |
| C | give `NeedsAuthority` a payload without a request | new variant shape, Breaking |

Recommended: B, deterministic with no new outcome variant.

## Acceptance

- `crates/loom-commission-testkit/tests/host_stop.rs::a_frontier_whose_open_obligations_only_a_grant_moves_ends_needing_authority_beside_performed_actions`
- Unchanged: `host_stop.rs:516`, `:590`; `adversary_w20261022w1_host_stop.rs:347,376`;
  `adversary_w20261022w2_host_stop.rs:62` (their frontiers carry no obligations).

## ESS first

The host-stop declaration in `ess/commission/domains/responsibility.yaml` (:134-140, :166-168,
:327-329) states the new order; the new test fails on that commit.

## Depends on

`story:frontier-names-movers`.

## Not in scope

The unperformable stop (`story:unperformable-names-the-claim`). Both edit `runtime.rs`,
`responsibility.yaml`, the generated tree, AGENTS.md and CHANGELOG, so they run one after the other.

## Scope

- `crates/loom-commission/src/runtime.rs` (:330-345, :1738-1746, :3645-3648)
- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`
- AGENTS.md rows 130 and 150 and :332-342; `website/data/status.json:23`;
  `website/docs/reference/commission/` (regenerated); CHANGELOG Breaking line

## Decision status (2026-10-22)

The orchestrator wrote on 2026-10-22: "The stop-order fork (A/B/C) is part of a change to the
documented Loom contract, so it is Timo's; I put it in his queue. Build nothing from #110 until I
send his answer."

Open: options A, B and C above wait on Timo's answer. Until it comes, none of the three stories in
https://github.com/beyond10x/loom/pull/110 is built.
