---
format: aep.planning-md/3
id: review-result:engineering-protocols-as-vendored-data-design-r1
kind: review-result
status: active
title: Plan critic design, round 1, on epic:engineering-protocols-as-vendored-data
relations:
- reviews: epic:engineering-protocols-as-vendored-data
- reviews: story:engineering-protocol-lock
- reviews: story:engineering-protocols-dependency-removed
- reviews: story:engineering-protocols-vendor-command
- reviews: story:engineering-registry-calls-replaced
revision: 1
---
needs-revision

story:engineering-protocols-vendor-command — `lock.json` would be written by a second encoder in `loom-xtask`, while story 1's `lock.rs` holds the only decoder and neither body fixes the file's byte form; the body should say the command writes the lock through story 1's lock module, or story 1 should state the exact bytes (member order, path sort, trailing newline), so that the real-checkout `git status` check and the loader agree. — .engineering/planning/story/engineering-protocols-vendor-command.md:22, :60; .engineering/planning/story/engineering-protocol-lock.md:20, :75; crates/loom-xtask/Cargo.toml:11

**What I read:** 5 artifacts in the set, plus `story:canon-0-9-0-pin` and `story:bindings-document` for context.
- **Commands:** `aep plan artifact show` on each, `aep plan artifact relations`, `aep plan artifact graph --format json`, `aep plan artifact validate`. I also read the four manifests, `crates/loom-xtask/Cargo.toml`, the `AGENTS.md` headings and `generated/rust/intake/src/protocols.rs`, and ran `git grep canon_engineering`.
- **Edges walked:** 14 inside the set (4 `depends_on`, 4 `decomposes`, 6 `serves`). Outside the set I walked `story:canon-0-9-0-pin` and its 3 `depends_on` edges, and every inbound edge in the store. No outside artifact points at any of the four stories, so there is no cycle.
- **Shape:**
  - The chain lock → registry calls → dependency removed is an expand, migrate, contract cut with a stated reason on each edge, and the vendor command runs beside it.
  - The order after `story:canon-0-9-0-pin` is recorded where it has to be: story 1 `depends_on` it, and the other three reach it through story 1.
- **Acceptance reads:** I traced 9 acceptance reads to a producer in the set or to `story:canon-0-9-0-pin`. An edge records all 9, directly or through the chain.
- **Validator:** its 55 lines name no artifact in this set.

**Behind the finding:** `loom-xtask` links neither `loom-protocols` nor the generated intake crate. The generated domain types have no serde derive (`generated/rust/intake/src/protocols.rs:13`), so the lock file gets one hand-written writer and one hand-written reader.
- Story 2's temporary-repository tests never load the lock they write through the load check.
- Its only check that the two agree is the by-hand run on the real checkout. That run compares against a `lock.json` that story 1 writes by hand in an unstated form.

**What I could not establish:**
- I did not confirm that a consumer building `loom-protocols` from a Git dependency can reach `protocols/engineering/`. Story 1's `build.rs` reads it from outside the crate directory. This is feasibility, out of my lane; `task consumer-check` would show it.
- Out of my lane (parallel safety): story 2 edits `AGENTS.md` § Protocol composition (lines 44-80). Story 4 cites `AGENTS.md:35` and `:520-523` by line number with no edge to story 2, so those lines move if story 2 lands first. The sections differ; the epic's "another section" is correct.
- Out of my lane (scope): story 4 names Canon by the `b10x-canon` line of `crates/loom-governor/Cargo.toml` alone. `crates/loom-protocols/Cargo.toml:14` and `crates/loom-bindings/Cargo.toml:14` also carry that line.
- Out of my lane (acceptance): I did not check whether story 3's red step really fails to compile. `builtin()` is private, so its return type is not visible from `governed_case.rs`.

```findings
- file: .engineering/planning/story/engineering-protocols-vendor-command.md
  line: 22
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "lock.json would be written by a second encoder in loom-xtask, while story 1's lock.rs holds the only decoder and neither body fixes the file's byte form; the body should say the command writes the lock through story 1's lock module, or story 1 should state the exact bytes (member order, path sort, trailing newline), so that the real-checkout git status check and the loader agree."
```
