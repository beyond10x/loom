---
format: aep.planning-md/3
id: story:engineering-protocol-lock
kind: story
status: proposed
title: ProtocolCatalog::engineering reads ten vendored files from engineering-protocols 0.16.0, each checked against a sha256 lock at load
summary: intake.protocols.ProtocolLock with one encoder and decoder (loom_protocols::lock); ProtocolCatalog::from_vendored refuses a digest mismatch, an unlocked file, a missing locked file and a malformed lock; ten literal sha256 digests
tags:
- sdc
relations:
- decomposes: epic:engineering-protocols-as-vendored-data
- serves: vision:O1
- depends_on: story:canon-0-9-0-pin
scope:
- confidence: cited
  path: crates/loom-protocols/build.rs
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: cited
  path: crates/loom-protocols/src/lock.rs
- confidence: cited
  path: crates/loom-protocols/tests/adversary_w8_pin.rs
- confidence: inferred
  path: crates/loom-protocols/tests/catalog.rs
- confidence: cited
  path: crates/loom-protocols/tests/engineering_lock.rs
- confidence: cited
  path: ess/intake/domains/protocols.yaml
- confidence: cited
  path: protocols/engineering
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:18:30Z", actor: "human:timo", revision: 9}
---
## Outcome

`protocols/engineering/` holds the ten `protocol/1` documents of engineering-protocols at tag
`0.16.0` (commit `70e6eff48963c9ea178d515ae33e645cf95c9773`), byte for byte, at
`<name>/<major>.yaml`, and `protocols/engineering/lock.json`, a `loom.protocol-lock/1` document
naming the source repository, tag and commit and, per file, its path and sha256. The directory
sits at the repository root beside `protocols/system-query/1.yaml`, which
`crates/loom-protocols/src/lib.rs:16` already embeds from outside the crate directory
(`include_str!("../../../protocols/system-query/1.yaml")`).

One function builds the engineering catalog, and nothing else does:
`ProtocolCatalog::from_vendored(files: &[(&str, &[u8])], lock: &[u8]) -> Result<Self, VendoredError>`.
`ProtocolCatalog::engineering()` is `from_vendored(ENGINEERING_FILES, ENGINEERING_LOCK)` with the
error turned into its printed text (below). `ENGINEERING_FILES` (every file below
`protocols/engineering/` other than `lock.json`, as `(path relative to protocols/engineering/,
bytes)` sorted by path) and `ENGINEERING_LOCK` (the bytes of `lock.json`) are public constants, as
`SYSTEM_QUERY_YAML` is; `crates/loom-protocols/build.rs` writes them to `$OUT_DIR` with
`include_bytes!` and `cargo:rerun-if-changed` on the directory, so a file the lock omits reaches the
check rather than being skipped. `from_vendored` decodes the lock, checks the file set against it,
and only then adds each file with `add_yaml`, taking every entry's revision from the lock's `tag`
(no tag is written in `src/`). It refuses, building no catalog, with `VendoredError::Lock` carrying
the `LockRefusal` and a path:

- a file whose sha256 differs from its lock entry (`DigestMismatch`): the file's path, with the
  locked and the found digest;
- a file the lock does not name (`Unlocked`): the file's path;
- a lock entry with no file (`Missing`): the entry's path;
- a malformed lock (`Malformed`): the path `lock.json` when the bytes do not decode as JSON or a
  top-level member is wrong (another `format`, a `repository` other than `lock::REPOSITORY`, an
  undeclared or missing member); the entry's `path` value when one entry is wrong (a path that is
  not `<name>/<major>.yaml`, a path given twice, an undeclared or missing member of an entry that
  has a `path`), else `lock.json`.

A file that passes the lock but is not UTF-8 or that `add_yaml` refuses is
`VendoredError::Definition` with the file's path and the problem.

`VendoredError` prints one line, which `engineering()` returns as its `String` and
`story:engineering-registry-calls-replaced` reads (generated enums derive no `Display`, as
`SourceKind` at `generated/rust/intake/src/protocols.rs:39` shows, so `lock.rs` spells the variant
itself):

- `VendoredError::Lock`: `engineering protocol lock refused: <Variant> <path>`, where `<Variant>` is
  the `LockRefusal` variant as Rust spells it (`DigestMismatch`, `Unlocked`, `Missing`,
  `Malformed`); `DigestMismatch` appends ` (locked <digest>, found <digest>)` and `Malformed`
  appends `: <what is wrong>`.
- `VendoredError::Definition`: `engineering protocol <path>: <problem>`.

The lock has one encoder and one decoder, both in the public module `loom_protocols::lock`:
`REPOSITORY` (the constant `https://github.com/beyond10x/engineering-protocols`), `FORMAT`
(`loom.protocol-lock/1`), `encode(&ProtocolLock) -> Vec<u8>`, `decode(&[u8]) -> Result<ProtocolLock,
LockError>`, `locked_file(path, bytes) -> LockedFile` (computes the sha256) and `check`, with
`ProtocolLock`, `LockedFile` and `LockRefusal` re-exported from the generated intake crate. The
vendor command (`story:engineering-protocols-vendor-command`) writes the lock through this module,
so it needs no encoder of its own. `encode` writes these bytes exactly, never through a map whose
key order depends on a `serde_json` feature:

- one JSON object, members in the order `format`, `repository`, `tag`, `commit`, `files`;
- `files` an array of objects with members `path`, `sha256`, sorted by `path` as bytes;
- two-space indentation, one member per line, `": "` after each key, as `serde_json::to_vec_pretty`
  indents (the form `crates/loom-protocols/src/store.rs:186-188` writes for the installed catalog);
- one trailing newline.

The committed `lock.json` is the fixture both sides test against.

Each entry keeps the source it has today, with the lock's tag as revision: kind `Engineering`,
location `engineering-protocols`, revision `0.16.0`, path `protocols/<name>/<major>.yaml`. The
sha256 of each file, the same at `0.15.2` and `0.16.0` (`git show <tag>:protocols/<path> |
sha256sum` in `~/beyond10x/engineering-protocols`, 2026-11-21):

- `eval-gated-change/1.yaml` `631d759afb0a24401cca6abf7f970170052fab1bd2d29db6a09f65f935afb520`
- `incident-response/1.yaml` `10a6f9120fe3570c54498e6be054622c2509a9f4d8ab1876aa72fa50b4df1943`
- `loop-improvement/1.yaml` `04ba56c8fe24cc29f9deaa209697ec6a33f1c34c4e8fcaa8c4d0d3bbd198389f`
- `ml-experiment/1.yaml` `7b0ac3b9f9d079fb729a84987a31902867c5869e137957e4af9d3be4c95f98da`
- `protocol-adopt/1.yaml` `69d1304b7d6ac89f74f7d16cb9bc2d38197b9871182b5ba63977ac1dc4a28929`
- `self-improvement/1.yaml` `7947a8ebb461b56b608c1d15821b126ed353d12a20eaab8a30ebc2f5f71d9033`
- `software-change/1.yaml` `8367fdafba358845ce842c87298ecfb80970f4b72ba3466ec8402bc6272d2046`
- `story-delivery/1.yaml` `f24e9c1f98c0e4c6ba4a88ad82696c68ac403655378ebc5af51c3c8a775f8e39`
- `story-delivery/2.yaml` `37e129039e390b67c522ed35b95b19db534bcc4f76d579af3db1f8bce933ba13`
- `support-triage/1.yaml` `d970d928e6299aa400566b0c4e2fb928479c42f7c96a82e5b78feab41277c67a`

The `b10x-canon-engineering` dependency stays in this story; only the tests use it.

`crates/loom-protocols/tests/adversary_w8_pin.rs` has one owner per change. This story adds an
assertion that is new: `every_engineering_entry_carries_the_tag_the_manifest_pins` (`:78-100`)
also compares each entry's revision with the tag `lock::decode(ENGINEERING_LOCK)` gives. It keeps
the existing comparison with `one_tag("b10x-canon-engineering")` (`:80`, `:91`), which
`story:engineering-protocols-dependency-removed` removes.

## Why

Step one of `epic:engineering-protocols-as-vendored-data`: the catalog must come from files before
the crate can go. Today `engineering()` lists and reads the crate's registry
(`crates/loom-protocols/src/lib.rs:160,162`) and stamps a hand-written revision (`:169`).
`add_yaml` (`:183`) already parses, validates and compiles a document and records the sha256 of
its bytes (`:215`), but checks it against nothing; only `InstallStore` compares a recorded digest
(`crates/loom-protocols/src/store.rs:230`). The ten documents are byte-equal at `0.15.2` and
`0.16.0`, so every entry's `definition.sha256`, which the governor admits a catalog by
(`crates/loom-governor/src/lib.rs:771,809`), stays the same.

Because the files are byte-equal to the registry's, a test that compares bytes or digests passes
on today's code as well. What tells the vendored load from the registry is that the refusals are
observed through `from_vendored`, the function `engineering()` is, and that `src/` names no
`canon_engineering`. Likewise a revision written into `src/` would equal the lock's tag today, so
only a lock with another tag shows the revision comes from the lock.

The files live outside the crate directory as Loom's own bundled protocols do (`lib.rs:16,18,21`),
and as engineering-protocols' own `crates/canon-engineering/build.rs:1-5` (tag `0.16.0`) reads
`protocols/` from outside its crate, which Loom builds today as a Git dependency: a Git consumer
checks out the whole repository. `consumers/pinned/` depends on `b10x-loom-sdk` by path, which
depends on `b10x-loom-protocols` by path (`crates/loom-sdk/Cargo.toml:14`), so `task
consumer-check` runs this build script from a crate outside the workspace.

## Waits on

`story:canon-0-9-0-pin`: it moves the four manifests to engineering-protocols `0.16.0`, the tag the
files are taken from, and both stories change `crates/loom-protocols/src/lib.rs` and
`crates/loom-protocols/tests/adversary_w8_pin.rs`.

## ESS first

`ess/intake/domains/protocols.yaml` (domain `intake.protocols`, which already declares
`ProtocolSource` and `ProtocolDefinition`): `ProtocolLock` (`format`, `repository`, `tag`,
`commit`, `files`), `LockedFile` (`path`, `sha256`) and the enum `LockRefusal` (`Malformed`,
`DigestMismatch`, `Unlocked`, `Missing`); the domain summary says engineering definitions are
vendored files held to a lock. On that commit `task intake-drift` fails (the generated
`generated/rust/intake/src/protocols.rs` lacks the types), and
`crates/loom-protocols/tests/engineering_lock.rs` does not compile.

## Acceptance

- `git ls-files protocols/engineering` lists exactly ten `<name>/<major>.yaml` files and
  `lock.json`.
- `git grep -n 'canon_engineering' -- crates/loom-protocols/src` prints nothing (today it prints
  `lib.rs:160` and `:162`).
- `engineering_is_the_vendored_load` (`engineering_lock.rs`): `ProtocolCatalog::engineering()`
  holds exactly the entries `ProtocolCatalog::from_vendored(ENGINEERING_FILES, ENGINEERING_LOCK)`
  builds, and each entry's `definition.yaml` bytes are the `ENGINEERING_FILES` bytes at its path.
- `the_engineering_digests_are_the_released_ones` (`engineering_lock.rs`) holds the ten digests of
  § Outcome as string literals and asserts that the engineering registrations are exactly those
  ten and that each entry's `definition.sha256` equals its literal.
- `the_revision_is_the_locks_tag` (`engineering_lock.rs`): `from_vendored` over
  `ENGINEERING_FILES` and `lock::encode` of the committed lock with its `tag` set to `9.9.9` builds
  ten entries, each with revision `9.9.9`; it fails when `src/` writes the revision itself.
- `a_changed_byte_is_refused_at_load` (`engineering_lock.rs`): `from_vendored` with one byte of
  `software-change/1.yaml` changed in a copy of `ENGINEERING_FILES` returns
  `VendoredError::Lock` with `DigestMismatch`, that path, the locked digest and the found one.
- `an_unlocked_file_is_refused_at_load` (`engineering_lock.rs`): `from_vendored` with
  `ENGINEERING_FILES` plus `extra/1.yaml` returns `VendoredError::Lock` with `Unlocked` and
  `extra/1.yaml`.
- `a_missing_locked_file_is_refused_at_load` (`engineering_lock.rs`): `from_vendored` with
  `ENGINEERING_FILES` less `support-triage/1.yaml` returns `VendoredError::Lock` with `Missing` and
  `support-triage/1.yaml`.
- `a_malformed_lock_is_refused_at_load` (`engineering_lock.rs`): `from_vendored` with
  `ENGINEERING_FILES` and each malformed lock returns `VendoredError::Lock` with `Malformed` and the
  path § Outcome gives it: `lock.json` for bytes that do not decode, another `format`, another
  `repository`, an undeclared top-level member and a missing top-level member;
  `software-change.yaml` for that path as an entry; `software-change/1.yaml` for that path given
  twice.
- `the_refusal_text_names_the_variant_and_the_path` (`engineering_lock.rs`): the printed
  `VendoredError` of the changed-byte, unlocked, missing and undecodable-lock cases above equals
  `engineering protocol lock refused: DigestMismatch software-change/1.yaml (locked <a>, found
  <b>)`, `… Unlocked extra/1.yaml`, `… Missing support-triage/1.yaml` and `… Malformed lock.json: …`
  respectively (the first three exactly, the last up to its `: `), and `engineering()`'s error type
  is that text.
- `the_committed_lock_names_the_release` (`engineering_lock.rs`): `lock::decode(ENGINEERING_LOCK)`
  gives `repository` equal to `lock::REPOSITORY`, which is
  `https://github.com/beyond10x/engineering-protocols`, tag `0.16.0` and commit
  `70e6eff48963c9ea178d515ae33e645cf95c9773`.
- `the_committed_lock_is_the_encoders_bytes` (`engineering_lock.rs`):
  `lock::encode(&lock::decode(ENGINEERING_LOCK)?)` equals `ENGINEERING_LOCK` byte for byte.
- `the_vendored_files_equal_the_pinned_registry` (`engineering_lock.rs`): the registrations
  `canon_engineering::registry::list()` gives are the catalog's engineering registrations, and each
  file's bytes equal `registry::get(name, major).yaml`.
- `every_engineering_entry_matches_its_lock_entry` (`engineering_lock.rs`): each engineering
  entry's `definition.sha256` equals the `sha256` of its lock entry.
- `crates/loom-protocols/tests/catalog.rs` passes unchanged, and each engineering entry's source is
  kind `Engineering`, location `engineering-protocols`, revision `0.16.0`, path
  `protocols/<name>/<major>.yaml`.
- `every_engineering_entry_carries_the_tag_the_manifest_pins` (`adversary_w8_pin.rs:78-100`) gains
  a new assertion, that each entry's revision equals the tag of `lock::decode(ENGINEERING_LOCK)`,
  beside its existing comparison with the manifest tag, and passes.
- `git grep -n -e '"0\.15\.2"' -e '"0\.16\.0"' -- crates/loom-protocols/src` prints nothing.
- `b10x-loom protocols list` prints each engineering entry as before
  (`crates/loom-cli/tests/catalog_route.rs` passes unchanged).
- `task consumer-check` exits 0: `consumers/pinned/` builds `b10x-loom-protocols`' build script
  from outside the workspace.
- `CHANGELOG.md` `### Changed`: `ProtocolCatalog::engineering` reads vendored files held to
  `protocols/engineering/lock.json`; `### Added`: `loom_sdk::protocols` gains
  `ProtocolCatalog::from_vendored`, `VendoredError`, `ENGINEERING_FILES`, `ENGINEERING_LOCK` and the
  `lock` module.
- `task check` exits 0.

## Files

`ess/intake/domains/protocols.yaml`, `generated/rust/intake/src/protocols.rs` (regenerated),
`protocols/engineering/**` (new), `crates/loom-protocols/build.rs` (new),
`crates/loom-protocols/src/lib.rs`, `crates/loom-protocols/src/lock.rs` (new),
`crates/loom-protocols/tests/engineering_lock.rs` (new),
`crates/loom-protocols/tests/adversary_w8_pin.rs`, `CHANGELOG.md`.

## Out of scope

The command that writes the files (`story:engineering-protocols-vendor-command`; this story copies
them once by hand from the tag and writes the lock through `lock::encode`). Replacing the
governor's and the tests' registry calls (`story:engineering-registry-calls-replaced`) and removing
the dependency (`story:engineering-protocols-dependency-removed`), including the manifest-tag
comparison in `adversary_w8_pin.rs`. `protocols/vocabulary.yaml`. The upstream catalog manifest.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-protocols` (`ProtocolCatalog::engineering`, a new `lock` module, a new `build.rs`) — cited, § Outcome and § Files
- **Files (code):** `crates/loom-protocols/src/lib.rs:157-175` (`engineering()`: `:160,162` call the registry, `:169` stamps `"0.15.2"`) — cited, verified in tree
- **Files (code):** `crates/loom-protocols/src/lock.rs` (new) — cited
- **Files (code):** `crates/loom-protocols/build.rs` (new; none exists under `crates/*/build.rs` today) — cited
- **Files (tests):** `crates/loom-protocols/tests/engineering_lock.rs` (new) — cited
- **Files (tests):** `crates/loom-protocols/tests/adversary_w8_pin.rs:79-98` (`every_engineering_entry_carries_the_tag_the_manifest_pins`) — cited
- **Files (ESS):** `ess/intake/domains/protocols.yaml` (adds `ProtocolLock`, `LockedFile`, `LockRefusal`; `List<…>` syntax is in use in sibling domains, e.g. `bindings.yaml:54`) — cited
- **Files (data):** `protocols/engineering/**` (ten `<name>/<major>.yaml` + `lock.json`), new, beside `protocols/{system-query,inbound-answer,operator-reply}` — cited
- **Generated (regenerated, house rule):** `generated/rust/intake/src/protocols.rs` — cited
- **Symbols:** `ProtocolCatalog::engineering`, `ProtocolCatalog::add_yaml` (`lib.rs:183`; sha256 at `:215`), `ProtocolCatalog::from_vendored`, `VendoredError`, `ENGINEERING_FILES`, `ENGINEERING_LOCK`, `loom_protocols::lock::{REPOSITORY, FORMAT, encode, decode, locked_file, check}` — cited
- **Also likely:** `crates/loom-protocols/tests/catalog.rs` is asserted unchanged; it calls `engineering()` and `bundled()` (`catalog.rs:25-30`), so it is exercised, not edited — inferred
- **Also likely:** `crates/loom-protocols/Cargo.toml` is not edited by this story. `sha2` and `serde_json` are already dependencies, `build.rs` is auto-detected, and `b10x-canon-engineering` stays (§ Outcome last line) — cited (Cargo.toml read)
- **Documents:** none beyond `CHANGELOG.md`. `loom_sdk::protocols` is `pub use loom_protocols as protocols` (`crates/loom-sdk/src/lib.rs:72`), so the new items surface with no sdk edit — cited
- **Confidence:** high — every file, line and symbol in § Files and § Why resolves in the tree, and the ten digests and the commit were re-derived from `~/beyond10x/engineering-protocols` tag `0.16.0`
- **Would collide with `story:canon-0-9-0-pin`** on `crates/loom-protocols/src/lib.rs` and `tests/adversary_w8_pin.rs` (its scope lists both) — ordered: direct `depends_on`
- **Would collide with `story:engineering-registry-calls-replaced`** on `crates/loom-protocols/tests/engineering_lock.rs` (new here, edited there; its § Files) — ordered: direct `depends_on` this story
- **Would collide with `story:engineering-protocols-dependency-removed`** on `crates/loom-protocols/tests/adversary_w8_pin.rs` (its § Files) — ordered: `dependency-removed -> registry-calls-replaced -> engineering-protocol-lock`, transitive
- **Would collide with `story:engineering-protocols-vendor-command`** on `crates/loom-protocols/src/lock.rs` (it calls `lock::encode`, edits no file here) and on the content of `protocols/engineering/**` (it rewrites them) — ordered: direct `depends_on`
- **Would collide with `story:protocol-argument-schema`** (draft, `decomposes epic:fast-selector`) on `crates/loom-protocols/src/lib.rs` (its body, line 163) and `tests/catalog.rs` — **not ordered**: it has no `depends_on` edge
- **Would collide with `story:dw-workflow-into-protocol`** (draft, depends on `dw-bugfix-workflow` and `dw-incident-workflow`) on `crates/loom-protocols/src/lib.rs` (its body, line 55) — **not ordered**: no `depends_on` path to this story
- **Would collide with every story that edits `CHANGELOG.md`** — house rule, ordered only where the stories above are
- **Safety fact:** the ten files are byte-equal to the `0.15.2` registry the crate builds from today, so every `definition.sha256` the governor admits a catalog by (`crates/loom-governor/src/lib.rs:771,809`) is unchanged — re-ran `git show 0.15.2:<p> | sha256sum` and `git show 0.16.0:<p> | sha256sum` for all ten, all equal and equal to the story's literals; level 4 for the digests, unproven for the governor path
- **Safety fact:** container images copy the whole repository (`deploy/images/loom-agent/Dockerfile:4` and `loom-operator/Dockerfile:4`, `COPY . .`; `.dockerignore` does not exclude `protocols/`), so a `build.rs` reading `../../protocols/engineering` finds it there — level 2, unproven
- **Safety fact:** `crates/loom-protocols` is `publish = false` (`Cargo.toml:7`), so reading files outside the crate directory breaks no `cargo package` — level 2, unproven
