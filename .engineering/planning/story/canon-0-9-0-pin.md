---
format: aep.planning-md/3
id: story:canon-0-9-0-pin
kind: story
status: implemented
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
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-bindings/Cargo.toml
- confidence: cited
  path: crates/loom-governor/Cargo.toml
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary2_governor.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_w20261121w1_u3.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_w20261121w1_u3_pass2.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_w20261121w2_u1.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_w20261121w2_u1_pass2.rs
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
- confidence: cited
  path: docs/contracts/loom-action-selection.md
- confidence: cited
  path: website/docs/guides/move-to-loom.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:46Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-11-21T02:18:31Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-11-21T04:16:30Z", actor: "human:timo", revision: 16, decided_on: {"recorded":{"test_result":1,"review_outcome":3,"verification":1}}}
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

Rewritten 2026-11-21 at the close of wave 2026-11-21-w2 from the unit's commits (`git diff --stat 61fd500b c9816cfd`: 23 files, 1217 insertions, 57 deletions). Every line is cited from that diff.

- **Primary surface:** `crates/loom-governor` — cited, 11 of the 23 changed files are under it (manifest, `src/lib.rs`, three new test files, six existing test files)
- **Files (manifests):** `crates/loom-governor/Cargo.toml` (4 lines), `crates/loom-protocols/Cargo.toml` (4 lines), `crates/loom-intake-slice/Cargo.toml`, `crates/loom-intake-router/Cargo.toml`, `crates/loom-bindings/Cargo.toml` (2 lines each) — cited, `b10x-canon` moves to tag `0.9.0` (`8f1f4527`) and `b10x-canon-engineering` to tag `0.16.0` (`70e6eff4`)
- **Files:** `Cargo.lock` — cited, 20 lines changed for the two new revisions
- **Files:** `crates/loom-governor/src/lib.rs` — cited, 79 lines: the `# Dependencies` doc names the two versions, the claim mapping handles Canon 0.9.0's `children` form, and `evaluate` keeps the first `case_outcome` record per child among the records Canon takes alone, in one pass, and sets later ones aside
- **Files:** `crates/loom-protocols/src/lib.rs` — cited, 2 lines: the engineering entry's `revision`
- **Files (new tests):** `crates/loom-governor/tests/case_composition.rs` (512 lines, the story's case-composition test), `crates/loom-governor/tests/adversary_w20261121w2_u1.rs` (346 lines) and `crates/loom-governor/tests/adversary_w20261121w2_u1_pass2.rs` (257 lines), the two adversary passes' cases, committed unchanged
- **Files (existing tests):** `crates/loom-governor/tests/adversary2_governor.rs` (1 line, the `EvidenceRecord` literal gains `child_case`), `crates/loom-governor/tests/adversary_w20261121w1_u3.rs` (3 lines), `crates/loom-governor/tests/adversary_w20261121w1_u3_pass2.rs`, `crates/loom-governor/tests/canon_decision_bytes.rs`, `crates/loom-governor/tests/governed_case.rs`, `crates/loom-governor/tests/incident_response_slice.rs` (2 lines each), `crates/loom-protocols/tests/adversary_w8_pin.rs` (7 lines) — cited
- **Files (documents):** `AGENTS.md` (15 lines: the pins in § Governor and the `evaluate` set-aside rule for a second `case_outcome` record), `CHANGELOG.md` (2 lines: the move and the reserved kind `case_outcome`), `docs/contracts/loom-action-selection.md` (3 lines), `website/docs/guides/move-to-loom.md` (4 lines), `website/docs/reference/sdk-stability.md` (1 line, the pin table) — cited
- **Symbols:** no public symbol added or removed — cited; `loom_governor::evaluate` keeps its signature, and its documented behaviour gains the first-per-child rule
- **Not predicted and touched:** `crates/loom-governor/tests/adversary_w20261121w1_u3.rs`, `crates/loom-governor/tests/adversary_w20261121w1_u3_pass2.rs`, `docs/contracts/loom-action-selection.md`, the two adversary test files
- **Predicted and not touched:** `crates/loom-executor/tests/intake_import.rs`, `crates/loom-xtask/tests/pinned_consumer.rs` (both named as tests that must keep passing), `crates/loom-protocols/tests/catalog.rs` (inferred; the catalog list did not change)
