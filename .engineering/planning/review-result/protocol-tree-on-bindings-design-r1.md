---
format: aep.planning-md/3
id: review-result:protocol-tree-on-bindings-design-r1
kind: review-result
status: active
title: Plan critic design, round 1, on epic:protocol-tree-on-bindings
relations:
- reviews: epic:protocol-tree-on-bindings
- reviews: story:canon-0-9-0-pin
- reviews: story:substrate-0-21-pin
- reviews: story:confinement-opens-from-engine-config
- reviews: story:bound-commands-run-confined
- reviews: story:bindings-document
- reviews: story:bound-program-result-table
- reviews: story:bound-artifact-revisions
- reviews: story:bound-agent-node
- reviews: story:deterministic-default-selector
- reviews: story:child-case-runtime
- reviews: story:record-copy-between-cases
- reviews: story:hash-chained-run-log
- reviews: story:tree-run-command
- reviews: story:agent-node-run-records
revision: 1
---
needs-revision

- `story:tree-run-command` — its `--max-agent-calls` acceptance counts agent steps from the run logs, but the `action started` agent marker and the agent steps come from `story:agent-node-run-records` and `story:bound-agent-node`, and no `depends_on` runs to either; the count is one abstraction split across two stories, so move the flag and its bullet into `story:agent-node-run-records` — .engineering/planning/story/tree-run-command.md:83, .engineering/planning/story/agent-node-run-records.md:24
- `story:agent-node-run-records` — its acceptance reads "the count `--max-agent-calls` uses", a flag of `story:tree-run-command`, with no `depends_on` to it, so once the flag moves here it needs `depends_on story:tree-run-command` (the reverse edge would be a cycle) — .engineering/planning/story/agent-node-run-records.md:61, .engineering/planning/story/tree-run-command.md:83
- `story:hash-chained-run-log` — its acceptance "`action finished` is appended before the case's revision moves" reads the revision move that `story:bound-artifact-revisions` creates, and no `depends_on` records it. The `child opened`/`child closed` events likewise name emitters in `story:child-case-runtime`, and no story says who writes the events. Add `depends_on story:bound-artifact-revisions`, or move the emission-order bullet to `story:tree-run-command` — .engineering/planning/story/hash-chained-run-log.md:58, .engineering/planning/story/bound-artifact-revisions.md:22
- `story:bound-program-result-table` — it says a record names "the artifact's revision in the governor when the program ended", but `story:bound-artifact-revisions` says a writing step's evidence names the revision computed after the program ended, before the governor holds it; for a write step the two rules contradict, so this body should say its rule applies only until that story lands — .engineering/planning/story/bound-program-result-table.md:58, .engineering/planning/story/bound-artifact-revisions.md:22
- `story:bound-agent-node` — its retry acceptance reads "`retry_when` matched", a match that only `story:deterministic-default-selector` defines (in its outcome, not its acceptance), and no `depends_on` runs to it; add `depends_on story:deterministic-default-selector` or name where the match is computed — .engineering/planning/story/bound-agent-node.md:68, .engineering/planning/story/deterministic-default-selector.md:24
- `story:child-case-runtime` — a child terminates "when nothing useful is left in it", and usefulness (`open_evidence` on `FrontierAction`) is defined by `story:deterministic-default-selector`, with no `depends_on` to it; add the edge or state where this story's termination test gets usefulness — .engineering/planning/story/child-case-runtime.md:29, .engineering/planning/story/deterministic-default-selector.md:54
- `story:confinement-opens-from-engine-config` — its outcome says bound commands run under the one adapter, but the code that makes them do so (removing `Command::new`) is in `story:bound-commands-run-confined`, and this story's own acceptance never exercises them; drop "bound commands" from the list here, or say it lands in `story:bound-commands-run-confined` — .engineering/planning/story/confinement-opens-from-engine-config.md:20-21

What I read: 17 artifacts, shown whole with `aep plan artifact show` (the epic, the 14 stories, the blocker, and `story:child-launches-through-confinement-adapters`). I also ran `aep plan artifact relations`, `aep plan artifact graph` and `aep plan artifact validate`. I walked all 18 `depends_on` edges, plus one `blocks`, 15 `decomposes`/`serves` edges and the `decides` edge into the existing draft. The walk went outside the set to `story:child-launches-through-confinement-adapters` and `story:substrate-0-21-pin`. The declared `depends_on`/`blocks` graph is acyclic and does not serialise: it has four roots (the two pins, `story:bindings-document`, `story:hash-chained-run-log`). I traced about 22 acceptance reads to a producer in the set. 16 have a `depends_on` edge, directly or transitively; 6 do not, and they are the six findings above. `validate` printed `valid`; its notes about old review-results are not mine.

What I could not establish:
- Where the selector's "performed successfully against the current case revision" history comes from. No story in the set says which store holds it.
- Whether "the step record" in `story:bound-artifact-revisions` is the same thing as the run-log `action finished` line. If it is, that story also reads the log.
- Out of my lane, for the parallel-safety critic: `story:deterministic-default-selector` shares `FrontierAction` with the draft `story:frontier-action-names-its-claims`. The body says "must not be worked at once" and the graph has no edge for it.
- Out of my lane, for the scope critic: `story:tree-run-command` accepts "no process of the run starts outside it", but `story:confinement-opens-from-engine-config` puts the session executor's harness spawn out of scope. The shipped agent adapter in `story:bound-agent-node` starts Claude Code through that spawn, so no story confines it.

```findings
- file: .engineering/planning/story/tree-run-command.md
  line: 83
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its --max-agent-calls acceptance counts agent steps from the run logs, but the action-started agent marker and the agent steps come from story:agent-node-run-records and story:bound-agent-node, and no depends_on runs to either; the count is one abstraction split across two stories, so move the flag and its bullet into story:agent-node-run-records"
- file: .engineering/planning/story/agent-node-run-records.md
  line: 61
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance reads the count --max-agent-calls uses, a flag of story:tree-run-command, with no depends_on to it, so once the flag moves here it needs depends_on story:tree-run-command (the reverse edge would be a cycle)"
- file: .engineering/planning/story/hash-chained-run-log.md
  line: 58
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its acceptance that action finished is appended before the case's revision moves reads the revision move story:bound-artifact-revisions creates, no depends_on records it, and no story says who writes the child opened and child closed events; add depends_on story:bound-artifact-revisions or move the emission-order bullet to story:tree-run-command"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 58
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it says a record names the artifact's revision in the governor when the program ended, while story:bound-artifact-revisions says a writing step's evidence names the revision computed after the program ended and before the governor holds it; the two rules contradict for a write step, so this body should say its rule applies only until that story lands"
- file: .engineering/planning/story/bound-agent-node.md
  line: 68
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its retry acceptance reads retry_when matched, a match only story:deterministic-default-selector defines, and no depends_on runs to it; add depends_on story:deterministic-default-selector or name where the match is computed"
- file: .engineering/planning/story/child-case-runtime.md
  line: 29
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "a child terminates when nothing useful is left in it, and usefulness (open_evidence on FrontierAction) is defined by story:deterministic-default-selector with no depends_on to it; add the edge or state where this story's termination test gets usefulness"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 20
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its outcome says bound commands run under the one adapter, but the change that makes them do so is in story:bound-commands-run-confined and this story's acceptance never exercises them; drop bound commands from the list here or say it lands in story:bound-commands-run-confined"
```
