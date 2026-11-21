---
format: aep.planning-md/3
id: review-result:protocol-tree-on-bindings-parallel-safety-r2
kind: review-result
status: active
title: Plan critic parallel-safety, round 2, on epic:protocol-tree-on-bindings
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

story:bindings-document — `crates/loom-bindings` checks protocols against Canon's compiled form (`may_produce`, `independence_dimensions`, `ProtocolEntry::model`), and the body does not say whether `crates/loom-bindings/Cargo.toml` names `b10x-canon`. story:canon-0-9-0-pin pins "the four manifests that name them" and fixes `ProtocolEntry::model` to Canon's `Protocol`. Canon 0.9.0 is breaking for `Action` and `EvidenceRecord` struct literals. No depends_on edge orders the two (canon-0-9-0-pin depends only on substrate-0-21-pin and deterministic-default-selector). The collision rests on an inferred surface, a manifest that does not exist yet — .engineering/planning/story/bindings-document.md:18, .engineering/planning/story/canon-0-9-0-pin.md:40, .engineering/planning/story/canon-0-9-0-pin.md:13
story:hash-chained-run-log — `loom-runlog` must append "under the store's lock", but that lock is `LockedDir`, `pub(crate)` in `crates/loom-governor/src/locked_dir.rs:45`, next to the case-file stem rule in `crates/loom-governor/src/file_store.rs:133`. The body names no change to `crates/loom-governor` for it, yet story:bound-artifact-revisions (cited at `file_store.rs:136-146`) and story:child-case-runtime also change that crate, and no edge orders the log against them. The log's side is inferred — .engineering/planning/story/hash-chained-run-log.md:21, .engineering/planning/story/bound-artifact-revisions.md:39
story:child-case-runtime — this and story:bound-agent-node both write evidence whose source is Loom's own effect (`from: effect`) and both change `crates/loom-bindings`. child-case-runtime's side is inferred, because its body names no crate for the `open` effect port or the load-time `open`/`case_open` check that bindings-document hands to it. Neither depends on the other; record-copy-between-cases joins them downstream — .engineering/planning/story/child-case-runtime.md:34, .engineering/planning/story/bound-agent-node.md:75

Round-1 findings, by outcome:
- All seven are resolved, none repeated. The revised sections are the ones named below.
- canon-0-9-0-pin against substrate-0-21-pin: resolved by the new edge (`canon-0-9-0-pin.md:13`, reason stated at `:65-68`).
- deterministic-default-selector against canon-0-9-0-pin and child-case-runtime: resolved by the edges `canon-0-9-0-pin.md:14` and `child-case-runtime.md:15`. `child-case-runtime.md:13` and `:16` also order canon-0-9-0-pin and bound-artifact-revisions against it.
- bindings-document against bound-commands-run-confined on `main.rs`: resolved. `bindings-document.md:84-86` now forbids touching `crates/loom-cli` and `crates/loom-intake-slice`.
- hash-chained-run-log against bindings-document on `ess/system.yaml`: resolved by `hash-chained-run-log.md:13`.
- record-copy-between-cases against the `ess/domains/bindings.yaml` stories: resolved by `record-copy-between-cases.md:14`, which sits behind bound-agent-node and bound-artifact-revisions. The unnamed bindings crate is now `crates/loom-bindings`.
- hash-chained-run-log reading revision moves: resolved. `hash-chained-run-log.md:75` leaves emitting events to tree-run-command.
- The `--max-agent-calls` cycle: resolved. The read moved to agent-node-run-records (`agent-node-run-records.md:26`, `:69-71`), which depends on tree-run-command (`:15`).

**What I read:** 15 story bodies, 13 of them revised, plus the epic's edges. I took edges from the frontmatter and from `aep plan artifact graph`. I ran `aep plan artifact waves --kind story --status draft`. I checked `~/beyond10x/canon/CHANGELOG.md` 0.9.0, `crates/loom-governor/src/file_store.rs` and `locked_dir.rs`, `crates/loom-intake-slice/src/mandate_log.rs`, and `git grep` over `crates/*/Cargo.toml` and `may_produce`.
- **Surfaces:** 14 of 14 cited, 0 unplaced, 3 inferred (one per finding above).
- **Reads:** about 25 acceptance reads traced to a producer in the set, 24 recorded by an edge. The one not recorded is the effect-sourced evidence path in the third finding.
- **Order:** the edges now give a total order along `bindings-document` to `bound-program-result-table` to `bound-artifact-revisions` to `bound-agent-node` to `record-copy-between-cases` to `tree-run-command` to `agent-node-run-records`. The files that were shared (the three new crates, `ess/system.yaml`, `responsibility.yaml`, `crates/loom-governor/src/lib.rs`, `ess/domains/{bindings,runlog,tree}.yaml`, `crates/loom-cli`) are ordered by it.

**What I could not establish:**
- Where `budget.max_runs` is counted. `bound-program-result-table.md:78-79` says "already run … times in this case" and names no state. If the source is the run log, there is no edge to hash-chained-run-log. This is for the acceptance critic.
- The collision between deterministic-default-selector and story:frontier-action-names-its-claims. It is admitted in prose (`deterministic-default-selector.md:51-55`), but that story is outside my set.
- `CHANGELOG.md`, `AGENTS.md`, `Cargo.lock` and `generated/rust/` are touched by many stories under the house rule. A person decides whether that is acceptable.
- `website/docs/reference/cli.md` regeneration is shared by confinement-opens-from-engine-config, bound-commands-run-confined and tree-run-command. Those three are ordered, so I make no claim.

```findings
- file: .engineering/planning/story/bindings-document.md
  line: 18
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "crates/loom-bindings checks protocols against Canon's compiled form and the body does not say whether its Cargo.toml names b10x-canon; story:canon-0-9-0-pin pins 'the four manifests that name them' and moves ProtocolEntry::model to Canon's Protocol, Canon 0.9.0 breaks Action and EvidenceRecord struct literals, and no depends_on edge orders the two (inferred: the loom-bindings manifest does not exist yet)"
- file: .engineering/planning/story/hash-chained-run-log.md
  line: 21
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the append 'under the store's lock' needs LockedDir, which is pub(crate) in crates/loom-governor/src/locked_dir.rs, and the case-file stem rule in file_store.rs; the body names no change to crates/loom-governor, yet story:bound-artifact-revisions and story:child-case-runtime also change that crate and no edge orders the log against them (inferred for the log's side)"
- file: .engineering/planning/story/child-case-runtime.md
  line: 34
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "this and story:bound-agent-node both write from: effect evidence and both change crates/loom-bindings; child-case-runtime names no crate for the open effect port or the load-time open/case_open check, and neither depends on the other (inferred for child-case-runtime's side)"
```
