---
format: aep.planning-md/3
id: story:engineering-protocols-dependency-removed
kind: story
status: active
title: No Loom manifest names b10x-canon-engineering and Cargo.lock holds no engineering-protocols package
summary: Four lines and their lock packages go; the three one-copy tests hold one Canon and no registry; AGENTS.md names Canon by its three manifest lines, moves it with cargo update -p b10x-canon and names no engineering-protocols tag
tags:
- sdc
relations:
- decomposes: epic:engineering-protocols-as-vendored-data
- serves: vision:O1
- depends_on: story:engineering-registry-calls-replaced
- depends_on: story:engineering-protocols-vendor-command
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-executor/tests/intake_import.rs
- confidence: cited
  path: crates/loom-governor/Cargo.toml
- confidence: cited
  path: crates/loom-governor/tests/governed_case.rs
- confidence: cited
  path: crates/loom-governor/tests/incident_response_slice.rs
- confidence: cited
  path: crates/loom-intake-router/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: crates/loom-protocols/Cargo.toml
- confidence: cited
  path: crates/loom-protocols/tests/adversary_w8_pin.rs
- confidence: inferred
  path: crates/loom-protocols/tests/engineering_lock.rs
- confidence: cited
  path: crates/loom-xtask/tests/pinned_consumer.rs
- confidence: cited
  path: website/docs/guides/move-to-loom.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:18:31Z", actor: "human:timo", revision: 9}
- {from: "proposed", to: "active", at: "2026-11-21T13:30:58Z", actor: "agent:sdc", revision: 12, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w7"}
---
## Outcome

No Loom manifest names `b10x-canon-engineering`, and `Cargo.lock` holds no package from
engineering-protocols. Canon stays a compile-time dependency, named by the `b10x-canon` line of
each crate that pins it (`crates/loom-governor/Cargo.toml:15`, `crates/loom-protocols/Cargo.toml:14`,
`crates/loom-bindings/Cargo.toml:14`, the lines `git grep -n '^b10x-canon ' -- '*.toml'` prints at
`39f4da88`), and moves with `cargo update -p b10x-canon` alone. The three tests that held one copy
of the registry beside one Canon now hold one Canon and no engineering-protocols package.
`AGENTS.md` names no engineering-protocols crate, package or tag: § Governor names Canon by those
manifest lines, § Boundary says engineering definitions are vendored, and § Connectors no longer
names `b10x-assertion-providers`, which leaves the graph. The site names no
`b10x-canon-engineering` crate.

`crates/loom-protocols/tests/adversary_w8_pin.rs` has one owner per change. This story removes the
comparison of each engineering entry's revision with `one_tag("b10x-canon-engineering")` (`:83`,
`:96-102`) from `every_engineering_entry_carries_the_tag_the_manifest_pins` (`:79-113`), since no
manifest names the crate any more (`manifest_tags` panics on none, `:44`). The comparison with
`protocols/engineering/lock.json`'s tag, which `story:engineering-protocol-lock` added to that test
(`:84-86`, `:103-109`), stays and is then the test's only tag check; the test is renamed
`every_engineering_entry_carries_the_tag_the_vendored_lock_names`.

## Why

Step four of `epic:engineering-protocols-as-vendored-data`, the outcome session `sdc` asked for: a
Canon move no longer waits on an engineering-protocols release. After
`story:engineering-registry-calls-replaced` the four lines (`crates/loom-protocols/Cargo.toml:15`,
`crates/loom-governor/Cargo.toml:19`, `crates/loom-intake-slice/Cargo.toml:18`,
`crates/loom-intake-router/Cargo.toml:15`) serve nothing. Their packages in `Cargo.lock`, read at
`aa943153`, are `b10x-canon-engineering` (`:214`),
`b10x-canon-engineering-assertions` (`:226`), `b10x-assertion-providers` (`:188`) and
`b10x-canon-expr` (`:242`), which only the assertions crate names (`:232`).

`AGENTS.md` § Governor (committed `:556-559`) defines Canon's reference as the one
`b10x-canon-engineering` uses and moves Canon "together with the `b10x-canon-engineering` tag (now
`0.16.0`)"; that definition must move to Loom's own lines, and the tag goes with it. § Boundary
(committed `:34-35`) names the crate as the owner of engineering definitions. § Connectors
(committed `:567-569`) says `serde_json`'s `arbitrary_precision` was already on for
`b10x-loom-sdk` through `b10x-canon-expr` `0.9.0` and `b10x-assertion-providers` `0.16.0`, and
that `connectors-core` also turns it on since `v0.62.0`; with the package gone, the section drops
the first clause and names `connectors-core` alone. § Bindings document (committed
`:1181-1182`) already names Canon by `crates/loom-governor/Cargo.toml`'s reference and needs no
change. `story:engineering-protocols-vendor-command` inserted lines into § Protocol composition
and system queries (`:49-50`), above all three sections, naming `protocols/engineering/`, so this
story cites `AGENTS.md` by heading and checks each section's text under its own heading.

`story:canon-0-9-0-pin` landed and moved every pin it names, `AGENTS.md` § Governor and
§ Connectors included, to engineering-protocols `0.16.0` and Canon `0.9.0` (its § Acceptance,
"Every place that names the pin moves with it"). The tag this story removes from `AGENTS.md` is
`0.16.0`.

On the site, `b10x-canon-engineering` is named by `website/docs/guides/move-to-loom.md:49,57` and
`website/docs/reference/sdk-stability.md:63` and the pin table's head `:67`
(`git grep -n 'b10x-canon-engineering' -- website`, `39f4da88`).

## Waits on

- `story:engineering-registry-calls-replaced`: no source may name the crate when its line goes.
  That story also rewrote the governor's doc comments: `crates/loom-governor/src/lib.rs:130-134`
  now names Canon as `b10x-canon` at the reference its `Cargo.toml` names and names no engineering
  crate (it said that `b10x-canon-engineering` is pinned to a tag), so this story does not edit
  `crates/loom-governor/src/lib.rs`.
- `story:engineering-protocols-vendor-command`: both stories edit `Cargo.lock` (that story added
  `b10x-loom-protocols` to the `loom-xtask` package's dependency list, `:3731`; this one removes
  four packages and removes the engineering crate from four crates' lists: `b10x-loom-governor`,
  `b10x-loom-intake-router`, `b10x-loom-intake-slice` and `b10x-loom-protocols`, `Cargo.lock:615`,
  `:648`, `:660`, `:758` at `aa943153`) and `AGENTS.md` (that story § Protocol composition and
  system queries; this one § Boundary, § Governor and § Connectors).
- `story:engineering-protocol-lock`: the `lock.json` tag assertion in `adversary_w8_pin.rs` this
  story keeps (reached through `story:engineering-registry-calls-replaced`).

## ESS first

Exempt: no declaration of Loom's specification changes; the change is manifests, the lock file,
three tests and documents. The red step is the rewritten
`the_lockfile_holds_one_canon_and_no_engineering_package` (below), which fails while `Cargo.lock`
still holds `b10x-canon-engineering`.

## Acceptance

- The four lines are deleted; `git grep -n 'b10x-canon-engineering' -- '*.toml' Cargo.lock`
  prints nothing.
- `cargo tree --locked -i b10x-canon-engineering` matches no package.
- `crates/loom-protocols/tests/adversary_w8_pin.rs`:
  `the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags` (`:115-131`) becomes
  `the_lockfile_holds_one_canon_and_no_engineering_package`: one `b10x-canon` from the tag every
  manifest names, and no `[[package]]` whose source is `github.com/beyond10x/engineering-protocols`.
- `every_engineering_entry_carries_the_tag_the_vendored_lock_names` (`adversary_w8_pin.rs`, the
  renamed `:79-113`) no longer calls `one_tag("b10x-canon-engineering")`, compares each entry's
  revision with the tag of `protocols/engineering/lock.json` only, and passes.
- `crates/loom-executor/tests/intake_import.rs`: `SINGLE_COPY` (`:26-32`) holds four packages
  without `b10x-canon-engineering`, the doc comment (`:9-12`) says so, and
  `lock_resolves_one_path_copy_of_each_absorbed_crate` also fails when `Cargo.lock` holds a package
  named `b10x-canon-engineering`.
- `crates/loom-xtask/tests/pinned_consumer.rs`: the duplicate filter (`:145`) drops the name and
  still refuses a second `b10x-canon`; `task consumer-check` exits 0.
- `git grep -n -e 'b10x-canon-engineering' -e 'b10x-assertion-providers' -e '0\.15\.2' -e
  '0\.16\.0' -- AGENTS.md` prints nothing: no section names an engineering-protocols crate,
  package or tag. Before this story it prints a line of § Boundary (the crate), lines of
  § Governor (the crate and its tag `0.16.0`) and a line of § Connectors
  (`b10x-assertion-providers` and its tag); at `aa943153` it prints § Boundary `:35`, § Governor
  `:556`, `:558`, `:559` and § Connectors `:568`.
- `AGENTS.md` § Governor names Canon by the `b10x-canon` line of each of
  `crates/loom-governor/Cargo.toml`, `crates/loom-protocols/Cargo.toml` and
  `crates/loom-bindings/Cargo.toml`, and moves it with `cargo update -p b10x-canon`:
  `awk '/^## /{s=$0} /cargo update -p b10x-canon/{print s}' AGENTS.md` prints exactly one line,
  `## Governor` (at `aa943153` it prints `## Governor` for `:558`, the line that also names
  `b10x-canon-engineering`, which the grep above refuses).
- `AGENTS.md` § Boundary says engineering protocol definitions are engineering-protocols',
  vendored under `protocols/engineering/`: `awk '/^## /{s=$0} /protocols\/engineering\//{print s}'
  AGENTS.md` prints a `## Boundary` line (at `aa943153` it prints only `## Protocol composition and
  system queries`, for the lines `:49` and `:50` that `story:engineering-protocols-vendor-command`
  added, which does not count).
- `git grep -n 'b10x-canon-engineering' -- website` prints nothing:
  `website/docs/reference/sdk-stability.md:60-72` names the Canon release and the
  engineering-protocols tag the vendored files come from (the lock's), with the pin table's third
  column (`:67-70`) headed `engineering-protocols` (its rows `0.57.2` and `0.58.0` keep their
  tags), and
  `website/docs/guides/move-to-loom.md:49-58` says the engineering protocols are vendored files
  from that tag and that Canon is the reference Loom's manifests name.
- The doc comments `crates/loom-governor/tests/governed_case.rs:9` and
  `incident_response_slice.rs:23` drop the crate's name
  (`git grep -n 'b10x-canon-engineering' -- crates/loom-governor` prints nothing).
- `CHANGELOG.md` `### Removed`: no Loom crate depends on `b10x-canon-engineering`; a host that
  linked it beside Loom chooses its tag itself. Not a `**Breaking:**` bullet: no `loom_sdk` path
  carried it after `story:engineering-registry-calls-replaced`.
- `task check` exits 0.

## Files

`crates/loom-protocols/Cargo.toml`, `crates/loom-governor/Cargo.toml`,
`crates/loom-intake-slice/Cargo.toml`, `crates/loom-intake-router/Cargo.toml`, `Cargo.lock`
(shared with `story:engineering-protocols-vendor-command`, ordered before this story),
`crates/loom-protocols/tests/adversary_w8_pin.rs`, `crates/loom-executor/tests/intake_import.rs`,
`crates/loom-xtask/tests/pinned_consumer.rs`, `crates/loom-governor/tests/governed_case.rs`,
`crates/loom-governor/tests/incident_response_slice.rs`, `AGENTS.md` (§ Boundary, § Governor,
§ Connectors; shared with the same story), `CHANGELOG.md`, `website/docs/reference/sdk-stability.md`,
`website/docs/guides/move-to-loom.md`.

## Out of scope

Moving Canon (`story:canon-0-9-0-pin` did). The release that carries the break. Historical lines
of `README.md` (`:89`, `:103`) and `CHANGELOG.md`, which record earlier pins. Whether
`b10x-canon-expr` stays in `Cargo.lock`: it depends on Canon `0.9.0`'s own graph.
`crates/loom-governor/src/lib.rs` (`story:engineering-registry-calls-replaced` rewrote its doc
comments). `website/docs/concepts/governor-and-intake.md`: `:14-17` names neither the crate nor a
tag and links to the engineering protocols site, which stays right.
`story:doc-dependency-tags-match-lock` (draft, another epic, no edge to this set): its test
compares the engineering-protocols tag the docs name with `Cargo.lock`, which after this story
holds no engineering-protocols package, so it cannot hold as written and must be revised (compare
with `protocols/engineering/lock.json`) or retired. This story does not edit it.

## As built, wave 2026-11-21-w7

Where the unit's commit (`2a4779ae`) differs from the text above, the commit is what holds:

- 13 packages left `Cargo.lock`, not four (`name = ` lines 552 before, 539 after). The four are
  `b10x-assertion-providers`, `b10x-canon-engineering`, `b10x-canon-engineering-assertions` and
  `b10x-canon-expr`; the lock holds no `b10x-canon-expr` now, which "Out of scope" left open. Nine
  more had no other dependant and went with them: `fancy-regex` 0.16.2, `fluent-uri` 0.3.2,
  `fraction` 0.15.4, `jsonschema` 0.33.0, `lazy_static`, `logos`, `logos-codegen`, `logos-derive`
  and `referencing` 0.33.0. No manifest named any of the nine. The coordinator accepted the lock
  as cargo wrote it (merge `855279a6`).
- Kept blocks of `Cargo.lock` also changed: references to `jsonschema`, `referencing`,
  `fancy-regex`, `fraction` and `fluent-uri` lose their version suffix because one copy of each
  remains, and `uuid-simd` loses its `uuid` edge. No package is added and no version, source or
  checksum changes (commit body of `2a4779ae`).
- The `awk` command over `protocols/engineering/` in Acceptance prints three lines on `2a4779ae`:
  `## Boundary` once and `## Protocol composition and system queries` twice (the lines of
  `story:engineering-protocols-vendor-command`). Two headings print, not one; the check is that
  `## Boundary` is among them.
- Two of the three one-copy tests failed on the base lock, as the red step says: the lock test of
  `adversary_w8_pin.rs` and `lock_resolves_one_path_copy_of_each_absorbed_crate`.
  `crates/loom-xtask/tests/pinned_consumer.rs` cannot go red by this change: it loses one name
  from a duplicate filter, which cannot fail while one copy of each name is locked. It passed
  (5 passed) with the tests changed and the manifests and lock at base.
- The test count did not move: 664 before and after (lanes 69 + 4 + 5 + 139 + 15 + 432). Three
  tests were rewritten and none was added or removed.
- `crates/loom-protocols/tests/engineering_lock.rs` changed (one doc-comment line). Scope lists
  it as inferred and the commit confirms it; the Files section does not name it.
- `task check`, the last line of Acceptance, was not run by the unit; the integration branch runs
  it. `cargo tree --locked -i b10x-canon-engineering` exits 101, "did not match any packages".

## Scope

Rewritten 2026-11-21 at the close of wave 2026-11-21-w7 from the unit's commits (`git diff --stat
c1a1f443 2a4779ae`: 15 files, 102 insertions, 242 deletions). Every line is cited from that diff.

- **Primary surface:** the four manifest lines naming `b10x-canon-engineering` and `Cargo.lock` —
  cited, 5 of the 15 changed files (`crates/loom-governor/Cargo.toml`,
  `crates/loom-intake-router/Cargo.toml`, `crates/loom-intake-slice/Cargo.toml`,
  `crates/loom-protocols/Cargo.toml`, `Cargo.lock`); the three one-copy tests, two test doc comments
  and the documents make up the other 10
- **Files (manifests):** `crates/loom-governor/Cargo.toml`, `crates/loom-intake-router/Cargo.toml`,
  `crates/loom-intake-slice/Cargo.toml`, `crates/loom-protocols/Cargo.toml` (1 line each, all
  deletions: the `b10x-canon-engineering` line at tag `0.16.0`) — cited
- **Files (generated output):** `Cargo.lock` — cited, 7 insertions and 175 deletions, rewritten by
  cargo after the four lines went: 13 packages leave (`b10x-assertion-providers`,
  `b10x-canon-engineering`, `b10x-canon-engineering-assertions`, `b10x-canon-expr`, `fancy-regex`,
  `fluent-uri`, `fraction`, `jsonschema`, `lazy_static`, `logos`, `logos-codegen`, `logos-derive`,
  `referencing`), and the 7 inserted lines are dependency entries that lose their version suffix
  because one copy of `jsonschema`, `fancy-regex`, `fraction`, `referencing` and `fluent-uri`
  remains
- **Files (existing tests):** `crates/loom-protocols/tests/adversary_w8_pin.rs` (80 lines: the
  module doc names the two places Canon's pin is written;
  `every_engineering_entry_carries_the_tag_the_manifest_pins` is renamed
  `every_engineering_entry_carries_the_tag_the_vendored_lock_names` and drops the
  `one_tag("b10x-canon-engineering")` comparison;
  `the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags` becomes
  `the_lockfile_holds_one_canon_and_no_engineering_package`, with the new helper `locked_packages`;
  no `#[test]` added or removed), `crates/loom-executor/tests/intake_import.rs` (20 lines:
  `SINGLE_COPY` shrinks from 5 to 4 packages, the new `ABSENT` list holds `b10x-canon-engineering`,
  `lock_resolves_one_path_copy_of_each_absorbed_crate` fails when the lock holds it, the module doc
  says so), `crates/loom-xtask/tests/pinned_consumer.rs` (5 lines: the duplicate filter drops the
  name), `crates/loom-protocols/tests/engineering_lock.rs` (2 lines, a doc comment: "the registry
  engineering-protocols' tag holds"), `crates/loom-governor/tests/governed_case.rs` and
  `crates/loom-governor/tests/incident_response_slice.rs` (4 lines each, doc comments: the fixture's
  tag is the one `protocols/engineering/lock.json` names) — cited
- **Files (new tests):** none — cited
- **Files (documents):** `AGENTS.md` (18 lines: § Boundary names the engineering definitions as
  vendored under `protocols/engineering/`, § Governor names Canon by the three manifest lines and
  moves it with `cargo update -p b10x-canon` alone, § Connectors drops `b10x-canon-expr` and
  `b10x-assertion-providers`), `CHANGELOG.md` (4 lines: a `### Removed` heading and one bullet),
  `website/docs/guides/move-to-loom.md` (10 lines: the engineering protocols are vendored files,
  Canon is the reference Loom's manifests name), `website/docs/reference/sdk-stability.md` (11
  lines: the Canon section and the pin table's third column, now headed `engineering-protocols`) —
  cited
- **Symbols:** no public item added, changed or removed; no file under a crate's `src/` is in the
  diff — cited. Test-local items only: const `ABSENT` and `SINGLE_COPY`'s length in
  `intake_import.rs`, fn `locked_packages` in `adversary_w8_pin.rs`, and the two renamed or
  rewritten test functions named above
- **Not predicted and touched:** none; `CHANGELOG.md` is named by the stored Scope section but is
  not a row of the typed scope (a house-rule file, not added);
  `crates/loom-protocols/tests/engineering_lock.rs` was inferred and is touched
- **Predicted and not touched:** none; every row of the typed scope is in the diff.
  `b10x-canon-expr`, which the story left open as an out-of-scope question, left `Cargo.lock` with
  the other 12 packages
- **Review:** none planned; the coordinator read the commit
