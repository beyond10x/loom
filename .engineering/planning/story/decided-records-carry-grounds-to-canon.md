---
format: aep.planning-md/3
id: story:decided-records-carry-grounds-to-canon
kind: story
status: draft
title: An explicit decision carries its grounds to Canon, and one Canon sets aside supersedes its request
summary: ground_revisions and ground_evidence on canon-decisions/1, on Canon 0.10.0
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:requests-from-canon-pending-decisions
- depends_on: story:canon-0-10-0-pin
revision: 6
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
- Two ways a decision stops applying, told apart. The case moved: the governor evaluates with an
  explicit decision only until the case moves (`crates/loom-governor/src/lib.rs:1051-1053`), and in
  a Loom-held case an `artifact_revision` ground cannot move alone, because moving the artifact
  raises the case revision (`lib.rs:1042`); the request stays Decided with its record and the stop
  is routed at the new revision under a new record key. A ground alone, at an unchanged case
  revision: of the grounds Canon receives, only an evidence ground can. Session `canon` answered on
  2026-11-21 that expiry of cited evidence makes the ground stop holding with the case revision
  unchanged, the entry is set aside, and the reason lists
  `stale_grounds: [{"evidence": "<id>", "reason": "expired"}]`. Expiry needs the evidence kind to
  declare `max_age` (`~/beyond10x/canon/website/docs/reference/protocol.md:57`), the record to
  carry `observed_at`, and the evaluation to be given an instant (`--at`). Canon's
  `story:decision-grounds` is planned, not built; Loom reads the key names of Canon's release, not
  of that answer.

## Why

§ Ownership gives Canon "the grounds as citations to artifact revisions and evidence records,
which Canon invalidates when a revision moves, as it does for evidence"
(`~/beyond10x/sdc/docs/decisions.md:252-253`). Canon's `story:decision-grounds` adds
`ground_revisions` and `ground_evidence` to `canon-decisions/1` entries and sets an entry aside
when a ground no longer holds (session `canon`, 2026-11-21). This story covers explicit decisions;
the same for approvals, which Loom starts handing Canon in
`story:approvals-supplied-to-canon-evaluation`, is
`story:authority-records-bound-and-grounded-in-canon`.

## Waits on

- `story:canon-0-10-0-pin`: it moves the Canon pin to tag `0.10.0`, which carries
  `story:decision-grounds` (`ground_revisions` and `ground_evidence` on `canon-decisions/1`,
  `stale_grounds`; `crates/canon/src/model/explicit.rs` at `0.10.0`).
- `story:requests-from-canon-pending-decisions`: both change `crates/loom-governor/src/lib.rs`
  and `ess/domains/governor.yaml`.

## ESS first

`ess/domains/governor.yaml`: the stored explicit decision gains `ground_revisions` and
`ground_evidence`; `StaleDecision` is added (outcome, decision, stale grounds, with the members
Canon's release names for each stale ground). On that commit `task drift` fails, and
`crates/loom-governor/tests/decision_grounds.rs` does not compile.

## Acceptance

With a fixture protocol whose evidence kind `check` declares `max_age: 1h`:

- An outcome request Decided with grounds `spec@t1` and evidence `e7` is recorded; the stored
  explicit decision carries `ground_revisions: {spec: t1}` and `ground_evidence: [e7]`, and the
  outcome is legitimate.
- After `spec` moves to `t2`, which raises the case revision, the outcome is blocked again; the
  request stays Decided with its record, and routing the stop opens a request under the record key
  of the new case revision.
- An outcome request Decided with ground `check-1`, a `check` record with `observed_at` 12:00: an
  evaluation given the instant 13:30, with the case revision unchanged, reports the decision set
  aside with a stale ground naming evidence `check-1` and the reason `expired`; the outcome is
  blocked; the request is Superseded with that reason and a new Open request names it in
  `supersedes`. The same evaluation given 12:30 keeps the decision.
- A request whose grounds are only `path_line` citations is recorded with empty Canon grounds, and
  no evaluation reports it with `stale_grounds`.
- A case stored before this story (explicit decisions without grounds) is read, and its decisions
  apply as before.
- `task check` exits 0.

## Out of scope

Approvals (`story:approvals-supplied-to-canon-evaluation`,
`story:authority-records-bound-and-grounded-in-canon`). Revoking a Canon record. Handing Canon a
`path_line` or record-id ground.
