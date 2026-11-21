---
format: aep.planning-md/3
id: review-result:tree-daemon-under-enforced-limits-parallel-safety-r1
kind: review-result
status: active
title: Plan critic parallel-safety, round 1, on epic:tree-daemon-under-enforced-limits
relations:
- reviews: epic:tree-daemon-under-enforced-limits
- reviews: story:substrate-daemon-runner
- reviews: story:action-limits-enforced
- reviews: story:limit-hits-in-run-log
- reviews: story:executable-allow-list
- reviews: story:confinement-level-required
- reviews: story:named-egress-per-action
- reviews: story:agent-session-through-substrate
- reviews: story:tree-daemon
revision: 1
---
needs-revision

- story:confinement-level-required — both it and `story:substrate-daemon-runner` add variants to `EngineConfinement` in `ess/intake/domains/confinement.yaml` (cited, both `## ESS first`), its Outcome names "a Substrate daemon", the variant the other story creates, and no `depends_on` orders them. Either an edge recording the shared enum as its reason, or splitting the surface, would fix it. — .engineering/planning/story/confinement-level-required.md:40, .engineering/planning/story/substrate-daemon-runner.md:34
- story:substrate-daemon-runner — it and the reviewed `story:bound-commands-run-confined` both hang off `story:confinement-opens-from-engine-config` and both edit `ess/intake/domains/confinement.yaml` (cited: `Daemon` and the applied record here, `NoRunner` there); the two also share the `SubstrateRunner::run_child` code in `crates/loom-intake-slice/src/confinement.rs` (inferred); neither body mentions the other, and no edge orders them. — .engineering/planning/story/substrate-daemon-runner.md:34, .engineering/planning/story/bound-commands-run-confined.md:115
- story:action-limits-enforced — five stories land on `crates/loom-intake-slice/src/confinement.rs` and are ordered pairwise by no edge except through `story:agent-session-through-substrate`. Cited: this one (`ChildCommand` fixed limits at `:2954-2962`), `story:named-egress-per-action` (`ChildCommand::with_egress`, `:1243`) and `story:substrate-daemon-runner` (`run_child` `:475`, `spawn_child` `:525`). Inferred: `story:executable-allow-list` (hands lists to Substrate) and `story:confinement-level-required` (maps levels to facts). The fix is an edge per pair or splitting `ChildCommand`, and each of the other four needs the same sentence. — .engineering/planning/story/action-limits-enforced.md:32, .engineering/planning/story/named-egress-per-action.md:34
- story:action-limits-enforced — its acceptance runs "under the `linux` adapter or the Substrate daemon", so the daemon arm reads the runner `story:substrate-daemon-runner` creates, and its only edge is `depends_on story:tree-run-command`; it needs a `depends_on` edge to `story:substrate-daemon-runner` (or merging the two, or dropping the daemon arm). — .engineering/planning/story/action-limits-enforced.md:50, .engineering/planning/story/substrate-daemon-runner.md:18
- story:confinement-level-required — it adds `BoundAction.confinement` to `ess/domains/bindings.yaml` (not there yet; `story:bindings-document` creates it), and so do `story:action-limits-enforced`, `story:named-egress-per-action`, `story:limit-hits-in-run-log` and `story:agent-session-through-substrate`. No edge orders it against any of the four, and each pair is unordered on the file (cited, all five `## ESS first`). The load-time refusals land in `crates/loom-bindings/src` (inferred). An edge per pair or splitting the file would fix it. — .engineering/planning/story/confinement-level-required.md:42, .engineering/planning/story/action-limits-enforced.md:42, .engineering/planning/story/named-egress-per-action.md:42, .engineering/planning/story/limit-hits-in-run-log.md:40, .engineering/planning/story/agent-session-through-substrate.md:43
- story:named-egress-per-action — it shares `ess/domains/bindings.yaml` and `ess/domains/tree.yaml` with `story:action-limits-enforced` (cited, both `## ESS first`), and `bindings.yaml` with `story:limit-hits-in-run-log`, with no edge between either pair, since both depend only on `story:tree-run-command` or on a sibling. — .engineering/planning/story/named-egress-per-action.md:41, .engineering/planning/story/action-limits-enforced.md:44, .engineering/planning/story/limit-hits-in-run-log.md:40
- story:executable-allow-list — it adds `ExecutablePolicy` to `ess/domains/tree.yaml` (cited), where `story:action-limits-enforced`, `story:named-egress-per-action` and `story:agent-session-through-substrate` also add engine-configuration types. It depends only on `story:tree-run-command`, so none of the three pairs is ordered, and its load-time check lands in `crates/loom-bindings/src` with the others (inferred). — .engineering/planning/story/executable-allow-list.md:41, .engineering/planning/story/agent-session-through-substrate.md:44
- story:tree-daemon — it adds `TreeDaemon` and `ServedTree` to `ess/domains/tree.yaml` (cited) and depends only on `story:agent-node-run-records`, so it is unordered against `story:action-limits-enforced`, `story:executable-allow-list`, `story:named-egress-per-action` and `story:agent-session-through-substrate`, which all edit the same file. It also changes `crates/loom-cli/src/lib.rs` (Why, cited) and regenerates `website/docs/reference/cli.md` (acceptance). — .engineering/planning/story/tree-daemon.md:41, .engineering/planning/story/tree-daemon.md:13
- story:limit-hits-in-run-log — its Outcome has `tree status` count limit events per case, which changes `TreeStatus` in `ess/domains/tree.yaml` and the `tree` code in `loom-cli`. The body names neither, and `story:tree-daemon` routes `tree status` through the daemon without an edge between the two. Cited: `TreeStatus` lives in `tree.yaml` per `story:tree-run-command`'s scope. Inferred: the `loom-cli` change. — .engineering/planning/story/limit-hits-in-run-log.md:25, .engineering/planning/story/tree-daemon.md:53
- story:agent-session-through-substrate — its Outcome says the harness runs "under the action's limits and executable policy", but `ExecutablePolicy` is created by `story:executable-allow-list` and it has no edge to it (its edges are `bound-agent-node`, `action-limits-enforced`, `named-egress-per-action`); the body needs an edge or must drop the claim. — .engineering/planning/story/agent-session-through-substrate.md:23, .engineering/planning/story/executable-allow-list.md:41
- story:agent-session-through-substrate — `AgentRun` in `ess/domains/bindings.yaml` and `crates/loom-session-executor/src/lib.rs` (the `SpawnRunner` seam at `:1398`, cited) are also in the scope of the reviewed `story:agent-node-run-records` (inferred: `AgentRun` change and the event-loop edit at `lib.rs:1439/1525/1594`), and this story has no edge to it although `story:limit-hits-in-run-log` and `story:tree-daemon` do. This rests on an inferred surface. — .engineering/planning/story/agent-session-through-substrate.md:33, .engineering/planning/story/agent-node-run-records.md:130
- story:action-limits-enforced — four stories each add members to "each step's record" (`enforced: false`, `descendants_held`, "network was not held", adapter, level and facts) and none names the type or file that record is. The surface is shared and unplaced, and no edge orders the four. — .engineering/planning/story/action-limits-enforced.md:62, .engineering/planning/story/executable-allow-list.md:43, .engineering/planning/story/named-egress-per-action.md:59, .engineering/planning/story/confinement-level-required.md:56

**What you read**
- 11 artifacts: the epic, 8 stories and 2 upstream-blockers.
- 14 stories of `epic:protocol-tree-on-bindings`, their relations and Scope sections.
- Commands: `aep plan artifact show`, `graph --format json`, `relations`, `waves`.
- Source: `crates/loom-intake-slice/src/confinement.rs` symbol lines, `ess/intake/domains/confinement.yaml`, and `ess/domains/` (no `bindings.yaml`, `tree.yaml` or `runlog.yaml` yet).
- Surfaces: 8 items placed cited (every story names an ESS file plus a test or code path), 0 inferred as primary surface, 0 unplaceable.
  - The code-file collisions on `loom-bindings/src`, `loom-tree` and `loom-cli` rest on inferred secondary surfaces and are marked so.
  - `ess/domains/bindings.yaml`, `tree.yaml` and `runlog.yaml` do not exist yet. `story:bindings-document`, `story:tree-run-command` and `story:hash-chained-run-log` create them, and every collision finding on them is on a file that is not there.
- Acceptance reads traced to a producer in the set: 5.
  - Recorded by a `depends_on` edge: 4 (limit-hits to action-limits, limit-hits to daemon-runner, agent-session to action-limits, agent-session to named-egress).
  - Not recorded: 1 (action-limits to daemon-runner).
  - Two further Outcome-level reads, agent-session to executable policy and confinement-level to the `Daemon` variant, have no edge either.
- Edges that hold: every set story reaches the rest of the upstream epic through `story:tree-run-command`, except `story:substrate-daemon-runner`, which depends only on `story:confinement-opens-from-engine-config`.
- Edges that hold: `ess/domains/runlog.yaml` is ordered, because `story:limit-hits-in-run-log` depends on `story:agent-node-run-records`.

**What you could not establish**
- None of the 8 stories is in `aep plan artifact waves` output (no `## Scope`), so I could not compare the tool's collision list with mine.
- Where the "engine configuration" lives: five stories add a section to it and no story in the set or the upstream epic creates its document or loader (`story:bindings-document` says only that adapters and apertures are engine configuration, `.engineering/planning/story/bindings-document.md:76`). That is also a design question.
- Out of lane:
  - `story:limit-hits-in-run-log` names an egress-bytes limit in its Outcome with no acceptance line.
  - `story:executable-allow-list`'s "adapter that proves the executable fact" acceptance can only be checked on `macos` until `upstream-blocker:substrate-executable-allow-list` clears.

```findings
- file: .engineering/planning/story/confinement-level-required.md
  line: 40
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "both this story and story:substrate-daemon-runner add variants to EngineConfinement in ess/intake/domains/confinement.yaml (cited, both ESS first), this Outcome names a Substrate daemon, and no depends_on orders them; an edge recording the shared enum or splitting the surface would fix it"
- file: .engineering/planning/story/substrate-daemon-runner.md
  line: 34
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "this story and the reviewed story:bound-commands-run-confined both depend on story:confinement-opens-from-engine-config and both edit ess/intake/domains/confinement.yaml (cited), and share SubstrateRunner::run_child in confinement.rs (inferred); neither body mentions the other and no edge orders them"
- file: .engineering/planning/story/action-limits-enforced.md
  line: 32
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "five stories land on crates/loom-intake-slice/src/confinement.rs with no edge between them: this one (ChildCommand limits :2954-2962), named-egress-per-action (with_egress), substrate-daemon-runner (run_child, spawn_child) cited; executable-allow-list and confinement-level-required inferred; an edge per pair or splitting ChildCommand would fix it"
- file: .engineering/planning/story/action-limits-enforced.md
  line: 50
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance runs under the Substrate daemon, which story:substrate-daemon-runner creates, and the only edge is depends_on story:tree-run-command; it needs a depends_on edge to story:substrate-daemon-runner, or merging the two, or dropping the daemon arm"
- file: .engineering/planning/story/confinement-level-required.md
  line: 42
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "BoundAction.confinement is added to ess/domains/bindings.yaml (file not there yet) by this story and also by action-limits-enforced, named-egress-per-action, limit-hits-in-run-log and agent-session-through-substrate, with no edge between this story and any of the four (load-time refusals in crates/loom-bindings/src are inferred)"
- file: .engineering/planning/story/named-egress-per-action.md
  line: 41
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "this story shares ess/domains/bindings.yaml and ess/domains/tree.yaml with action-limits-enforced and bindings.yaml with limit-hits-in-run-log (cited, all ESS first), and no edge orders either pair"
- file: .engineering/planning/story/executable-allow-list.md
  line: 41
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "ExecutablePolicy goes into ess/domains/tree.yaml (cited, file not there yet) beside the engine-configuration types of action-limits-enforced, named-egress-per-action and agent-session-through-substrate, and this story depends only on story:tree-run-command, so none of the three pairs is ordered"
- file: .engineering/planning/story/tree-daemon.md
  line: 41
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "TreeDaemon and ServedTree go into ess/domains/tree.yaml (cited) and this story depends only on agent-node-run-records, so it is unordered against action-limits-enforced, executable-allow-list, named-egress-per-action and agent-session-through-substrate, which edit the same file; it also changes loom-cli lib.rs and regenerates cli.md"
- file: .engineering/planning/story/limit-hits-in-run-log.md
  line: 25
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the tree status count changes TreeStatus in ess/domains/tree.yaml (cited via tree-run-command's scope) and the tree code in loom-cli (inferred); the body names neither and story:tree-daemon, which routes tree status through the daemon, has no edge to or from this story"
- file: .engineering/planning/story/agent-session-through-substrate.md
  line: 23
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome runs the harness under the action's executable policy, which story:executable-allow-list creates, and this story has no depends_on edge to it; it needs the edge or must drop the claim"
- file: .engineering/planning/story/agent-session-through-substrate.md
  line: 33
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "inferred surface: AgentRun in bindings.yaml and crates/loom-session-executor/src/lib.rs are in the scope of the reviewed story:agent-node-run-records, and this story has no edge to it although limit-hits-in-run-log and tree-daemon do"
- file: .engineering/planning/story/action-limits-enforced.md
  line: 62
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "four stories (this one, executable-allow-list, named-egress-per-action, confinement-level-required) each add members to 'each step's record' and none names the type or file that record is, so the shared surface is unplaced and no edge orders the four"
```
