---
format: aep.planning-md/3
id: architecture-decision-record:canon-dependency-allowed
kind: architecture-decision-record
status: accepted
title: Loom may depend on Canon
relations:
- decides: story:agent-executor
revision: 2
transitions:
- {from: "proposed", to: "accepted", at: "2026-10-04T22:33:15Z", actor: "human:timo", revision: 2}
---
## Decision

Loom may depend on `b10x-canon`. The test `lib_imports_no_canon`
(`crates/loom/tests/agent_executor.rs`) and the sentence "Loom does not depend on `b10x-canon`"
(`AGENTS.md:81`) are removed.

Basis: operator, 2026-10-05: "Loom is forbidden from depending on Canon <- this has no actual reason,
or? remove this stuopid restriction".

## Why the rule existed

- `story:agent-executor` (its "Bootstrap types" item) removed Loom's bootstrap imports of Canon's `Frontier`,
  `ActionCandidate`, `ActionId`, `ActionStatus` because Canon's wave-1 change deleted those types,
  and Loom moved to Commission's generated `Frontier`. Acceptance 6 then guarded that no Canon import
  came back.
- Atlas ADR 0071 says "Loom does not own protocol semantics or case truth". That is about ownership;
  it does not forbid using Canon as a library.
- Atlas ADR 0089 (Context) cites both no-Canon tests as the reason the governor sits outside
  Commission and Loom.

No source gives a reason for Loom specifically beyond the migration guard.

## What stays

- Loom's executor still reads Commission's generated `Frontier`; Canon's types are not used by the
  executor contract.
- Commission's own rule is not changed: "Commission should be domain-neutral" (Atlas
  `docs/design/governed-autonomy/REPOS.md:66`), checked by
  `commission/crates/commission-testkit/tests/skeleton.rs:440-471`.

## Follow-up

Atlas ADRs 0071 and 0089 still state the old rule; the amendment goes into the Atlas ADR for the
repository consolidation under discussion (2026-10-05).
