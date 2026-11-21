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
revision: 12
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

## Scope

Derived 2026-11-21 by `story-scoper` at `aa943153`. Every line is **cited** (read from the story or
the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** the four manifest lines naming `b10x-canon-engineering`, with `Cargo.lock`
  and the three one-copy tests: cited (`crates/loom-protocols/Cargo.toml:15`,
  `crates/loom-governor/Cargo.toml:19`, `crates/loom-intake-slice/Cargo.toml:18`,
  `crates/loom-intake-router/Cargo.toml:15`; `git grep -n 'b10x-canon-engineering' -- '*.toml'`)
- **Files (tests):** `crates/loom-protocols/tests/adversary_w8_pin.rs:79-113` and `:115-131`;
  `crates/loom-executor/tests/intake_import.rs:9-12`, `:26-32` (`SINGLE_COPY`), `:166`, `:190`;
  `crates/loom-xtask/tests/pinned_consumer.rs:145`: cited
- **Files (test doc comments):** `crates/loom-governor/tests/governed_case.rs:9`,
  `crates/loom-governor/tests/incident_response_slice.rs:23`: cited
- **Files (lock):** `Cargo.lock`: cited. Packages `b10x-assertion-providers` `:188`,
  `b10x-canon-engineering` `:214`, `b10x-canon-engineering-assertions` `:226`, `b10x-canon-expr`
  `:242` (only `:232` names it). Dependency lists `:615`, `:648`, `:660`, `:758`
- **Symbols:** `SINGLE_COPY`, `manifest_tags`, `one_tag`, `locked_sources`,
  `every_engineering_entry_carries_the_tag_the_manifest_pins`,
  `the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags`,
  `lock_resolves_one_path_copy_of_each_absorbed_crate`: cited
- **Documents:** `AGENTS.md` § Boundary `:35`, § Governor `:556-559`, § Connectors `:567-569`;
  `website/docs/reference/sdk-stability.md:60-72`; `website/docs/guides/move-to-loom.md:49-58`;
  `CHANGELOG.md` `### Removed` (new heading under `[Unreleased]`): cited
- **Also likely:** `crates/loom-protocols/tests/engineering_lock.rs:5`, a doc comment that says the
  files equal "the registry of the pinned `b10x-canon-engineering`": inferred, false after removal
- **Confidence:** high: the story names every file, and the tree gave every line above
- **Would collide with:** `Cargo.lock` (`story:decision-request-entity` adds a package;
  `story:bound-program-result-table` edits the `b10x-loom-bindings` block; different hunks);
  `AGENTS.md`, `sdk-stability.md`, `move-to-loom.md` (`co-pinned-consumer-check`,
  `consumer-sdk-paths`, `breaks-between-releases`, `doc-dependency-tags-match-lock`; all draft,
  unordered, other sections; the last compares docs with `Cargo.lock` and breaks semantically)
  In wave 7: `story:bound-program-result-table` edits `AGENTS.md` § Bindings document and adds
  `CHANGELOG.md` bullets, and `story:decision-request-entity` adds `CHANGELOG.md` bullets; this
  story edits § Boundary, § Governor and § Connectors and adds `### Removed`. Other sections,
  merged by keeping both sides: inferred
- **Safety fact:** no Rust source names the crate: `git grep -n -e canon_engineering -e
  assertion_providers` outside `.engineering/` prints nothing; step 2 is unproven (no `cargo tree`)
