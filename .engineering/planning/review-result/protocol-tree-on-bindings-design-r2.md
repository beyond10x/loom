---
format: aep.planning-md/3
id: review-result:protocol-tree-on-bindings-design-r2
kind: review-result
status: active
title: Plan critic design, round 2, on epic:protocol-tree-on-bindings
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
approve

What I read: 17 artifacts, shown whole again with `aep plan artifact show` (the epic, the 14 stories, the upstream blocker and `story:child-launches-through-confinement-adapters`). I also ran `aep plan artifact graph` and `aep plan artifact validate`, which printed `valid`. I walked 24 `depends_on` edges and the one `blocks` edge, including the edges outside the set into `story:child-launches-through-confinement-adapters` and `story:substrate-0-21-pin`. The graph is acyclic. It does not serialise: `story:substrate-0-21-pin`, `story:bindings-document` and `story:deterministic-default-selector` have no `depends_on` edges, and `story:hash-chained-run-log` depends only on `story:bindings-document`. I traced about 30 acceptance reads to a producer in the set, and every one has a `depends_on` edge, direct or along a chain.

Each round-1 finding is closed by a changed line:
- **`--max-agent-calls` (`story:tree-run-command`, `story:agent-node-run-records`):** the flag, its count and the agent marker now sit in one story, `story:agent-node-run-records`. That story gained `depends_on story:tree-run-command` and its text at line 631-633. `story:tree-run-command` now lists the cap as out of scope.
- **Emission order and revision moves (`story:hash-chained-run-log`):** the log story declares and writes events only, and says emission belongs to `story:tree-run-command`. `story:tree-run-command` states that it emits them and has the acceptance line "`action finished` precedes the `evaluated` line".
- **Revision rule (`story:bound-program-result-table`):** its outcome now says that until `story:bound-artifact-revisions` lands, the record names the revision the case held when the program started. The two stories no longer contradict.
- **`retry_when` match (`story:bound-agent-node`):** it now defines `retry_records` itself, with acceptance lines for it. `story:tree-run-command` builds `due` on it, through `story:record-copy-between-cases`.
- **Termination (`story:child-case-runtime`):** termination is now an explicit `terminate(case, outcome, instant)` call, and the story has edges to `story:deterministic-default-selector` and `story:bound-artifact-revisions`.
- **Bound commands (`story:confinement-opens-from-engine-config`):** its outcome now says bound commands reach the runner in `story:bound-commands-run-confined`.

The new edges all state a shared file beside them, so I report none as a chain. The pairs and their files:
- `story:canon-0-9-0-pin` to `story:substrate-0-21-pin`: pin manifests, `Cargo.lock`, `AGENTS.md`, `CHANGELOG.md`.
- `story:canon-0-9-0-pin` to `story:deterministic-default-selector`: `crates/loom-governor/src/lib.rs`.
- `story:child-case-runtime` to `story:deterministic-default-selector`: `ess/commission/domains/responsibility.yaml` and `crates/loom-governor/src/lib.rs`.
- `story:record-copy-between-cases` to `story:bound-agent-node`: `ess/domains/bindings.yaml`.
- `story:hash-chained-run-log` to `story:bindings-document`: `ess/system.yaml`.

What I could not establish:
- Out of my lane, for parallel-safety: `story:deterministic-default-selector` shares `FrontierAction` with the draft `story:frontier-action-names-its-claims`. The body keeps that apart by prose ("must not be accepted into a wave with it"), not by an edge.
- Out of my lane, for acceptance and scope:
  - `story:tree-run-command` names the flag `--confinement linux|local`, while `story:confinement-opens-from-engine-config` gives `b10x-loom run --confinement` the values `substrate|local|none`.
  - The `claude-code` adapter's process runs outside Substrate. The epic states this in its Not in scope, and the acceptance run uses stub programs.
- I did not read the Loom source behind the `path:line` citations in the bodies. Whether `story:child-case-runtime`'s opening effect port belongs in `crates/loom-governor` or the bindings crate is a placement detail I could not settle from the bodies.

```findings
[]
```
