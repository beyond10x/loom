---
format: aep.planning-md/3
id: review-result:engineering-protocols-as-vendored-data-acceptance-r1
kind: review-result
status: active
title: Plan critic acceptance, round 1, on epic:engineering-protocols-as-vendored-data
relations:
- reviews: epic:engineering-protocols-as-vendored-data
- reviews: story:engineering-protocol-lock
- reviews: story:engineering-protocols-dependency-removed
- reviews: story:engineering-protocols-vendor-command
- reviews: story:engineering-registry-calls-replaced
revision: 1
---
needs-revision

- `story:engineering-protocol-lock` — the acceptance never shows `ProtocolCatalog::engineering()` building its entries from the vendored files or checking them against the lock. Today's code, still reading `canon_engineering::registry` (the crate stays in this story), passes every bullet: file bytes equal the registry, `definition.sha256` equals the lock entry, and `catalog.rs` and `catalog_route.rs` pass unchanged. The refusal tests (`lock.rs` unit tests) exercise the check function on in-memory input, not the load. The acceptance has no test that `engineering()` calls the check, or that an entry's `definition.yaml` is the embedded file. The epic delegates "refused when the catalog loads" to this story (`.engineering/planning/epic/engineering-protocols-as-vendored-data.md:108-109`). — `.engineering/planning/story/engineering-protocol-lock.md:68-80`
- `story:engineering-protocols-vendor-command` — the lock's `repository` field has no stated source and no assertion. The command takes only `--checkout` and `--tag`, and the Outcome lists "repository" among the fields it writes. The temp-repo test never asserts `repository`. The by-hand check, "`git status --porcelain protocols/engineering` empty", passes or fails on a value the story does not define (a checkout's remote URL can be ssh or https, or absent). The body must say where `repository` comes from and the test must assert it. — `.engineering/planning/story/engineering-protocols-vendor-command.md:22`, `:62-63`
- `story:engineering-registry-calls-replaced` — the Outcome promises that a catalog failing its lock check is `OpenError::InvalidProtocol` naming the protocol and the refusal. No acceptance line or red step observes it. The embedded files are valid, so no listed test can reach the branch. Either add an observable check (an injectable source, or a stated unit test of the mapping) or drop the clause. — `.engineering/planning/story/engineering-registry-calls-replaced.md:24-26`, `:59-79`
- `epic:engineering-protocols-as-vendored-data` — "each engineering entry with the sha256 it had on the crate (… `engineering_lock.rs`)" cannot be checked when the epic closes. The only comparison against the crate is `the_vendored_files_equal_the_pinned_registry`, which story 3 deletes (`story/engineering-registry-calls-replaced.md:67-68`). What remains compares an entry to the lock, which was itself copied from the crate's bytes. The bullet is true only at story 1's close, so the epic acceptance should name a standing observable instead, such as literal expected digests in a test. — `.engineering/planning/epic/engineering-protocols-as-vendored-data.md:105-107`
- `epic:engineering-protocols-as-vendored-data` — the Outcome promises that "A maintainer moves the vendored files with one `loom-xtask` command … with no network call", but no Acceptance bullet names the command or defers to `story:engineering-protocols-vendor-command`. The epic could be closed with story 2 unfinished and every epic bullet still true. — `.engineering/planning/epic/engineering-protocols-as-vendored-data.md:23-24`, `:101-112`

Checked and not findings:
- Acceptance sections are bulleted checklists, as in `story:canon-0-9-0-pin`, and each bullet is one checkable statement.
- The order after `story:canon-0-9-0-pin` is recorded where it must be. Story 1 has `depends_on story:canon-0-9-0-pin` (`aep plan artifact show`, relations). Stories 2 and 3 depend on story 1 and story 4 on story 3, so all four are ordered transitively. The epic body and story 1 § Waits on state the reason.
- The `file:line` citations I opened hold. The 12 call sites (`grep canon_engineering` finds 12 test calls plus `lib.rs:160,162`, `classify.rs:32` and `governor lib.rs:1724-1736`). `Cargo.lock` has engineering-protocols sources only on the four named packages, and `b10x-canon-expr` comes from the Canon repository. `catalog.rs` and `catalog_route.rs` exist. `SINGLE_COPY` has 5 entries, so 4 remain. `AGENTS.md:520-523` and `:35` are correct at `b38eb7d1`; the working-tree `AGENTS.md` is modified by someone else, so those lines have shifted there.

What I read: 5 artifacts (epic and 4 stories) in full via `aep plan artifact show`, plus `story:canon-0-9-0-pin`, `aep plan artifact kinds` and `aep plan artifact lifecycle story`, `e5-plan.md`, the critic procedure and rubric. In the tree I opened `loom-protocols` `lib.rs` and `adversary_w8_pin.rs`, `catalog.rs`, governor `builtin()`, `classify.rs`, `Cargo.lock`, the `intake_import.rs` and `pinned_consumer.rs` fragments, `sdk-stability.md`, `Taskfile.yml`, and `release_check.rs` by grep.

What I could not establish:
- Whether the engineering-protocols checkout exists, so the digest equality at `0.15.2` and `0.16.0` is not re-checked. Doing so is outside the acceptance question.
- `~/beyond10x/sdc/docs/engineering-protocols-boundary.md:88` is not re-read; the epic itself says so.

Out of my lane (did not set the verdict):
- Design: story 3's red step says a new governor test "fails to compile until `builtin()` returns a catalog entry". `builtin()` is private and `ProtocolCatalog::engineering()` already exists, so that test would compile and pass today.
- Parallel-safety: story 2 and story 4 both edit `AGENTS.md`, and story 3 edits `CHANGELOG.md` (also edited by stories 1 and 4, which are ordered). The epic acknowledges the story 2 and 4 overlap.

```findings
- file: .engineering/planning/story/engineering-protocol-lock.md
  line: 68
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance never shows ProtocolCatalog::engineering() builds from the vendored files or runs the lock check at load: every listed bullet also passes while it still reads canon_engineering::registry, and the refusal tests call the check function on in-memory input, not the load"
- file: .engineering/planning/story/engineering-protocols-vendor-command.md
  line: 22
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the lock's repository field has no stated source (the command takes only --checkout and --tag) and the temporary-repository test never asserts it, so the by-hand check that git status --porcelain protocols/engineering is empty depends on an undefined value"
- file: .engineering/planning/story/engineering-registry-calls-replaced.md
  line: 26
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says a catalog that fails its lock check is OpenError::InvalidProtocol naming the protocol and the refusal, but no acceptance line or red step observes it and the embedded files are valid, so no listed test reaches that branch"
- file: .engineering/planning/epic/engineering-protocols-as-vendored-data.md
  line: 105
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "each engineering entry with the sha256 it had on the crate cannot be checked when the epic closes, because the only comparison against the crate (the_vendored_files_equal_the_pinned_registry) is deleted by story:engineering-registry-calls-replaced; the epic names no standing check of those digests"
- file: .engineering/planning/epic/engineering-protocols-as-vendored-data.md
  line: 101
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises one loom-xtask command that moves the vendored files with no network call, but no Acceptance bullet names the command or story:engineering-protocols-vendor-command, so the epic could close with that story unfinished"
```
