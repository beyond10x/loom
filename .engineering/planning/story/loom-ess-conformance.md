---
format: aep.planning-md/3
id: story:loom-ess-conformance
kind: story
status: draft
title: Carry the Loom ESS specification to a synthesized conformance suite in task check
refs:
- provider: taskboard
  reference: I-006
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:agent-executor
- depends_on: story:frontier-projection
- depends_on: story:action-selector
- depends_on: story:argument-generator
- depends_on: story:selection-revalidation
- depends_on: story:session-transcript-streaming
- depends_on: story:compaction-contract
- depends_on: story:interruption-recovery
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

The Loom ESS specification, complete for this epic, carried to a synthesized conformance suite that
`task check` keeps current. Every command the L stories added has scenarios, and every UNMAPPED
marker left in `ess/` is held by an open decision-blocker.

## Scope

- `task check` runs `ess specify validate --path ess`, then `ess verify conform synthesize --path ess`
  into a temporary path and compares it with the committed suite, beside the model drift check that
  `story:agent-executor` adds.
- 0 refusals; every synthesize `note:` relayed in the closing report.
- Executing the suite against the Rust implementation is not in this story: ess 0.52.0 writes
  runnable suites only for Go and TypeScript, and its built-in `run` targets do not execute an
  adopter implementation. That question is `decision-blocker:rust-conformance-target`.

## Acceptance

`task check` fails when the committed Loom conformance suite differs from a fresh
`ess verify conform synthesize --path ess`, and the committed suite holds at least one scenario for
every command in `ess/` with 0 refusals.

## Source

TASKBOARD I-006; Atlas ADR 0071; workspace AGENTS.md § ESS drives every product repository;
`ess:specifying` § Conformance is a record, not a claim.
