---
format: aep.planning-md/3
id: story:agent-executor-host-port
kind: story
status: implemented
title: AgentExecutor is listed among the ports a host implements
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:authority-sees-the-request
- depends_on: story:governor-exposes-canon-decision
- depends_on: story:sdk-confines-any-child
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T17:46:13Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-20T17:46:14Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-10-20T23:18:25Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":3}}}
---
## Outcome

`AgentExecutor` is listed among the ports a host implements on the SDK stability page, with a test
implementing it through `loom_sdk` alone, so a release that changes it must say so. Today the page
lists it only as a type an embedding calls (`website/docs/reference/sdk-stability.md:138`), not in
"Ports a host implements" (`:52-79`). The row at `:138` stays: the types table lists what
control-plane imports, and `every_control_plane_path_on_the_page_resolves` checks it.

## Why

takt implements `AgentExecutor` (its harness step, its validator executor). A port takt implements
that Loom can change silently is the break `crates/loom-sdk/tests/host_ports.rs` exists to catch.

## Acceptance

- `crates/loom-sdk/tests/host_ports.rs::a_host_implements_every_port_through_loom_sdk_alone`
  implements `AgentExecutor` through `loom_sdk::commission::ports::executor::AgentExecutor`.
- `…::the_stability_page_lists_exactly_the_ports_this_test_implements` passes with the new row in
  the ports table, and `…::every_control_plane_path_on_the_page_resolves` passes with the types row
  kept.
- `task check` exits 0.

## ESS first

No behaviour change: a documentation row and a test of an existing trait. `ess/` is untouched.

## Depends on

`story:authority-sees-the-request`, `story:governor-exposes-canon-decision` and
`story:sdk-confines-any-child`, which change `host_ports.rs` or the stability page first (the page's
pin table is changed by `story:engineering-protocols-090`, before the governor story).

## Not in scope

- Changing `AgentExecutor`.

## Scope

Crates: `loom-sdk` (test only).

- `crates/loom-sdk/tests/host_ports.rs`, `website/docs/reference/sdk-stability.md` (one row in the
  ports table)
- `CHANGELOG.md`

`host_ports.rs` and the page are also edited by `story:delete-harness-port` and
`story:metaharness-session-executor`, which land after this story.

## Correction at close (wave 2026-10-20-w4)

- Adversary pass 1 found the ports table left out four more traits a host supplies; `Governor`,
  `CredentialResolver`, `TestRunner` and `Briefing` are now rows, each implemented through `loom_sdk`
  alone in `host_ports.rs`, and `loom_sdk` re-exports `serde_json` for `Briefing::arguments_schema`.
  `Briefing`'s provided methods take llm types no `loom_sdk` path reaches.
- The adversary's cases guard what the unit-struct witness missed: a `Sync` or `'static` supertrait
  on `AgentExecutor`, a dyn-incompatible provided method, and the control-plane table's kind column.
