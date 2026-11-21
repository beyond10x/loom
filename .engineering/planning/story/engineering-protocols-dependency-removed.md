---
format: aep.planning-md/3
id: story:engineering-protocols-dependency-removed
kind: story
status: proposed
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
- confidence: cited
  path: crates/loom-xtask/tests/pinned_consumer.rs
- confidence: cited
  path: website/docs/guides/move-to-loom.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:18:31Z", actor: "human:timo", revision: 9}
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
comparison of each engineering entry's revision with `one_tag("b10x-canon-engineering")` (`:80`,
`:91`) from `every_engineering_entry_carries_the_tag_the_manifest_pins` (`:78-100`), since no
manifest names the crate any more (`manifest_tags` panics on none, `:43`). The comparison with
`protocols/engineering/lock.json`'s tag, which `story:engineering-protocol-lock` adds to that test,
stays and is then the test's only tag check; the test is renamed
`every_engineering_entry_carries_the_tag_the_vendored_lock_names`.

## Why

Step four of `epic:engineering-protocols-as-vendored-data`, the outcome session `sdc` asked for: a
Canon move no longer waits on an engineering-protocols release. After
`story:engineering-registry-calls-replaced` the four lines (`crates/loom-protocols/Cargo.toml:15`,
`crates/loom-governor/Cargo.toml:19`, `crates/loom-intake-slice/Cargo.toml:18`,
`crates/loom-intake-router/Cargo.toml:15`) serve nothing. Their packages in `Cargo.lock`, read at
`b38eb7d1` (lines move with `story:canon-0-9-0-pin`), are `b10x-canon-engineering` (`:214`),
`b10x-canon-engineering-assertions` (`:226`), `b10x-assertion-providers` (`:188`) and
`b10x-canon-expr` (`:242`), which only the assertions crate names (`:232`).

`AGENTS.md` § Governor (committed `:520-523`) defines Canon's reference as the one
`b10x-canon-engineering` uses and moves Canon "together with the `b10x-canon-engineering` tag (now
`0.15.2`)"; that definition must move to Loom's own lines, and the tag goes with it. § Boundary
(committed `:34-35`) names the crate as the owner of engineering definitions. § Connectors
(committed `:529-531`) says `serde_json`'s `arbitrary_precision` was on through `b10x-canon-expr`
and `b10x-assertion-providers` `0.15.2`; with the package gone, `connectors-core` is what turns it
on. § Bindings document (committed `:1116-1117`) already names Canon by
`crates/loom-governor/Cargo.toml`'s reference and needs no change. `story:engineering-protocols-vendor-command`
inserts lines into § Protocol composition and system queries, above all three sections and
possibly naming `protocols/engineering/`, so this story cites `AGENTS.md` by heading and checks
each section's text under its own heading.

`story:canon-0-9-0-pin` lands first and moves every pin it names, `AGENTS.md` § Governor and
§ Connectors included, to engineering-protocols `0.16.0` and Canon `0.9.0` (its § Acceptance,
"Every place that names the pin moves with it"). The tag this story removes from `AGENTS.md` is
then `0.16.0`, not `0.15.2`.

On the site, `b10x-canon-engineering` is named by `website/docs/guides/move-to-loom.md:49,57` and
`website/docs/reference/sdk-stability.md:63` and the pin table's head `:67`
(`git grep -n 'b10x-canon-engineering' -- website`, `39f4da88`).

## Waits on

- `story:engineering-registry-calls-replaced`: no source may name the crate when its line goes.
  That story also owns the governor's doc comments, `crates/loom-governor/src/lib.rs:129-133`
  included (the sentence that `b10x-canon-engineering` is pinned to a tag), so this story does not
  edit `crates/loom-governor/src/lib.rs`.
- `story:engineering-protocols-vendor-command`: both stories edit `Cargo.lock` (that story adds
  `b10x-loom-protocols` to the `loom-xtask` package's dependency list; this one removes four
  packages and removes the engineering crate from four crates' lists: `b10x-loom-governor`,
  `b10x-loom-intake-router`, `b10x-loom-intake-slice` and `b10x-loom-protocols`, `Cargo.lock:614`,
  `:647`, `:659`, `:757` at `39f4da88`) and `AGENTS.md` (that story § Protocol composition and
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
  `the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags` (`:103-118`) becomes
  `the_lockfile_holds_one_canon_and_no_engineering_package`: one `b10x-canon` from the tag every
  manifest names, and no `[[package]]` whose source is `github.com/beyond10x/engineering-protocols`.
- `every_engineering_entry_carries_the_tag_the_vendored_lock_names` (`adversary_w8_pin.rs`, the
  renamed `:78-100`) no longer calls `one_tag("b10x-canon-engineering")`, compares each entry's
  revision with the tag of `protocols/engineering/lock.json` only, and passes.
- `crates/loom-executor/tests/intake_import.rs`: `SINGLE_COPY` (`:26-32`) holds four packages
  without `b10x-canon-engineering`, the doc comment (`:9-12`) says so, and
  `lock_resolves_one_path_copy_of_each_absorbed_crate` also fails when `Cargo.lock` holds a package
  named `b10x-canon-engineering`.
- `crates/loom-xtask/tests/pinned_consumer.rs`: the duplicate filter (`:145`) drops the name and
  still refuses a second `b10x-canon`; `task consumer-check` exits 0.
- `git grep -n -e 'b10x-canon-engineering' -e 'b10x-assertion-providers' -e '0\.15\.2' -e
  '0\.16\.0' -- AGENTS.md` prints nothing: no section names an engineering-protocols crate,
  package or tag. Before this story, with `story:canon-0-9-0-pin` landed, it prints a line of
  § Boundary (the crate), lines of § Governor (the crate and its tag `0.16.0`) and a line of
  § Connectors (`b10x-assertion-providers` and its tag); at `39f4da88` it prints § Boundary `:35`,
  § Governor `:520`, `:522`, `:523` and § Connectors `:530`.
- `AGENTS.md` § Governor names Canon by the `b10x-canon` line of each of
  `crates/loom-governor/Cargo.toml`, `crates/loom-protocols/Cargo.toml` and
  `crates/loom-bindings/Cargo.toml`, and moves it with `cargo update -p b10x-canon`:
  `awk '/^## /{s=$0} /cargo update -p b10x-canon/{print s}' AGENTS.md` prints exactly one line,
  `## Governor` (at `39f4da88` it prints `## Governor` for the line that also names
  `b10x-canon-engineering`, which the grep above refuses).
- `AGENTS.md` § Boundary says engineering protocol definitions are engineering-protocols',
  vendored under `protocols/engineering/`: `awk '/^## /{s=$0} /protocols\/engineering\//{print s}'
  AGENTS.md` prints a `## Boundary` line (at `39f4da88` it prints nothing; lines
  `story:engineering-protocols-vendor-command` adds print `## Protocol composition and system
  queries`, which does not count).
- `git grep -n 'b10x-canon-engineering' -- website` prints nothing:
  `website/docs/reference/sdk-stability.md:61-69` names the Canon release and the
  engineering-protocols tag the vendored files come from (the lock's), with the pin table's third
  column headed `engineering-protocols` (its earlier rows keep their tags), and
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
`crates/loom-governor/src/lib.rs` (`story:engineering-registry-calls-replaced` rewrites its doc
comments). `website/docs/concepts/governor-and-intake.md`: `:14-17` names neither the crate nor a
tag and links to the engineering protocols site, which stays right.
`story:doc-dependency-tags-match-lock` (draft, another epic, no edge to this set): its test
compares the engineering-protocols tag the docs name with `Cargo.lock`, which after this story
holds no engineering-protocols package, so it cannot hold as written and must be revised (compare
with `protocols/engineering/lock.json`) or retired. This story does not edit it.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** the four manifests that name `b10x-canon-engineering`, with `Cargo.lock` and the three one-copy tests that read it — cited (`crates/loom-protocols/Cargo.toml:15`, `crates/loom-governor/Cargo.toml:19`, `crates/loom-intake-slice/Cargo.toml:18`, `crates/loom-intake-router/Cargo.toml:15`; the line numbers are the story's and I checked them at `39f4da88`)
- **Files (manifests):** the four `Cargo.toml` files above — cited. `git grep -n 'b10x-canon-engineering' -- '*.toml'` prints exactly these four lines at `39f4da88`.
- **Files (tests):** `crates/loom-protocols/tests/adversary_w8_pin.rs:78-100` and `:103-118`, `crates/loom-executor/tests/intake_import.rs:26-32` (`SINGLE_COPY`) and `:9-12`, `crates/loom-xtask/tests/pinned_consumer.rs:145` — cited. All four ranges match the story.
- **Files (test doc comments):** `crates/loom-governor/tests/governed_case.rs:9`, `crates/loom-governor/tests/incident_response_slice.rs:23` — cited
- **Files (lock):** `Cargo.lock` — cited. Packages: `b10x-assertion-providers` `:188`, `b10x-canon-engineering` `:214`, `b10x-canon-engineering-assertions` `:226`, `b10x-canon-expr` `:242` (listed, but its removal is out of scope).
- **Symbols:** `SINGLE_COPY`, `manifest_tags`, `one_tag`, `locked_sources`, `every_engineering_entry_carries_the_tag_the_manifest_pins`, `the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags`, `lock_resolves_one_path_copy_of_each_absorbed_crate` — cited
- **Documents:** `AGENTS.md` § Boundary, § Governor, § Connectors; `website/docs/reference/sdk-stability.md:61-69`; `website/docs/guides/move-to-loom.md:49-58`; `CHANGELOG.md` `### Removed` — cited. At `39f4da88`: `AGENTS.md` `:35`, `:520`, `:522`, `:523`, `:530` (committed numbering), `sdk-stability.md:63,67`, `move-to-loom.md:49,57`.
- **Also likely:** none. The story's Files list is closed and I found no other site.
- **Confidence:** high. The story names every file and line, and the tree agrees with the story on every `file:line` I checked. The one caveat is `adversary_w8_pin.rs`, which two other stories also edit (below).
- **Would collide with:**
  - `story:engineering-registry-calls-replaced` (draft) on `crates/loom-governor/tests/governed_case.rs` (its sites `:613,621` and ours `:9`, different lines of one file). Ordered: direct `depends_on`.
  - `story:engineering-protocol-lock` on `crates/loom-protocols/tests/adversary_w8_pin.rs`. That story rewrites `every_engineering_entry_carries_the_tag_the_manifest_pins`, the same test this story rewrites. Ordered: path dependency-removed -> registry-calls-replaced -> lock.
  - `story:engineering-protocols-vendor-command` on `Cargo.lock` (the `loom-xtask` package list) and on `AGENTS.md` (§ Protocol composition and system queries; ours are different sections). Ordered: direct `depends_on`.
  - `story:canon-0-9-0-pin` (proposed, not landed at `39f4da88`) on all four manifests (the tag text), `adversary_w8_pin.rs`, `intake_import.rs`, `pinned_consumer.rs`, `governed_case.rs`, `incident_response_slice.rs`, `move-to-loom.md`, `sdk-stability.md`, `AGENTS.md` § Governor and § Connectors. Ordered: transitive path through lock.
  - `story:co-pinned-consumer-check` (draft) on `website/docs/reference/sdk-stability.md`. It adds a new "Pin together" table; we edit the "Canon release" section and pin table at `:61-69`. NOT ordered: its only edge is `depends_on story:release-names-embedder-breaks` (implemented). It names `b10x-canon` and `b10x-llm-core`, not the engineering crate. Likely a merge-only collision.
  - `story:consumer-sdk-paths` and `story:generated-types-release-surface` (draft) on `sdk-stability.md`, in other sections (their "Used by control-plane" table and generated-types bullet). NOT ordered. Merge-only.
  - `story:breaks-between-releases` (draft) on `website/docs/guides/move-to-loom.md`, where it adds a new section. We edit `:49-58`. NOT ordered. Merge-only.
  - `story:doc-dependency-tags-match-lock` (draft, no `depends_on`) on `AGENTS.md`, `move-to-loom.md` and a new `doc_dependency_tags.rs`. Its test reads the engineering-protocols tag the docs name and compares it with `Cargo.lock`. After this story no such lock package exists, so that test cannot hold as written. NOT ordered: a semantic collision, not only a textual one.
  - `story:tree-run-command` (proposed) lists `Cargo.lock` and conditionally `sdk-stability.md` (its own scope, `:216`). NOT ordered. House-rule file, merge-only.
- **Safety fact:** no source names the crate once `story:engineering-registry-calls-replaced` lands, so deleting the four lines cannot break a build. At `39f4da88` the crate is still used in `crates/loom-protocols/src/lib.rs:160,162`, `crates/loom-governor/src/lib.rs:1724-1736`, and the test and router sites that story lists (`git grep -n 'canon_engineering'`), step 2, unproven. `crates/loom-plugin/src/turn.rs` and `crates/loom-xtask/tests/release_check.rs` are in that story's Files, but I did not read them for `canon_engineering`.

---

Documents land in `AGENTS.md`, `CHANGELOG.md` and two website pages; the rest is manifests, the lock and three tests.
