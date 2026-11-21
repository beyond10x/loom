---
format: aep.planning-md/3
id: story:authority-records-bound-and-grounded-in-canon
kind: story
status: draft
title: Approvals handed to Canon carry principal, case revision and grounds, and Canon checks them
summary: On authority-revision-binding and decision-grounds; Loom stops choosing records by revision and stops checking decided approvals' grounds; blocked until Canon releases
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:approvals-supplied-to-canon-evaluation
- depends_on: story:decided-records-carry-grounds-to-canon
revision: 1
---
## Outcome

The `canon-authority/1` entries Loom hands Canon (`story:approvals-supplied-to-canon-evaluation`)
carry the approval's `principal` and `case_revision`, and its grounds as `ground_revisions` and
`ground_evidence`, so Canon binds an approval to its revision and sets it aside when a ground
moves; Loom stops doing either itself.

- The governor hands Canon every authority record of the case with its `principal` and
  `case_revision`, and no longer chooses records by the current revision: Canon applies an entry
  only at its case revision (`story:authority-revision-binding`, session `canon`, 2026-11-21).
- `GrantRecordWriter` and `CaseApprovalWriter` keep the decided request's grounds with the record:
  every `artifact_revision` citation into `ground_revisions` and every `canon_evidence`
  `record_id` into `ground_evidence`, as `story:decided-records-carry-grounds-to-canon` does for
  explicit decisions; the governor hands them to Canon on the entry.
- When an evaluation sets an authority entry aside with `stale_grounds`, the governor answers it
  beside the stale explicit decisions, and the slice and `tree run` supersede the Decided request
  that produced the record and route the stop anew.
- `DecisionAuthority` (`story:approval-decisions-resume-held-runs`) and the tree
  (`story:tree-waits-open-decision-requests`) no longer check a Decided approval's grounds before
  they allow; Loom's own grounding check stays for pending recommendations only (session `sdc`,
  2026-11-21, answer h).

## Why

Session `sdc`, 2026-11-21 (answer h): once Canon's `story:decision-grounds` lands, Canon checks
grounds on approvals too, and Loom's own check stays for pending recommendations only. Canon's
`story:authority-revision-binding` adds `principal` and `case_revision` to `canon-authority/1`
decisions, and `story:decision-grounds` adds `ground_revisions` and `ground_evidence` to its
entries (session `canon`, 2026-11-21); neither is released.

## Waits on

- `upstream-blocker:canon-decision-side-unreleased`: no Canon release carries
  `story:authority-revision-binding` or `story:decision-grounds`.
- `story:approvals-supplied-to-canon-evaluation`: the entries this extends.
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

- A granted approval decided at case revision `r3` with ground `spec@t1` is handed to Canon with
  principal, `case_revision: r3` and `ground_revisions: {spec: t1}`; the action is admissible at
  `r3`.
- After the case moves to `r4`, the entry is still handed over and Canon reports the action
  approval-required; Loom chose no records by revision (a test inspects the document handed over).
- After `spec` moves to `t2` with the case at `r3`, Canon sets the entry aside with
  `stale_grounds` naming `spec`; the request is Superseded and a new Open request names it.
- `DecisionAuthority` makes no grounding call on a Decided approval (a counting resolver), and a
  pending recommendation is still checked.
- A store written before this story (records without grounds) is read, and its approvals apply as
  before at their revision.
- `task check` exits 0.

## Out of scope

Revoking a Canon record. Mandate approvals. Grounds on a question's artifact revision record.
