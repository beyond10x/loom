---
format: aep.planning-md/3
id: review-result:engineering-protocols-as-vendored-data-design-r2
kind: review-result
status: active
title: Plan critic design, round 2, on epic:engineering-protocols-as-vendored-data
relations:
- reviews: epic:engineering-protocols-as-vendored-data
- reviews: story:engineering-protocol-lock
- reviews: story:engineering-protocols-vendor-command
- reviews: story:engineering-registry-calls-replaced
- reviews: story:engineering-protocols-dependency-removed
revision: 1
---
needs-revision

story:engineering-protocol-lock — The body never says what text `VendoredError` shows when printed. Story 3's acceptance checks that text, and story 3 may not edit the file that defines it. Story 1 should state that the text names the `LockRefusal` variant and the path, and test it. — .engineering/planning/story/engineering-protocol-lock.md:21-33 (`engineering()` is `from_vendored` "with the error turned into its `String`"; no form is given), .engineering/planning/story/engineering-registry-calls-replaced.md:57-62 (`problem` names `DigestMismatch` and `software-change/1.yaml`) and :101-102 (story 3 may not change `crates/loom-protocols/src/`), generated/rust/intake/src/protocols.rs:37 (generated enums derive only `Debug`, with no `Display`)

**What I read:** 5 artifacts, plus `story:canon-0-9-0-pin` for context.
- **Commands:** `aep plan artifact show` on each id, `aep plan artifact relations`, `aep plan artifact graph --format json`, `aep plan artifact validate`, and `git grep` at `39f4da88`.
- **Source files I opened:**
  - `crates/loom-xtask/Cargo.toml:1-20`
  - `crates/loom-protocols/src/lib.rs:14-22,150-175`
  - `crates/loom-protocols/src/store.rs:185-189`
  - `crates/loom-governor/src/lib.rs:494-540,1715-1742`
  - `generated/rust/intake/src/protocols.rs:1-60`
- **Edges walked:**
  - Inside the set: 14 (5 `depends_on`, 4 `decomposes`, 5 `serves`).
  - Outside the set: `story:canon-0-9-0-pin`'s 3 `depends_on` edges, and every inbound edge in the store. Outside the set, only `review-result` artifacts point at the five. No cycle.
- **Shape:**
  - Lock → registry calls → dependency removed is still an expand, migrate, contract cut, and each step leaves the tree green.
  - Story 3 can leave the dependency lines in place: no workspace manifest, `Taskfile.yml` or CI file sets `unused_crate_dependencies`, machete or udeps (`git grep`).
  - The vendor command runs beside story 3. The edge from story 4 to story 2 has its reason written beside it in both bodies: `Cargo.lock` and `AGENTS.md`. That makes it a shared-file order, not a serialising chain.
  - The order after `story:canon-0-9-0-pin` is recorded where it has to be: story 1 depends on it, and the other three reach it through story 1.
- **Round-1 design finding (second encoder) is fixed.** Story 2 writes the lock through `lock::locked_file`/`lock::encode` (vendor-command.md:9-10). Story 1 states the exact bytes (lock.md:55-60). `the_committed_lock_is_the_encoders_bytes` checks them, and story 2's temporary-repository test loads what it wrote through `from_vendored` and re-encodes it to the same bytes. Story 1 also produces the revision-from-tag that story 2's `t1` check reads (lock.md:63).
- **Acceptance reads:** 11 traced to a producer in the set or to `story:canon-0-9-0-pin`, and an edge records all 11, directly or through the chain:
  - story 2 reads `lock`, `from_vendored` and the vendored files;
  - story 3 reads `from_vendored`, `ENGINEERING_FILES` and the error text, and deletes a test in `engineering_lock.rs`;
  - story 4 reads the w8 test that story 1 rewrote, story 2's `AGENTS.md` insertion and story 3's grep-clean source.

  Only the error-text read is missing a stated form, which is the finding above.
- **Validator:** 55 lines; none names an artifact in this set.

**What I could not establish:**
- Out of my lane (acceptance): story 4's grep that `0\.16\.0` does not appear in `AGENTS.md` lists hits at `39f4da88`. `story:canon-0-9-0-pin`, which runs first, may add `0.16.0` lines to § Governor. I did not read that story's body to check.
- Out of my lane (scope): the 12 test call sites match `git grep canon_engineering -- 'crates/*/tests/*'` exactly. No unlisted site remains.
- Open 3 (`loom-xtask` compiles Canon) and the location of `protocols/engineering/` were decided by the coordinator. I did not raise either.

```findings
- file: .engineering/planning/story/engineering-protocol-lock.md
  line: 21
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "The body never says what text VendoredError shows when printed. Story 3's acceptance checks that text, and story 3 may not edit the file that defines it. Story 1 should state that the text names the LockRefusal variant and the path, and test it."
```
