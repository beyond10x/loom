---
format: aep.planning-md/3
id: story:authority-records-bound-and-grounded-in-canon
kind: story
status: draft
title: Approvals handed to Canon carry their grounds, and Canon checks the grounds it can see
summary: On Canon 0.10.0's decision-grounds; Loom stops checking the artifact and evidence grounds of decided approvals; the case moving and a ground alone told apart
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:approvals-supplied-to-canon-evaluation
- depends_on: story:decided-records-carry-grounds-to-canon
- depends_on: story:canon-0-10-0-pin
revision: 8
---
## Outcome

The `canon-authority/1` entries Loom hands Canon (`story:approvals-supplied-to-canon-evaluation`,
which already gives each entry the approval's `principal` and `case_revision`) carry its grounds as
`ground_revisions` and `ground_evidence`, so Canon sets an approval aside when a ground it can see
moves; Loom stops checking those grounds itself.

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
    ground can do this. Session `canon` answered on 2026-11-21 that expiry of cited evidence makes
    the ground stop holding with the case revision unchanged, the entry is set aside, and the
    reason lists `stale_grounds: [{"evidence": "<id>", "reason": "expired"}]`. Expiry needs the
    kind to declare `max_age` (`~/beyond10x/canon/website/docs/reference/protocol.md:57`), the
    record to carry `observed_at`, and the evaluation to be given an instant (`--at`). When an
    evaluation sets an authority entry aside with `stale_grounds`, the governor answers it beside
    the stale explicit decisions, and the slice and `tree run` supersede the Decided request that
    produced the record and route the stop anew. Canon's `story:decision-grounds` is released in
    tag `0.10.0`; Loom reads the key names of that release, not of that answer.
- `DecisionAuthority` (`story:approval-decisions-resume-held-runs`), the slice's and the tree's
  `AuthorityRecords` supply (`story:approvals-supplied-to-canon-evaluation`,
  `story:tree-waits-open-decision-requests`) no longer check a Decided approval's
  `artifact_revision` and `canon_evidence` grounds. They keep checking the grounds Canon's entry
  cannot carry (`path_line`, `aep_artifact`, `sdc_decisions`, and `canon_decisions` and
  `canon_authority` record ids), which are also the grounds that go stale while the case stays.
  Loom's grounding check of pending recommendations is unchanged.

## Why

Session `sdc`, 2026-11-21 (answer h): once Canon's `story:decision-grounds` lands, Canon checks
grounds on approvals too, and Loom's own check stays for pending recommendations only. Canon's
`story:authority-revision-binding` adds `principal` and `case_revision` to `canon-authority/1`
decisions, and `story:decision-grounds` adds `ground_revisions` and `ground_evidence` to its
entries (session `canon`, 2026-11-21); both are released in Canon `0.10.0`
(`crates/canon/src/model/authority.rs:56-74` at `0.10.0`), and
`story:approvals-supplied-to-canon-evaluation` hands over the first two. Canon's two ground
members hold artifact revisions and evidence ids only, so a `path_line` or other record-id
ground never reaches Canon; Loom keeps checking those on decided approvals, accepted by session
`sdc` on 2026-11-21 (fifth message, point 1).

## Waits on

- `story:canon-0-10-0-pin`: it moves the Canon pin to tag `0.10.0`, which carries
  `story:authority-revision-binding` and `story:decision-grounds`.
- `story:approvals-supplied-to-canon-evaluation`: the entries this extends, with their `principal`
  and `case_revision`, and the supply whose grounds check this narrows.
- `story:decided-records-carry-grounds-to-canon`: the stale-decision answer this extends, and both
  change `crates/loom-governor/src/lib.rs` and `ess/domains/governor.yaml`.

## ESS first

`ess/domains/governor.yaml`: `HeldAuthorityRecord` gains `ground_revisions` and
`ground_evidence`; `StaleDecision` gains the authority variant (capability, case revision, stale
grounds with the members Canon's release names). `ess/intake/domains/routing.yaml`: the slice's
`Grant` gains the two ground members. `ess/domains/tree.yaml`: `CaseApproval` gains them. On that
commit `task drift` and `task intake-drift` fail, and
`crates/loom-governor/tests/authority_grounds.rs` does not compile.

## Acceptance

With a fixture protocol whose action `merge` requires capability `change.merge`, and whose evidence
kind `check` declares `max_age: 1h`:

- A granted approval decided at case revision `r3` with ground `spec@t1` is handed to Canon with
  `ground_revisions: {spec: t1}` beside the principal and `case_revision: r3` that
  `story:approvals-supplied-to-canon-evaluation` gives it; the action is admissible at `r3`.
- After `spec` moves to `t2`, which raises the case to `r4`, the entry is still handed over with
  its ground (a test inspects the document handed over) and Canon reports `merge`
  approval-required; the next routing opens a request with record key `change.merge@r4`, and the
  `r3` request stays Decided with its record.
- A granted approval decided at `r3` with ground `check-1`, a `check` record with `observed_at`
  12:00: an evaluation given the instant 13:30, with the case still at `r3`, sets the entry aside
  with a stale ground naming evidence `check-1` and the reason `expired`; the request is
  Superseded and a new Open request names it. The same evaluation given 12:30 keeps `merge`
  admissible.
- `DecisionAuthority` makes no grounding call for the `artifact_revision` and `canon_evidence`
  grounds of a Decided approval, and one for its `path_line` ground (a counting resolver); a
  pending recommendation is still checked.
- A store written before this story (records without grounds) is read, and its approvals apply as
  before at their revision.
- `task check` exits 0.

## Out of scope

Revoking a Canon record. Mandate approvals. Grounds on a question's artifact revision record.
Handing Canon a `path_line` or record-id ground: Canon's entry has no member for one.
