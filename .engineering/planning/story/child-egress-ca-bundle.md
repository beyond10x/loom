---
format: aep.planning-md/3
id: story:child-egress-ca-bundle
kind: story
status: draft
title: A confined child under an egress aperture trusts the CA bundle the host names
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:harness-child-egress-profile
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
revision: 2
---
## Outcome

A host that confines a child under an egress aperture names the CA bundle the child trusts, so a
child reaching an HTTPS vendor API through the aperture verifies its certificate. The bundle is part
of the shared driver's key. Today `ChildCommand::with_egress` names one aperture and no trust anchor:
Substrate's `HostConfig.ca_bundle` is not exposed, so a real HTTPS vendor API fails certificate
checks under the aperture (story:harness-child-egress-profile, wave 2026-10-20-w2, U5 report).

## Why

takt's confined-harness-child story runs Claude Code or Codex under a profile that reaches the
vendor's API (Atlas ADR 0099 point 4). Over HTTPS, without a trust anchor, that call fails.

## Acceptance

- `crates/loom-intake-slice/tests/confined_child.rs::a_child_under_an_aperture_trusts_the_named_ca_bundle`
  (delegated, loopback only, skipped on a named missing prerequisite): a child reaches a loopback TLS
  listener signed by a test CA when the host names that CA's bundle, and fails certificate
  verification when it names another.
- Two children with different bundles in one directory never share a driver.
- `task check` exits 0.

## Open

- Substrate allows one aperture per run (`substrate-wire` `ConfinementRequest.aperture: Option<String>`,
  per the U5 report). A harness that needs several hosts is not served; whether that is Substrate's
  change or a Loom story is not decided.

## Depends on

story:harness-child-egress-profile.

## Scope

- `crates/loom-intake-slice/src/confinement.rs`, `crates/loom-intake-slice/tests/confined_child.rs`
- `CHANGELOG.md`, `AGENTS.md`
