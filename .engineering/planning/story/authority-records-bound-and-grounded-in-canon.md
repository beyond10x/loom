---
format: aep.planning-md/3
id: story:authority-records-bound-and-grounded-in-canon
kind: story
status: draft
title: Approvals handed to Canon carry principal, case revision and grounds, and Canon checks the grounds it can see
summary: On authority-revision-binding and decision-grounds; Loom stops choosing records by revision and stops checking the artifact and evidence grounds of decided approvals; the case moving and a ground alone told apart; blocked until Canon releases
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:approvals-supplied-to-canon-evaluation
- depends_on: story:decided-records-carry-grounds-to-canon
revision: 4
---
## Outcome

The `canon-authority/1` entries Loom hands Canon (`story:approvals-supplied-to-canon-evaluation`)
carry the approval's `principal` and `case_revision`, and its grounds as `ground_revisions` and
`ground_evidence`, so Canon binds an approval to its revision and sets it aside when a ground it
can see moves; Loom stops doing either itself for those grounds.

- The governor hands Canon every authority record of the case with its `principal` and
  `case_revision`, and no longer chooses records by the current revision: Canon applies an entry
  only at its case revision (`story:authority-revision-binding`, session `canon`, 2026-11-21).
- `GrantRecordWriter` and `CaseApprovalWriter` keep the decided request's grounds with the record:
  every `artifact_revision` citation into `ground_revisions` and every `canon_evidence`
  `record_id` into `ground_evidence`, as `story:decided-records-carry-grounds-to-canon` does for
  explicit decisions; the governor hands them to Canon on the entry.
- Two ways an entry stops applying, told apart:
  - the case moved. In a Loom-held case an `artifact_revision` ground cannot move alone: moving
    the artifact raises the case revision (`crates/loom-governor/src/lib.rs:1042`), so the entry no
    longer applies by its `case_revision`, and Canon reports the action approval-required. The
    request that produced the record stays Decided with its record; the next stop is routed at the
    new revision under a new record key (`story:decision-routing-by-holder`);
  - a ground alone, at an unchanged case revision. Of the grounds Canon receives, only an evidence
    ground can do this: Canon excludes a record older than its kind's `max_age` at the evaluation
    instant as `expired` (`~/beyond10x/canon/website/docs/reference/protocol.md:57`) with no
    revision moving. When an evaluation sets an authority entry aside with `stale_grounds`, the
    governor answers it beside the stale explicit decisions, and the slice and `tree run`
    supersede the Decided request that produced the record and route the stop anew.
- `DecisionAuthority` (`story:approval-decisions-resume-held-runs`), the slice's and the tree's
  `AuthorityRecords` supply (`story:approvals-supplied-to-canon-evaluation`,
  `story:tree-waits-open-decision-requests`) no longer check a Decided approval's
  `artifact_revision` and `canon_evidence` grounds. They keep checking the grounds Canon's entry
  cannot carry (`path_line`, `aep_artifact`, `sdc_decisions`, and `canon_decisions` and
  `canon_authority` record ids), which are also the grounds that go stale while the case stays.
  Loom's grounding check of pending recommendations is unchanged (session `sdc`, 2026-11-21,
  answer h).

## Why

Session `sdc`, 2026-11-21 (answer h): once Canon's `story:decision-grounds` lands, Canon checks
grounds on approvals too, and Loom's own check stays for pending recommendations only. Canon's
`story:authority-revision-binding` adds `principal` and `case_revision` to `canon-authority/1`
decisions, and `story:decision-grounds` adds `ground_revisions` and `ground_evidence` to its
entries (session `canon`, 2026-11-21); neither is released. Canon's two ground members hold
artifact revisions and evidence ids only, so a `path_line` or other record-id ground never reaches
Canon, and answer h's "pending recommendations only" cannot hold for those grounds: Loom keeps
checking them on decided approvals.

## Waits on

- `upstream-blocker:canon-decision-side-unreleased`: no Canon release carries
  `story:authority-revision-binding` or `story:decision-grounds`.
- `story:approvals-supplied-to-canon-evaluation`: the entries this extends, and the supply whose
  grounds check this narrows.
- `story:decided-records-carry-grounds-to-canon`: the Canon pin (through
  `story:requests-from-canon-pending-decisions`), the stale-decision answer this extends, and both
  change `crates/loom-governor/src/lib.rs` and `ess/domains/governor.yaml`.

## ESS first

`ess/domains/governor.yaml`: `HeldAuthorityRecord` gains `ground_revisions` and
`ground_evidence`; `StaleDecision` gains the authority variant (capability, case revision, stale
grounds). `ess/intake/domains/routing.yaml`: the slice's `Grant` gains the two ground members.
`ess/domains/tree.yaml`: `CaseApproval` gains them. On that commit `task drift` and
`task intake-drift` fail, and `crates/loom-governor/tests/authority_grounds.rs` does not compile.

## Acceptance

With a fixture protocol whose action `merge` requires capability `change.merge`, and whose evidence
kind `check` has `max_age: 1h`:

- A granted approval decided at case revision `r3` with ground `spec@t1` is handed to Canon with
  principal, `case_revision: r3` and `ground_revisions: {spec: t1}`; the action is admissible at
  `r3`.
- After `spec` moves to `t2`, which raises the case to `r4`, the entry is still handed over and
  Canon reports `merge` approval-required; Loom chose no records by revision (a test inspects the
  document handed over); the next routing opens a request with record key `change.merge@r4`, and
  the `r3` request stays Decided with its record.
- A granted approval decided at `r3` with ground `check-1`, a `check` record observed at 12:00: an
  evaluation at 13:30 with the case still at `r3` sets the entry aside with `stale_grounds` naming
  `check-1`; the request is Superseded and a new Open request names it.
- `DecisionAuthority` makes no grounding call for the `artifact_revision` and `canon_evidence`
  grounds of a Decided approval, and one for its `path_line` ground (a counting resolver); a
  pending recommendation is still checked.
- A store written before this story (records without grounds) is read, and its approvals apply as
  before at their revision.
- `task check` exits 0.

## Out of scope

Revoking a Canon record. Mandate approvals. Grounds on a question's artifact revision record.
Handing Canon a `path_line` or record-id ground: Canon's entry has no member for one.
