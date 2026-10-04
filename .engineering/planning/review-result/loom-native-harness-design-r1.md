---
format: aep.planning-md/3
id: review-result:loom-native-harness-design-r1
kind: review-result
status: active
title: loom-native-harness decomposition — design critic, round 1
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

story:frontier-projection — its scope names `story:harness-module-map` as the place that decides where projection lives (`TurnEnvironmentProvider`, "expected place, per `story:harness-module-map`"), but the only edge it declares is to `story:agent-executor`, so nothing orders it after the map; add `depends_on story:harness-module-map`, or drop the reference. — .engineering/planning/story/frontier-projection.md:29 and `aep plan artifact graph` (frontier-projection edges: decomposes, depends_on agent-executor, serves only)

story:frontier-projection — its second domain relation is inferred from `canon/crates/canon/src/lib.rs:52` (`actions: Vec<ActionCandidate>` at canon `cf29c4b`), which Canon's wave-1 change removes, and the story records no dependency on Commission, which will define Frontier; re-ground that relation on Commission's Frontier and add a "Depends on, outside this store" line for `commission:story:frontier-admission` (M-002). — .engineering/planning/story/frontier-projection.md:48, and commission `aep plan artifact show story:frontier-admission` ("works on Canon's `Frontier`")

story:selection-revalidation — its outcome and acceptance assume Loom hands the action to an "execution adapter" that its fake records calls on, but whether Loom or the Commission runtime invokes the effect is the open `decision-blocker:effect-invocation-owner`, which blocks only the two binding stories. Commission's `story:stale-revision-action-request` and `story:local-runtime-loop` already revalidate before anything is done with the request. Add that blocker with `blocks story:selection-revalidation`, or restate the story to end at returning `ProposedAction` with no adapter. — .engineering/planning/story/selection-revalidation.md:20 and :47; `aep plan artifact show decision-blocker:effect-invocation-owner` (§ Evidence, "Commission runtime invokes"); `docs/design/loom-design.md` § Commission integration ("Commission/runtime revalidates")

story:loom-ess-conformance — its scope excludes running the suite and points to `decision-blocker:rust-conformance-target` as the open question, but that blocker is cleared. Its answer is that Loom's I-006 runs the synthesized suite through a Rust target on the `ess-conformance` crate, as Mandate does. As drafted, only the generate and drift-check half exists, and the epic acceptance clause "`task check` runs the Loom ESS conformance suite" is in no story. Fold the Rust runner into this story, or add a story that depends on it. — .engineering/planning/story/loom-ess-conformance.md:37-39 and `aep plan artifact show decision-blocker:rust-conformance-target` (§ Answer 2026-10-04)

What I read: 10 stories, the epic, the 3 blockers that touch them, the Commission stories `agent-executor-port`, `frontier-admission`, `governor-port`, `local-runtime-loop`, `run-outcomes`, `stale-revision-action-request` and `authority-provider-port`, `ess/domains/run.yaml`, `loom-design.md`, the contract, `AGENTS.md` and `crates/loom/src/lib.rs`. Commands: `aep plan artifact show`, `relations`, `graph` (both stores), `validate` (valid), `waves`. I walked all 120 or so edges in the graph, outside the set too. There is no cycle and no `depends_on` from Commission to Loom (grep of Commission's graph for "loom" returned nothing), so ADR 0075 holds. The set is two lanes (executor → projection → selector → arguments → revalidation → recovery, and executor → session → compaction) joined by a fan-in, so it is not a queue.

What I could not establish:
- Whether the `selection-revalidation` → `argument-generator` edge is needed. The body never mentions arguments and no reason sits beside the edge, so revalidation may only need `action-selector`. This is an unease, not a finding.
- Out of my lane, for the parallel-safety critic: every story edits `ess/domains/run.yaml`, the generated model crate and `crates/loom/src/lib.rs`, and none declares a scope.
- Out of my lane, for the acceptance critic: no story acceptance names a conformance scenario, although workspace `AGENTS.md` § "ESS drives every product repository" asks for one.
- The Canon wave-1 removal is taken from your note. I did not read Canon.

```findings
- file: .engineering/planning/story/frontier-projection.md
  line: 29
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: the scope defers projection placement to story:harness-module-map but the story declares no depends_on edge to it, so the ordering is unrecorded; add depends_on story:harness-module-map or drop the reference
- file: .engineering/planning/story/frontier-projection.md
  line: 48
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: the Frontier.actions relation is inferred from canon/crates/canon/src/lib.rs:52, which Canon's wave-1 change removes, and the story records no dependency on Commission story frontier-admission (M-002), which will define Frontier; re-ground the relation on Commission's Frontier and record that dependency
- file: .engineering/planning/story/selection-revalidation.md
  line: 20
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: the outcome and acceptance presume Loom invokes the effect through an execution adapter, which is the open decision-blocker:effect-invocation-owner, and Commission's action-request and runtime-loop stories already revalidate; add blocks from that blocker to this story or restate the story to end at returning ProposedAction
- file: .engineering/planning/story/loom-ess-conformance.md
  line: 37
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: the scope excludes running the suite and cites decision-blocker:rust-conformance-target as open, but it is cleared with the answer that Loom's I-006 runs the suite through a Rust ess-conformance target, so the epic clause that task check runs the conformance suite is in no story; fold the Rust runner into this story or add a story depending on it
```
