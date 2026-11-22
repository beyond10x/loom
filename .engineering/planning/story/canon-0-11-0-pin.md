---
format: aep.planning-md/3
id: story:canon-0-11-0-pin
kind: story
status: draft
title: Loom builds on Canon 0.11.0, one copy, and every decision document it hands out is Canon 0.11.0's
summary: five manifests and Cargo.lock to tag 0.11.0; four compile fixes (Predicate::Descended arm, CaseArtifact.descends_from twice, EvidenceRecord.item); decision documents name 0.11.0, frontier ids change; when, revised_by, records_together and descended register; descended stays unknown in Loom
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:decision-requests-in-the-runtime
- depends_on: story:canon-0-10-0-pin
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
  path: crates/loom-executor/Cargo.toml
- confidence: cited
  path: crates/loom-governor/Cargo.toml
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/action_facts.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary2_governor.rs
- confidence: cited
  path: crates/loom-governor/tests/canon_0_10_0.rs
- confidence: cited
  path: crates/loom-protocols/Cargo.toml
- confidence: cited
  path: crates/loom-tree/Cargo.toml
- confidence: cited
  path: docs/contracts/loom-action-selection.md
- confidence: cited
  path: website/docs/guides/move-to-loom.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 17
---
## Outcome

Loom builds on Canon 0.11.0 (tag `0.11.0`, `e81f970decf228931d984eddf97975030045b3be`), one
copy. The five manifests that name `b10x-canon` name tag `0.11.0`, and `Cargo.lock` holds one
`b10x-canon`, from that tag. Every `canon-decision/1` document Loom hands out is Canon 0.11.0's.

This story is a re-pin. It adopts no new Canon construct into Loom's own behaviour (D1).

The five manifest lines, all `tag = "0.10.0"` today:

- `crates/loom-bindings/Cargo.toml:14`
- `crates/loom-executor/Cargo.toml:10`
- `crates/loom-governor/Cargo.toml:18`
- `crates/loom-protocols/Cargo.toml:14`
- `crates/loom-tree/Cargo.toml:14`

`Cargo.lock:179-181` is the `b10x-canon` package at `0.10.0`, `2c7a1ea6`.

What holds afterwards:

- **The bytes.** Every `canon-decision/1` document (`CanonGovernor::decision_document`, the
  `canon` member of `loom_governor::evaluate`, `b10x-loom evaluate`) has
  `explanation.computed_from.semantics.canon` equal to `0.11.0`. Loom gives no
  `descends_from`, so `computed_from.case.artifacts` gains nothing.
- **Frontier ids.** The frontier id the governor issues changes again, because it hashes the
  rendered decision. No store holds one, as at 0.10.0 (`canon-0-10-0-pin.md`, Acceptance,
  restart).
- **New members register.** A protocol that uses a 0.11.0 member registers through
  `ProtocolCatalog::add_yaml` and `CanonGovernor::with_protocol_yaml`, and Canon decides it. The
  members are `when` on a decision requirement, `revised_by` on an artifact, `records_together`
  on an action, and a `descended` form. Canon 0.10.0 refused each as an unknown field (Canon
  `CHANGELOG.md` § 0.11.0, lines 42-44).
- **`when` passes through.** A decision requirement with a `when` counts only while the `when`
  is true. That is Canon's evaluation, and the governor's document shows it; Loom adds no
  behaviour of its own (`story:outcome-decision-when-reads-the-run-count` does).
- **What Loom does not read** (D2):
  - Loom keeps no revision ancestry, so a case Loom hands Canon has no `descends_from`. A
    `descended` form is therefore `unknown` in every Loom case.
  - `records_together` is read by `canon check` and `canon simulate`; Loom's runner does not
    enforce it.

## Why

`story:outcome-decision-when-reads-the-run-count` and Runbook's use of a condition on a
decision need `when` (`CANON-OUTCOME-003`). Canon 0.10.0 refuses the key.

### What 0.11.0 breaks for Loom

Each item below was read in the plan tree at `c260bdd1` with `git grep`, against Canon's
"What breaks" list (Canon `CHANGELOG.md` § 0.11.0, lines 14-27). No build was run.

**Compile breaks (4):**

1. `crates/loom-governor/src/lib.rs:2281-2306`, `evidence_reads`'s `walk`, is an exhaustive
   `match` over `Predicate` with no wildcard. `Predicate` gains `Descended(Box<EvidenceDescended>)`
   (`first`, `then`: each a kind and a result; `subject`), so this is E0004.
   - Fix (D3 a): `first.kind` is an order reader (whole history), and `then.kind` is a match
     reader (bound to the subject's current revision), both with `subject`.
2. `crates/loom-governor/src/lib.rs:2061-2063` builds `CaseArtifact { revision }`. `CaseArtifact`
   gains `descends_from: Option<Vec<Revision>>`, so this is E0063. Fix: `descends_from: None`.
3. `crates/loom-governor/tests/adversary2_governor.rs:689` builds `EvidenceRecord { … }`.
   `EvidenceRecord` gains `item: Option<ItemId>`, so this is E0063. Fix: `item: None`.
4. `crates/loom-governor/tests/adversary2_governor.rs:720` builds `CaseArtifact { … }`. Same
   break as 2, same fix.

**Read and not broken:**

- `OutcomeRequirement`: the one use is `if let OutcomeRequirement::Predicate(predicate)`
  (`lib.rs:2326`). Loom never matches `Decision(_)` and never calls `decision()`.
  - `evidence_reads` does not walk a decision's `when`, and needs not. A `when` holds claims and
    inputs only. The claims it names are walked through `ir.claims` (`lib.rs:2313-2315`), and an
    input reads no record.
- `Ir::artifacts` (now `ir::Artifact`, no longer `Described`): Loom reads its keys only, at
  `lib.rs:1928-1940` and `crates/loom-governor/src/children.rs:707-709`.
- `ir::Action`, `validate::Problem` and `simulate::Report`: Loom builds none of them by literal
  and matches none of them. `simulate` is not called. The `Problem` matched in
  `crates/loom-commission-xtask/src/main.rs:600-607` is that crate's own type.
- `model::Artifact` gains `revised_by`, and `model::Action` gains `records_together`:
  - `crates/loom-protocols/src/lib.rs:146-165` (`clock_compatible`) clones the protocol and
    clears `description` on each declaration, with no struct literal. It compiles.
  - It now also compares `revised_by` and `records_together`, which the system clock protocol
    does not use.
- `EvidenceRecord`, as Loom builds it in source, comes through `evidence_from_value`
  (`lib.rs:2110-2121`, `canon_record`) and not a literal. An `item` member in a record's facts
  was refused by 0.10.0 as an unknown field and is now read. Loom's program records write no
  `item` (`crates/loom-bindings/src/program.rs:657-664`).
- The remaining uses of `b10x_canon` in `loom-bindings`, `loom-executor`, `loom-protocols` and
  `loom-tree` are `model::parse`, `validate::validate`, `validate::check_arguments`,
  `ir::compile`, `ir::check_openings`, `model::Protocol` and `model::Instant`. Their signatures
  are not in the break list.

**Tests and files whose bytes move:**

- `crates/loom-governor/tests/canon_0_10_0.rs:272-280`, `the_decision_document_names_canon_0_10_0`,
  asserts `json!("0.10.0")`. It goes red on the move and becomes
  `the_decision_document_names_canon_0_11_0` (Acceptance 1).
- `crates/loom-governor/tests/canon_decision_bytes.rs:29` and
  `crates/loom-governor/tests/decision_document.rs:101` compare with what the same Canon renders,
  so they pass unchanged.
- No tracked file outside `.engineering/` holds a decision document, a `computed_from` with a
  version, or a frontier id literal (`git grep`).
- No file under `generated/` or `ess/` names Canon's version. `task drift` and `task docs-check`
  stay green with no regeneration; Acceptance 8 runs both to show it.

## Decisions

- **D1. What the move adopts.** Default **a**.
  - **a.** Re-pin only: the manifests, `Cargo.lock`, the four compile fixes, the tests and the
    documents.
  - **b.** Also have Loom use `when`. No: that is
    `story:outcome-decision-when-reads-the-run-count`.
- **D2. A protocol that uses `descended`.** Default **a**.
  - **a.** It registers, and the form stays `unknown` in Loom. `AGENTS.md` says so. This is how
    the 0.10.0 move treated a `children` form's `subject` and `revision`.
  - **b.** It is refused at registration. That is a refusal a later story would remove when Loom
    keeps revision ancestry.
- **D3. How `evidence_reads` reads `descended`.** Default **a**.
  - **a.** `first` as an order reader and `then` as a match reader, which is Canon's rule: `then`
    is about the artifact's current revision, and `first` is about revisions it descends from.
  - **b.** Both as order readers, which is simpler. A `then` record about an old revision would
    then still count as present, so the producing action would not look useful again after the
    subject moved.

## ESS first

Exempt: the move changes no noun and no rule of Loom's specification.
`ess/domains/evaluation.yaml` holds the decision document as the whole document Canon wrote, as
at 0.10.0.

**Commit order:**

1. The tests of Acceptance 1 to 4 alone. They are red on 0.10.0 (the version reads `0.10.0`;
   each new member is an unknown field).
2. The five manifest lines, `Cargo.lock` (`cargo update -p b10x-canon`) and the four compile
   fixes. Everything turns green.
3. The documents.

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step (`task test-step`) ends within 120 s.
- No test makes a network request, a model call or a clock call, and no test reads a credential
  file.
- Fixtures are POSIX sh, never Python.
- A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30
  minutes).
- Each new test is timed and named with its time in the report if it takes over 5 s.

**Placement.** The tests extend `crates/loom-governor/tests/canon_0_10_0.rs`, or its module
under `tests/main.rs` if `story:one-test-binary-per-crate` merged first. No new test binary is
added. The protocols are written in the test, in the forms of Canon's fixtures at tag `0.11.0`
(`conformance/scenarios/decision-condition.yaml`, `revision-ordering.yaml`), with one catalog per
test run.

1. `the_decision_document_names_canon_0_11_0`. It replaces `…_0_10_0` (`:272-280`): the
   `canon` member of `evaluate` has `explanation.computed_from.semantics.canon` equal to
   `"0.11.0"`. Failing first: it reads `0.10.0`.
2. `a_protocol_with_0_11_0_members_registers_and_is_decided`.
   - The protocol has `revised_by` on an artifact, `records_together` on an action, a claim over
     a `descended` form, and an outcome `requires: {decision: d, when: {claim: c}}`.
   - It registers through both `ProtocolCatalog::add_yaml` and
     `CanonGovernor::with_protocol_yaml`.
   - Failing first: 0.10.0 refuses `revised_by` as an unknown field.
3. `a_decision_counts_only_while_its_when_holds`.
   - Before a record makes `c` true, `CanonGovernor::decision_document` lists the outcome as
     blocked by `c`, with no `d` under `pending_decisions`.
   - After the record, `d` is listed.
   - Failing first: the protocol does not register.
4. `a_descended_form_is_unknown_in_a_loom_case`. With records of both kinds about the current
   revision, the claim over the `descended` form is `unknown`, because Loom gives no
   `descends_from` (D2 a). Failing first: the protocol does not register.
5. `descended_reads_then_at_the_current_revision` (D3 a), in
   `crates/loom-governor/tests/action_facts.rs`.
   - A `then` record about `r1` counts for the producing action's facts at `r1`, and does not
     count once the subject moves to `r2`.
   - A `first` record about `r1` still counts at `r2`.
   - Failing first: `lib.rs` does not compile on 0.11.0 without the arm. Under D3 b, the second
     half of the first assertion fails.
6. `the_lockfile_holds_one_canon_and_no_engineering_package`
   (`crates/loom-protocols/tests/adversary_w8_pin.rs:128`) passes.
   `git grep -n 'canon", tag = "0.10.0"' -- '*Cargo.toml'` prints nothing, and
   `git grep -c 'canon", tag = "0.11.0"' -- '*Cargo.toml'` counts five.
7. Unchanged and passing:
   - `canon_decision_bytes.rs:29`;
   - `decision_document.rs:101`;
   - every other test of the five crates.
8. **Drift and docs.** `task drift` and `task docs-check` exit 0, with no change under
   `generated/` and no change to a generated page under `website/docs/reference/`. The
   hand-written `website/docs/reference/sdk-stability.md` is changed by item 9.
9. **Documents.**
   - `AGENTS.md` § Governor (`:717-735`) names tag `0.11.0` and all five manifests (it names
     three today, `:717-718`). It says what 0.11.0 lets a protocol declare that Loom does not
     read: `descended` without `descends_from`, and `records_together`.
   - `CHANGELOG.md` `[Unreleased]` gets a `**Breaking:**` bullet: Canon `0.10.0` -> `0.11.0`.
     `ProtocolEntry::model` is 0.11.0's `Protocol`, decision documents name `0.11.0`, and
     frontier ids change.
   - `website/docs/reference/sdk-stability.md:72` reads `| unreleased | 0.11.0 | 0.16.0 |`.
   - `website/docs/guides/move-to-loom.md:59` names tag `0.11.0`.
   - `docs/contracts/loom-action-selection.md:160-165` says how `descended` is read (D3).
10. **Gate.** `task check` exits 0.

## Out of scope

- Using `when` for a Loom claim (`story:outcome-decision-when-reads-the-run-count`).
- Keeping revision ancestry and giving Canon `descends_from`.
- Enforcing `records_together` in the runner.
- An `item` on the records Loom writes.
- Calling `canon simulate`, `floor` or `mutate`.
- Runbook's own Canon move.

## Scope

Read at `c260bdd1`; Canon read at tag `0.11.0`, read-only. **Cited** means read at the line
named; **inferred** means a reading that could be wrong.

- The five manifests (lines above) and `Cargo.lock` (`:179-181`) — cited.
- `crates/loom-governor/src/lib.rs` (`:2061-2063`, `:2281-2306`) — cited.
- `crates/loom-governor/tests/adversary2_governor.rs` (`:689`, `:720`) — cited.
- `crates/loom-governor/tests/canon_0_10_0.rs` (`:272-280`) and
  `crates/loom-governor/tests/action_facts.rs` — cited.
- `crates/loom-protocols/tests/adversary_w8_pin.rs` (`:128`), read and not changed — cited.
- `AGENTS.md`, `CHANGELOG.md`, `website/docs/reference/sdk-stability.md`,
  `website/docs/guides/move-to-loom.md` and `docs/contracts/loom-action-selection.md` — cited.
- **Not touched:** `ess/`, `generated/`, `protocols/` — cited from the grep. A compile error the
  grep missed is fixed in the crate it names, and the report lists it.

## Edges

- `depends_on story:canon-0-10-0-pin` (active): the pin this moves, and its test file.
- **Order:** first of the next batch, and small. `story:outcome-decision-when-reads-the-run-count`
  carries `depends_on story:canon-0-11-0-pin` (recorded). `story:outcome-when-reads-the-run-count`
  works on Canon 0.10.0 and does not wait for this story.
- **Would collide with**, no edge (the later one rebases):
  - On `Cargo.lock`:
    - `story:engineering-protocols-dependency-removed` (active), which also touches
      `crates/loom-governor/Cargo.toml` and `crates/loom-protocols/Cargo.toml`;
    - `story:drop-the-substrate-git-feature` (active);
    - `story:protocol-argument-schema` (active), which also touches
      `crates/loom-executor/Cargo.toml`.
  - On `crates/loom-governor/Cargo.toml`: `story:decision-request-file-store` (active).
  - `story:tree-library-exports-what-an-engine-needs` (active), on `crates/loom-tree/Cargo.toml`
    (this story edits `:14`) and on `website/docs/reference/sdk-stability.md` (this story edits
    `:72`; that story edits `:33-35` and `:112`). The regions are disjoint, so a textual merge
    suffices.
  - On `AGENTS.md`: `story:cancel-a-running-bound-program` and
    `story:person-action-runs-a-program` both scope it. This story edits only § Governor
    (`:717-735`).
  - On `crates/loom-governor/tests/*.rs`: `story:one-test-binary-per-crate` (active), which turns
    the crate's 33 test binaries into one. This story's tests go wherever that story has put
    `canon_0_10_0.rs`.
  - `story:outcome-when-reads-the-run-count` records neither `crates/loom-governor` nor a Canon
    manifest in its scope, so it does not overlap this story.
