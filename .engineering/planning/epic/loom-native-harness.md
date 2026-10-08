---
format: aep.planning-md/3
id: epic:loom-native-harness
kind: epic
status: implemented
title: Loom native harness implements AgentExecutor
summary: ESS-led frontier projection, selection/argument split, revalidation, and the Harness loop ported in.
refs:
- provider: atlas
  reference: epic:ga-loom-native-harness
relations:
- serves: vision:governed-autonomy
- serves: vision:O1
- serves: vision:O3
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-04T00:01:26Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-04T01:17:41Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-08T10:40:12Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":1}}}
---
## Outcome

Loom implements Commission's AgentExecutor and carries over Harness's loop, led by the ESS
specification in `ess/`. Covers TASKBOARD L-001 … L-006, L-011 … L-013 and I-006 (the Loom ESS
specification).

## Acceptance

With Commission's scripted fake governor serving the software-change frontier, Loom runs to the
approval stop: the catalogue never contains `repository.merge` while `tests.pass` is not `TRUE`, an
action id outside the frontier is refused at the execution boundary, and `task check` runs the Loom
ESS conformance suite.

## Rule

Model types come from `ess generate synthesize`, not hand transcription (ess:specifying).

## Source

Atlas `epic:ga-loom-native-harness`; Atlas ADRs 0071, 0072; `docs/design/loom-design.md`.
