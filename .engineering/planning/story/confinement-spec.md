---
format: aep.planning-md/3
id: story:confinement-spec
kind: story
status: active
title: A host declares a child's confinement as one ConfinementSpec, and a field the pin cannot honour is refused
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:several-egress-apertures
- depends_on: story:child-workdir
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T08:15:51Z", actor: "human:timo", revision: 2, correlation: "wave-2026-10-23-w3"}
- {from: "proposed", to: "active", at: "2026-10-23T08:15:51Z", actor: "human:timo", revision: 3, correlation: "wave-2026-10-23-w3"}
---
## Outcome

A host declares a child's confinement as one `ConfinementSpec` value (read-only roots and files,
writable roots, workdir, egress apertures, certificate bundle, timeout, profile) and
`ChildCommand::confined(&spec)` applies it, refusing every field the pinned Substrate cannot honour
with the field named.

## Why

Each field is a separate builder today (`confinement.rs:1074-1246`), and Agency stores an agent's
confinement as data (design § 3.2). A field silently ignored would let a child write where the
host did not mean.

## Acceptance

- `crates/loom-intake-slice/tests/confinement_spec.rs`
  (`a_spec_equals_its_builders`): a spec and the same builder calls give equal `ChildCommand`s.
- (`a_field_the_pin_cannot_honour_is_refused_naming_it`): a non-empty `writable_roots` before
  `story:writable-host-root` lands is refused naming `writable_roots`.
- Limits stay as the builders hold them: at most 4 read-only roots and 16 files; stdin capped at
  16 MiB, absent by default (`confinement.rs:1011`, `:1034`).
- `ConfinementSpec` is exported through `loom_sdk` and listed on `sdk-stability.md`.

## ESS first

`ConfinementSpec` is declared in `intake.confinement` (or `loom.agent`, if `story:agent-domain`
puts it there) and generated; `task intake-drift` is red first. No hand-written type shadows it.

## Not in scope

New Substrate features; this story only gathers the builders.

## Scope

Crates: `loom-intake-slice`, `loom-sdk`, `ess/intake/`. `confinement.rs`, the new test,
`website/docs/reference/sdk-stability.md`, `crates/loom-sdk/tests/host_ports.rs`, CHANGELOG.md.

## Ready

Waits on `story:several-egress-apertures` and `story:child-workdir`; `writable_roots` stays refused
until `story:writable-host-root`.
