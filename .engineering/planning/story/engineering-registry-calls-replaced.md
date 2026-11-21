---
format: aep.planning-md/3
id: story:engineering-registry-calls-replaced
kind: story
status: implemented
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
  path: crates/loom-governor/tests/adversary_w20261121w4_u3.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_w20261121w4_u3_threads.rs
- confidence: cited
  path: crates/loom-governor/tests/governed_case.rs
- confidence: cited
  path: crates/loom-intake-router/src/classify.rs
- confidence: cited
  path: crates/loom-intake-router/tests/adversary_classify.rs
- confidence: cited
  path: crates/loom-intake-router/tests/adversary_w20261121w4_u3.rs
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
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:18:31Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-11-21T07:20:39Z", actor: "agent:sdc", revision: 9, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w4"}
- {from: "active", to: "implemented", at: "2026-11-21T09:11:56Z", actor: "agent:sdc", revision: 14, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w4"}
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

Rewritten 2026-11-21 at the close of wave 2026-11-21-w4 from the unit's commits (`git diff --stat 63668a97 365ef1c8`: 17 files, 808 insertions, 120 deletions). Every line is cited from that diff.

- **Primary surface:** `crates/loom-governor` — cited, 7 of the 17 changed files are under it (`src/lib.rs`, `src/file_store.rs`, two new test files, three existing test files); `crates/loom-intake-router` holds 4 more (`src/classify.rs`, one new test file, two existing test files)
- **Files (ESS sources, generated output, manifests):** none — cited; no file under `ess/`, `generated/`, `protocols/` or any `Cargo.toml` or `Cargo.lock` is in the diff
- **Files:** `crates/loom-governor/src/lib.rs` — cited, 196 lines (168 added, 28 deleted): the `# Evaluation` and `# Dependencies` docs describe the vendored catalog, the `builtin` fallback reads `ProtocolCatalog::engineering()` once per process (a `OnceLock`) and maps through the new `builtin_in`, a lock refusal is `InvalidProtocol` with the catalog's error text, and 4 new unit tests
- **Files:** `crates/loom-governor/src/file_store.rs` — cited, 2 lines: the doc comment on `:15` says "a vendored engineering protocol"
- **Files:** `crates/loom-intake-router/src/classify.rs` — cited, 156 lines (146 added, 10 deleted): `RouterError::Registry` removed with its `Display` and `source` arms, `classify` delegates to a new private `classify_in` that takes the catalog result, docs name the vendored catalog, and 1 new unit test
- **Files:** `crates/loom-plugin/src/turn.rs` — cited, 8 lines: the `RouterError::Registry(_)` arm of `propose_case` goes
- **Files (new tests):** `crates/loom-governor/tests/adversary_w20261121w4_u3.rs` (103 lines, 2 cases), `crates/loom-governor/tests/adversary_w20261121w4_u3_threads.rs` (54 lines, 1 case), `crates/loom-intake-router/tests/adversary_w20261121w4_u3.rs` (139 lines, 2 cases), the adversary pass's cases, committed unchanged
- **Files (new test code in existing files):** `crates/loom-governor/src/lib.rs` (`mod tests`, 4 `#[test]`: the story's `a_catalog_failing_its_lock_is_invalid_protocol_naming_the_refusal`, `an_unknown_well_formed_name_under_a_refused_catalog_is_invalid_protocol`, `a_malformed_name_under_a_refused_catalog_is_unknown_protocol`, `the_engineering_catalog_holds_each_builtin_and_nothing_else`), `crates/loom-intake-router/src/classify.rs` (1 `#[tokio::test]`, `a_catalog_failing_its_lock_is_a_catalog_error_and_asks_no_model`), `crates/loom-governor/tests/governed_case.rs` (1 `#[test]`, `a_governor_without_a_catalog_refuses_an_unknown_protocol`), `crates/loom-xtask/tests/release_check.rs` (1 `#[test]`, `the_router_registry_removal_passes_release_check`)
- **Files (existing tests):** `crates/loom-governor/tests/governed_case.rs` (42 lines: `software_change_ir` and `incident_ir` go through a new `vendored_ir` over the catalog, plus the new case), `crates/loom-governor/tests/adversary_governor.rs` (21 lines: `ir_of` reads the catalog, two doc comments), `crates/loom-governor/tests/adversary2_governor.rs` (8 lines: `ir_of` reads the catalog), `crates/loom-intake-router/tests/classify.rs` (19 lines: `registry()` and `assert_request` read the catalog), `crates/loom-intake-router/tests/adversary_classify.rs` (29 lines: `registry()` sorts the catalog's names by name then numeric major, the order check reads the catalog), `crates/loom-intake-slice/tests/adversary_case.rs` (17 lines: `adversary_every_registry_protocol_opens_on_a_git_workspace` iterates the catalog), `crates/loom-protocols/tests/engineering_lock.rs` (24 lines deleted: `the_vendored_files_equal_the_pinned_registry`), `crates/loom-xtask/tests/release_check.rs` (96 lines, 92 added: the helper `release_pair_with_page`, with `release_pair` now calling it with `STABILITY_PAGE`, the helper `unreleased_removed`, the constant `ROUTER_REGISTRY` and the new case) — cited
- **Files (documents):** `AGENTS.md` (9 lines: § Governor says a governor without a catalog takes any other protocol from `ProtocolCatalog::engineering` and what a lock refusal is, and the router bullet names `RouterError::Catalog`), `CHANGELOG.md` (5 lines: a `Changed` bullet for the governor and a new `### Removed` section with the `**Breaking:**` bullet for `RouterError::Registry`) — cited
- **Symbols:** public item removed: `loom_intake_router::RouterError::Registry` (and with it `loom_sdk::intake::router::RouterError::Registry`). No public item added or changed: `classify` and `CanonGovernor::open` keep their signatures. Private, added or changed: `loom_governor::builtin` (now `Result<ProtocolEntry, OpenError>`), `builtin_in` (new), `loom_intake_router::classify_in` (new), `checked_builtin_inputs` and `checked_builtin_independence` (`pub(crate)`, doc text only) — cited
- **Not predicted and touched:** `crates/loom-governor/tests/adversary_w20261121w4_u3.rs`, `crates/loom-governor/tests/adversary_w20261121w4_u3_threads.rs`, `crates/loom-intake-router/tests/adversary_w20261121w4_u3.rs` (the adversary pass's cases). `AGENTS.md` (9 lines) was not in the typed scope; the stored Files line omits it and names `CHANGELOG.md` only. The private `classify_in` in `classify.rs` and the 4 governor unit tests beyond the named one are not in the stored text — cited
- **Predicted and not touched:** none — cited; all 12 files of the typed scope are in the diff, and no manifest was touched, as the story's Files section says
- **Review:** `review-result:engineering-registry-calls-replaced-adversary-1` (head `7ebac517`, not the unit head): green, 5 cases added, 221 executed, 0 red; two notes, both introduced: a mutant survives by reasoning (a refused catalog with a malformed name, or an unknown well-formed name, had only the `software-change@1` case; the diff's unit tests now hold both), and `the_engineering_catalog_holds_each_builtin_and_nothing_else` named more than it checked (the diff's version loops over all 10 vendored files). Stale prose noted: `AGENTS.md` line 463 ("never an unknown protocol") and test comments that say "ELS registry/built-in". No security review recorded.
