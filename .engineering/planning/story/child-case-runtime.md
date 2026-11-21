---
format: aep.planning-md/3
id: story:child-case-runtime
kind: story
status: proposed
title: Performing a case_open action opens the child case and its outcome is written into the parent as case_outcome
summary: Parent-child link and termination in the case store; subject_revision is the revision the outcome was computed against
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:canon-0-9-0-pin
- depends_on: story:bindings-document
- depends_on: story:deterministic-default-selector
- depends_on: story:bound-artifact-revisions
- depends_on: story:bound-agent-node
scope:
- confidence: inferred
  path: crates/loom-bindings
- confidence: cited
  path: crates/loom-bindings/src
- confidence: cited
  path: crates/loom-bindings/tests/bound_open.rs
- confidence: cited
  path: crates/loom-governor/src/file_store.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/child_cases.rs
- confidence: inferred
  path: crates/loom-governor/tests/fixtures
- confidence: cited
  path: crates/loom-governor/tests/restart_durability.rs
- confidence: cited
  path: crates/loom-governor/tests/restart_durability_adversary.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: ess/domains/governor.yaml
- confidence: inferred
  path: website/docs/concepts/governor-and-intake.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:46Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":3}}}
---
## Outcome

When an action that declares Canon's `case_open` is performed, Loom opens the child case or
cases, keeps the link between parent and child in the case store, and writes each child's outcome
into the parent as the one `case_outcome` evidence record Canon reads for that child: result
`open` from the moment the child is opened, and the child's outcome once the child is terminated.

- The child's protocol is the one `case_open.protocol` names, resolved in the host's catalog; the
  binding's `open` executor gives only the child's name (or the argument whose entries each name
  one child) and the child's bindings file. A bindings file is refused at load when an `open`
  executor sits on an action with no `case_open`, or a `case_open` action is bound to anything
  else.
- The child's id is `<parent id>.<name>`; opening for the same name again gives `<id>.2`, `<id>.3`.
  A child whose subject was revised is never reopened or rewritten: the parent opens a new one.
- The child is opened with the `independence` map of its own bindings file.
- After the children are opened, the opening action's own `evidence` entries with `from: effect`
  are recorded in the parent like any step's evidence.
- A case is terminated by one governor operation, `terminate(case, outcome, instant)`, which
  Canon must judge legitimate. The case then holds that termination: later evaluations pass it to
  Canon, and no action of a terminated case is performed. When to call it is the caller's
  (`story:tree-run-command`); this story's tests call it directly.
- `subject_revision` of the outcome record is the revision the child's outcome was computed
  against. When the child's protocol declares the artifact `case_open.subject` names, it is the
  child's own revision of that artifact at its terminating evaluation. When it does not, it is the
  parent's revision of the subject at the moment the child was opened. While the child is open
  the record carries the opening revision.
- The cases of one tree share the root's workspace.

## Why

`~/beyond10x/sdc/docs/boundary.md` row 27 and top change 1: Canon has the primitive and "opens
nothing itself; whoever takes the action opens the child case", and Loom has no case link
(`CaseState`, `crates/loom-governor/src/lib.rs:184-203`; `StoredCase`,
`ess/domains/governor.yaml:107`), records no termination (`completion()` re-derives the outcome,
`lib.rs:1333-1350`; the snapshot passes `termination: None`, `:1661`; the `Case` lifecycle is only
`Open`, `ess/commission/domains/responsibility.yaml:1055-1067`), and holds evidence append-only.
Six of the eight SDC protocols open or are opened as children, and each opening action also
produces a kind a parent claim reads (`intake_opened` and the like,
`~/beyond10x/sdc/protocol/sdc.protocol.yaml:25-30`). Canon reads one record per child and refuses
two (`duplicate-child-case`), so `open` to outcome replaces the record. The revision rule is
session `sdc`'s, 2026-11-21: Canon's draft story `child-outcome-revision` (`revision: current` on
the `children` form) depends on a record that names the revision the outcome was computed
against, so an outcome about a superseded subject stops counting.

## ESS first

`ess/domains/governor.yaml` first: `StoredCase` gains optional `parent` (`CaseLink`: parent case,
opening action, outcome evidence id, opening subject revision) and optional `termination`
(outcome and instant), and a `children` list of links. Then
`ess/commission/domains/responsibility.yaml`: the `Case` lifecycle gains `Terminated`. On that
commit `task drift` and `task commission:drift` fail, and
`crates/loom-governor/tests/child_cases.rs` does not compile.

Where it lands: the case link, the record replacement and `terminate` are in
`crates/loom-governor`; the `open` effect port and the load-time check of `open` against
`case_open` are in `crates/loom-bindings`, with their test
`crates/loom-bindings/tests/bound_open.rs`. This story changes `responsibility.yaml` and
`crates/loom-governor/src/lib.rs` after `story:deterministic-default-selector`, and
`crates/loom-bindings` after `story:bound-agent-node`; it depends on both for that order. The
value `performed` of a `from: effect` entry is declared by `story:bindings-document`.

## Acceptance

With fixture protocols `parent/1` (one `case_open` action that may produce `child_opened`, a
claim with `children: {in: [passed]}`) and `child/1`, stub programs and no model:

- Performing the opening action creates the child case under `child/1` and one `case_outcome`
  record in the parent with `child_case` = the child's id and result `open`; the parent's
  `children` claim is `unknown`, and the parent holds a `child_opened` record from the binding's
  `from: effect` entry.
- After the child's evidence makes `passed` legitimate and `terminate(child, passed, t)` is
  called, the same record has result `passed` and instant `t`, no second record exists for that
  child, and the parent's claim is `true`. A child terminated `failed` makes it `false`.
- A child that declares the subject artifact and moved it twice before termination gives a record
  whose `subject_revision` is the child's last revision of it. A child that does not declare it
  gives the parent's revision at opening, also after the parent's subject moved while the child
  ran.
- An `each` argument with three entries opens three children and three records; a repeated
  opening for an existing name gets the next numbered id and leaves the first child's record
  unchanged.
- A child whose bindings file lists `independence` for a producer accepts that producer's record
  on a match that demands the dimension, and drops another producer's.
- A file a child's program writes at a placed path moves the parent's revision of the artifact
  placed at the same path at the parent's next evaluation.
- A terminated case refuses a further action without a new error variant: its frontier lists no
  admissible action and `completion` reports the stored outcome. No method is added to the
  `CaseStore` or `FallibleCaseStore` traits, so the store implementations outside
  `crates/loom-governor/src` compile unchanged.
- A function of `crates/loom-governor` lists the cases of a tree from any of its cases, with
  each case's parent link.
- `CaseState` gains public fields and is on the SDK stability page, so `CHANGELOG.md` gets a
  `**Breaking:**` bullet; a case file written by this release with a link or a termination is
  refused by Loom 0.57.2 and earlier, and the bullet says so.
- Its stored termination survives a reopen of the store. `terminate` with an outcome Canon judges not legitimate returns the refusal and
  leaves the case file byte-identical.
- A catalog in which two protocols open each other is refused at registration by Canon's
  `check_openings`, naming the cycle.
- A bindings file with an `open` executor on an action without `case_open`, or a child protocol
  the catalog does not hold, is refused before anything is written.
- `task check` exits 0.

## Out of scope

Deciding when a case terminates and driving the children (`story:tree-run-command`). Copying a
record from parent to child (`story:record-copy-between-cases`). A workspace per child.
Invalidating a child's outcome when the parent's subject moves: that is Canon's `revision:
current`.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-governor`, which holds the case link, the replacement of the `case_outcome` record, `terminate`, the stored termination and the opening-cycle check at registration — cited
- **Files:** `crates/loom-governor/src/lib.rs`: `CaseState` :184-203, `opened` :1577, `snapshot` passing `termination: None` :1661, `completion` :1333, `frontier` :1286, `EvidencePort::receive` with its append-only `state.evidence.push` :1414-1433, `with_catalog` :768, which is where Canon's `check_openings` (`canon/src/ir/composition.rs:58`) gets called — cited
- **Files:** `ess/domains/governor.yaml:107` `StoredCase` gains `parent`, `children` and `termination` (plus new `CaseLink` and termination types) — cited
- **Files:** `ess/commission/domains/responsibility.yaml:1055-1067` the `Case` lifecycle gains `Terminated`. A transition there needs no command (compare :1203-1205) — cited
- **Files:** `crates/loom-governor/src/file_store.rs` has to grow with `StoredCase`: `stored_case` :310, `held_case` :369 (a `CaseState` literal at :409), `encode_case` :503, `decode_case` :674. The decoder refuses any member it does not list (`exactly` :653) — cited
- **Files:** `crates/loom-governor/tests/restart_durability.rs:464` and `crates/loom-governor/tests/restart_durability_adversary.rs:277,442` build `CaseState` with every field named. `CaseState` derives no `Default`, so adding a field stops them compiling — cited
- **Files:** `crates/loom-governor/tests/child_cases.rs` (new) — cited
- **Files:** `crates/loom-bindings/src` holds the `open` effect port and the load-time check of `open` against `case_open`. The crate does not exist yet (`story:bindings-document` creates it), so the file names are unknown — cited
- **Files:** `crates/loom-bindings/tests/bound_open.rs` (new) — cited
- **Symbols:** `CaseState`, `HeldEvidence`, `snapshot`, `completion`, `CanonGovernor::open_case_with_independence`, `with_catalog`, `OpenError`, `StoredCase`, `case_outcome`, `check_openings`, `terminate` (new) — cited
- **Not touched:** the other `CaseStore`/`FallibleCaseStore` implementations (`lib.rs:460` `MemoryCaseStore`, `tests/adversary2_governor.rs:270`, `tests/decision_document.rs:515`, `tests/hosted_protocol.rs:159,210`, `crates/loom-sdk/tests/host_ports.rs:153,183`) only pass `CaseState` along and never build one, so they are safe as long as the trait gains no method — cited
- **Not touched:** Commission's runtime, testkit and conformance crates. Outside `generated/`, nothing uses the generated `CaseState`, `AnyCase` or `case_state::*` from `responsibility.yaml`. The runtime checks `governor.completion` before it acts (`crates/loom-commission/src/runtime.rs:1665,3163`) — cited
- **Not touched:** no test outside `generated/` lists `Case` lifecycle states (searched `CaseState::`, `AnyCase`, `CaseSnapshot`). The `CaseSnapshot` hits are `loom.evaluation`'s, a different type — cited
- **Also likely:** `crates/loom-governor/tests/fixtures`, if the `parent/1` and `child/1` fixture protocols live in files rather than inline — inferred
- **Documents:** `website/docs/concepts/governor-and-intake.md:19-25`, whose text ("evaluates afresh every time", completion re-derived) becomes wrong once a termination is stored — inferred
- **Confidence:** high for `loom-governor` and the two ESS files, because the story names the sites and the tree confirms them. Medium for `loom-bindings`, because that crate does not exist and its file names cannot be read.
- **Would collide with:** any unit touching `crates/loom-governor/src/lib.rs` (`CaseState`, `snapshot`, `completion`, evidence receipt, catalog registration), the `crates/loom-governor/src/file_store.rs` case codec, `StoredCase` in `ess/domains/governor.yaml`, the `Case` lifecycle in `ess/commission/domains/responsibility.yaml`, or the load check or effect ports in `crates/loom-bindings/src`. Also anything that regenerates `generated/rust/loom/src/governor.rs` or `generated/rust/commission/src/responsibility.rs` — cited
- **Safety fact:** the file store's decoder refuses any member `StoredCase` does not declare (`file_store.rs:653`, applied at :676; held by `restart_durability_adversary.rs:65`). So `parent`, `children` and `termination`, written only when present, keep every existing case file readable. A case file that carries them is refused by Loom 0.57.2 and earlier, so there is no downgrade path — step 2, unproven
