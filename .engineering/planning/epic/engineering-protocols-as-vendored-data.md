---
format: aep.planning-md/3
id: epic:engineering-protocols-as-vendored-data
kind: epic
status: proposed
title: Loom loads the engineering protocol catalog from vendored files held to a sha256 lock and links no engineering-protocols crate
summary: Ten protocol files from engineering-protocols 0.16.0 under protocols/engineering/ with a lock; the four b10x-canon-engineering lines go, so a Canon move waits on no other release
tags:
- sdc
relations:
- serves: vision:O1
- serves: vision:governed-autonomy
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:18:30Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"review_outcome":6}}}
---
## Outcome

Loom loads the engineering protocol catalog from vendored files: the ten `protocol/1` documents of
engineering-protocols, copied byte for byte under `protocols/engineering/` at the repository root
(beside Loom's own `protocols/system-query/1.yaml`), each held to the sha256 a lock file beside
them names. `ProtocolCatalog::engineering()`, the governor's fallback for a protocol it was not
given, the router and every test read that catalog, and no Loom manifest names
`b10x-canon-engineering`. Moving Canon is then a Loom change alone: it waits on no
engineering-protocols release. Canon stays a compile-time dependency of Loom, pinned by the
`b10x-canon` lines of `crates/loom-governor`, `crates/loom-protocols` and `crates/loom-bindings`. A
maintainer moves the vendored files with one `loom-xtask` command from a local checkout of
engineering-protocols at a tag, with no network call.

## Why

Four Loom crates depend on `b10x-canon-engineering` at tag `0.15.2`
(`crates/loom-protocols/Cargo.toml:15`, `crates/loom-governor/Cargo.toml:19`,
`crates/loom-intake-slice/Cargo.toml:18`, `crates/loom-intake-router/Cargo.toml:15`), and
`AGENTS.md` § Governor (committed `:520-523`) names Canon "by the reference
`b10x-canon-engineering` uses", so every Canon move waits on an engineering-protocols release
built on that Canon. engineering-protocols `0.16.0` (Canon `0.9.0`) came 22 days after Canon
`0.9.0` (`~/beyond10x/sdc/docs/engineering-protocols-boundary.md:88`, tag dates; not re-read
here). The dependency also brings `b10x-canon-engineering-assertions`, `b10x-assertion-providers`
and `b10x-canon-expr` into `Cargo.lock` (`Cargo.lock:188`, `:214`, `:226`, `:242`),
process-spawning and network code that Loom never calls (no Loom source names
`assertion_providers` or `canon_engineering_assertions`; `grep`, 2026-11-21).

What Loom uses of the crate, read at `b38eb7d1`:

- Production: `ProtocolCatalog::engineering()` lists and reads the registry
  (`crates/loom-protocols/src/lib.rs:160,162`) and stamps revision `"0.15.2"` on each entry
  (`:169`); the governor's `builtin()` (`crates/loom-governor/src/lib.rs:1724-1742`, used at `:913`
  and through `protocol_ir` at `:1701`) reads it for a protocol no catalog admitted; and
  `RouterError::Registry(canon_engineering::registry::Error)`
  (`crates/loom-intake-router/src/classify.rs:32`) carries its error type, though nothing
  constructs that variant (only `:56`, `:81` and `crates/loom-plugin/src/turn.rs:282` name it).
  `loom-intake-slice` has no production use: its line serves only
  `tests/adversary_case.rs:244,248`.
- Tests, 12 calls: `crates/loom-governor/tests/governed_case.rs:613,621`,
  `adversary_governor.rs:1044`, `adversary2_governor.rs:643`;
  `crates/loom-intake-slice/tests/adversary_case.rs:244,248`;
  `crates/loom-intake-router/tests/adversary_classify.rs:116,476,477`, `classify.rs:91,145,146`.
- Three tests hold one copy: `crates/loom-protocols/tests/adversary_w8_pin.rs:103-118` (one
  `b10x-canon-engineering` and one `b10x-canon`, each from the tag every manifest names),
  `crates/loom-executor/tests/intake_import.rs:26-32` (`SINGLE_COPY`, read by
  `lock_resolves_one_path_copy_of_each_absorbed_crate`, `:166`) and
  `crates/loom-xtask/tests/pinned_consumer.rs:107-153` (no duplicate of either in
  `consumers/pinned`, filter at `:145`).

Loom already holds a protocol by its bytes and their sha256: `ProtocolCatalog::add_yaml`
(`crates/loom-protocols/src/lib.rs:183`) parses, validates and compiles a document with Canon and
records `sha256` of its bytes (`:215`) without checking it against anything; `InstallStore` checks
a recorded digest when it reads its manifest (`crates/loom-protocols/src/store.rs:230`), installed
from a file (`:62`) or a pinned commit (`:87`). The governor admits a catalog by those digests
(`CanonGovernor::with_catalog` and `validate_catalog`, `crates/loom-governor/src/lib.rs:771,809`).
The ten documents are byte-equal at tags `0.15.2` and `0.16.0` (sha256 of each,
`git show <tag>:protocols/<name>/<major>.yaml`), so vendoring them at `0.16.0` changes no digest
the governor admits.

The files sit outside the crate directory as Loom's bundled protocols already do
(`crates/loom-protocols/src/lib.rs:16,18,21`) and as engineering-protocols' own
`crates/canon-engineering/build.rs:1-5` (tag `0.16.0`) reads `protocols/`, which Loom builds today
as a Git dependency. `task consumer-check` builds them from `consumers/pinned/`, a crate outside
the workspace that reaches `b10x-loom-protocols` by path through `b10x-loom-sdk`.

## Stories

In dependency order:

1. `story:engineering-protocol-lock`: the vendored files, the lock, its one encoder and decoder,
   and the load-time check through `ProtocolCatalog::from_vendored` (depends on
   `story:canon-0-9-0-pin`, see below).
2. `story:engineering-protocols-vendor-command`: `loom-xtask protocols-vendor`, writing the lock
   through story 1's `lock` module.
3. `story:engineering-registry-calls-replaced`: the governor, the router and the 12 test sites read
   the vendored catalog; `RouterError::Registry` goes.
4. `story:engineering-protocols-dependency-removed`: the four lines go, `Cargo.lock`, the three
   one-copy tests and `AGENTS.md` follow.

Stories 2 and 3 both wait only on story 1 and share no file, so they can run in one wave. Story 4
waits on story 3 (no source may name the crate when its line goes) and on story 2: both edit
`Cargo.lock` (story 2 adds `b10x-loom-protocols` to `loom-xtask`'s dependency list) and
`AGENTS.md` (story 2 § Protocol composition and system queries; story 4 § Boundary, § Governor and
§ Connectors, below story 2's insertion, so story 4 cites `AGENTS.md` by heading).

## Order against story:canon-0-9-0-pin

This epic follows `story:canon-0-9-0-pin`. That story is unblocked
(`upstream-blocker:engineering-protocols-on-canon-0-9-0` is `cleared`), moves Canon with
engineering-protocols `0.16.0` in one step as `AGENTS.md` § Governor prescribes, and is on the path
of `epic:protocol-tree-on-bindings` (`story:child-case-runtime` depends on it). Both change the
four manifests, `Cargo.lock`, `AGENTS.md` § Governor, `crates/loom-protocols/src/lib.rs` (the entry
revision), `crates/loom-protocols/tests/adversary_w8_pin.rs`, the pin table of
`website/docs/reference/sdk-stability.md` and `website/docs/guides/move-to-loom.md`, so they
cannot run at once. Going first would hold the SDC's critical path behind four stories and leave
`story:canon-0-9-0-pin` with acceptance lines that name a dependency gone. Going second, story 1
vendors exactly the tag the crate is then pinned at and can prove the vendored files equal the
crate's registry before story 3 drops the crate. The first story depends on it for that reason.

## Acceptance

- `git grep -n 'b10x-canon-engineering' -- '*.toml' Cargo.lock` prints nothing
  (`story:engineering-protocols-dependency-removed`).
- `the_engineering_digests_are_the_released_ones` (`crates/loom-protocols/tests/engineering_lock.rs`)
  passes: the engineering registrations are exactly ten, each with the literal sha256 its file had
  on the crate at `0.15.2` (`story:engineering-protocol-lock`; the test stays after
  `story:engineering-registry-calls-replaced`).
- `ProtocolCatalog::bundled()` lists the same eleven registrations as before
  (`crates/loom-protocols/tests/catalog.rs` and
  `the_catalog_lists_what_the_changelog_says_callers_now_see` in
  `crates/loom-protocols/tests/adversary_w8_pin.rs` pass).
- A changed byte, an unlocked file, a missing locked file and a malformed lock are each refused by
  `ProtocolCatalog::from_vendored`, the function `ProtocolCatalog::engineering()` is, naming the
  file (`a_changed_byte_is_refused_at_load`, `an_unlocked_file_is_refused_at_load`,
  `a_missing_locked_file_is_refused_at_load`, `a_malformed_lock_is_refused_at_load` in
  `engineering_lock.rs`; `story:engineering-protocol-lock`).
- `loom-xtask protocols-vendor` rewrites the vendored files and the lock from a local checkout at a
  tag with no network call, and its output loads through `from_vendored`
  (`crates/loom-xtask/tests/protocols_vendor.rs` passes;
  `story:engineering-protocols-vendor-command`).
- `AGENTS.md` names no engineering-protocols crate, package or tag, and § Governor moves Canon
  with `cargo update -p b10x-canon` (`git grep -n -e 'b10x-canon-engineering' -e
  'b10x-assertion-providers' -e '0\.15\.2' -e '0\.16\.0' -- AGENTS.md` prints nothing;
  `story:engineering-protocols-dependency-removed`).
- `task check` exits 0.

## Not in scope

- Adopting the catalog manifest the engineering-protocols session is to publish (path, id,
  revision, sha256, Canon version per file). Its format does not exist; Loom's lock is its own
  until it does, and a story to read it is written when a tag carries it.
- A "requires Canon" field per file: `add_yaml` compiles each file with the Canon Loom links when
  the catalog loads, so a file that needs a newer Canon fails `task check`.
- `protocols/vocabulary.yaml`, the vocabulary check and the docs generator: Loom reads none of them.
- Moving observation collection (`assertion-providers`) behind Loom executors through Substrate:
  a design no one owns yet (`e5-observation-collection-note.md` is the reading).
- Any change in engineering-protocols, takt or Atlas (ADR 0094 decision 2).
- Changes to `story:canon-0-9-0-pin`.

## Source

Session `sdc`, 2026-11-21, one request relaying an operator decision ("option A") that session
`loom` has not heard from the operator itself. Evidence read:
`~/beyond10x/sdc/docs/engineering-protocols-boundary.md`; engineering-protocols at tag `0.16.0`
(`70e6eff48963c9ea178d515ae33e645cf95c9773`). Every `file:line` above was opened in the tree at
`b38eb7d1`; no file outside `.engineering/` changed up to `39f4da88` (`git diff --stat`). Revised
after plan-critic round 1 (`e5-round-1-*.md`); `AGENTS.md` is cited by section heading plus its
line at `39f4da88`.
