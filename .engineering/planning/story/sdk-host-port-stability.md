---
format: aep.planning-md/3
id: story:sdk-host-port-stability
kind: story
status: implemented
title: One page names the stable loom_sdk surface and the ports a host implements, held by a compile witness
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: README.md
- confidence: inferred
  path: crates/loom-sdk/src/lib.rs
- confidence: inferred
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: inferred
  path: website/data/status.json
- confidence: inferred
  path: website/docs/guides/embed-the-runtime.md
- confidence: inferred
  path: website/docs/reference/sdk-stability.md
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T02:54:28Z", actor: "human:timo", revision: 10}
- {from: "proposed", to: "active", at: "2026-10-14T02:54:28Z", actor: "human:timo", revision: 11}
- {from: "active", to: "implemented", at: "2026-10-14T07:29:12Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

One page, `website/docs/reference/sdk-stability.md`, names the stable crate surface of
`b10x-loom-sdk`: the ports a host implements (model, authority, suspension, case store, effects,
and the rest listed below), the types and functions an embedding calls, and every `loom_sdk` path
control-plane imports. It states the promise Loom makes for them, and a compile witness in
`crates/loom-sdk` implements every listed port through `loom_sdk` paths alone, so the list cannot
name a path that does not exist.

## Why

The crates, with `loom_sdk` pinned by tag, are one of Loom's two stable surfaces: they stay stable
per release, and a break is named in the CHANGELOG. A consumer cannot rely on that until the surface
is written down. The engineering factory is served first, through control-plane, the beyond10x
repository where its integration is proved and whose results move to the factory; control-plane
already embeds `loom_sdk` (`crates/control-plane-runtime`), so its imports are the surface read from
the consumer rather than assumed.

## Acceptance

The page lists every port a host implements and every `loom_sdk` path control-plane imports, with
the promise, and a test fails when the page and the SDK disagree:

- `crates/loom-sdk/tests/host_ports.rs::a_host_implements_every_port_through_loom_sdk_alone`
  implements, with trivial bodies and naming only `loom_sdk::` paths: `AuthorityProvider`,
  `SuspensionPort`, `FallibleCaseStore`, `CaseStore`, `EvaluationTime`, `EffectPort`,
  `ConnectorInvoker`, `ObservationPort`, `EvidencePort`, `ActionSelector`, `ArgumentGenerator`,
  `ModelPort`, `ToolPort`, `BearerSource`, `ApprovalPort`, `TurnEnvironmentProvider`, `HookPort`,
  `LoopSink`, `StreamSink` and `LoopContext`; a port that has no `loom_sdk` path today gets one
  re-exported in `crates/loom-sdk/src/lib.rs`.
- `crates/loom-sdk/tests/host_ports.rs::the_stability_page_lists_exactly_the_ports_this_test_implements`
  reads the page's port table and the test's own list and asserts they are equal.
- `crates/loom-sdk/tests/host_ports.rs::every_control_plane_path_on_the_page_resolves`: one `use`
  per path in the page's "Used by control-plane" table (`loom_sdk::run_until_blocked`,
  `loom_sdk::loom::harness::turn_loop::AgentLoop`, `loom_sdk::loom::selection::{SelectionContext,
  Choice, SelectorError}`, `loom_sdk::loom::arguments::ArgumentContext`,
  `loom_sdk::loom::model::run::{CatalogueEntry, SelectionStrategy}`,
  `loom_sdk::commission::ports::authority::AuthorityProviderError`,
  `loom_sdk::commission::ports::governor::Governor`,
  `loom_sdk::commission::model::primitives::Uuid`, and the rest the page lists), and the test
  asserts the table equals those lines.
- The page states the promise as decided: the surface stays stable within a release line and any
  break is named in that release's CHANGELOG entry under `Breaking:`; a consumer pins a bare-version
  tag (`tag = "<version>"`) and moves by changing it. It names what is outside the promise: items not
  on the page, `#[doc(hidden)]` items, and generated model types, which change with their ESS
  declaration and are named as breaks when they do.
- `cargo doc -p b10x-loom-sdk --no-deps` builds with no broken intra-doc links, and `task check` and
  `task website` exit 0.

## ESS first

No behaviour change: a documentation page, re-exports of existing items, and compile-only tests. No
type is declared or changed, so `ess/` is untouched. The red step is
`the_stability_page_lists_exactly_the_ports_this_test_implements`, which fails before the page exists.

## Domain relations

None. Ports are code traits of existing crates, not domain relations; the story introduces no noun.

## Not in scope

- Enforcing the CHANGELOG rule at release time (`story:release-names-embedder-breaks`).
- The command surface (`story:command-event-contract`).
- The agent platform's surface: its needs wait until the engineering factory is served.
- A 1.0 promise, a deprecation window or semantic-version rules beyond the statement above.
- Any change in control-plane.

## Scope

- `website/docs/reference/sdk-stability.md` (new)
- `website/docs/guides/embed-the-runtime.md`: a link to the page
- `crates/loom-sdk/tests/host_ports.rs` (new)
- `crates/loom-sdk/src/lib.rs`: a "Stability" paragraph in the crate doc, and a re-export for any
  listed port without a `loom_sdk` path
- `README.md` and `AGENTS.md`: one line each naming the page (a rule change updates both)
- `website/data/status.json`, `CHANGELOG.md` (**Unreleased**)

Touches none of `crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`,
`ess/intake/`, `generated/rust/intake`, `crates/loom-mandate`, `ess/domains/authority.yaml`,
`generated/rust/loom`, `Cargo.toml`, `Cargo.lock`. It names `AuthorityProvider`, which
`story:mandate-authority` implements in `crates/loom-mandate`; it edits neither the trait nor that
crate. `README.md`, `AGENTS.md`, `status.json` and `CHANGELOG.md` are shared; merge them.

## Ready

Ready now.
