---
format: aep.planning-md/3
id: review-result:engineering-protocols-as-vendored-data-scope-r1
kind: review-result
status: active
title: Plan critic scope, round 1, on epic:engineering-protocols-as-vendored-data
relations:
- reviews: epic:engineering-protocols-as-vendored-data
- reviews: story:engineering-protocol-lock
- reviews: story:engineering-protocols-dependency-removed
- reviews: story:engineering-protocols-vendor-command
- reviews: story:engineering-registry-calls-replaced
revision: 1
---
needs-revision

1. `story:engineering-protocols-dependency-removed` — the epic promises "`AGENTS.md` § Governor … names no engineering-protocols tag", but this story's AGENTS.md acceptance asks only for the Canon reference and `cargo update -p b10x-canon`, so that half of the promise has no claim. The text to remove is the current "tag (now `0.15.2`)" at `AGENTS.md:525`. The story's AGENTS.md acceptance should say that § Governor names no engineering-protocols tag. — `.engineering/planning/epic/engineering-protocols-as-vendored-data.md:110` against `.engineering/planning/story/engineering-protocols-dependency-removed.md:65`

**What I read.**
- Promises extracted from the epic: 11 (five from Outcome, five from Acceptance, one from the request's "Breaking entry" line). I traced 10 to an item. The eleventh is the AGENTS.md "names no tag" half above.
- The request's observation-collection item is out of the epic by the coordinator's decision, and `e5-plan.md` item 8 names it.
- I read 5 artifacts in full (`aep plan artifact show`), `e5-request.md`, `e5-plan.md`, the boundary doc, and the graph.
- I checked three claims in the tree by grep:
  - The 4 manifests and the 12 test sites are exactly the sites the stories list; no other manifest, `consumers/` or other crate names `canon_engineering`.
  - The graph holds no earlier story that already vendors the catalog. The existing `engineering-protocols-*` stories are all `implemented` pin moves.
  - `AGENTS.md:523-526` still has the tag sentence.

**Reach beyond the parent.** I found none that sets the verdict.
- Story 4 edits three website docs and two test doc comments. They go with "no Loom manifest names `b10x-canon-engineering`", and the epic § Order names `sdk-stability.md` and `move-to-loom.md` as files this change touches.
- Story 3's `OpenError::InvalidProtocol` for a lock refusal, and story 1's revision taken from the lock's tag, are consequences of the promised load check, not new outcomes.

**Overlap.** Stories 1 and 3 both touch `the_vendored_files_equal_the_pinned_registry`. Story 1 creates it and story 3 deletes it, in order, so no outcome is claimed twice.

**Where the order against `story:canon-0-9-0-pin` is recorded.** It is recorded correctly: `depends_on` is on story 1 only, and stories 2 to 4 inherit it through story 1. The epic body states the reason.

**What I could not establish, or leave to other lanes.**
- `e5-plan.md` § Edges says "the one edge out of the epic, to `story:canon-0-9-0-pin`", but the epic's `relations` hold only the two `serves` edges. That is a mismatch in the drafter's note, not in the store, so it is not a finding.
- Out of my lane (parallel-safety):
  - Stories 2 and 4 both edit `AGENTS.md`, yet only story 4 is ordered after story 3. The epic § Stories says they are different sections.
  - Stories 2 and 3 are said to be able to run together.
- Out of my lane (acceptance):
  - Story 4's AGENTS.md line citations `:520-523` are already off by 3 (the text sits at `AGENTS.md:523-526`). This will also move when `story:canon-0-9-0-pin` lands.
  - After story 3 deletes the registry-equality test, "the same eleven registrations" rests on `crates/loom-protocols/tests/catalog.rs`. I did not verify that it pins the full list.
- I did not check whether `b10x-canon-expr` leaves `Cargo.lock`. `e5-plan.md` open point 6 says it is not read.

```findings
- file: .engineering/planning/epic/engineering-protocols-as-vendored-data.md
  line: 110
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:engineering-protocols-dependency-removed does not claim the epic acceptance that AGENTS.md § Governor names no engineering-protocols tag; its AGENTS.md acceptance (story line 65) asks only for the Canon reference and cargo update -p b10x-canon, so the tag sentence at AGENTS.md:525 is not required to go"
```
