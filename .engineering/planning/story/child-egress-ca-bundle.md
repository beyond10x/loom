---
format: aep.planning-md/3
id: story:child-egress-ca-bundle
kind: story
status: implemented
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
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T08:57:07Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-20T08:57:08Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T15:47:26Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
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

## Correction at close (wave 2026-10-20-w3)

- What shipped: `ChildCommand::with_ca_bundle`. Loom reads the bundle once (its last component not
  followed, a regular file, at most 1 MiB) and hands Substrate a private read-only copy, which the
  child gets as `SSL_CERT_FILE`; the bundle's path and bytes are part of the shared driver's key.
  `b10x-loom-intake-slice` gained a `libc` dependency for the no-follow open.
- Refused before launch, besides the Acceptance's cases: a bundle with no aperture, relative, not
  UTF-8, a link, no regular file, empty, unreadable or over 1 MiB, and one beside an environment
  naming a listed trust-store variable (13 names) or a `GIT_CONFIG_KEY_<n>` naming a Git trust key.
- Two adversary passes (a blocker and five lesser findings, all fixed). The list keeps a host from
  overriding its bundle by accident; it is no guarantee about what the child trusts (configuration
  under `HOME`, a program's own flags); the aperture bounds what the child reaches.
- Still open: Substrate serves one aperture per run.
