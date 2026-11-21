---
format: aep.planning-md/3
id: story:canon-0-9-0-pin
kind: story
status: active
title: Loom builds on Canon 0.9.0 and the engineering-protocols tag built on it, one copy of each
summary: case_open, case_outcome and the children form load; on engineering-protocols tag 0.16.0, which pins both Canon crates at 0.9.0
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:substrate-0-21-pin
- depends_on: story:deterministic-default-selector
- depends_on: story:bindings-document
scope:
- confidence: cited
  path: crates/loom-bindings/Cargo.toml
- confidence: cited
  path: crates/loom-executor/tests/intake_import.rs
- confidence: cited
  path: crates/loom-governor/Cargo.toml
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary2_governor.rs
- confidence: cited
  path: crates/loom-governor/tests/canon_decision_bytes.rs
- confidence: cited
  path: crates/loom-governor/tests/case_composition.rs
- confidence: inferred
  path: crates/loom-governor/tests/governed_case.rs
- confidence: inferred
  path: crates/loom-governor/tests/incident_response_slice.rs
- confidence: cited
  path: crates/loom-intake-router/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: crates/loom-protocols/Cargo.toml
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: cited
  path: crates/loom-protocols/tests/adversary_w8_pin.rs
- confidence: inferred
  path: crates/loom-protocols/tests/catalog.rs
- confidence: cited
  path: crates/loom-xtask/tests/pinned_consumer.rs
- confidence: cited
  path: website/docs/guides/move-to-loom.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:46Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-11-21T02:18:31Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"review_outcome":1}}}
---
## Outcome

Loom builds on Canon 0.9.0 (tag `0.9.0`, `8f1f452`) and on engineering-protocols tag `0.16.0`
(`70e6eff48963c9ea178d515ae33e645cf95c9773`), which pins both Canon crates at tag `0.9.0`:
`b10x-canon` (`crates/canon-engineering/Cargo.toml:18` at that tag) and `b10x-canon-expr`
(`crates/canon-engineering-assertions/Cargo.toml:14`). A crate that depends on both resolves one
copy of `b10x-canon`, one of `b10x-canon-expr` and one of `b10x-canon-engineering`. A protocol
that declares `case_open` on an action or a `children` form in a claim registers in the catalog
and with the governor, and a `case_outcome` evidence record reaches Canon's checks.

## Why

`case_open`, `case_outcome` and the `children` form arrived in Canon 0.9.0
(`~/beyond10x/canon/CHANGELOG.md`). Loom pins `0.8.0` (`crates/loom-governor/Cargo.toml:15`,
`crates/loom-protocols/Cargo.toml:14`, `crates/loom-bindings/Cargo.toml:14`) and engineering
`0.15.2`, so it cannot load six of the eight SDC protocols
(`~/beyond10x/sdc/protocol/sdc.protocol.yaml:84`, `sdc-epic.protocol.yaml:105`), and
`story:child-case-runtime` has nothing to run on.
`upstream-blocker:engineering-protocols-on-canon-0-9-0` is `cleared`: session
`engineering-protocols` announced tag `0.16.0` on 2026-11-21, and the coordinator verified the tag
commit and both Canon pins (`approval-record:loom-wave-2026-11-21-w1`).

What the implementor must know:

- Canon 0.9.0 is "Breaking for Rust consumers that match exhaustively over `Predicate` or build
  `Action` or `EvidenceRecord` values with struct literals": `Predicate` gains
  `Children(ChildOutcomes)`, `Action` gains `case_open: Option<CaseOpening>`, `EvidenceRecord`
  gains `child_case: Option<CaseId>` (`~/beyond10x/canon/CHANGELOG.md`, entry 0.9.0). Read
  2026-11-21 at `39f4da88`, the tree has these sites:
  - the one exhaustive match over `Predicate`: `walk` in `evidence_reads`,
    `crates/loom-governor/src/lib.rs:1963-1990`, match at `:1966-1990` (added by
    `story:deterministic-default-selector`, `ba343770`). It needs a `Predicate::Children` arm.
    Canon reads every accepted `case_outcome` record at any revision for that form. That is the
    reading `counted_kinds` already gives a kind only order forms read (`:1880-1925`), so the arm
    can push `case_outcome` with the order forms. This is a reading, not a decision.
  - the one struct literal of `EvidenceRecord`: `crates/loom-governor/tests/adversary2_governor.rs:685`.
  - no struct literal of Canon's `Action` and no other `Predicate` pattern (`git grep` over
    `*.rs` for `Predicate`, `EvidenceRecord {` and `Action {` in every file that names
    `b10x_canon`). The production record path goes through `evidence_from_value`
    (`canon_record`, `lib.rs:1802`) and needs no change.
- The session `engineering-protocols` noted that on ubuntu-26.04 `/usr/bin/printf` is a multi-call
  link, so a test that copies it fails. No Loom test copies `printf` or another coreutils binary:
  searched for `fs::copy`, `hard_link`, `symlink` and a shell `cp` of a path under `/bin` or
  `/usr/bin`, and every copy is of a fixture, a repository file or a workspace file. Tests that run
  a coreutils path in place, which a multi-call binary serves under its own name:
  `crates/loom-intake-slice/tests/adversary_w20261020w2_u4.rs:178`, `:188`, `:303`;
  `crates/loom-intake-slice/tests/adversary_w20261021w6_u3.rs:259`;
  `crates/loom-intake-slice/tests/confined_child.rs:159`, `:1537`. Loom's Linux CI job runs on
  `ubuntu-latest` (`.github/workflows/check.yml:25`).

## ESS first

Exempt: the pin changes no noun and no rule of Loom's specification. The red step is the
story's own test below, which fails on Canon 0.8.0 because `case_open` is an unknown member.

## Acceptance

- The five manifests that name them (`crates/loom-governor`, `crates/loom-protocols`,
  `crates/loom-intake-slice`, `crates/loom-intake-router` and `crates/loom-bindings`) pin
  `b10x-canon` at tag `0.9.0` and `b10x-canon-engineering` at tag `0.16.0`;
  `Cargo.lock` holds one `b10x-canon` and one `b10x-canon-expr`, both from tag `0.9.0`, and one
  `b10x-canon-engineering` from tag `0.16.0`;
  `the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags`
  (`crates/loom-protocols/tests/adversary_w8_pin.rs`) and the single-copy tests in
  `crates/loom-executor/tests/intake_import.rs` and `crates/loom-xtask/tests/pinned_consumer.rs`
  pass.
- A new test, `crates/loom-governor/tests/case_composition.rs`, registers a protocol with one
  `case_open` action and one `children` claim through `CanonGovernor::with_protocol_yaml` and
  through `ProtocolCatalog::add_yaml`, opens a case, and reads a frontier that lists the opening
  action.
- In the same test a `case_outcome` record with `child_case` and result `open` is received and
  the `children` claim reads `unknown`. A `case_outcome` record without `child_case` is refused
  by Canon's own evidence check with the code `missing-child-case`; `loom_governor::evaluate`
  over a snapshot that holds it sets the record aside, as it does every readable record Canon
  refuses (`AGENTS.md` § Governor), so its decision equals the one without the record; the same
  record given to `receive` is held and does not apply. (Corrected on 2026-11-21 during wave
  2026-11-21-w2: the bullet said `evaluate` itself is refused, which would change the set-aside
  rule in a unit that changes no rule.)
- `evidence_reads` has an arm for `Predicate::Children`, and in the same test
  `CanonGovernor::action_facts` answers for the case before and after the `case_outcome` record
  is received.
- `crates/loom-governor/tests/adversary2_governor.rs:685` builds Canon's `EvidenceRecord` with
  `child_case: None`.
- `canon_decision_bytes.rs`: the governor's decision equals Canon 0.9.0's for each fixture.
- Every place that names the pin moves with it: `crates/loom-protocols/src/lib.rs` (entry
  revision), the doc comment in `crates/loom-governor/src/lib.rs`, `AGENTS.md` § Governor and
  § Connectors, `website/docs/reference/sdk-stability.md` (pin table),
  `website/docs/guides/move-to-loom.md`, and a `**Breaking:**` bullet in `CHANGELOG.md`
  (`ProtocolEntry::model` is Canon's `Protocol`), so the next release moves the minor version.
- `task check` exits 0.

## Out of scope

Opening a child case or writing an outcome back (`story:child-case-runtime`). Any change in
engineering-protocols itself. Changing a test because of the ubuntu-26.04 `printf` note: none
copies it.

Order: this story and `story:substrate-0-21-pin` both change
`crates/loom-intake-slice/Cargo.toml`, `Cargo.lock`, `AGENTS.md` and `CHANGELOG.md`; this
story and `story:deterministic-default-selector` both change `crates/loom-governor/src/lib.rs`;
and `story:bindings-document` added a fifth manifest that names `b10x-canon`,
`crates/loom-bindings/Cargo.toml`, which this story moves with the other four. All three are
`implemented` (wave 2026-11-21-w1), so the edges only record that order. Nothing upstream holds
this story any more.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** the `b10x-canon` and `b10x-canon-engineering` pins in the five manifests that name them, plus `crates/loom-governor/src/lib.rs` — cited
- **Files (manifests):** `crates/loom-governor/Cargo.toml:15,19`, `crates/loom-protocols/Cargo.toml:14,15`, `crates/loom-intake-slice/Cargo.toml:18`, `crates/loom-intake-router/Cargo.toml:15` — cited (story Acceptance; tree lines read). Only governor and protocols name `b10x-canon` today. Slice and router name only `b10x-canon-engineering`.
- **Files (fifth manifest):** `crates/loom-bindings/Cargo.toml` — cited (story Order paragraph). The path does not exist in this tree, because `story:bindings-document` creates it.
- **Files (code):** `crates/loom-protocols/src/lib.rs:169` (entry `revision: "0.15.2"`), `crates/loom-governor/src/lib.rs:129-132` (`# Dependencies` doc comment naming Canon `0.8.0` and engineering `0.15.2`) — cited
- **Files (tests):** `crates/loom-protocols/tests/adversary_w8_pin.rs` (comments at :125 and :150 name `0.15.2`), `crates/loom-governor/tests/canon_decision_bytes.rs:8` — cited
- **Files (tests, must keep passing):** `crates/loom-executor/tests/intake_import.rs`, `crates/loom-xtask/tests/pinned_consumer.rs` — cited. The story names them as single-copy tests, and I found no tag literal in either, so they probably need no edit.
- **Files (tests, compile break):** `crates/loom-governor/tests/adversary2_governor.rs:685` builds `EvidenceRecord` with a struct literal. Canon 0.9.0 adds `child_case: Option<CaseId>` (`~/beyond10x/canon/CHANGELOG.md`, "Breaking for Rust consumers"). The literal therefore will not compile and needs `child_case: None` — cited (literal read in tree, break read in Canon's changelog)
- **Files (test doc comments):** `crates/loom-governor/tests/governed_case.rs:9`, `crates/loom-governor/tests/incident_response_slice.rs:23` — inferred. They name `0.15.2` in prose, and the 0.8.0 pin commit `8a99f26c` edited the same two files.
- **Files (tests, catalog list):** `crates/loom-protocols/tests/catalog.rs` — inferred. The 0.15 pin edited it because the catalog changed, and the engineering tag built on 0.9.0 may change the catalog again.
- **Files (new test):** one new file under `crates/loom-governor/tests/`. It holds the `case_open` and `children` registration through `CanonGovernor::with_protocol_yaml` and `ProtocolCatalog::add_yaml`, plus the `case_outcome` records. — inferred. The story names no path. `crates/loom-governor/tests/adversary_w20261021w7_completion.rs:81-93` already uses both entry points together.
- **Symbols:** `ProtocolEntry::model`, `CanonGovernor::with_protocol_yaml`, `ProtocolCatalog::add_yaml`, `ProtocolCatalog::engineering`, `canon_record` (`crates/loom-governor/src/lib.rs:1699`), `EvidenceRecord`, `check_evidence` (`:1363`) — cited
- **Also likely:** none beyond the lines above.
- **Documents:** `website/docs/reference/sdk-stability.md:69` (pin table row `| 0.57.2 | 0.8.0 | 0.15.2 |`), `website/docs/guides/move-to-loom.md:49,57` — cited. Whether the table gets a new row or the current row changes is inferred, since the release owns that row.
- **House-rule files:** `AGENTS.md` (§ Governor, lines 506-509; § Connectors, lines 511-516), `CHANGELOG.md` (`**Breaking:**` bullet), `Cargo.lock`.
- **Confidence:** high for the manifests, `lib.rs` pins, docs and the one compile break, because the story and the tree both state them. Medium for the new test's path and for any change to `crates/loom-governor/src/lib.rs` beyond the doc comment (see "could not establish").
- **Would collide with:** any unit touching these surfaces:
  - `Cargo.lock`, `AGENTS.md` and `CHANGELOG.md`.
  - The `[dependencies]` of `crates/loom-intake-slice/Cargo.toml`, `crates/loom-governor/Cargo.toml`, `crates/loom-protocols/Cargo.toml`, `crates/loom-intake-router/Cargo.toml` and `crates/loom-bindings/Cargo.toml`.
  - `crates/loom-governor/src/lib.rs` (evaluation and evidence code).
  - `crates/loom-governor/tests/adversary2_governor.rs`.
  - The pin table in `website/docs/reference/sdk-stability.md`.
  - `crates/loom-protocols/tests/adversary_w8_pin.rs`.

  The story's Order paragraph already names `story:substrate-0-21-pin`, `story:deterministic-default-selector` and `story:bindings-document`.
- **Safety fact:** every `b10x-canon` consumer that builds Canon values by struct literal is in `crates/loom-governor/tests/adversary2_governor.rs:685` (`EvidenceRecord`). The production path (`canon_record`, `lib.rs:1699`) goes through `evidence_from_value`, so it needs no literal change. No other `Action {` or `Predicate::` literal over Canon types turned up in `crates/` — step 3 (`git grep`), unproven
