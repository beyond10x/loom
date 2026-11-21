---
format: aep.planning-md/3
id: review-result:protocol-tree-on-bindings-acceptance-r1
kind: review-result
status: active
title: Plan critic acceptance, round 1, on epic:protocol-tree-on-bindings
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

- story:confinement-opens-from-engine-config — "`b10x-loom run --confinement` maps onto the same value" names no observable. The flag today takes `substrate|none` (default `substrate`), while `Local` refuses a blank reason, and `story:tree-run-command` offers `linux|local` with `--local-reason`. So what `none` does is not stated. — .engineering/planning/story/confinement-opens-from-engine-config.md:57 (flag at crates/loom-cli/src/lib.rs:777-778, 985-989)
- story:confinement-opens-from-engine-config — "no other code path opens an adapter or a `HostDriver`" is a universal negative with no deciding check. It also restates `story:child-launches-through-confinement-adapters` acceptance 1 ("no `HostDriver::open` remains outside it"), so two stories claim one outcome. — .engineering/planning/story/confinement-opens-from-engine-config.md:47 (cf. .engineering/planning/story/child-launches-through-confinement-adapters.md:34)
- story:confinement-opens-from-engine-config — the Outcome says the test runner, `run_child`, `spawn_child` and bound commands all run under the one configured adapter. No bullet runs `spawn_child` or the test runner under a configured adapter; only `run_child` under `Local` and the old delegated cases are checked. — .engineering/planning/story/confinement-opens-from-engine-config.md:20
- story:bound-commands-run-confined — "its exit, report and evidence are handled as today" names no test that holds today's behaviour. The `ChildCommand` the Outcome promises (JSON on standard input, cleared environment) is checked by no bullet. — .engineering/planning/story/bound-commands-run-confined.md:49
- story:bound-commands-run-confined — the Outcome says the file's `unconfined_bindings: true` selects `local` with a reason naming which of the two opted in. The only opt-in bullet tests the flag, so the file route and the "which of the two" reason have no check. — .engineering/planning/story/bound-commands-run-confined.md:21
- story:bindings-document — the red test is "`tests/bindings_document.rs` in the crate that holds the decoder", and no crate is named. The red step cannot be located, and four later stories rely on it ("the bindings crate"). — .engineering/planning/story/bindings-document.md:59
- story:bindings-document — "The agent `model` defaults to `claude-haiku-5-5` when absent" states an outcome `story:bound-agent-node` also states ("With no `model`, the adapter is called with `claude-haiku-5-5`"). It names no decode-level observable, though this story executes nothing. — .engineering/planning/story/bindings-document.md:76
- story:bindings-document — "`--action-bindings` accepts either format by its `format` member" has no observable outcome. Nothing executes in this story, so a `loom.bindings/1` file given to the flag has no result to compare with the refusal it gets today. — .engineering/planning/story/bindings-document.md:79
- story:bindings-document — "documents every member" has no completeness check. Only one example is decoded, so a member left out of the page passes. — .engineering/planning/story/bindings-document.md:80
- story:bound-program-result-table — "the artifact's revision in the governor when the program ended" contradicts `story:bound-artifact-revisions` ("A record submitted by a writing step names the revision computed after the program ended"). Evidence is written before the case moves, so the governor still holds the old revision at that point, and the two stories expect different values for one record. — .engineering/planning/story/bound-program-result-table.md:58 (cf. .engineering/planning/story/bound-artifact-revisions.md:52)
- story:bound-program-result-table — the Outcome says the program gets its arguments as JSON on standard input with a cleared environment. No bullet checks either. — .engineering/planning/story/bound-program-result-table.md:22
- story:bound-artifact-revisions — the revision is defined as `t` plus the first 16 hex digits of a SHA-256, but no bullet pins the format or a known file's expected value. The bullets only check order independence and difference, so any digest passes. — .engineering/planning/story/bound-artifact-revisions.md:20
- story:bound-agent-node — "declared inputs" and "declared outputs" are the basis of the scratch-copy and copy-back bullets. `story:bindings-document` gives the `agent` executor only `adapter` and `model` and refuses undeclared members, so a fixture cannot declare them. — .engineering/planning/story/bound-agent-node.md:60 (cf. .engineering/planning/story/bindings-document.md:22)
- story:bound-agent-node — "a tool call outside it ... is refused and recorded as refused" names no record that holds the refusal. Run records are out of scope, and `ToolNotAllowed` is the only refusal named. — .engineering/planning/story/bound-agent-node.md:59
- story:deterministic-default-selector — the Outcome makes an action useful again when `retry_when` matches, and makes an action that may produce nothing useful until performed against the current case revision. No acceptance bullet tests either rule. — .engineering/planning/story/deterministic-default-selector.md:24
- story:deterministic-default-selector — "the default for a run driven by bindings" has no check. Out of scope says the default selector of `b10x-loom run <intent>` is unchanged, so which bindings-driven run gets it is unobservable here. — .engineering/planning/story/deterministic-default-selector.md:18
- story:child-case-runtime — the child's termination is the transition that writes the outcome into the parent, and nothing in the acceptance performs it. Termination needs "nothing useful is left", which belongs to the selector story this one does not depend on, and driving children is out of scope. The trigger of "reaches `passed` and terminates" is therefore unnamed. — .engineering/planning/story/child-case-runtime.md:70
- story:child-case-runtime — "The cases of one tree share the root's workspace" has no bullet. `story:bound-artifact-revisions` relies on it as defined here. — .engineering/planning/story/child-case-runtime.md:37
- story:record-copy-between-cases — "A subject the receiving protocol does not declare, or a source protocol with no case in the tree, is refused by name" joins two conditions and names no refusal. The ESS-first section declares only `NoSourceRecord`. — .engineering/planning/story/record-copy-between-cases.md:57
- story:record-copy-between-cases — "The action is useful while the source holds a record the receiving case lacks" has no bullet. The idempotence bullet checks only that nothing is written again. — .engineering/planning/story/record-copy-between-cases.md:25
- story:hash-chained-run-log — "An event holds no argument value, no file content and no credential" is a universal negative with no deciding check, such as a canary value in an argument and a scan of the log. The `stopped (… the next decisions)` event can carry text. — .engineering/planning/story/hash-chained-run-log.md:60
- story:tree-run-command — `tree verify` and `tree status` are Outcome members, and no bullet runs either. The epic's "run log of every case verifies" rests on `tree verify`. — .engineering/planning/story/tree-run-command.md:36
- story:tree-run-command — the `--max-agent-calls` bullet cannot be decided in this fixture ("stub programs, no model"), which has no agent steps. The `action started` agent marker it counts is defined in `story:agent-node-run-records`, which this story does not depend on. — .engineering/planning/story/tree-run-command.md:83 (cf. .engineering/planning/story/agent-node-run-records.md:24)
- story:tree-run-command — "no process of the run starts outside it" is a universal negative with no deciding check. The loop bullet names a test and this one names none. — .engineering/planning/story/tree-run-command.md:89
- story:tree-run-command — "a second `tree` command on a store in use is refused naming the holder" is an Outcome clause with no bullet. — .engineering/planning/story/tree-run-command.md:42
- story:agent-node-run-records — the `AgentStep` members turns, duration and transcript digest (and the `ModelCall` duration) are checked by no bullet. Only tool calls, allowed tools and token absence are. — .engineering/planning/story/agent-node-run-records.md:21

What I read: 16 of 16 ids (the 14 stories, `upstream-blocker:engineering-protocols-on-canon-0-9-0`, and the dependency `story:child-launches-through-confinement-adapters`), plus `epic:protocol-tree-on-bindings` for context. I used `aep plan artifact show` on each, `aep plan artifact kinds` and `lifecycle story`, `AGENTS.md` § ESS, `git grep` in the Loom tree, and the canon and substrate repositories (tags `0.9.0`, `0.21.0`; commit ids match). Prior acceptance reviews in the store set the bar for granularity.

What I could not establish:
- Whether Canon's IR exposes independence dimensions, which the `independence` load-time check needs.
- Whether `aep plan validate` reports anything. The subcommand does not exist here, so I ran no validator.
- The epic's own acceptance, which the SDC session runs outside Loom's gate. I judged only the story set.
- Out of my lane, not counted in the verdict:
  - `story:bindings-document` checks `case_open` at load, which needs Canon 0.9.0, but it has no `depends_on story:canon-0-9-0-pin` (design).
  - `arguments_from`, `handoff` and the top-level `independence` are declared by `story:bindings-document` and executed by no story (scope).
  - `from: effect` as a `ResultSource` is declared in both `story:bindings-document` and `story:bound-agent-node` (parallel-safety).
  - The Why of `story:child-launches-through-confinement-adapters` still describes the pre-0.14 wait that its rewritten "Waits on" section withdraws.

```findings
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 57
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"b10x-loom run --confinement maps onto the same value\" names no observable; the flag takes substrate|none today, Local refuses a blank reason, and story:tree-run-command offers linux|local with --local-reason, so what none does is not stated"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 47
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"no other code path opens an adapter or a HostDriver\" is a universal negative with no deciding check and restates story:child-launches-through-confinement-adapters acceptance 1, so two stories claim one outcome"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 20
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says the test runner, run_child, spawn_child and bound commands all run under the one configured adapter, but no bullet exercises spawn_child or the test runner under a configured adapter"
- file: .engineering/planning/story/bound-commands-run-confined.md
  line: 49
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"its exit, report and evidence are handled as today\" names no test that holds today's behaviour, and the ChildCommand the Outcome promises (JSON on standard input, cleared environment) is checked by no bullet"
- file: .engineering/planning/story/bound-commands-run-confined.md
  line: 21
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says the file's unconfined_bindings: true selects local with a reason naming which of the two opted in, but the only opt-in bullet tests the flag, so the file route and the which-of-the-two reason have no check"
- file: .engineering/planning/story/bindings-document.md
  line: 59
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the red test is tests/bindings_document.rs in \"the crate that holds the decoder\" and no crate is named, so the red step cannot be located and four later stories rely on it as \"the bindings crate\""
- file: .engineering/planning/story/bindings-document.md
  line: 76
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"The agent model defaults to claude-haiku-5-5 when absent\" states an outcome story:bound-agent-node also states, and names no decode-level observable though this story executes nothing"
- file: .engineering/planning/story/bindings-document.md
  line: 79
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"--action-bindings accepts either format by its format member\" has no observable outcome, since nothing executes in this story and a loom.bindings/1 file given to the flag has no result to compare with the refusal it gets today"
- file: .engineering/planning/story/bindings-document.md
  line: 80
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"documents every member\" has no completeness check; only one example is decoded, so a member left out of the page passes"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 58
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "\"the artifact's revision in the governor when the program ended\" contradicts story:bound-artifact-revisions (\"names the revision computed after the program ended\"); evidence is written before the case moves, so the two stories expect different values for one record"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 22
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says the program gets its arguments as JSON on standard input with a cleared environment, and no bullet checks either"
- file: .engineering/planning/story/bound-artifact-revisions.md
  line: 20
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the revision is defined as t plus the first 16 hex digits of a SHA-256, but no bullet pins the format or a known file's expected value; the bullets only check order independence and difference, so any digest passes"
- file: .engineering/planning/story/bound-agent-node.md
  line: 60
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "\"declared inputs\" and \"declared outputs\" are the basis of the scratch-copy and copy-back bullets, but story:bindings-document gives the agent executor only adapter and model and refuses undeclared members, so a fixture cannot declare them"
- file: .engineering/planning/story/bound-agent-node.md
  line: 59
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"a tool call outside it ... is refused and recorded as refused\" names no record that holds the refusal; run records are out of scope and ToolNotAllowed is the only refusal named"
- file: .engineering/planning/story/deterministic-default-selector.md
  line: 24
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome makes an action useful again when retry_when matches and makes an action that may produce nothing useful until performed against the current case revision, and no acceptance bullet tests either rule"
- file: .engineering/planning/story/deterministic-default-selector.md
  line: 18
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"the default for a run driven by bindings\" has no check, and Out of scope leaves the default selector of b10x-loom run <intent> unchanged, so which bindings-driven run gets it is unobservable here"
- file: .engineering/planning/story/child-case-runtime.md
  line: 70
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the child's termination is the transition that writes the outcome into the parent and nothing in the acceptance performs it; termination needs \"nothing useful is left\", which belongs to the selector story this one does not depend on, and driving children is out of scope, so the trigger of \"reaches passed and terminates\" is unnamed"
- file: .engineering/planning/story/child-case-runtime.md
  line: 37
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"The cases of one tree share the root's workspace\" has no bullet, and story:bound-artifact-revisions relies on it as defined here"
- file: .engineering/planning/story/record-copy-between-cases.md
  line: 57
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"A subject the receiving protocol does not declare, or a source protocol with no case in the tree, is refused by name\" joins two conditions and names no refusal; ESS first declares only NoSourceRecord"
- file: .engineering/planning/story/record-copy-between-cases.md
  line: 25
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"The action is useful while the source holds a record the receiving case lacks\" has no bullet; the idempotence bullet checks only that nothing is written again"
- file: .engineering/planning/story/hash-chained-run-log.md
  line: 60
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"An event holds no argument value, no file content and no credential\" is a universal negative with no deciding check such as a canary value in an argument and a scan of the log, and the stopped event carries the next decisions as text"
- file: .engineering/planning/story/tree-run-command.md
  line: 36
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "tree verify and tree status are Outcome members and no bullet runs either, though the epic's \"run log of every case verifies\" rests on tree verify"
- file: .engineering/planning/story/tree-run-command.md
  line: 83
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the --max-agent-calls bullet cannot be decided in a fixture of stub programs with no model and so no agent steps, and the action started agent marker it counts is defined in story:agent-node-run-records, which this story does not depend on"
- file: .engineering/planning/story/tree-run-command.md
  line: 89
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"no process of the run starts outside it\" is a universal negative with no deciding check, while the loop bullet names a test"
- file: .engineering/planning/story/tree-run-command.md
  line: 42
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"a second tree command on a store in use is refused naming the holder\" is an Outcome clause with no acceptance bullet"
- file: .engineering/planning/story/agent-node-run-records.md
  line: 21
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the AgentStep members turns, duration and transcript digest, and the ModelCall duration, are checked by no bullet; only tool calls, allowed tools and token absence are"
```
