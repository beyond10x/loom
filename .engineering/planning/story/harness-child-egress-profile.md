---
format: aep.planning-md/3
id: story:harness-child-egress-profile
kind: story
status: active
title: A confinement profile with named egress runs a harness child that must reach its vendor API
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:sdk-confines-any-child
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T04:14:24Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-20T04:14:24Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":1}}}
---
## Outcome

A host can confine a harness child that must reach its vendor's API: a profile whose network is
limited to the egress the host names (Substrate's aperture), with every other confinement guarantee
of the workspace profile kept. Today every Loom launch requests `NetworkMode::None`
(`crates/loom-intake-slice/src/confinement.rs:513`), the only network mode named outside tests.

## Why

A Claude Code or Codex child cannot run with no network, and running it unconfined is what ADR 0099
point 4 replaces. Substrate's wire already has an aperture mode (takt's plan cites
`substrate-wire` `lib.rs:1308-1311`; not read here).

## Acceptance

- `crates/loom-intake-slice/tests/confined_child.rs::a_child_reaches_only_the_named_egress`
  (delegated, skipped with a named missing prerequisite): under the egress profile, a connection to
  a named host is allowed and one to an unnamed host fails.
- `…::an_egress_profile_with_no_named_host_is_refused`: refused before any process starts.
- `…::the_applied_record_names_the_egress`: the reported applied confinement names the aperture,
  and a driver record that reports wider network is refused as the test runner refuses one today.
- `task check` exits 0.

## ESS first

No noun the specification lacks: the egress list is Substrate's own aperture type beside
`NetworkMode` (`crates/loom-intake-slice/src/confinement.rs:20-23`, `:513-514`), so `ess/` is
untouched. The red step is `a_child_reaches_only_the_named_egress`.

## Depends on

`story:sdk-confines-any-child`: the profile applies to the child launch that story adds, in the same
`confinement.rs` and test file.

## Not in scope

- Credentials for the vendor API: they never pass through Loom.
- Choosing which hosts a vendor needs: the host names them.

## Scope

Crates: `loom-intake-slice`.

- `crates/loom-intake-slice/src/confinement.rs`, `crates/loom-intake-slice/tests/confined_child.rs`
- `CHANGELOG.md`, `AGENTS.md`

Shared with `story:sdk-confines-any-child` (before) and `story:python-test-profile` (`confinement.rs`).
