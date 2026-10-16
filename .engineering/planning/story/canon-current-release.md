---
format: aep.planning-md/3
id: story:canon-current-release
kind: story
status: active
title: Loom links the current Canon release and a test holds the governor's decision bytes equal to Canon's
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
- informed_by: story:co-pinned-consumer-check
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: crates/loom-governor/Cargo.toml
- confidence: inferred
  path: crates/loom-governor/tests
- confidence: inferred
  path: website/docs/reference
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-16T16:34:54Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-16T16:34:54Z", actor: "human:timo", revision: 4}
---
## Outcome

Loom links the current Canon release (0.6.0 at the time of writing), and a conformance test shows
that a case evaluated through Loom's governor and through that Canon release's evaluator gives
identical `canon-decision/1` bytes, so a host that checks its protocol with the Canon CLI knows the
governor decides the same.

## Acceptance

`the_governor_decision_equals_the_pinned_canon_decision_bytes` (`crates/loom-governor/tests/`): for
each fixture case (Loom's own protocols and a protocol exercising decisions and independence), the
governor's rendered decision is byte-identical to the decision the pinned Canon release's
`evaluate_with` renders for the same inputs; and `Cargo.lock` resolves one `b10x-canon` at the
current Canon release.

## Why (Loom 0.22.0)

- Loom links Canon 0.4.0 (`crates/loom-governor/Cargo.toml:15`, `Cargo.lock:146-148`), while
  Canon's current release is 0.6.0; a host validating and model-checking its protocol with the
  0.6.0 CLI has no statement that Loom's decisions match.
- Nothing in the repository compares the governor's decision bytes with Canon's.

## Decisions followed

No accepted ADR in this store or in Canon's store names Canon version compatibility;
`architecture-decision-record:canon-dependency-allowed` (accepted) lets Loom depend on Canon and
says nothing about which version. The policy is proposed in
`architecture-decision-record:canon-release-tracking`.

## Sketch

Bump `b10x-canon` to 0.6.0 (and `b10x-canon-engineering` to a release built on it, if required),
add the byte-equality test, and state on the SDK stability page which Canon release a Loom release
links.

## Workaround it removes

The host checks its protocol with the 0.6.0 CLI and keeps the governor's decision record as the one
that applied.

## Scope (inferred)

`Cargo.toml`, `crates/loom-governor/Cargo.toml`, `Cargo.lock`, `crates/loom-governor/tests/`,
`website/docs/reference/` (stability page), `CHANGELOG.md`. Shares `Cargo.lock` with any pin move
(`story:protocol-argument-schema`, `story:co-pinned-consumer-check`).

## Not in scope

Changes to Canon; supporting two Canon versions at once.
