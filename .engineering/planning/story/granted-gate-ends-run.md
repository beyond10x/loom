---
format: aep.planning-md/3
id: story:granted-gate-ends-run
kind: story
status: draft
title: A granted gated action ends a Commission run as awaiting approval
relations:
- serves: vision:O1
revision: 1
---
## Why

Found by wave 2026-10-09-w2 U3 (plugin host): after one granted `source.read` on `inbound-answer@1`,
`run_until_blocked` ends the Run `AwaitingApproval [reply.propose, source.read]`. The gate fires
because the gated actions are unchanged, although the authority provider granted their
capabilities (`crates/loom-commission/src/runtime.rs:514`; `runtime_effect.rs:1047` holds the
behaviour). The plugin host works around it by starting another Run on the same case.

## Acceptance

- Decide in a design note whether an action whose required capability the authority grants counts
  as awaiting approval; if not, `run_until_blocked` continues past it. Test against the current
  `runtime_effect.rs:1047` case, which then states the decided behaviour.
- If the behaviour changes, the plugin host's re-run rule is removed and its test
  `a_turn_that_only_reads_stops_at_its_budget` still passes.

## Scope

`crates/loom-commission/src/runtime.rs`, `crates/loom-commission/tests/`, `docs/commission/`.
