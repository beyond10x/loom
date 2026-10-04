---
format: aep.planning-md/3
id: review-result:loom-native-harness-acceptance-r2
kind: review-result
status: active
title: loom-native-harness decomposition — acceptance critic, round 2
relations:
- reviews: epic:loom-native-harness
- reviews: story:action-selector
- reviews: story:agent-executor
- reviews: story:argument-generator
- reviews: story:compaction-contract
- reviews: story:ess-hard-gate
- reviews: story:frontier-projection
- reviews: story:harness-loop-port
- reviews: story:harness-module-map
- reviews: story:interruption-recovery
- reviews: story:loom-ess-conformance
- reviews: story:selection-revalidation
- reviews: story:session-transcript-streaming
revision: 1
---
needs-revision

ess-hard-gate — the lead sentence says the test `ess_gate` checks items 1 to 7, but item 6 (appending `# UNMAPPED: probe` to the working tree and requiring `task ess-gate` and `task check` to fail) cannot be a check that test performs, because `task check` runs the test and the test would call `task check` again, so the acceptance does not say who observes item 6 — .engineering/planning/story/ess-hard-gate.md:104
harness-loop-port — item 1 compares the request body "for each provider wire in `crates/loom/tests/fixtures/provider-wires/`", a directory this story creates, so it passes with zero wires and with one adapter unported, and the acceptance never names `anthropic-messages` and `openai-responses` that the Outcome copies in — .engineering/planning/story/harness-loop-port.md:100
harness-loop-port — the licence clause of item 5 ("every package that holds ported source with licence `Apache-2.0`") is already true before the port, because `Cargo.toml:7` sets the workspace licence to `Apache-2.0` and `crates/loom/Cargo.toml` inherits it, and it names no check that ported files carry that licence or that none carries `LicenseRef-B10x-Proprietary` — .engineering/planning/story/harness-loop-port.md:109
harness-loop-port — the Outcome promises "`beyond10x/harness` is not changed: no commit, tag or release there, and no consumer pin moves", and no acceptance item observes it — .engineering/planning/story/harness-loop-port.md:96
agent-executor — the Outcome says the Canon bootstrap imports (`lib.rs:5`, `ActionCandidate`, `ActionId`, `ActionStatus`, `Frontier`) "are removed", but `no-hand-model` (item 5) keys on `ess/` entity names and item 2 passes with those imports still present, so nothing observes the removal — .engineering/planning/story/agent-executor.md:127

**Read:** 12 of 12 stories that decompose `epic:loom-native-harness`, plus the epic and `review-result:loom-native-harness-acceptance-r1`. Commands: `aep plan artifact list`, `kinds`, `lifecycle story`, `show` on the epic and the review, and `cat`/`sed` over each story's Outcome and Acceptance. In the tree I checked `ess/` for `UNMAPPED`, `Taskfile.yml` and `Cargo.toml`. The four `UNMAPPED:` markers still sit at `ess/domains/run.yaml:3,44,81,99`, so `ess-hard-gate` items 2 and 4 are real transitions.

Round-1 acceptance findings: all 10 are addressed. I raise nothing against compaction-contract, session-transcript-streaming, selection-revalidation, interruption-recovery, argument-generator, harness-module-map or loom-ess-conformance. The numbered checks under one named test read as one checkable acceptance. The epic clause "task check runs the Loom ESS conformance suite" is now owned by `story:loom-ess-conformance` (:88). `story:frontier-projection` and `story:action-selector` have no finding.

**Not established:**
- I did not run `ess`, so whether the ESS-first changes will validate is unknown.
- The probe results quoted in `ess-hard-gate` (0 refusals, `Optional<Decimal>`) are the drafter's, not re-run.
- I did not check whether the Commission fake governor (M-009) delivers what the acceptances assume.

**Out of my lane:**
- The chain length claim ("eleven stories").
- Canon's wave-1 deletion of `Frontier`, `ActionCandidate` and related types, which several stories cite.

```findings
[
  {"file": ".engineering/planning/story/ess-hard-gate.md", "line": 104, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the lead sentence says the test `ess_gate` checks items 1 to 7, but item 6 (append `# UNMAPPED: probe` to the working tree, `task ess-gate` and `task check` must fail) cannot be a check that test performs, because `task check` runs the test and the test would call `task check` again, so the acceptance does not say who observes item 6"},
  {"file": ".engineering/planning/story/harness-loop-port.md", "line": 100, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "item 1 compares the request body 'for each provider wire in `crates/loom/tests/fixtures/provider-wires/`', a directory this story creates, so it passes with zero wires and with one adapter unported, and the acceptance never names `anthropic-messages` and `openai-responses` that the Outcome copies in"},
  {"file": ".engineering/planning/story/harness-loop-port.md", "line": 109, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the licence clause of item 5 ('every package that holds ported source with licence `Apache-2.0`') is already true before the port, because `Cargo.toml:7` sets the workspace licence to `Apache-2.0` and `crates/loom/Cargo.toml` inherits it, and it names no check that ported files carry that licence or that none carries `LicenseRef-B10x-Proprietary`"},
  {"file": ".engineering/planning/story/harness-loop-port.md", "line": 96, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the Outcome promises '`beyond10x/harness` is not changed: no commit, tag or release there, and no consumer pin moves', and no acceptance item observes it"},
  {"file": ".engineering/planning/story/agent-executor.md", "line": 127, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the Outcome says the Canon bootstrap imports (`lib.rs:5`, `ActionCandidate`, `ActionId`, `ActionStatus`, `Frontier`) 'are removed', but `no-hand-model` (item 5) keys on `ess/` entity names and item 2 passes with those imports still present, so nothing observes the removal"}
]
```
