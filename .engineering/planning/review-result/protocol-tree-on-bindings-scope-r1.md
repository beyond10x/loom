---
format: aep.planning-md/3
id: review-result:protocol-tree-on-bindings-scope-r1
kind: review-result
status: active
title: Plan critic scope, round 1, on epic:protocol-tree-on-bindings
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

epic:protocol-tree-on-bindings — "runs the eight SDC protocols to the root outcome `operating`" needs each `case_open` action to record its declared evidence (`intake_opened`, `direction_opened`, `epics_opened`, `stories_opened`, `release_opened`), because the SDC claims `intake.opened` and the others are `true_when` that kind exists; no story writes it, since `story:child-case-runtime` writes only the `case_outcome` record and an `open` executor has no program report for `evidence.from` to read; `story:child-case-runtime` would take it — `.engineering/planning/epic/protocol-tree-on-bindings.md:76`, `~/beyond10x/sdc/protocol/sdc.protocol.yaml:25-30`

epic:protocol-tree-on-bindings — the SDC's `stories.open` opens one child per entry of the list that its `args_from` argv prints (`each: stories`), and `story:bindings-document` decodes `arguments_from` but no story runs it, so the acceptance cannot reach the stories; `story:bound-program-result-table` and the `each` bullet of `story:child-case-runtime` would take it — `.engineering/planning/epic/protocol-tree-on-bindings.md:76`, `~/beyond10x/sdc/driver/examples/sdc/sdc-epic.bindings.json:63-66`, `.engineering/planning/story/bindings-document.md:26`

epic:protocol-tree-on-bindings — the `independence` map of the bindings file (the SDC's `$independence` in the spec and story bindings) is decoded and checked at load, but no story says it reaches the case at `tree open` or at child open, and the governor holds it per case; `story:tree-run-command` for the root and `story:child-case-runtime` for children would take it — `.engineering/planning/story/bindings-document.md:38`, `~/beyond10x/sdc/driver/examples/sdc/sdc-spec.bindings.json:6-10`

epic:protocol-tree-on-bindings — the Outcome ("runs the executor the binding names inside the configured Substrate adapter") and the Acceptance ("No process of that run is started outside the configured Substrate adapter") are silently narrowed for agent nodes, because `story:confinement-opens-from-engine-config` excludes "the session executor's harness spawn", `story:bound-agent-node` ships an adapter that starts Claude Code through that executor, and the epic's Not in scope names neither; the exclusion needs to be recorded there — `.engineering/planning/epic/protocol-tree-on-bindings.md:19`, `.engineering/planning/story/confinement-opens-from-engine-config.md:65`

story:bindings-document — the document declares `handoff` ("the text shown when this writer has done what it may and a person continues"), but no sentence in the epic or the seven requests asks for it and no story prints it, including the wait output of `story:tree-run-command`, so it is either decoded and never used or it belongs in that story's wait output — `.engineering/planning/story/bindings-document.md:34`, `.engineering/planning/story/tree-run-command.md:29`

story:tree-run-command — `tree status` (a JSON tree with per-case protocol, revision, outcome, obligations and log path) traces to no sentence of the epic's Outcome, Acceptance or the seven requests, which ask for run, approve, close and verify; the epic lists no consumer for it — `.engineering/planning/story/tree-run-command.md:37`

story:confinement-opens-from-engine-config — the Outcome claims that bound commands run under the configured adapter, which `story:bound-commands-run-confined` also claims ("starts only through the runner the host injected") and which this story's acceptance never tests, so both could be marked done on one promise; "bound commands" belongs only in the later story — `.engineering/planning/story/confinement-opens-from-engine-config.md:21`, `.engineering/planning/story/bound-commands-run-confined.md:18`

**What you read:** 17 artifacts (the epic, 14 stories, the upstream-blocker, and `story:child-launches-through-confinement-adapters` with its uncommitted diff). I ran `aep plan artifact show` on each and `aep plan artifact graph`. I read `story:frontier-action-names-its-claims`, `story:protocol-argument-schema` and `story:tool-decisions-on-step` for overlap. I also read `~/beyond10x/sdc/protocol/sdc.protocol.yaml`, `~/beyond10x/sdc/driver/examples/sdc/*.bindings.json`, `~/beyond10x/sdc/driver/src/main.rs` and `~/beyond10x/sdc/scripts/run-full-cycle.sh`. I extracted 25 promises from the epic and the seven requests, wrote the list before opening any story, and traced 21 to an item. The 4 not fully traced are the findings on open-action evidence, `arguments_from`, `independence` and agent confinement.

**What you could not establish:**
- Whether the drafter named the open-action evidence, `arguments_from` and `independence` gaps anywhere. I found no report from the drafter and nothing in the epic's exclusions.
- Whether the Canon governor already accepts a case-level `independence` at `open_case`, which decides how large the independence gap is. `crates/loom-governor/src/file_store.rs` stores it per case, but I did not trace the open path.
- Out of my lane, so not in the verdict:
  - `story:deterministic-default-selector` shares `FrontierAction` with the draft `story:frontier-action-names-its-claims`. The selector story names the clash, and it is a parallel-safety matter.
  - The drafter edited the existing `story:child-launches-through-confinement-adapters` (revision 5 to 6, its Waits on section rewritten). The epic says it does not re-plan that story, which is a design call.
  - Whether the 1 MiB report cap, the 60-line retry tail and the `claude-haiku-5-5` default are checkable belongs to the acceptance critic.

```findings
- file: .engineering/planning/epic/protocol-tree-on-bindings.md
  line: 76
  category: scope
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "\"runs the eight SDC protocols to the root outcome `operating`\" needs each case_open action to record its declared evidence (intake_opened, direction_opened, epics_opened, stories_opened, release_opened), because the SDC claims are true_when that kind exists, and no story writes it: story:child-case-runtime writes only the case_outcome record and an open executor has no program report for evidence.from to read; story:child-case-runtime would take it"
- file: .engineering/planning/epic/protocol-tree-on-bindings.md
  line: 76
  category: scope
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the SDC's stories.open opens one child per entry of the list its args_from argv prints, and story:bindings-document decodes arguments_from but no story runs it, so the acceptance cannot reach the stories; story:bound-program-result-table and the each bullet of story:child-case-runtime would take it"
- file: .engineering/planning/story/bindings-document.md
  line: 38
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the independence map of the bindings file (the SDC's $independence in the spec and story bindings) is decoded and checked at load, but no story says it reaches the case at tree open or at child open, where the governor holds it per case; story:tree-run-command for the root and story:child-case-runtime for children would take it"
- file: .engineering/planning/epic/protocol-tree-on-bindings.md
  line: 19
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome (runs the executor the binding names inside the configured Substrate adapter) and the Acceptance (no process started outside it) are silently narrowed for agent nodes, because story:confinement-opens-from-engine-config excludes the session executor's harness spawn, story:bound-agent-node ships an adapter that starts Claude Code through it, and the epic's Not in scope names neither; the exclusion needs recording there"
- file: .engineering/planning/story/bindings-document.md
  line: 34
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the document declares handoff (the text shown when a person continues), but no sentence in the epic or the seven requests asks for it and no story prints it, including the wait output of story:tree-run-command, so it is either decoded and never used or it belongs in that story's wait output"
- file: .engineering/planning/story/tree-run-command.md
  line: 37
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "tree status (a JSON tree with per-case protocol, revision, outcome, obligations and log path) traces to no sentence of the epic's Outcome, Acceptance or the seven requests, which ask for run, approve, close and verify"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 21
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome claims that bound commands run under the configured adapter, which story:bound-commands-run-confined also claims and which this story's acceptance never tests, so both could be marked done on one promise; bound commands belong only in the later story"
```
