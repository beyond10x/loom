---
format: aep.planning-md/3
id: story:requests-from-canon-pending-decisions
kind: story
status: draft
title: Loom builds on the Canon release with pending_decisions and builds every decision stop from it
summary: Canon pin move and stop_from_pending; blocked until Canon releases
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:tree-waits-open-decision-requests
- depends_on: story:canon-0-9-0-pin
- depends_on: story:approvals-supplied-to-canon-evaluation
revision: 5
---
## Outcome

Loom builds on the Canon release that carries `story:pending-decisions`, one copy of it, with the
engineering-protocols tag built on that release, and builds every decision stop from Canon's
`pending_decisions` section of `canon-decision/1`.

- `CanonGovernor::pending_decisions(case, frontier)` returns the section of the document behind a
  frontier id the governor issued, as `decision_document` returns the whole document
  (`AGENTS.md` § Governor, `:484-492`).
- `stop_from_pending` in `b10x-loom-governor` turns an `authority` entry `{action, capability,
  scope?, current, case_revision?, options: [granted, denied]}` into an approval stop with options
  `A` granted and `B` denied, both producing a record, and a `decisions` entry `{outcome,
  decision, case_revision?, options: [taken]}` into an outcome stop whose action is the outcome
  (`story:decision-routing-by-holder`), with `A` take, which produces the explicit decision
  record, and `B` leave it untaken, which produces none (session `sdc`, 2026-11-21, answer d). The
  subject is given by the caller: Canon names no subject artifact of an action, and the caller maps
  the action to its subject through the bindings file (session `canon`, 2026-11-21).
- Canon gives one `authority` entry per requirement that makes an action approval-required. Each
  entry is one stop; the router keys requests by `record_key` (`<capability>@r<N>`, `r<N>` the
  case revision), and `question_key` stays `<case>|<action>|<question id or empty>`
  (`story:decision-routing-by-holder`). So two entries of one action with two capabilities give
  two requests under one `question_key`, and two entries with one capability at one revision give
  one request: the second routing finds the open request for that record key and reuses it.
- `DecisionHoldingExecutor` (`story:question-and-outcome-decisions-hold-runs`), `DecisionAuthority`
  (`story:approval-decisions-resume-held-runs`) and `tree run`
  (`story:tree-waits-open-decision-requests`) build their stops with it, and the interim reading
  of a blocked outcome's reason `{"decision": <name>, "present": false}` is removed.

## Why

§ Ownership gives the pending decision, "a fact derived from a case", to Canon
(`~/beyond10x/sdc/docs/decisions.md:249-251`). Until Canon ships it, this epic derives stops from
Loom's own stop and the decision document; this story replaces that derivation with Canon's, so
Loom and Canon cannot disagree about what is pending. Loom pins Canon at `0.8.0`
(`crates/loom-governor/Cargo.toml:15`).

## Waits on

- `upstream-blocker:canon-decision-side-unreleased`: no Canon release carries
  `pending_decisions`.
- `story:canon-0-9-0-pin`: it moves every manifest naming `b10x-canon` to `0.9.0`; this story
  moves the same manifests again.
- `story:tree-waits-open-decision-requests`: one of the three callers, and both change
  `crates/loom-tree`. The slice callers come before it in the epic's order.
- `story:approvals-supplied-to-canon-evaluation`: both change `crates/loom-governor/src/lib.rs`
  and `ess/domains/governor.yaml`; that story is not held by Canon, so it goes first.

## ESS first

`ess/domains/governor.yaml` gains `PendingAuthority`, `PendingExplicitDecision` and
`PendingDecisions`, the section as Loom reads it. On that commit `task drift` fails, and
`crates/loom-governor/tests/pending_decisions.rs` does not compile.

## Acceptance

- `Cargo.lock` holds one copy of `b10x-canon` at the new tag
  (`the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags` passes), and every
  manifest that names `b10x-canon` names that tag.
- For a fixture case with one approval-required action and one outcome blocked on a missing
  decision, `pending_decisions` has one entry in each list, and `stop_from_pending` gives an
  approval stop with options `granted`, `denied`, both producing a record, and an outcome stop
  with options take, producing a record, and leave untaken, producing none.
- A fixture action `merge` that requires two capabilities, `change.merge` and `release.approve`,
  at case revision `r3`, gives two `authority` entries and two requests with the record keys
  `change.merge@r3` and `release.approve@r3`, both under the `question_key` `<case>|merge|`.
- A fixture action `merge` with two requirements naming one capability `change.merge`, at case
  revision `r3`, gives two `authority` entries and one request: the second routing finds the open
  request for `change.merge@r3` and reuses it, and the store holds one request.
- The slice and tree fixture runs of stories 13, 14 and 16 give the same requests, by id, question,
  options and subject, as before this story.
- A grep finds no reading of `"present": false` outside `crates/loom-governor`.
- `task check` exits 0.

## Out of scope

Grounds on decided records (`story:decided-records-carry-grounds-to-canon`). Supplying approvals
to Canon (`story:approvals-supplied-to-canon-evaluation`). Mapping an action to its subject in
Canon.
