---
format: aep.planning-md/3
id: story:decided-records-carry-grounds-to-canon
kind: story
status: draft
title: An explicit decision carries its grounds to Canon, and one Canon sets aside supersedes its request
summary: ground_revisions and ground_evidence on canon-decisions/1; blocked until Canon releases
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:requests-from-canon-pending-decisions
revision: 2
---
## Outcome

An explicit decision Loom records for a decided outcome request carries the request's grounds to
Canon, and a decision Canon sets aside because a ground moved supersedes the request.

- `ExplicitDecisionWriter` (`story:decided-record-written-then-recorded`) passes the request's
  grounds to `CanonGovernor::decide`: every `artifact_revision` citation into
  `ground_revisions` (artifact to revision) and every `canon_evidence` `record_id` into
  `ground_evidence`. Other citation forms are not Canon grounds and stay on the request only.
- `FileCaseStore` keeps the grounds with the explicit decision, and the governor hands them to
  Canon's evaluation in the `canon-decisions/1` entry.
- When an evaluation sets a decision aside with `stale_grounds`,
  `CanonGovernor::stale_decisions(case, frontier)` answers it from the frontier's decision
  document, and the hosts of this epic (the slice and `tree run`) supersede the Decided
  request that produced the record (reason: the stale grounds) and route the stop anew. The Canon
  record is not revoked by Loom (accepted by session `sdc`, 2026-11-21); Canon has set it aside.

## Why

§ Ownership gives Canon "the grounds as citations to artifact revisions and evidence records,
which Canon invalidates when a revision moves" (`~/beyond10x/sdc/docs/decisions.md:201-202`).
Canon's `story:decision-grounds` adds `ground_revisions` and `ground_evidence` to
`canon-decisions/1` entries and sets an entry aside when a ground no longer holds (session
`canon`, 2026-11-21). This story covers explicit decisions; the same for approvals, which Loom
starts handing Canon in `story:approvals-supplied-to-canon-evaluation`, is
`story:authority-records-bound-and-grounded-in-canon`.

## Waits on

- `upstream-blocker:canon-decision-side-unreleased`: no Canon release carries
  `story:decision-grounds`.
- `story:requests-from-canon-pending-decisions`: it moves the Canon pin to the release this
  needs, and both change `crates/loom-governor/src/lib.rs` and `ess/domains/governor.yaml`.

## ESS first

`ess/domains/governor.yaml`: the stored explicit decision gains `ground_revisions` and
`ground_evidence`; `StaleDecision` is added (outcome, decision, stale grounds).
On that commit `task drift` fails, and `crates/loom-governor/tests/decision_grounds.rs` does not
compile.

## Acceptance

- An outcome request Decided with grounds `spec@t1` and evidence `e7` is recorded; the stored
  explicit decision carries `ground_revisions: {spec: t1}` and `ground_evidence: [e7]`, and the
  outcome is legitimate.
- After `spec` moves to `t2`, the next evaluation reports the decision set aside with
  `stale_grounds` naming `spec`; the outcome is blocked; the request is Superseded with that
  reason and a new Open request names it in `supersedes`.
- A request whose grounds are only `path_line` citations is recorded with empty Canon grounds and
  is not superseded when an artifact moves.
- A case stored before this story (explicit decisions without grounds) is read, and its decisions
  apply as before.
- `task check` exits 0.

## Out of scope

Approvals (`story:approvals-supplied-to-canon-evaluation`,
`story:authority-records-bound-and-grounded-in-canon`). Revoking a Canon record.
