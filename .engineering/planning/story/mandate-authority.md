---
format: aep.planning-md/3
id: story:mandate-authority
kind: story
status: draft
title: A host answers the authority port with Mandate
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
revision: 1
---
## Outcome

A host can answer Commission's authority port with Mandate: an `AuthorityProvider` in Loom asks
Mandate's `check` for each action request (subject, actor, action, resource) and maps its
decision to Commission's grant, approval-required or denial; a Mandate failure yields no grant.

## Why

`epic:effect-bindings` asks that a consequential action execute after a Mandate approval. Loom
links no Mandate crate today; its authority providers are the testkit's fakes, `PluginAuthority`
and the CLI's `GrantAuthority`. Mandate v0.9.0 (2026-10-13) ships `mandate_authz::check` with
approval-required decisions and execution-bound delegations.

## Depends on

Mandate only (tag `v0.9.0`). No Connectors or Substrate change.

## Acceptance (draft)

- An action request Mandate allows is granted; one it denies is denied; an approval-required
  decision suspends as today's approval gate does; an unreachable or failing Mandate grants
  nothing (AGENTS.md § Rules: fail toward less authority).
- Commission core gains no dependency; the provider lives in a host-side crate.

## ESS first

The mapping from Mandate's decision to Commission's authority outcome is declared before code.
