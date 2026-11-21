---
format: aep.planning-md/3
id: story:canon-0-10-0-pin
kind: story
status: active
title: Loom builds on Canon 0.10.0, one copy, and every decision document it hands out is Canon 0.10.0's
summary: three manifests and Cargo.lock to tag 0.10.0; no source change; decision documents gain pending_decisions and missing_evidence, frontier ids change; imports refused
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:decision-requests-in-the-runtime
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
- confidence: inferred
  path: crates/loom-governor/tests/canon_0_10_0.rs
- confidence: cited
  path: crates/loom-protocols/Cargo.toml
- confidence: cited
  path: docs/contracts/loom-action-selection.md
- confidence: cited
  path: website/docs/guides/move-to-loom.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T22:20:08Z", actor: "agent:sdc", revision: 6, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w8"}
- {from: "proposed", to: "active", at: "2026-11-21T22:20:09Z", actor: "agent:sdc", revision: 7, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w8"}
---
## Outcome

Loom builds on Canon 0.10.0 (tag `0.10.0`, `2c7a1ea6a9dc2e19633fb225290c826b1c9988ce`). The three
manifests that name `b10x-canon` (`crates/loom-governor/Cargo.toml`,
`crates/loom-protocols/Cargo.toml`, `crates/loom-bindings/Cargo.toml`) name tag `0.10.0`, and
`Cargo.lock` holds one `b10x-canon`, from that tag. No Loom source file changes for the move:
Loom builds no struct literal of a Canon type that 0.10.0 widened and matches no Canon enum that
gained a variant (§ Why).

What holds afterwards:

- Every `canon-decision/1` document Loom hands out (`CanonGovernor::decision_document`, the
  `canon` member of `loom_governor::evaluate`, `b10x-loom evaluate`) is Canon 0.10.0's: its
  `explanation.computed_from.semantics.canon` reads `0.10.0`, an approval-required action is
  listed under `pending_decisions.authority`, and an admissible action that may produce evidence
  is listed under `missing_evidence`. The governor still supplies no `canon-authority/1` document
  (`Supplied::authority` stays `None`).
- The frontier id the governor issues for a case at a revision is a new value, because it hashes
  the rendered decision. No store Loom writes today holds a frontier id or a decision digest, so
  nothing stored goes stale (§ Acceptance, restart).
- A protocol that uses a 0.10.0 member registers through `ProtocolCatalog::add_yaml` and
  `CanonGovernor::with_protocol_yaml` and is decided by Canon: a `scope` on a capability
  requirement, `supplied_by` on an evidence kind, `author`, `revises` and `decided_by`, and a
  `children` form with `revision: current` or `subject`. Canon 0.9.0 refused each as an unknown
  member.
- A protocol with `imports` is refused at registration, naming Canon's `missing-import`: Loom
  hands Canon no imported document.
- Loom's own refusal of a repeated evidence id in `evaluate` (`duplicate-identifier`, "evidence
  `<id>` is given more than once") stays as it is, though Canon 0.10.0 counts an equal copy once.
- What a planned story can now call, listed and not built here:
  - the `canon-authority/1` entry keyed by capability, scope, case revision and principal, with
    `require_approval` and the precedence denied over `require_approval` over granted:
    `story:approvals-supplied-to-canon-evaluation`;
  - `b10x_canon::validate::check_arguments` (one call's arguments against an action's schema):
    `story:protocol-argument-schema`;
  - `pending_decisions`: `story:requests-from-canon-pending-decisions`;
  - `ground_revisions` and `ground_evidence` on authority and explicit decisions, and
    `stale_grounds`: `story:decided-records-carry-grounds-to-canon`,
    `story:authority-records-bound-and-grounded-in-canon`;
  - `children` with `subject` and `revision: current`, read only from the records a runner
    writes about the `case_open` subject: `story:child-case-runtime`;
  - `supplied_by`, under which a record may name artifacts the protocol does not declare:
    `story:record-copy-between-cases`;
  - `author: judgement` and `decided_by: judgement`, which mark the actions an agent or a person
    decides: `story:bound-agent-node`, `story:decider-port-and-grounding-check`;
  - `missing_evidence`, Canon's own list of kinds an admissible action still lacks:
    `story:selector-stop-names-set-aside-record`;
  - `b10x_canon::simulate::simulate` and `canon simulate`: `story:bindings-check-command`.

## Why

Runbook writes `canon-authority/1` entries as `{capability, decision}` and refuses a second
decision on a capability "Canon refuses a capability decided twice"
(`~/beyond10x/sdc/driver/src/main.rs:1812-1834`); who decided is kept outside Canon,
"canon-authority/1 has no principal; the event log keeps who decided" (`main.rs:1835-1843`), and
the file goes to `canon evaluate --authority` (`main.rs:408-420`). Canon 0.9.0 reads an entry as
capability and decision only and refuses a capability given twice
(`crates/canon/src/eval/authority.rs:54-76` at `0.9.0`). Canon 0.10.0 keys an entry by
`(CapabilityId, Scope, Option<Revision>, Option<Principal>)` (`eval/authority.rs:65` at `0.10.0`),
holds them in `Authority` (`:77`) and decides by `max_by_key(precedence)` (`:131`; `Grant` and
`precedence`, `model/authority.rs:84-108`). `story:approvals-supplied-to-canon-evaluation` hands
Canon one entry per held approval with its principal and case revision; at 0.9.0 two principals'
approvals of one capability cannot be two entries.

Loom pins `0.9.0`: `crates/loom-governor/Cargo.toml:15`, `crates/loom-protocols/Cargo.toml:14`,
`crates/loom-bindings/Cargo.toml:14`, `Cargo.lock:188-190` (`8f1f4527`). Loom 0.58.0 was tagged
before Canon 0.10.0; `architecture-decision-record:canon-release-tracking` (accepted) has the next
Loom release link the Canon release current when it is cut. The engineering protocols are vendored
files, so Canon moves without an engineering-protocols release
(`crates/loom-protocols/tests/adversary_w8_pin.rs:124-126`).

What Canon 0.10.0 breaks (`~/beyond10x/canon/CHANGELOG.md`, `## [0.10.0]`, "What breaks"), and
what each does to Loom, read at `8a8fad3b`:

- New members on `Protocol`, `EvidenceKind`, `Action`, `CapabilityRequirement`,
  `EvidenceProduction`, `ChildOutcomes`, `AuthorityDecision`, `ExplicitDecision`, `Decision`,
  `ir::Action`, `ir::EvidenceKind`, `conform::Scenario`; `Grant::RequireApproval`; thirteen
  `validate::Problem` variants. Loom has no struct literal of any of them and no match on `Grant`
  or `Problem`. Its one match over `Predicate` (`crates/loom-governor/src/lib.rs:1995-2019`)
  takes `Predicate::Children(_)` whole, and `Predicate` gained no variant. The Canon values Loom
  builds are `Case`, `CaseArtifact` (`lib.rs:1767-1790`) and `EvidenceRecord`
  (`tests/adversary2_governor.rs:689`), which 0.10.0 left unchanged (no diff under
  `crates/canon/src/model/case.rs` or `evidence.rs`). The functions Loom calls keep their
  signatures: `model::parse`, `validate::validate`, `ir::compile`, `eval::evaluate`,
  `evaluate_with`, `Supplied`, `case_from_value`, `evidence_from_value`, `read_case`,
  `read_evidence`, `render`. Loom's public API carries Canon's `Protocol`
  (`ProtocolEntry::model`, `crates/loom-protocols/src/lib.rs:64`; `b10x_loom_bindings::check`,
  `crates/loom-bindings/src/load.rs:30`), so a consumer that builds one by struct literal breaks.
- `canon-decision/1` bytes: `semantics.canon` (`CANON_VERSION`,
  `crates/canon/src/explain/mod.rs:106`, `:324`) and the sections `missing_evidence` and
  `pending_decisions`
  (`crates/canon/src/model/decision.rs:53`, `:70`). Loom renders the document at three lines
  (`lib.rs:1416`, `:1417` for the frontier id, `:2430` for `evaluate`'s `canon`); the frontier id
  is FNV-1a over case, revision and rendered bytes (`lib.rs:2468-2474`). Eleven test files name
  a decision document or Canon's `render`; the byte tests compare it with what the same Canon
  renders (`tests/canon_decision_bytes.rs:29`, `tests/decision_document.rs:101`), and no tracked
  file outside `.engineering/` holds `computed_from` with a version or a frontier id literal
  (`git grep`). Loom commits no golden decision file.
- A repeated evidence id: Loom refuses it before Canon sees it (`lib.rs:2322-2332`), so Canon's
  new rule and its changed message reach no Loom output
  (`tests/evaluate_adversary.rs:190`).
- Argument-schema numbers by exact value: no protocol Loom loads declares `arguments:`
  (`protocols/`), so nothing Loom registers changes.
- `canon diff` classes: Loom calls no `diff`.

What 0.10.0 adds that Loom does not read, kept as it is here: Loom's frontier carries one
capability id per action (`lib.rs:1413`, `representable` `:1675-1682`), so a requirement's scope
reaches the frontier only inside Canon's reason; `evidence_reads` counts a `children` form's
`case_outcome` records about any artifact at any revision (`lib.rs:2018`), where Canon 0.10.0
reads only those about the form's `subject` and, with `revision: current`, at its current
revision. Refusing such protocols at registration was the other form; it costs a refusal that
`story:child-case-runtime` and the approvals stories would remove again.

The size of the move: `git -C ~/beyond10x/canon diff --stat 0.9.0 0.10.0 -- crates` is 91 files,
21787 insertions, 1169 deletions; under `crates/canon/src` 34 files, 11290 insertions, 785
deletions. The 0.9.0 move (`story:canon-0-9-0-pin`) changed 23 Loom files, 1217 insertions, 57
deletions, most of it new tests; it needed one new `Predicate` arm and one struct literal field.
Canon has no feature table, no `rust-version`, edition 2024 at both tags, and no new dependency
(`crates/canon/Cargo.toml` at both tags). No Loom workflow installs a `canon` binary
(`.github/workflows/`).

## ESS first

Exempt: the move changes no noun and no rule of Loom's specification. `ess/domains/evaluation.yaml`
holds the decision document as the whole document Canon wrote (`:114-116`), not its sections.

The first commit adds `crates/loom-governor/tests/canon_0_10_0.rs` and nothing else. On Canon
0.9.0 it is red: `the_decision_document_names_canon_0_10_0` reads `0.9.0`,
`an_approval_required_action_is_listed_under_pending_decisions` finds no section, and
`canon_reads_authority_entries_by_principal_and_case_revision` is refused because 0.9.0 does not
read a `principal` key. The second commit moves the three manifest lines and `Cargo.lock`
(`cargo update -p b10x-canon`) and turns it green. The third moves the documents.

## Acceptance

The test file writes its own protocols, in the forms of Canon's fixtures at tag `0.10.0`
(`fixtures/investigation/capability-scope.yaml`, `supplied-evidence/`, `judgement-marking.yaml`,
`child-outcome-subject.yaml`, `pending-decisions.yaml`, `missing-evidence.yaml`), builds one
`ProtocolCatalog` per test binary (`OnceLock`), and makes no model, network or clock call.

- `the_lockfile_holds_one_canon_and_no_engineering_package`
  (`crates/loom-protocols/tests/adversary_w8_pin.rs:128`) passes with every manifest at `0.10.0`;
  `git grep -n 'canon", tag = "0.9.0"' -- '*Cargo.toml'` prints nothing.
- `the_decision_document_names_canon_0_10_0` (`tests/canon_0_10_0.rs`): the `canon` member of
  `evaluate` for a case of the test protocol has `explanation.computed_from.semantics.canon`
  equal to `"0.10.0"`.
- `an_approval_required_action_is_listed_under_pending_decisions`: for an action that requires
  one capability, `CanonGovernor::decision_document` holds under `pending_decisions.authority`
  exactly `{action, capability, current: "none", case_revision: "r1", options: ["granted",
  "denied"]}`.
- `an_admissible_action_lists_its_missing_evidence`: an admissible action that may produce
  `review` lists `["review"]` under `missing_evidence` before a `review` record arrives, and `[]`
  after.
- `canon_reads_authority_entries_by_principal_and_case_revision`: through
  `b10x_canon::eval::evaluate_with` over the IR the catalog entry compiles, one `granted` entry
  with `principal` and `case_revision: r1` makes the action admissible at `r1`; a second entry
  `denied` by another principal makes it blocked; the `granted` entry alone with `case_revision:
  r2` leaves it approval-required at `r1`. The principal is matched against nothing: a third
  entry whose principal names another one changes no status.
- `two_authority_entries_equal_in_their_four_keys_are_refused`: Canon's code is
  `duplicate-identifier`.
- `a_protocol_with_0_10_0_members_registers_and_is_decided`: a protocol with a scoped capability
  requirement, a `supplied_by` kind, `author: judgement`, `revises`, `decided_by` and a `children`
  form with `revision: current` and `subject` registers through `ProtocolCatalog::add_yaml` and
  `CanonGovernor::with_protocol_yaml`; its frontier lists the scoped action `ApprovalRequired`
  with `capability` the capability id and a reason that names the scope.
- `judgement_marks_change_no_frontier`: the same protocol without `author`, `revises` and
  `decided_by` gives an equal frontier (statuses, capabilities, reasons) and an equal `actions`
  section; a mark in protocol text cannot change what the governor admits.
- `a_protocol_with_imports_is_refused_naming_missing_import`: `ProtocolCatalog::add_yaml` returns
  `Err` whose text holds `MissingImport` and the import's id; the catalog does not hold it.
- `the_governor_decision_equals_the_pinned_canon_decision_bytes`
  (`tests/canon_decision_bytes.rs:29`) and
  `the_document_equals_what_evaluate_returns_for_the_same_snapshot`
  (`tests/decision_document.rs:101`) pass unchanged.
- `adversary_a_duplicate_record_beside_a_terminated_snapshot_is_named_as_the_record`
  (`tests/evaluate_adversary.rs:190`) passes unchanged: two equal records with one id are still
  refused `duplicate-identifier` at index 1.
- Restart: the move writes no store. A case file holds no frontier id and no decision bytes
  (`crates/loom-governor/src/file_store.rs:318-372`), so a case stored before the move is read
  and decided after it, under a frontier id that differs from the one issued before. The run
  log's `Evaluated` line names a frontier and a decision digest
  (`crates/loom-runlog/src/lib.rs:541`), but no crate writes that line yet (`git grep
  decision_digest -- crates` finds only `crates/loom-runlog`); a line written before a later
  Canon move names a digest the moved Loom does not reproduce.
- `AGENTS.md` § Governor names tag `0.10.0` and says what 0.10.0 lets a protocol declare that
  Loom does not read (scope in the frontier, a `children` form's `subject` and `revision` in
  `action_facts`, `imports`); `grep -c 'tag = "0.10.0"' AGENTS.md` prints `1`.
- `CHANGELOG.md` `[Unreleased]` has a `**Breaking:**` bullet: Canon `0.9.0` -> `0.10.0`,
  `ProtocolEntry::model` is Canon 0.10.0's `Protocol`, every decision document gains
  `missing_evidence` and `pending_decisions` where they apply and names `0.10.0`, and frontier ids
  change (story:canon-0-10-0-pin).
- `website/docs/reference/sdk-stability.md` gains the row `| unreleased | 0.10.0 | 0.16.0 |`;
  `website/docs/guides/move-to-loom.md` names tag `0.10.0`;
  `docs/contracts/loom-action-selection.md` says a `children` form's `subject` and `revision` are
  not read by `action_facts`.
- `cargo test -p b10x-loom-governor -p b10x-loom-protocols -p b10x-loom-bindings` exits 0. Each
  new test is timed and named with its time when over 5 s; none runs 60 s; the gate's test step
  ends within 120 s.

## Out of scope

- Supplying authority entries to Canon, and the format's use:
  `story:approvals-supplied-to-canon-evaluation`.
- Reading `pending_decisions` into stops: `story:requests-from-canon-pending-decisions`, which
  loses its pin move to this story.
- Calling `check_arguments`: `story:protocol-argument-schema`.
- Grounds on entries: `story:decided-records-carry-grounds-to-canon`,
  `story:authority-records-bound-and-grounded-in-canon`.
- `action_facts` reading a `children` form's `subject` and `revision`, and a scope in the
  frontier: left as they are; a story that needs them takes them.
- Following Canon in counting an equal repeated evidence record once.
- Composing imported protocols.
- Runbook: its `canon evaluate` cases record `"canon": "0.9.0"` (158 files, for example
  `~/beyond10x/sdc/cases/01-idea-only/decision.json:179`); Runbook regenerates them when it moves
  its own `canon`.

## Scope

Read at `8a8fad3b`. Every line is **cited** (read in the tree) or **inferred**.

- **Primary surface:** the Canon pin: three manifests and `Cargo.lock` — cited
  (`crates/loom-governor/Cargo.toml:15`, `crates/loom-protocols/Cargo.toml:14`,
  `crates/loom-bindings/Cargo.toml:14`, `Cargo.lock:188-190`)
- **Files (manifests):** the three lines above, `tag = "0.9.0"` -> `tag = "0.10.0"` — cited
- **Files:** `Cargo.lock`, the `b10x-canon` package's version and source — cited
- **Files (new test):** `crates/loom-governor/tests/canon_0_10_0.rs` — inferred (name chosen)
- **Files (documents):** `AGENTS.md` (§ Governor, `:579-585`), `CHANGELOG.md` (`[Unreleased]`),
  `website/docs/reference/sdk-stability.md` (`:68-71`), `website/docs/guides/move-to-loom.md`
  (`:57-60`), `docs/contracts/loom-action-selection.md` (`:162-163`) — cited
- **Not touched:** `crates/loom-governor/src/lib.rs`, `crates/loom-protocols/src/lib.rs`,
  `crates/loom-bindings/src/`, `ess/`, `generated/`, `protocols/` — inferred from the grep in
  § Why; a compile error the grep missed is fixed in the crate it names, and the report lists it
- **Symbols:** none added or removed — cited (§ Why)
- **Would collide with:** any story that changes `Cargo.lock`, `crates/loom-governor/Cargo.toml`
  (`story:decision-request-file-store` adds a line beside `:15`), or a Canon manifest line
  (`story:protocol-argument-schema` if it calls Canon from `crates/loom-executor`) — cited
- **Safety fact:** the governor supplies no authority to Canon (`lib.rs:2093-2097`,
  `..Supplied::default()`), so no action is admitted that 0.9.0 held at approval-required —
  cited
