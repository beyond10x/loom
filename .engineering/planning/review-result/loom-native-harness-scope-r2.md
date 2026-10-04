---
format: aep.planning-md/3
id: review-result:loom-native-harness-scope-r2
kind: review-result
status: active
title: loom-native-harness decomposition — scope critic, round 2
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
approve

What I read: 1 parent epic, the Atlas epic it refines, ADR 0076 and 18 story files (the 12 that decompose the parent, plus the other six in the store to check they did not claim parent outcomes). Commands: `aep plan artifact show epic:loom-native-harness`, `aep plan artifact graph`, `aep plan artifact show review-result:loom-native-harness-scope-r1`, `cat -n` on each story and decision-blocker under `.engineering/planning/`, and TASKBOARD lines 54-82.

I extracted 9 promises from the parent (outcome, acceptance, rule) and traced all 9 to a story. The Atlas epic adds two more, which are also covered.

| Promise | Claimed by |
|---|---|
| Loom implements Commission's `AgentExecutor` | `story:agent-executor` |
| Carries over Harness's loop (`loom-native-harness.md:21`) | `story:harness-loop-port`, `story:session-transcript-streaming`; map by `story:harness-module-map` |
| Led by the ESS specification in `ess/` | `story:ess-hard-gate` (ADR 0076's four conditions) |
| Covers L-001 … L-006, L-011 … L-013, I-006 | each id has one story: L-001 `harness-module-map`, L-002 `agent-executor`, L-003 `frontier-projection`, L-004 `selection-revalidation`, L-005 `action-selector`, L-006 `argument-generator`, L-011 `session-transcript-streaming`, L-012 `compaction-contract`, L-013 `interruption-recovery`, I-006 `loom-ess-conformance` and `ess-hard-gate` |
| Runs to the approval stop | `agent-executor` AC2 |
| Catalogue never has `repository.merge` while `tests.pass` is not `TRUE` | `frontier-projection` AC1-2 |
| Action id outside the frontier refused at the execution boundary | `selection-revalidation` AC1; `action-selector` AC1; `harness-loop-port` AC4 |
| `task check` runs the Loom ESS conformance suite | `loom-ess-conformance` (`task conform`, AC1-4) |
| Model types come from `ess generate synthesize` (Rule) | `agent-executor` (`generate`, `drift`, `no-hand-model`) |
| Atlas epic: every skipped scenario named in `ess/SKIPPED.md` | `loom-ess-conformance` AC4 |

Round-1 findings, both resolved:
- **Rust execution target:** `story:loom-ess-conformance` now carries a Rust target in `crates/loom-conformance/` and a `conform` task, with acceptance lines 88-97, matching the cleared `decision-blocker:rust-conformance-target`.
- **Harness loop gap:** `story:harness-loop-port` (new) claims the model client, provider adapters and tool loop, with acceptance lines 223-238.

No reach beyond the parent: all 12 stories trace to a parent or Atlas-epic sentence. The `UNMAPPED:` markers that `ess-hard-gate` closes name L-002, L-005 and the cleared `catalogue-ownership` blocker. `selection-revalidation` narrows "execution boundary" to "before returning a `ProposedAction`", and says so on the open `decision-blocker:effect-invocation-owner` (`story/selection-revalidation.md:219-225`). That is a named omission, not a gap.

No two stories claim the same outcome. `ess-hard-gate` runs validate, compile, synthesize and the marker scan. `loom-ess-conformance` runs the synthesized suite. `agent-executor` owns drift and no-hand-model.

What I could not establish:
- No drafter report was available, so I read the stories' own "Source" lines in its place.
- ADR 0076 is not cited in the parent body (revision 2). I traced `ess-hard-gate` through "led by the ESS specification" and I-006 instead. Whether to add the citation is a drafting matter, not a coverage defect.
- `harness-loop-port` and `agent-executor` both ref L-002, but their outcomes differ, so I did not count it as a duplicate.

```findings
[]
```
