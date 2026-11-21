---
format: aep.planning-md/3
id: story:decision-inbox-commands
kind: story
status: draft
title: b10x-loom decisions lists a principal's open requests and decides, takes, escalates and supersedes them
summary: The inbox as CLI and SDK calls over the file store, completing what it decides
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decided-record-written-then-recorded
revision: 2
---
## Outcome

`b10x-loom decisions` is the inbox, over a store directory (`--store DIR`) that holds a
`FileDecisionStore` and the governor's `FileCaseStore`:

- `decisions list --principal P` prints `OpenDecisionsForPrincipal` for `P` as JSON, one entry per
  request: id, case, question, subject at revision, recommendation and strength or the reason for
  none, opened and expiry instants.
- `decisions show ID` prints the request as JSON, with the `EscalationSuppressedRecord`s kept
  beside it; `decisions history --artifact A` prints `ArtifactDecisionHistory`.
- `decisions decide ID OPTION --as P [--reason TEXT]...`, `decisions take ID --as P`,
  `decisions escalate ID --to P --reason TEXT` and `decisions supersede ID --by ID2 --reason TEXT`
  apply `Decide`, `TakeRecommendation`, `Escalate` and `Supersede` with principal kind `person`.
- Every command first runs `complete_pending` with the writers the command line has for the
  store (in this story the governor's `ExplicitDecisionWriter`), and `decide` and `take` complete
  the request they decided, so a decision made here is recorded before the command exits.
- A refusal exits non-zero, names the domain's error and the request, and changes nothing.
- Every instant comes from the host clock, or from `--at` when given. The principal named by `--as`
  is host-trusted input, as the principal of `b10x-loom approve` is; authenticating it is not
  Loom's.

The subcommand's code goes in its own module of `crates/loom-cli` (`decisions.rs`), so `lib.rs`
gains the `mod` line and the `Command` variant; `--state DIR` (`story:approval-decisions-resume-held-runs`)
and `--tree STORE` (`story:tree-waits-open-decision-requests`) are added there later, each with
its host's writers.

`b10x-loom-sdk` exposes the same calls (`loom_sdk::decisions`: open requests for a principal,
show, history, decide, take, escalate, supersede), each over a `DecisionStore` and a set of
writers the host gives.

## Why

Item 3 of the request: the open-decisions query per principal and escalation to a named person
(`~/beyond10x/sdc/docs/decisions.md:307-309`). The inbox is a store plus CLI and SDK calls, with
no service of its own (coordinator's decision, accepted by session `sdc` on 2026-11-21, and now
`decisions.md:264-265`). Under `person` or `advised` an
Escalate is a person re-addressing the request (session `sdc`, 2026-11-21, point 4), which is this
command. The command line today has `approve` and `resume` for a held capability only
(`crates/loom-cli/src/lib.rs:235-242`).

## Waits on

`story:decided-record-written-then-recorded`: `decide` and `take` complete what they decide.
`story:decision-request-file-store` through it.

## ESS first

No new noun: the commands apply `loom.decision` as declared. The red test is
`crates/loom-cli/tests/decisions_inbox.rs`, which fails before the subcommand exists. The
generated CLI reference (`website/docs/reference/cli.md`) is regenerated in the implementing
commits.

## Acceptance

With a fixture store holding a governor case and requests opened by `route_stop`:

- `decisions list --principal timo` prints the two requests addressed to `timo` and not the one
  addressed to `ana` or the Decided one.
- `decisions decide <outcome request> A --as timo` prints the produced record; `decisions show`
  then shows `produced_record` `canon_decisions` and the governor holds the explicit decision with
  principal `timo`.
- `decisions decide` by a principal not in `eligible` exits non-zero naming `NotEligible`, and the
  request file is byte-identical.
- `decisions take` on a request with a `moderate` recommendation exits non-zero naming
  `NotEligible`; `decisions decide` with the same option succeeds (session `sdc`, 2026-11-21,
  point 5).
- `decisions escalate ID --to ana` moves the request to `ana`'s list; a second escalate from Escalated
  to `ben` moves it again.
- Under holder `decider`, `decisions escalate ID --to ana --reason "needs ana"` leaves the request with
  holder `person`; `decisions show ID` prints it so.
- `decisions supersede DR-1 --by DR-2 --reason "spec moved"` leaves `DR-1` Superseded with
  `superseded_by: DR-2`; `decisions list` no longer shows it, and a second supersede exits non-zero
  naming `DecisionStateConflict`.
- `decisions history --artifact roadmap` prints the requests whose subject is `roadmap`, at every
  revision and in every state, and none whose subject is `vision`.
- `decisions show ID` on a request with two `EscalationSuppressedRecord`s prints both, in the
  order they were appended, with class, subject at revision, reasons, `best` and cause.
- `decisions decide ID A --as timo --at 2026-11-21T12:00:00Z` records `decided_at`
  `2026-11-21T12:00:00Z`; without `--at` the instant comes from the host clock (a test passes a
  fixed clock).
- A store left with a Decided request without a record is completed by the next `decisions list`.
- Two `decisions decide` processes on one request leave one Decided request; the second exits
  non-zero naming `DecisionStoreConflict` or `DecisionStateConflict`.
- The SDK calls give the same store bytes as the commands for the same sequence.
- `website/docs/reference/cli.md` is regenerated; a guide page shows one `list`, `show` and
  `decide` with pasted output.
- `task check` exits 0.

## Out of scope

Expiry (`story:silence-and-expiry-at-a-host-tick`). Approval records and resuming a run
(`story:approval-decisions-resume-held-runs`). A web inbox, notifications, a service.
Resolving or authenticating principals.
