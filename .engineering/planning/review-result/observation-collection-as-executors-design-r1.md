---
format: aep.planning-md/3
id: review-result:observation-collection-as-executors-design-r1
kind: review-result
status: active
title: Plan critic design, round 1, on epic:observation-collection-as-executors
relations:
- reviews: epic:observation-collection-as-executors
- reviews: story:dns-resolution-through-a-resolver-aperture
- reviews: story:gate-program-identity-bound-to-spawn
- reviews: story:gate-run-collects-a-gates-document
- reviews: story:gates-document-evaluated-in-the-governor
- reviews: story:observe-compares-generated-trees
- reviews: story:observe-executor-reads-workspace-files
- reviews: story:observe-git-changed
- reviews: story:observe-reads-documents
- reviews: story:report-formats-checked-before-the-table
- reviews: story:tcp-reachability-through-an-aperture
revision: 1
---
needs-revision

story:gate-program-identity-bound-to-spawn — the body defines admission only when the program effect port is built, for a bindings document's `argv[0]` and the host's search path, yet story 6 admits `git` and story 8 admits project commands at run time against the project's `search_path`; the body needs to say admission is one callable unit taking a name and a search path, or story 8 needs to add that unit and list `admission.rs`/`program.rs` in its Files — .engineering/planning/story/gate-program-identity-bound-to-spawn.md:21-22, .engineering/planning/story/observe-git-changed.md:28, .engineering/planning/story/gate-run-collects-a-gates-document.md:32-33

story:observe-git-changed — it turns the observe executor into a process launcher (git through `run_child`; story 8 adds project commands) but does not say it changes story 1's rule that observe runs "with no process", its test "No observe action starts a process", or the reader-only build of the observe port (`NoReader`); story 1 and story 6 each describe half of the observe port, and the body names no launch port added to it — .engineering/planning/story/observe-executor-reads-workspace-files.md:20, :158, .engineering/planning/story/observe-git-changed.md:25, :124

story:tcp-reachability-through-an-aperture — its Waits on names `story:observe-git-changed`, but the graph has `depends_on story:gate-run-collects-a-gates-document`; the body also does not say it lifts story 8's refusal of `net.tcp.reachable` and updates `observe_operations_cover_the_catalog`, or how a gate request's `host`/`port` gets the `network` aperture argument; no story owns the gate-run half of TCP, and its Files leave out `observe/gates.rs` and `observe_catalog.rs` (story 10 lists `gates.rs` only for the TTL) — .engineering/planning/story/tcp-reachability-through-an-aperture.md:13, :48, :84-86, .engineering/planning/story/gate-run-collects-a-gates-document.md:104

story:observe-executor-reads-workspace-files — it puts the `WorkspaceReader` (a trait in `b10x-loom-bindings`) implementation in `crates/loom-intake-slice/src/confinement/read.rs`, but `story:bound-program-result-table` makes `loom-bindings` depend on `loom-intake-slice` for the runner; implementing the trait there would make `loom-intake-slice` depend on `loom-bindings`, a crate cycle; the body needs to say the impl lives in `loom-bindings` over a public driver handle in `loom-intake-slice` — .engineering/planning/story/observe-executor-reads-workspace-files.md:45, :203, .engineering/planning/story/bound-program-result-table.md:113

What I read: the epic, 10 stories and 2 upstream blockers (`aep plan artifact show`). Commands: `aep plan artifact relations` and `aep plan artifact graph --format json`. I followed 66 `depends_on` edges from the set across 36 nodes, including outside the set, and found no cycle. `aep plan artifact validate` printed `valid`, with only old review-outcome reminders outside the set. I traced 14 acceptance reads to a producer in the set or upstream. 13 have an edge, directly or through the chain. The 14th, story 8's `net.*` refusal being lifted, has no producer, which is the TCP finding above. I also checked the crate manifests (governor, executor, bindings, intake-slice, commission), `AGENTS.md` § Governor (lines 437-526), and the catalog bytes at tag `0.16.0`: the sha256 matches story 7, `d86dca5d…`.

What I could not establish:
- **The `1-3-4-5-6-8-9-10` queue.** Eight of the ten items run in one order. Every edge in it has a shared-file reason written beside it, so it is not a finding. The trade-off: keep the order, or declare the whole closed `ObserveOperation`/`ObserveRefusal` list in story 1's ESS commit (`ess/domains/bindings.yaml`, `ess/domains/observation.yaml`) and give each operation group its own module, which takes 3, 4, 5 and 6 off the shared `observe.rs` match.
- **Governor point from the brief.** Story 7 meets the letter of `AGENTS.md` § Governor (the code is in `loom-governor`, `now` is an argument, the dependency direction allows bindings → governor). Story 8 calls it from inside an observe action, so the gates evaluation runs in the acting path and only its result reaches the governor's protocol evaluation as evidence. Whether "the only crate that evaluates protocols with Canon" covers that is something I'm uneasy about, but I can't name a fix, so it is not a finding. The 26-function catalog is held to the tag's bytes.
- **Outside my lane (parallel safety).** No edge orders 2 against 3, 4 or 5, although all of them edit `ess/domains/bindings.yaml` and 2 and 5 both edit `crates/loom-bindings/src/program.rs`. No edge orders 7 against 3, which both edit `Cargo.lock`. The epic's line "never share a wave" (epic `:123`, `:126`) is not fully backed by edges.
- **Outside my lane (scope/acceptance).** No collector story carries the TCP-host preflight (`tcp_host_admission_is_preflighted_before_any_collection`). Story 8 lets a project `search_path` come from the repository, but story 2's checks refusing workspace entries are stated for the host's search path.

```findings
- file: .engineering/planning/story/gate-program-identity-bound-to-spawn.md
  line: 21
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the body defines admission only when the program effect port is built, for a bindings document's argv[0] and the host's search path, yet story 6 admits git and story 8 admits project commands at run time against the project's search_path; the body needs to say admission is one callable unit taking a name and a search path, or story 8 needs to add that unit and list admission.rs/program.rs in its Files"
- file: .engineering/planning/story/observe-git-changed.md
  line: 25
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it turns the observe executor into a process launcher (git through run_child; story 8 adds project commands) but does not say it changes story 1's rule that observe runs with no process, its test that no observe action starts a process, or the reader-only build of the observe port (NoReader); story 1 and story 6 each describe half of the observe port, and the body names no launch port added to it"
- file: .engineering/planning/story/tcp-reachability-through-an-aperture.md
  line: 48
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its Waits on names story:observe-git-changed, but the graph has depends_on story:gate-run-collects-a-gates-document; the body also does not say it lifts story 8's refusal of net.tcp.reachable and updates observe_operations_cover_the_catalog, or how a gate request's host/port gets the network aperture argument; no story owns the gate-run half of TCP, and its Files leave out observe/gates.rs and observe_catalog.rs"
- file: .engineering/planning/story/observe-executor-reads-workspace-files.md
  line: 203
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it puts the WorkspaceReader (a trait in b10x-loom-bindings) implementation in crates/loom-intake-slice/src/confinement/read.rs, but story:bound-program-result-table makes loom-bindings depend on loom-intake-slice for the runner; implementing the trait there would make loom-intake-slice depend on loom-bindings, a crate cycle; the body needs to say the impl lives in loom-bindings over a public driver handle in loom-intake-slice"
```
