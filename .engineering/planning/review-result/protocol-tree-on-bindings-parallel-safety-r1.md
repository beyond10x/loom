---
format: aep.planning-md/3
id: review-result:protocol-tree-on-bindings-parallel-safety-r1
kind: review-result
status: active
title: Plan critic parallel-safety, round 1, on epic:protocol-tree-on-bindings
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

story:canon-0-9-0-pin — both this and story:substrate-0-21-pin edit `crates/loom-intake-slice/Cargo.toml`, `Cargo.lock`, `AGENTS.md` and `CHANGELOG.md` (cited, both bodies); neither body mentions the other, and no depends_on edge runs between them — .engineering/planning/story/canon-0-9-0-pin.md:38, .engineering/planning/story/canon-0-9-0-pin.md:51, .engineering/planning/story/substrate-0-21-pin.md:36, .engineering/planning/story/substrate-0-21-pin.md:44
story:bindings-document — both this and story:bound-commands-run-confined land on the `--action-bindings` load in `crates/loom-cli/src/main.rs` (cited by bound-commands at `main.rs:983-1003`; bindings-document names the flag and `git grep` places it at `main.rs:990` and `:1088`, so that side is inferred), and no edge orders them — .engineering/planning/story/bindings-document.md:78, .engineering/planning/story/bound-commands-run-confined.md:30
story:hash-chained-run-log — both this and story:bindings-document add a new domain to `ess/system.yaml` (cited, both ESS first sections). The new domain files `ess/domains/bindings.yaml` and `ess/domains/runlog.yaml` do not exist yet, but `ess/system.yaml` does, and no edge orders the two stories — .engineering/planning/story/hash-chained-run-log.md:41, .engineering/planning/story/bindings-document.md:56
story:deterministic-default-selector — both this and story:child-case-runtime change `ess/commission/domains/responsibility.yaml` and so regenerate `generated/rust/commission/src/responsibility.rs` (cited, both ESS first sections). Both likely also edit `crates/loom-governor/src/lib.rs` (inferred for the selector from its citation at `lib.rs:1779` and its first acceptance line; cited for child-case-runtime). The body admits the shared file with story:frontier-action-names-its-claims only, and no edge to child-case-runtime exists — .engineering/planning/story/deterministic-default-selector.md:44, .engineering/planning/story/child-case-runtime.md:58
story:deterministic-default-selector — this edits the governor to derive `effect` and `open_evidence` from Canon's IR, and story:canon-0-9-0-pin changes the Canon types and the doc comment in `crates/loom-governor/src/lib.rs`. The `lib.rs` side of the selector is inferred, so this is a weaker claim, and no edge exists — .engineering/planning/story/deterministic-default-selector.md:54, .engineering/planning/story/canon-0-9-0-pin.md:51
story:record-copy-between-cases — this adds `CopyExecutor` and `CopiedRecord` to `ess/domains/bindings.yaml`, as do story:bound-program-result-table, story:bound-artifact-revisions and story:bound-agent-node (cited, all four ESS first sections). The file is created by story:bindings-document and does not exist yet. record-copy reaches bindings-document only through child-case-runtime, so no edge orders it against those three. The "bindings crate" that holds their tests is also never named — .engineering/planning/story/record-copy-between-cases.md:39, .engineering/planning/story/bound-program-result-table.md:43, .engineering/planning/story/bound-artifact-revisions.md:42, .engineering/planning/story/bound-agent-node.md:45
story:hash-chained-run-log — its acceptance "`action finished` is appended before the case's revision moves" reads the revision move that story:bound-artifact-revisions creates, and "between the event's write and the command's exit" reads a command that story:tree-run-command creates (tree-run-command depends on the log, so that edge cannot run the other way). The log has no depends_on edge at all, so the bullet cannot be checked in the story's own wave — add the edge to bound-artifact-revisions and move the command-exit bullet to tree-run-command (or merge) — .engineering/planning/story/hash-chained-run-log.md:57, .engineering/planning/story/bound-artifact-revisions.md:22
story:tree-run-command — `--max-agent-calls` counted "from the run logs" reads the agent-step mark in `action started` that story:agent-node-run-records writes, and agent steps come from story:bound-agent-node. Neither is a depends_on target, and adding the edge to agent-node-run-records would cycle, because that story's acceptance reads the `--max-agent-calls` count this story creates, so the bullet must live in exactly one of the two — .engineering/planning/story/tree-run-command.md:83, .engineering/planning/story/agent-node-run-records.md:61

**What I read:** 15 story bodies and the epic, using `aep plan artifact show` on the epic, `cat` on the story files, `aep plan artifact waves --kind story --status draft`, `git status`, and `git grep` and `ls` for `crates/loom-cli/src/main.rs`, `ess/` and `crates/`. Surfaces: 14 of 14 items cited, 0 inferred, 0 unplaced. In 8 stories the implementing crate is only "the bindings crate" or "the crate that holds the log", so the ESS files and test paths are cited but the crate is not. Acceptance reads traced to a producer in the set: 20, of which 17 are recorded by a depends_on edge (all of them direct) and 3 are not, namely the log's revision-move and command-exit bullets and tree-run-command's `--max-agent-calls` read. The agent-node-run-records read of `--max-agent-calls` is the other half of the cycle in the last finding.

**What I could not establish:**
- The crate of the `loom.bindings` decoder, of the bindings-crate tests, and of the run log. If any two land in one crate, they also share its `lib.rs` and `Cargo.toml`, and I cannot assess that until a scoper names the crate.
- Whether story:confinement-opens-from-engine-config and story:bindings-document touch the same hunks of `crates/loom-cli/src/main.rs`. The `--confinement` mapping sits at `main.rs:1017` and `:1151`, away from `--action-bindings` at `:990`, so I did not report it.
- The collision of story:deterministic-default-selector with story:frontier-action-names-its-claims. The selector admits it in prose and I cannot see that item, so I make no claim.
- `CHANGELOG.md`, `AGENTS.md`, `Cargo.lock` and `generated/rust/` are touched by every story under the house rule. Listing them in scopes would serialise all 14 stories, as `aep plan artifact waves` already shows for other drafts. Whether that is acceptable is a person's decision, so I did not list it as a finding.
- Out of my lane: whether story:record-copy-between-cases should state its own usefulness rule while story:deterministic-default-selector owns the definition of "useful" (design critic).

```findings
- file: .engineering/planning/story/canon-0-9-0-pin.md
  line: 38
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "both this and story:substrate-0-21-pin edit crates/loom-intake-slice/Cargo.toml, Cargo.lock, AGENTS.md and CHANGELOG.md (cited, both bodies); neither body mentions the other and no depends_on edge runs between them"
- file: .engineering/planning/story/bindings-document.md
  line: 78
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "both this and story:bound-commands-run-confined land on the --action-bindings load in crates/loom-cli/src/main.rs (cited by bound-commands at main.rs:983-1003; bindings-document side inferred from a git grep of the flag at main.rs:990 and :1088), and no edge orders them"
- file: .engineering/planning/story/hash-chained-run-log.md
  line: 41
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "both this and story:bindings-document add a new domain to ess/system.yaml (cited, both ESS first sections); ess/domains/runlog.yaml and ess/domains/bindings.yaml do not exist yet, ess/system.yaml does, and no edge orders the two stories"
- file: .engineering/planning/story/deterministic-default-selector.md
  line: 44
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "both this and story:child-case-runtime change ess/commission/domains/responsibility.yaml and regenerate generated/rust/commission/src/responsibility.rs (cited); crates/loom-governor/src/lib.rs is also likely shared (inferred for the selector side); the body admits the shared file only with story:frontier-action-names-its-claims and no edge to child-case-runtime exists"
- file: .engineering/planning/story/deterministic-default-selector.md
  line: 54
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the governor change that derives effect and open_evidence from Canon's IR collides with story:canon-0-9-0-pin's change of the Canon types and the doc comment in crates/loom-governor/src/lib.rs; the selector's lib.rs side is inferred, so this claim is weaker; no edge exists"
- file: .engineering/planning/story/record-copy-between-cases.md
  line: 39
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "this adds CopyExecutor and CopiedRecord to ess/domains/bindings.yaml, as do story:bound-program-result-table, story:bound-artifact-revisions and story:bound-agent-node (cited); the file is created by story:bindings-document and does not exist yet; record-copy reaches bindings-document only through child-case-runtime so no edge orders it against those three, and the bindings crate holding the tests is never named"
- file: .engineering/planning/story/hash-chained-run-log.md
  line: 57
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance 'action finished is appended before the case's revision moves' and 'between the event's write and the command's exit' read a revision move created by story:bound-artifact-revisions and a command created by story:tree-run-command, and the log has no depends_on edge at all; add the edge to bound-artifact-revisions and move the command-exit bullet to tree-run-command, or merge"
- file: .engineering/planning/story/tree-run-command.md
  line: 83
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "--max-agent-calls counted from the run logs reads the agent-step mark that story:agent-node-run-records writes (and agent steps from story:bound-agent-node) with no depends_on edge; adding the edge to agent-node-run-records would cycle because that story's acceptance reads the --max-agent-calls count this story creates, so the bullet must live in exactly one of the two"
```
