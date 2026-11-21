---
format: aep.planning-md/3
id: story:engineering-registry-calls-replaced
kind: story
status: proposed
title: The governor fallback, the router and every test read the vendored catalog, and no Rust source names canon_engineering
summary: builtin() over ProtocolCatalog::engineering, a lock refusal is InvalidProtocol (unit test over a tampered set); 12 test calls; RouterError::Registry removed (Breaking)
tags:
- sdc
relations:
- decomposes: epic:engineering-protocols-as-vendored-data
- serves: vision:O1
- depends_on: story:engineering-protocol-lock
scope:
- confidence: cited
  path: crates/loom-governor/src/file_store.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary2_governor.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_governor.rs
- confidence: cited
  path: crates/loom-governor/tests/governed_case.rs
- confidence: cited
  path: crates/loom-intake-router/src/classify.rs
- confidence: cited
  path: crates/loom-intake-router/tests/adversary_classify.rs
- confidence: cited
  path: crates/loom-intake-router/tests/classify.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/adversary_case.rs
- confidence: cited
  path: crates/loom-plugin/src/turn.rs
- confidence: cited
  path: crates/loom-protocols/tests/engineering_lock.rs
- confidence: cited
  path: crates/loom-xtask/tests/release_check.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:18:31Z", actor: "human:timo", revision: 8}
---
## Outcome

Nothing in Loom's Rust source calls the engineering registry: the governor, the router and every
test read engineering protocols from the vendored catalog, `ProtocolCatalog::engineering()`.

- The governor's fallback for a protocol it was not given (`builtin()`,
  `crates/loom-governor/src/lib.rs:1724-1742`, used by `outcome_order` at `:913` and `protocol_ir`
  at `:1701`) returns the catalog's `ProtocolEntry` for that registration. It is
  `builtin_in(ProtocolCatalog::engineering(), protocol)`, where the private
  `builtin_in(catalog: Result<ProtocolCatalog, String>, protocol: &str) -> Result<ProtocolEntry,
  OpenError>` holds the mapping: a registration that is not `name@major`, or one the catalog does
  not hold, is `OpenError::UnknownProtocol` as today; a catalog that cannot be built, a lock
  refusal included, is `OpenError::InvalidProtocol` with that protocol and, as `problem`, the
  catalog's error text unchanged, the printed `VendoredError` that
  `story:engineering-protocol-lock` states (`engineering protocol lock refused: <Variant> <path>…`).
- The governor's doc comments describe the vendored catalog (`crates/loom-governor/src/lib.rs`,
  read at `39f4da88`; `story:canon-0-9-0-pin` changes only the tags in `:129-133` before this
  story). This story owns `:129-133` whole, including the sentence that `b10x-canon-engineering` is
  pinned to an engineering-protocols tag; `story:engineering-protocols-dependency-removed` does not
  touch the file. Afterwards:
  - `:50-51` (# Evaluation): the protocol is taken from the vendored engineering catalog
    (`ProtocolCatalog::engineering`) or registered by `CanonGovernor::with_protocol`;
  - `:128-133` (# Dependencies): Canon is `b10x-canon` at the reference the crate's `Cargo.toml`
    names, pinned by `Cargo.lock`, and compiles the catalog's engineering protocols; Commission is
    the path dependency `b10x-loom-commission`; engineering protocols are the files
    `b10x-loom-protocols` vendors under `protocols/engineering/`, held to their lock. No tag and
    no engineering-protocols crate is named;
  - `:1699` (`protocol_ir`): the vendored engineering `protocol`, `<name>@<major>`, compiled by
    Canon;
  - `:1723` (`builtin`): the vendored engineering `protocol`, `<name>@<major>`, as
    `ProtocolCatalog::engineering` holds it;
  - `:1617` (`checked_builtin_inputs`), `:1627` (`checked_builtin_independence`) and
    `crates/loom-governor/src/file_store.rs:15`: "the vendored engineering `protocol`" (or
    "protocol") where they say "the ELS built-in".
- `RouterError::Registry` is removed (`crates/loom-intake-router/src/classify.rs:30-32`, its
  `Display` arm `:56` and `source` arm `:81`), and with it the arm in
  `crates/loom-plugin/src/turn.rs:282`. Nothing constructs it today; a catalog that cannot be built
  is `RouterError::Catalog` already (`classify.rs:108`).
- The 12 test calls read the catalog's entries (`entry.name()`, `entry.definition.yaml`,
  `entry.model`) instead of `registry::list()` and `registry::get()`.
- `crates/loom-protocols/tests/engineering_lock.rs`, which `story:engineering-protocol-lock`
  creates (it does not exist at `39f4da88`), loses `the_vendored_files_equal_the_pinned_registry`,
  the last test that reads the registry.
- `crates/loom-xtask/tests/release_check.rs` gains
  `release_pair_with_page(name, page, first, second_version, second, tag_second)`, which is
  `release_pair` (`:379-422`) with the stability page as an argument. `release_pair` keeps its
  signature and becomes `release_pair_with_page(name, STABILITY_PAGE, …)`, so its callers (7 calls
  at `39f4da88`) and the constant `STABILITY_PAGE` (`:372-373`) do not change. The new test passes
  a page that lists `loom_sdk::intake::router`.

The four dependency lines stay; `story:engineering-protocols-dependency-removed` deletes them.

## Why

Step three of `epic:engineering-protocols-as-vendored-data`. The dependency cannot go while
anything names `canon_engineering`. `RouterError::Registry` is the only public item that carries an
engineering-protocols type (`canon_engineering::registry::Error`), and it reaches consumers as
`loom_sdk::intake::router::RouterError` (`crates/loom-sdk/src/lib.rs:77`; the module is listed on
`website/docs/reference/sdk-stability.md:146`). Removing a variant inside a listed module is no
removal of the listed path (`covers`, `crates/loom-xtask/src/release.rs:442`; its test at `:734`),
so `release-check` takes a `**Breaking:**` bullet without a prior deprecation. The repository case
of `release_check.rs` runs `release-check` at the released workspace version
(`the_repository_passes_release_check_at_its_workspace_version`, `:656-667`) and never reads
`[Unreleased]`, so the new bullet gets its own fixture run. `release_pair` writes the fixed
`STABILITY_PAGE`, which lists only `loom_sdk::Port` and `loom_sdk::Other`; a fixture on that page
would pass without the covers rule ever seeing `loom_sdk::intake::router`, hence the page argument.

The embedded files are valid, so no governor test over `ProtocolCatalog::engineering()` reaches
the lock-refusal branch. `builtin_in` takes the catalog result as an argument so that a unit test
can hand it `ProtocolCatalog::from_vendored` (`story:engineering-protocol-lock`) over a tampered
copy of `ENGINEERING_FILES`, the same load the governor's catalog comes from.

## Waits on

`story:engineering-protocol-lock`: the catalog these sites read, `ProtocolCatalog::from_vendored`,
`ENGINEERING_FILES`, the printed form of `VendoredError`, the file `engineering_lock.rs`, and the
removal of the registry calls from `crates/loom-protocols/src/lib.rs:160,162`, without which this
story's first acceptance bullet cannot be met (this story edits nothing in
`crates/loom-protocols/src/`).

## ESS first

Exempt from an `ess/` change: `RouterError` is hand-written and no ESS document declares it
(`grep -rn Registry ess/` finds only a comment, `ess/intake/domains/routing.yaml:247`), and the
governor's refusals keep their kinds. The red step is the unit test
`a_catalog_failing_its_lock_is_invalid_protocol_naming_the_refusal` in
`crates/loom-governor/src/lib.rs`. It fails on today's code: it calls `builtin_in`, which does not
exist, and today's `builtin()` returns `canon_engineering::registry::Builtin`, read from the
registry, where no lock check can refuse.

## Acceptance

- `git grep -n 'canon_engineering' -- crates consumers` prints nothing (the hits in
  `crates/loom-protocols/src/lib.rs:160,162` go with `story:engineering-protocol-lock`; this
  story removes the rest).
- `a_catalog_failing_its_lock_is_invalid_protocol_naming_the_refusal`
  (`crates/loom-governor/src/lib.rs`, unit test): `builtin_in` given
  `ProtocolCatalog::from_vendored` over `ENGINEERING_FILES` with one byte of
  `software-change/1.yaml` changed, and `ENGINEERING_LOCK`, for `software-change@1` returns
  `OpenError::InvalidProtocol` whose `protocol` is `software-change@1` and whose `problem` starts
  with `engineering protocol lock refused: DigestMismatch software-change/1.yaml`.
- The 12 sites read the catalog: `crates/loom-governor/tests/governed_case.rs:613,621`,
  `adversary_governor.rs:1044`, `adversary2_governor.rs:643`;
  `crates/loom-intake-slice/tests/adversary_case.rs:244,248`;
  `crates/loom-intake-router/tests/adversary_classify.rs:116,476,477`, `classify.rs:91,145,146`;
  each test asserts what it asserted before.
- `the_vendored_files_equal_the_pinned_registry` (`crates/loom-protocols/tests/engineering_lock.rs`,
  created by `story:engineering-protocol-lock`) is deleted with the last call;
  `the_engineering_digests_are_the_released_ones` and
  `every_engineering_entry_matches_its_lock_entry` stay and pass.
- A governor without a catalog opens `software-change@1` and `incident-response@1` as before
  (`governed_case.rs`, `incident_response_slice.rs` pass).
- `a_governor_without_a_catalog_refuses_an_unknown_protocol` (`governed_case.rs`, new) opens
  `no-such@1` and gets `UnknownProtocol`.
- `RouterError` has no `Registry` variant; `crates/loom-plugin` builds and its tests pass.
- The governor's doc comments read as § Outcome gives them: `git grep -n -i -e 'registry' -e 'ELS
  built-in' -e 'canon-engineering' -e '0\.15\.2' -- crates/loom-governor/src` prints nothing (at
  `39f4da88` it prints `file_store.rs:15` and `lib.rs:51`, `:129`, `:130`, `:132`, `:1617`,
  `:1627`, `:1699`, `:1723`, `:1724`, `:1734-1736`).
- `CHANGELOG.md` `[Unreleased]` gets one bullet under `### Removed` that starts `**Breaking:**`,
  names `loom_sdk::intake::router::RouterError::Registry` as removed and says a catalog that cannot
  be built stays `RouterError::Catalog`.
- `the_router_registry_removal_passes_release_check` (`crates/loom-xtask/tests/release_check.rs`,
  new): it reads the repository's `CHANGELOG.md`, fails when `[Unreleased]` holds no `### Removed`
  bullet naming `loom_sdk::intake::router::RouterError::Registry`, puts that bullet as the `0.2.0`
  entry of a `release_pair_with_page` fixture whose `0.1.0` stability page lists
  `loom_sdk::intake::router`, and asserts `release-check --tag 0.2.0` exits 0.
- The existing `release_check.rs` tests that call `release_pair` pass unchanged.
- `task check` exits 0.

## Files

`crates/loom-governor/src/lib.rs`, `crates/loom-governor/src/file_store.rs` (doc comment `:15`),
`crates/loom-governor/tests/governed_case.rs`,
`crates/loom-governor/tests/adversary_governor.rs`,
`crates/loom-governor/tests/adversary2_governor.rs`,
`crates/loom-intake-slice/tests/adversary_case.rs`, `crates/loom-intake-router/src/classify.rs`,
`crates/loom-intake-router/tests/adversary_classify.rs`,
`crates/loom-intake-router/tests/classify.rs`, `crates/loom-plugin/src/turn.rs`,
`crates/loom-protocols/tests/engineering_lock.rs`, `crates/loom-xtask/tests/release_check.rs`
(a new helper and a new test; `release_pair` and its callers unchanged), `CHANGELOG.md`. No
manifest: the three crates already depend on `b10x-loom-protocols`
(`crates/loom-governor/Cargo.toml:14`, `crates/loom-intake-slice/Cargo.toml:14`,
`crates/loom-intake-router/Cargo.toml:14`). `release_check.rs` is in no other story of the set
(`story:engineering-protocols-vendor-command` adds `protocols_vendor.rs` beside it).

## Out of scope

Deleting the dependency lines and changing the one-copy tests
(`story:engineering-protocols-dependency-removed`). The comment "ELS registry" in
`ess/intake/domains/routing.yaml:247` and `classify.rs:88` (an `ess/` edit would need its own red
step; the Rust comment may change with the variant). Changing which protocols the router offers.
Changing `crates/loom-protocols/src/` (`story:engineering-protocol-lock` owns `from_vendored` and
the text of `VendoredError`). Changing `release_pair`'s signature or `STABILITY_PAGE`.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-governor` (`src/lib.rs` `builtin()` and its doc comments, plus 4 test call sites) — cited (story § Outcome, `lib.rs:1724-1742`, verified)
- **Also on:** `crates/loom-intake-router` (`RouterError::Registry`, `src/classify.rs:32,56,81`; 6 test calls), `crates/loom-plugin/src/turn.rs:282`, `crates/loom-intake-slice/tests/adversary_case.rs:244,248` — cited
- **Files (governor):** `crates/loom-governor/src/lib.rs` (`:51`, `:129-133`, `:913`, `:1617`, `:1627`, `:1699-1701`, `:1723-1742`), `crates/loom-governor/src/file_store.rs:15`, `tests/governed_case.rs:613,621`, `tests/adversary_governor.rs:1044`, `tests/adversary2_governor.rs:643` — cited; all lines read at `39f4da88` and say what the story says
- **Files (router, slice, plugin):** `crates/loom-intake-router/src/classify.rs`, `tests/adversary_classify.rs:116,476,477`, `tests/classify.rs:91,145,146`, `crates/loom-intake-slice/tests/adversary_case.rs`, `crates/loom-plugin/src/turn.rs` — cited
- **Files (xtask and protocols tests):** `crates/loom-xtask/tests/release_check.rs` (new test), `crates/loom-protocols/tests/engineering_lock.rs` (delete one test; the file does not exist at `39f4da88`, `story:engineering-protocol-lock` creates it) — cited
- **Files (shared prose):** `CHANGELOG.md` (`[Unreleased]` has no `### Removed` section yet; the story adds it) — cited
- **Symbols:** `builtin`, `builtin_in` (new), `outcome_order`, `protocol_ir`, `OpenError::{UnknownProtocol,InvalidProtocol}`, `ProtocolCatalog::{engineering,from_vendored}`, `ENGINEERING_FILES`, `ENGINEERING_LOCK`, `ProtocolEntry::{name,definition.yaml,model}`, `RouterError::{Registry,Catalog}` — cited. The last four exist at `39f4da88` except `from_vendored`, `ENGINEERING_FILES` and `ENGINEERING_LOCK`, which the lock story creates.
- **Documents:** none outside doc comments in the files above. `ess/` untouched; the story says so (§ ESS first) and `ess/intake/domains/routing.yaml:247` is a comment — cited
- **Confidence:** high. The story names every file and line, and I read each one in the tree. The only guessed part is the collision picture below.
- **Would collide with, `story:engineering-protocol-lock`:** `crates/loom-protocols/tests/engineering_lock.rs` and the removal of `canon_engineering` from `loom-protocols/src/lib.rs:160,162`. Ordered: `depends_on` — cited
- **Would collide with, `story:canon-0-9-0-pin`:** `loom-governor/src/lib.rs:129-132`, `governed_case.rs:9` and `adversary2_governor.rs` (it fixes `:685`). Ordered: mine → lock → pin, so the pin lands first — cited
- **Would collide with, `story:engineering-protocols-dependency-removed`:** `governed_case.rs:9` (doc comment) and `CHANGELOG.md`. Ordered: it `depends_on` mine — cited
- **Would collide with, `story:engineering-protocols-vendor-command`:** `CHANGELOG.md` only (its bullet, body line 115). Not ordered: it depends on the lock story, not on mine. Trivial merge in a house-rule file — cited
- **Would collide with, `story:child-case-runtime`:** `loom-governor/src/lib.rs` (`opened` `:1577`, `snapshot` `:1661`, near my `:1617`, `:1627`, `:1699`) and `adversary2_governor.rs`. NOT ordered: it depends on `canon-0-9-0-pin`, not on mine. Hunks are separate but close — inferred from its Scope lines
- **Would collide with, `story:bound-artifact-revisions` and `story:record-copy-between-cases`:** `loom-governor/src/lib.rs`. Neither is ordered against mine. `bound-artifact-revisions` edits `:1017-1046`, `:1415-1432` and `:1779-1791` and reads `adversary_governor.rs:75-112`; mine is `:1699-1742`. Likely clean — inferred
- **Would collide with, `story:hash-chained-run-log`:** `loom-governor/src/lib.rs:175-179` (a `pub use`) and `file_store.rs:136-147`; mine is `file_store.rs:15`, a comment. NOT ordered. Likely clean — inferred
- **Would collide with, `story:co-pinned-consumer-check`, `story:command-invocation-contract`, `story:generated-types-release-surface`, `story:release-binaries`:** `crates/loom-xtask/tests/release_check.rs`. Each adds tests there; none is ordered against mine (the first three depend on `release-names-embedder-breaks`, which is implemented). Appended tests, likely clean, but my story may change the shape of `release_pair` (see defects) — cited (their Files lines)
- **Would collide with, `story:protocol-argument-schema` and `story:send-input-in-the-declared-shape`:** `loom-plugin/src/turn.rs`. They edit `:392-393` and `:800-834`; mine is `:282`. Not ordered, disjoint hunks — cited
