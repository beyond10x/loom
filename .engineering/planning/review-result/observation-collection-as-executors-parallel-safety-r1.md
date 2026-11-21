---
format: aep.planning-md/3
id: review-result:observation-collection-as-executors-parallel-safety-r1
kind: review-result
status: active
title: Plan critic parallel-safety, round 1, on epic:observation-collection-as-executors
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

- `epic:observation-collection-as-executors` — § Stories says the stories that add `ObserveOperation` variants never share a wave and that story 7 "can run beside 2, 4, 5 or 6, not beside 3". That is wrong for both claims. Stories 2 and 7 have no edge to 3, 4 or 5 (7 has none to 6 either). Each pair shares `generated/rust/loom/` (`plan.json` and `PLAN.md` carry the ESS `source_digest`, so any `ess/` change rewrites them) and `CHANGELOG.md`. The pairs 2/5 and 7/3 share a further file (surfaces below). The epic admits only 7/3 on `Cargo.lock`. Remedy A is an ordering edge per pair that records the shared path as its reason. Remedy B is splitting the surface, for example one regeneration owner. (cited, all bodies) — `.engineering/planning/epic/observation-collection-as-executors.md:123-126`, `.engineering/planning/story/gate-program-identity-bound-to-spawn.md:155-161`, `.engineering/planning/story/gates-document-evaluated-in-the-governor.md:107-112`
- `story:gate-program-identity-bound-to-spawn` — it is unordered against stories 3, 4 and 5. Its `## Waits on` names only story 1 as the reason for the edge. It shares `ess/domains/bindings.yaml`, `generated/rust/loom/` and `CHANGELOG.md` with all three. It shares `crates/loom-bindings/src/program.rs` with story 5. `program.rs` is not in the tree yet: `story:bound-program-result-table` does not name it, and both bodies cite it as "the program effect port". Those edge reasons are the same as the ones that justify its edge to story 1, and neither body admits the pair. Remedy A is an ordering edge, such as 5 behind 2 or 2 behind 3. Remedy B is moving the `report_format` check out of `program.rs`. (cited, both bodies; the `program.rs` name is cited but the file does not exist) — `.engineering/planning/story/gate-program-identity-bound-to-spawn.md:71-76`, `:155-161`, `.engineering/planning/story/report-formats-checked-before-the-table.md:105-111`
- `story:gates-document-evaluated-in-the-governor` — its `## Waits on` names only story 1 and the dependency-removed story. It is unordered against stories 2, 3, 4, 5 and 6. Against all five it shares `generated/rust/loom/` (`plan.json`, `PLAN.md`, the module list in `src/lib.rs`) and `CHANGELOG.md`. Against story 3 it also shares `Cargo.lock`, and story 3 lists `Cargo.lock` too. An epic sentence about `Cargo.lock` is not an edge, so `aep plan artifact waves` will place 3 and 7 in one wave. Remedy A is an ordering edge recording the shared files. Remedy B is leaving 7 out of any wave that holds a story with an ESS-first commit. (cited, both bodies) — `.engineering/planning/story/gates-document-evaluated-in-the-governor.md:52-56`, `:107-112`, `.engineering/planning/story/observe-reads-documents.md:91-96`

The three findings share one cause. Only stories 1, 3, 4, 5 and 6 are chained, and stories 2 and 7 hang off story 1 alone. Every ESS-first story rewrites the same generated crate and the same changelog.

What I read: 12 artifacts, the epic and its ten stories plus the two upstream blockers, with `aep plan artifact show` on each, `aep plan artifact graph --format json`, `aep plan artifact relations` and `aep plan artifact waves`. Beyond those I read `story:bound-program-result-table`, `story:bound-commands-run-confined`, `story:engineering-protocols-dependency-removed`, `story:engineering-protocols-vendor-command`, `story:engineering-protocol-lock` (grep only) and `story:canon-0-9-0-pin` for scope and edges. I also read the vendored-data parallel-safety r1 result for the store's convention. In the tree I opened `generated/rust/loom/` (`plan.json` header, `PLAN.md` header, `src/lib.rs`), `ess/domains/bindings.yaml:24-28`, `Cargo.lock:753` and `:769`, `confinement.rs:475` and `:1157`, `AGENTS.md:436-443` and `:2129-2137`, and `crates/loom-bindings/src` and `tests`.

Surfaces per item (from each body's `## Files`):
- Placed: 10 stories cited, 0 inferred, 0 unplaced.
- The epic is an umbrella with no file surface of its own.
- Stories 9 and 10 name `confinement.rs` and `network.rs` provisionally. The real Substrate-facing surface is unknown until the blockers clear.

Acceptance reads traced to a producer in the set: 13. A direct `depends_on` edge records 8. A path through other edges records the other 5, which are stories 4, 5 and 6 reading story 1's reader through 3, and story 8 reading stories 1 to 5 through story 6. None is unordered.

The edges to `upstream-blocker:*` are `blocks` edges from the blockers, which I accepted as the hold.

Pairs checked. Unordered pairs: 2/3, 2/4, 2/5, 7/2, 7/3, 7/4, 7/5, 7/6, all in the findings above. Every other pair is ordered by an edge or a path.

What I could not establish:
- Whether `crates/loom-bindings/src/program.rs` is the file `story:bound-program-result-table` creates. That story's own scope names only the crate directory.
- The real surface of stories 9 and 10. The Substrate-facing code is written "against the surface when it clears". Story 10 lists no runner or driver file, and story 9's `confinement.rs` is provisional. Keep both out of any concurrent set until the blockers clear. This is an unassessed surface, not a finding.
- Collisions with items outside the set, for example `story:ess-format-31` on `ess/system.yaml` and `generated/rust/loom/`. Out of scope.
- Out of my lane, so they do not set the verdict:
  - Stories 9 and 10 do not list `observe/gates.rs`, `tests/observe_catalog.rs` or `tests/gate_run.rs`. Story 8 refuses `net.*` by name there "until stories 9 and 10". For the scope and acceptance critics.
  - Story 7 names the whole directory `crates/loom-xtask/src/` rather than `protocols_vendor.rs`. It also changes the vendor command's behaviour without listing that command's test. For scope and design.
  - Whether the governor's `gates::evaluate` is "Canon inside the governor" and holds the catalog to the tag's bytes. Design. I did not assess the carry-over of the four engineering-protocols gate stories either (acceptance).

```findings
- file: .engineering/planning/epic/observation-collection-as-executors.md
  line: 123
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the epic says stories 1, 3, 4, 5 and 6 never share a wave and that story 7 can run beside stories 2, 4, 5 or 6, but stories 2 and 7 have no edge to 3, 4 and 5 (7 none to 6 either) and each pair shares generated/rust/loom/ plan.json and PLAN.md (the ESS source_digest) and CHANGELOG.md, with program.rs also shared by 2 and 5 and Cargo.lock by 7 and 3 (cited, all bodies); remedies are an ordering edge per pair recording the shared path or splitting the surface"
- file: .engineering/planning/story/gate-program-identity-bound-to-spawn.md
  line: 71
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:gate-program-identity-bound-to-spawn names only story 1 as the reason for its edge, yet no edge orders it against story:observe-reads-documents, story:observe-compares-generated-trees or story:report-formats-checked-before-the-table, which share ess/domains/bindings.yaml, generated/rust/loom/ and CHANGELOG.md with it, and story 5 also shares crates/loom-bindings/src/program.rs (cited in both bodies; the file is not in the tree yet and story:bound-program-result-table does not name it); remedies are an ordering edge recording the shared files or splitting the surface"
- file: .engineering/planning/story/gates-document-evaluated-in-the-governor.md
  line: 52
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:gates-document-evaluated-in-the-governor waits only on story 1 and the dependency-removed story, and no edge orders it against stories 2, 3, 4, 5 or 6, which share generated/rust/loom/ (plan.json, PLAN.md, src/lib.rs module list) and CHANGELOG.md with it, and story 3 also Cargo.lock (cited, both bodies), so one wave would hold them; remedies are an ordering edge recording the shared files or splitting the surface"
```
