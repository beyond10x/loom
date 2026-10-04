---
format: aep.planning-md/3
id: review-result:loom-native-harness-scope-r1
kind: review-result
status: active
title: loom-native-harness decomposition — scope critic, round 1
relations:
- reviews: epic:loom-native-harness
- reviews: story:action-selector
- reviews: story:agent-executor
- reviews: story:argument-generator
- reviews: story:compaction-contract
- reviews: story:frontier-projection
- reviews: story:harness-module-map
- reviews: story:interruption-recovery
- reviews: story:loom-ess-conformance
- reviews: story:selection-revalidation
- reviews: story:session-transcript-streaming
revision: 1
---
needs-revision
story:loom-ess-conformance — the epic promises that `task check` runs the Loom ESS conformance suite, but this story narrows that to synthesize-and-drift-check and defers execution to `decision-blocker:rust-conformance-target`, which is already cleared and says "Loom's I-006 does the same" (a Rust target on `ess-conformance`); the body needs a Rust-target execution scope item and acceptance clause — .engineering/planning/story/loom-ess-conformance.md:37 (promise at .engineering/planning/epic/loom-native-harness.md:29-30)
epic:loom-native-harness — "carries over Harness's loop" is promised, but no story claims porting the model client, provider adapters and tool loop; `story:harness-module-map` only assigns dispositions, `story:session-transcript-streaming` covers sessions, transcripts and streaming, and `story:reasoning-model-selector` says "No story for either existed", so `story:agent-executor` or `story:session-transcript-streaming` should take the port or the gap should be named as undecided — .engineering/planning/epic/loom-native-harness.md:21 (corroborated at .engineering/planning/story/reasoning-model-selector.md:48-49)

What I read: the parent epic, its 10 decomposing stories, `epic:ga-loom-native-harness` in the Atlas store, the two blockers on the parent (`catalogue-ownership`, `rust-conformance-target`), TASKBOARD L-001…L-006, L-011…L-013 and I-006, via `aep plan artifact show` and `aep plan artifact graph`. I extracted 8 promises from the parent and traced 6 cleanly to a story. The two findings above are the other two.

Everything else traces as follows:
- **TASKBOARD ids:** each of the 10 ids maps to exactly one story (L-001 harness-module-map, L-002 agent-executor, L-003 frontier-projection, L-004 selection-revalidation, L-005 action-selector, L-006 argument-generator, L-011 session-transcript-streaming, L-012 compaction-contract, L-013 interruption-recovery, I-006 loom-ess-conformance).
- **Duplicates and reach:** no two stories claim the same outcome, and no story reaches beyond the parent. The `epic:fast-selector` and `epic:effect-bindings` stories are excluded as not decomposing this parent.
- **Named exclusions:** the argument-schema validation and the catalogue-ownership marker are held by open blockers, which is honest omission and not a gap.

What I could not establish:
- Whether the loop-port gap was a deliberate omission. I had no drafter report, and nothing in the stories or blockers names it.
- Out of my lane, not counted in the verdict: the model-type drift check is placed in `story:agent-executor` and referenced from `story:loom-ess-conformance`, which is a design-critic matter.
- Out of my lane, not counted in the verdict: the `depends_on` edges between the stories are for parallel-safety and design to judge.

```findings
[
  {
    "file": ".engineering/planning/story/loom-ess-conformance.md",
    "line": 37,
    "category": "scope",
    "severity": "blocker",
    "verdict": "needs-revision",
    "origin": "introduced",
    "message": "the epic promises \"task check runs the Loom ESS conformance suite\" but this story narrows to synthesize and drift-check and defers execution to decision-blocker:rust-conformance-target, which is cleared with the answer that Loom's I-006 runs the suite through a Rust target on ess-conformance; the body needs a Rust-target execution scope item and acceptance clause"
  },
  {
    "file": ".engineering/planning/epic/loom-native-harness.md",
    "line": 21,
    "category": "scope",
    "severity": "warning",
    "verdict": "needs-revision",
    "origin": "introduced",
    "message": "\"carries over Harness's loop\" is promised and no story claims porting the model client, provider adapters and tool loop (module-map assigns dispositions only, session-transcript-streaming covers sessions/transcripts/streaming, reasoning-model-selector.md:48-49 says no story exists); agent-executor or session-transcript-streaming should claim it or the omission should be named"
  }
]
```
