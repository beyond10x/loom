---
format: aep.planning-md/3
id: story:bounded-context
kind: story
status: implemented
title: Bound CLI working context with retrievable history
relations:
- decomposes: epic:vertical-slices
- depends_on: story:result-references
- serves: vision:O3
scope:
- confidence: cited
  path: crates/loom-cli
- confidence: cited
  path: crates/loom-intake-slice/src
- confidence: cited
  path: ess/intake/domains/context.yaml
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-06T20:46:03Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-06T20:46:03Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-06T20:59:20Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":3}}}
---
## Outcome
Implement the operator-approved bounded working context plan for single-intent CLI runs, opt-in with legacy unchanged. Briefing owns state and history, results retain bulk payloads; governed-loop compaction is excluded.

## Acceptance
Recorded workflows match final files, typed test observations and refusals in both modes. Every serialized bounded request is at most 65536 bytes including schemas and escaped lookups. Long history reduces cumulative bytes including retrieval overhead. Exercise >64 events, retrieval of early failure, reference edits after retirement, stale tests after edits, forged success text, changed approvals, Unicode/escaping, mandatory oversize, and archive exhaustion. Capacity failure stops later model calls without hiding completed effects. Both stages allow eight deterministic lookups, whole records only. Report request bytes by phase, checkpoints, retrievals, model calls, elapsed time and optional provider usage without source data. No summarization model calls.

## ESS first
ESS-only commit e2dc1b9 declares intake.context in ess/intake/domains/context.yaml. Before regeneration, task intake-drift failed (task 201, inner 1): src/context.rs missing. Both preexisting ESS gates passed. The generated declarations precede implementation; runtime acceptance tests cover the contract (type-only ESS synthesis has zero scenarios).

## Scope
Cited owner: crates/loom-intake-slice/src/selector.rs Briefing and model exchanges; results.rs existing exact selection semantics; run.rs embedding boundary; crates/loom-cli/src command entry. New context and metrics modules remain inside intake-slice. ESS intake types, generated Rust and docs, CLI/README/AGENTS/status/changelog and recorded tests follow the same change.

## Authorization
User explicitly requested implementation of the supplied plan in this session, including opt-in rollout. No release or default-policy change is authorized or intended.
