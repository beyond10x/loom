---
format: aep.planning-md/3
id: story:design-docs-after-the-port
kind: story
status: implemented
title: The design documents describe Loom after its port of harness is gone
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:delete-harness-port
scope:
- confidence: cited
  path: docs/design/harness-map.md
- confidence: cited
  path: docs/design/loom-design.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T11:43:02Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-21T11:43:03Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-21T12:21:01Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

The design documents describe Loom after its port of harness is gone: `docs/design/harness-map.md`
(and the test suites that read it) and `docs/design/loom-design.md` either describe what Loom calls
now (llm for decisions, metaharness for agent work, ADR 0098) or are retired with their suites.
`story:delete-harness-port` (wave 2026-10-21-w5) deleted the port and left these out of its scope.

## Acceptance

- `git grep -nE 'harness::|LoopExecutor|ModelPort' docs/design` prints nothing, or each remaining
  line describes the port as removed in 0.31.0.
- The suites that read `docs/design/harness-map.md` are rewritten or deleted with the document.
- `task check` exits 0.

## Scope

- `docs/design/harness-map.md`, `docs/design/loom-design.md`
- the tests that read `docs/design/harness-map.md` (find them with `git grep -l harness-map crates`)

## Shipped

Shipped inside story:delete-harness-port as 00d8fdff (merged 6855dbaf): docs/design describes Loom after the port, and the Harness map is retired.
