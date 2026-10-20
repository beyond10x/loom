---
format: aep.planning-md/3
id: story:registration-name-protocol-id
kind: story
status: draft
title: A protocol id with a dot registers, or its refusal names the registration form
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: cited
  path: crates/loom-protocols/tests/catalog.rs
revision: 2
---
## Outcome

A host registering a protocol whose id has a dot (`self.improvement/1`) either registers it under
its own id, or is refused with a message that names the registration form the catalog expects for
that id and why. Today `ProtocolCatalog::add_yaml` refuses `self.improvement@1`:
`registration_major` (`crates/loom-protocols/src/lib.rs:208-227`) admits only lowercase letters,
digits and hyphens before `@`, and its refusal says so without naming the mapping the bundled
catalog follows (`software.change/1` is registered as `software-change@1`).

## Why

takt registers its own protocol `self.improvement/1` with loom-sdk and had to guess the name
`self-improvement@1` (reported by takt through adr-changes, 2026-10-20).

## Acceptance

- `crates/loom-protocols/tests/catalog.rs::a_protocol_id_with_a_dot_registers_under_the_documented_name`:
  the decision taken in this story holds, either `add_yaml("self.improvement@1", …)` registers, or
  its refusal names `self-improvement@1` as the registration for id `self.improvement/1`.
- The bundled names and every stored user catalog read as before (`crates/loom-protocols/tests/` pass unchanged).
- `task check` exits 0.

## Decision to take in the story

Accept the protocol's own id (a second accepted spelling, which every place that matches names must
then treat as the same protocol), or keep one form and make the refusal name it. The second changes
no stored name.

## ESS first

No noun the specification lacks; the registration name is not declared in `ess/`. The red step is
`a_protocol_id_with_a_dot_registers_under_the_documented_name`.

## Scope

- `crates/loom-protocols/src/lib.rs`, `crates/loom-protocols/tests/catalog.rs`
- `CHANGELOG.md`, `AGENTS.md` (§ Protocol composition and system queries)
