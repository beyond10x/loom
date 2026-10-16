---
format: aep.planning-md/3
id: architecture-decision-record:canon-release-tracking
kind: architecture-decision-record
status: accepted
title: Each Loom release links the current Canon release and guarantees decision bytes equal to that release only
relations:
- decides: story:canon-current-release
revision: 2
transitions:
- {from: "proposed", to: "accepted", at: "2026-10-15T20:32:20Z", actor: "human:timo", revision: 2}
---
## Decision (proposed)

Each Loom release links the Canon release current when it is cut, unless the release names why
not, and Loom holds a conformance test that the governor's `canon-decision/1` bytes equal those of
the linked Canon release's evaluator for the same inputs. The SDK stability page states the linked
Canon release. Loom guarantees decision equality only with that release, not across Canon versions.

## Context

Loom 0.22.0 links Canon 0.4.0 (`crates/loom-governor/Cargo.toml:15`); hosts validate and
model-check protocols with Canon's 0.6.0 CLI. `architecture-decision-record:canon-dependency-allowed`
allows the dependency and sets no version policy.

## Consequences

- A Canon release that changes decision bytes shows as a failing test on Loom's pin move, and the
  Loom release's `Breaking:` sentence names it.
- Pin moves become regular release work, sharing `Cargo.lock` with other pin stories.

## Alternatives

- Guarantee byte identity across a range of Canon versions. Rejected: Loom cannot test versions it
  does not link.
- No policy (status quo). Rejected: hosts cannot tell whether their checked protocol behaves the
  same under the governor.

## Source

story:canon-current-release.
