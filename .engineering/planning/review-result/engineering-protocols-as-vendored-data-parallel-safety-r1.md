---
format: aep.planning-md/3
id: review-result:engineering-protocols-as-vendored-data-parallel-safety-r1
kind: review-result
status: active
title: Plan critic parallel-safety, round 1, on epic:engineering-protocols-as-vendored-data
relations:
- reviews: epic:engineering-protocols-as-vendored-data
- reviews: story:engineering-protocol-lock
- reviews: story:engineering-protocols-dependency-removed
- reviews: story:engineering-protocols-vendor-command
- reviews: story:engineering-registry-calls-replaced
revision: 1
---
needs-revision

- `story:engineering-protocols-vendor-command` — its Files add a sha256 crate to `crates/loom-xtask/Cargo.toml`, which also rewrites the `loom-xtask` package block in `Cargo.lock`, and `story:engineering-protocols-dependency-removed` edits `Cargo.lock` too; no edge orders stories 2 and 4 and neither body admits the shared file (surface cited for the Cargo.toml and the Cargo.lock edit on story 4, inferred for story 2's Cargo.lock edit; both remedies stand: an ordering edge naming `Cargo.lock` as its reason, or splitting the surface) — `.engineering/planning/story/engineering-protocols-vendor-command.md:69-70`, `.engineering/planning/story/engineering-protocols-dependency-removed.md:82`, `Cargo.lock:3716-3726`
- `story:engineering-protocols-vendor-command` — it and story 4 also share `AGENTS.md`, which only the epic admits; story 2's insertion in § Protocol composition (`AGENTS.md:44-80`) shifts the `AGENTS.md:35` and `:520-523` line citations that story 4's acceptance rests on, and story 4 says only that `Cargo.lock` lines move (cited, both bodies; same pair and cause as the first finding) — `.engineering/planning/story/engineering-protocols-vendor-command.md:62`, `.engineering/planning/story/engineering-protocols-dependency-removed.md:65-66`, `.engineering/planning/epic/engineering-protocols-as-vendored-data.md:84-85`

The two findings share one pair and one cause: stories 2 and 4 are unordered and both edit `Cargo.lock` and `AGENTS.md`. The epic's claim that stories 2 and 3 share no file holds. The epic names the `AGENTS.md` share with story 4 but not the `Cargo.lock` one.

What I read: 5 artifacts (the epic and its four stories) plus `story:canon-0-9-0-pin`, read only for its scope. I used `aep plan artifact show` on each and `aep plan artifact graph --format json`. I opened the manifests of `loom-protocols`, `loom-xtask`, `loom-governor`, `loom-intake-slice`, `loom-intake-router` and `loom-plugin`, `Cargo.lock:3716`, `AGENTS.md:30-37` and `:515-525`, `crates/loom-executor/tests/intake_import.rs`, and `crates/loom-xtask/tests/pinned_consumer.rs`.

Surfaces, per item:
- **Placed:** 4 cited, 0 inferred, 0 unplaced.
- **Inferred sub-surface:** story 2's `Cargo.lock` edit is inferred from its new dependency.
- **Pairs checked:**
  - Stories 1 and 3 collide on `engineering_lock.rs`, `CHANGELOG.md` and `adversary_w8_pin.rs`. They are ordered by an edge.
  - Stories 3 and 4 collide on `governed_case.rs` and `CHANGELOG.md`. They are ordered by an edge.
  - Stories 1 and 4 collide on `adversary_w8_pin.rs`. They are ordered through story 3.
  - Stories 1, 3 and 4 collide with `story:canon-0-9-0-pin` on the manifests, `Cargo.lock`, `adversary_w8_pin.rs`, `intake_import.rs`, `pinned_consumer.rs`, `AGENTS.md`, the two website docs and the governor and protocols sources. They are ordered because story 1 has `depends_on story:canon-0-9-0-pin`.
  - Stories 2 and 3 share no file, and that holds. Story 3 needs no manifest change, since all three crates already depend on `b10x-loom-protocols` (`crates/loom-governor/Cargo.toml:14`, `crates/loom-intake-slice/Cargo.toml:14`, `crates/loom-intake-router/Cargo.toml:14`).
  - Stories 2 and 4 collide, per the findings above.

The order against `story:canon-0-9-0-pin` is recorded where it must be. Story 1 holds the edge and states the reason, and stories 2 to 4 inherit it. Story 2 shares no file with the pin story.

Acceptance reads traced to a producer in the set: 6. They are story 1 reading the pin story's pinned registry, story 2 reading story 1's layout and files, story 3 reading story 1's catalog, story 4 reading story 3's removal, story 4 reading story 1's `adversary_w8_pin.rs` assertion, and story 4 reading the pin story's manifests and `Cargo.lock`. A direct `depends_on` edge records 4 of them. The other 2 are covered by a path through story 3 to story 1, and by story 1's edge to the pin story. None is unordered.

What I could not establish:
- Whether story 2 really changes `Cargo.lock`. The body names only `crates/loom-xtask/Cargo.toml`. `sha2` is already in the lock at `Cargo.lock:4963` and `:4974`, so only the `loom-xtask` dependency list would change.
- Whether stories 2 and 4 collide textually on `AGENTS.md`. The sections differ, so a merge would probably be clean. That is the operator's decision.
- Out of my lane: story 2's lock writer has no test against story 1's loader. Its `git status --porcelain protocols/engineering` check is manual, and the writer and loader share no type. That is for the acceptance and design critics.

```findings
- file: .engineering/planning/story/engineering-protocols-vendor-command.md
  line: 69
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:engineering-protocols-vendor-command adds a sha256 crate to crates/loom-xtask/Cargo.toml, which also rewrites the loom-xtask block of Cargo.lock (inferred for the Cargo.lock edit; cited for the manifest), and story:engineering-protocols-dependency-removed edits Cargo.lock too; no edge orders the two and neither body names the shared file; remedies are an ordering edge recording Cargo.lock as its reason or splitting the surface"
- file: .engineering/planning/story/engineering-protocols-vendor-command.md
  line: 62
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:engineering-protocols-vendor-command and story:engineering-protocols-dependency-removed both edit AGENTS.md (cited), admitted only in the epic (epic body lines 84-85) and in neither story; story 2's added lines in AGENTS.md section Protocol composition shift the AGENTS.md:35 and :520-523 citations story 4's acceptance rests on, and story 4 says only that Cargo.lock lines move"
```
