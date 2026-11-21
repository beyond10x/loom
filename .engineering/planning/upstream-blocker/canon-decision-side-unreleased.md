---
format: aep.planning-md/3
id: upstream-blocker:canon-decision-side-unreleased
kind: upstream-blocker
status: open
title: No Canon release carries pending decisions or decision grounds
summary: Canon's pending-decisions, decision-grounds and authority-revision-binding are accepted on a local wave branch, unreleased
tags:
- sdc
relations:
- blocks: story:requests-from-canon-pending-decisions
- blocks: story:decided-records-carry-grounds-to-canon
- blocks: story:authority-records-bound-and-grounded-in-canon
revision: 3
---
## What is missing

A Canon release that carries the decision side of decision requests. Canon's newest release is
`0.9.0` (`main` at `8f1f452`); Loom pins `0.8.0` (`crates/loom-governor/Cargo.toml:15`). Session
`canon` reported on 2026-11-21 three stories, accepted on its local wave branch (commit `c295fae`,
not pushed), none implemented or released:

- `story:authority-revision-binding` (canon wave 3, next to start): `principal` and
  `case_revision` on `canon-authority/1` decisions.
- `story:pending-decisions` (canon wave 5): a section `pending_decisions` in `canon-decision/1`,
  with `authority` entries `{action, capability, scope?, current: none|require_approval,
  case_revision?, options: [granted, denied]}` and `decisions` entries `{outcome, decision,
  case_revision?, options: [taken]}`; no principal, holder, routing, expiry, recommendation or
  subject artifact.
- `story:decision-grounds` (canon wave 6): `ground_revisions` and `ground_evidence` on the entries
  of `canon-authority/1` and `canon-decisions/1`; an entry applies only while every ground holds,
  and a set-aside entry reports `stale_grounds`.

`story:pending-decisions` and `story:decision-grounds` both depend on
`story:authority-revision-binding` and on `story:missing-evidence` (canon wave 4). Read
2026-11-21.

## What it blocks

- `story:requests-from-canon-pending-decisions`: needs `pending_decisions`.
- `story:decided-records-carry-grounds-to-canon`: needs `ground_revisions`, `ground_evidence` and
  `stale_grounds` on `canon-decisions/1`.
- `story:authority-records-bound-and-grounded-in-canon`: needs `principal` and `case_revision` on
  `canon-authority/1` decisions, and `ground_revisions`, `ground_evidence` and `stale_grounds` on
  its entries.

Nothing else in `epic:decision-requests-in-the-runtime` waits on it: until Canon ships, stops are
built from Loom's own stop and `CanonGovernor::decision_document`, Loom applies Canon's two ground
rules to a pending recommendation and to its approvals itself, and
`story:approvals-supplied-to-canon-evaluation` hands Canon its approvals in the `canon-authority/1`
shape of the pinned release.

## What clears it

A Canon tag that contains `story:pending-decisions` and `story:decision-grounds` (and so their two
dependencies), and an engineering-protocols tag built on it: Loom moves Canon together with the
`b10x-canon-engineering` tag (`AGENTS.md` § Governor), as
`upstream-blocker:engineering-protocols-on-canon-0-9-0` records for `0.9.0`.

## Who

Session `canon` owns the three stories and reported them to this planning on 2026-11-21 through
the coordinator. Session `sdc` sent the original requests, "To the Canon session" and "To the Loom
session" (`~/beyond10x/sdc/docs/decisions.md:279-315`); Canon's answer to the first two is at
`decisions.md:295-297`. No release date is known.
