---
format: aep.planning-md/3
id: review-result:loom-native-harness-parallel-safety-r1
kind: review-result
status: active
title: loom-native-harness decomposition — parallel-safety critic, round 1
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

- story:session-transcript-streaming — it and story:frontier-projection, story:action-selector, story:argument-generator and story:selection-revalidation are not ordered against each other, and all of them edit `ess/domains/run.yaml` and regenerate the synthesized model. This story extends `loom.run.Session` and `Turn` (run.yaml:46-78). The others add commands on `ActionCatalogue` (run.yaml:83), `Selection` (run.yaml:100) and `ArgumentRequest` (run.yaml:123). Cited for the `run.yaml` and regenerate-the-model surface, inferred for the shared `b10x-loom` test location. No body says another story edits the same file. Remedies: a `depends_on` edge recording `ess/domains/run.yaml` and the generated model crate as the reason, or splitting the surface. — `.engineering/planning/story/session-transcript-streaming.md:30`
- story:compaction-contract — it and story:interruption-recovery both add commands and outcomes on `loom.run.Session`, the same entity in the same file. They share no edge: compaction-contract depends on session-transcript-streaming and frontier-projection, while interruption-recovery depends on session-transcript-streaming and selection-revalidation. Both bodies are silent about the other, and both regenerate the model. Cited from both bodies. Remedies: an ordering edge naming `loom.run.Session` in `ess/domains/run.yaml` as the reason, or splitting the surface. — `.engineering/planning/story/compaction-contract.md:30`
- story:compaction-contract — it is not ordered against story:action-selector, story:argument-generator or story:selection-revalidation. It edits `loom.run.Session` and they edit `Selection` and `ArgumentRequest`, all in `ess/domains/run.yaml`, and all regenerate the same synthesized model crate. Cited for `run.yaml`, inferred for the test location. The body does not admit the shared surface. Remedies: an ordering edge naming the shared file, or splitting the surface. — `.engineering/planning/story/compaction-contract.md:30`
- story:agent-executor — it creates the generated model crate ("for example `crates/loom-model`", a path not yet decided and not yet in the tree). The other ESS stories, which all depend on it directly or by chain, regenerate that crate as "the synthesized model" without naming a path. The body does not fix the crate path or say that every later ESS story rewrites it. Cited for the creation (it does not exist yet), inferred for the downstream rewrites. Remedy: fix the path and record that later stories regenerate it, or give each story its own output. — `.engineering/planning/story/agent-executor.md:29`

Read: 10 stories that decompose `epic:loom-native-harness`, plus the epic, `crates/loom/src/lib.rs`, `ess/domains/run.yaml` and `Taskfile.yml`. Commands run: `aep plan artifact list`, `aep plan artifact graph`, `aep plan artifact waves --kind story` and `cat` on each story body. Surface count: 10 cited, 0 inferred, 0 unplaced. The generated model crate and the test location are inferred, not cited.

Could not establish:
- `aep plan artifact waves --kind story` reports "0 wave(s), 0 collision(s), 16 unassessed" because no story declares a scope. Scopes recorded with `aep artifact scope` would let the tool find these collisions itself. The wave tool has not assessed any of these 10 stories.
- The exact test files each story adds under `crates/loom` are not named in any body. Those collision claims are inferred.
- story:harness-module-map lands only on `docs/design/harness-map.md` (cited), which no other story touches, so it has no collision.
- Out of my lane, not counted in the verdict: the sequencing and split questions behind these pairs belong to the design critic.

```findings
- file: .engineering/planning/story/session-transcript-streaming.md
  line: 30
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: no ordering edge to story:frontier-projection, story:action-selector, story:argument-generator or story:selection-revalidation, yet all edit ess/domains/run.yaml and regenerate the synthesized model (cited for run.yaml, inferred for the shared b10x-loom test location) and no body admits it; add an ordering edge recording the shared file or split the surface
- file: .engineering/planning/story/compaction-contract.md
  line: 30
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: story:compaction-contract and story:interruption-recovery both add commands on loom.run.Session in ess/domains/run.yaml with no edge between them and neither body says so (cited); add an ordering edge recording the shared entity or split the surface
- file: .engineering/planning/story/compaction-contract.md
  line: 30
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: not ordered against story:action-selector, story:argument-generator or story:selection-revalidation although all edit ess/domains/run.yaml and regenerate the same model crate (cited for run.yaml, inferred for the test location); add an ordering edge or split the surface
- file: .engineering/planning/story/agent-executor.md
  line: 29
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: the generated model crate path is only "for example crates/loom-model" and does not exist yet, while every later ESS story regenerates it without naming it; fix the path and state that later stories rewrite it (rewrites inferred)
```
