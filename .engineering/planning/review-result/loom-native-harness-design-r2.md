---
format: aep.planning-md/3
id: review-result:loom-native-harness-design-r2
kind: review-result
status: active
title: loom-native-harness decomposition — design critic, round 2
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

story:agent-executor — it removes the Canon imports (`ActionCandidate`, `ActionId`, `ActionStatus`, `Frontier`) at `crates/loom/src/lib.rs:5` in link 1, but `ActionSelector`, `FirstAdmissibleSelector`, `ArgumentGenerator`, `EmptyObjectArguments` and `Loom::run` all use those types, and `story:action-selector` and `story:argument-generator` each claim to rewrite those same seams (links 3 and 4). Neither the body nor the two later stories say which item keeps the tree green in between. State in agent-executor's Outcome the minimal selector and generator seam it leaves over Commission's frontier and that links 3 and 4 then replace it, or move the import removal into the stories that rewrite each trait. — .engineering/planning/story/agent-executor.md:63-64, .engineering/planning/story/action-selector.md:37, .engineering/planning/story/argument-generator.md:38-39, crates/loom/src/lib.rs:5,14-20,71-100

story:frontier-projection — it names `commission:story:frontier-admission` (M-002) as the story that defines the frontier and decides how it holds its actions. It also says that decision is "still the UNMAPPED marker" and leaves the entry-to-action cardinality undeclared. Commission's current plan puts that in `commission:story:ess-hard-gate` (frontier marker closed; `ActionStatus`, `FrontierClaim`, `FrontierObligation` and `FrontierAction` declared), and `frontier-admission` only consumes it. Re-point this dependency, and the same line in agent-executor and selection-revalidation, at `commission:story:ess-hard-gate`. The cardinality line can then be settled. — .engineering/planning/story/frontier-projection.md:40,85-86,91, .engineering/planning/story/agent-executor.md:103, .engineering/planning/story/selection-revalidation.md:69,73; commission `aep plan artifact show story:ess-hard-gate` (Outcome (b)) and `story:frontier-admission` § ESS ("The frontier's contents are not this story's")

What I read: 12 stories decomposing the epic, the epic, the four `-r1` review-results, and 4 Commission stories (`ess-hard-gate`, `frontier-admission`, `agent-executor-port`, plus the Commission graph). I ran `aep plan artifact list`, `show`, `relations`, `graph` and `validate` (valid) here, and `list`, `show` and `graph` in the Commission store. I walked all 140 or so declared edges, including edges to and from artifacts outside the set (`epic:fast-selector`, `epic:effect-bindings`, the decision-blockers, visions). There is no cycle among `depends_on` edges. Commission's graph has no `loom` match, so no edge runs from Commission to Loom.

Round-1 design findings, all four landed:
- The `frontier-projection` to `harness-module-map` edge now exists.
- The Commission dependency is recorded.
- `selection-revalidation` now ends at returning `ProposedAction`.
- `loom-ess-conformance` now runs the suite through the Rust target.

What I could not establish:
- Chain shape (not a finding). 11 of the 12 stories form one linear chain, from `ess-hard-gate` through `loom-ess-conformance`, with `harness-module-map` off it. Every link's reason is written in its `## Shared surface` section (`run.yaml`, `generated/rust/loom/`, `lib.rs`, `Taskfile.yml`). The alternative to the order is splitting that surface, so I do not ask for edges to be removed.
- `story:ess-hard-gate` closes the markers for `Selection.confidence` and `Turn.catalogue`, which `action-selector` and `frontier-projection` then consume. The edges are recorded, each side is demonstrable alone, and I count it as a deliberate split, not a defect.
- Out of my lane, for acceptance: `argument-generator` acceptance 3 expects a returned `ProposedAction` at link 4, but link 5 (`selection-revalidation`) is where revalidation is added. The acceptance does not say the unrevalidated path is only a stage.
- I did not read Canon.

```findings
- file: .engineering/planning/story/agent-executor.md
  line: 63
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: it removes the Canon imports from crates/loom/src/lib.rs:5 in link 1, yet ActionSelector, FirstAdmissibleSelector, ArgumentGenerator and Loom::run use those types and story:action-selector and story:argument-generator each claim to rewrite the same seams, so no body says which item keeps the tree green between links 1 and 4; state the minimal seam agent-executor leaves and that links 3 and 4 replace it, or move the import removal into the stories that rewrite each trait
- file: .engineering/planning/story/frontier-projection.md
  line: 85
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: it names commission story:frontier-admission (M-002) as deciding how a frontier holds its actions and leaves the entry-to-action cardinality undeclared, but Commission's current plan has story:ess-hard-gate declare FrontierAction, ActionStatus, FrontierClaim and FrontierObligation and close that marker; re-point the dependency (also at agent-executor.md:103 and selection-revalidation.md:69,73) at commission:story:ess-hard-gate and settle the cardinality
```
