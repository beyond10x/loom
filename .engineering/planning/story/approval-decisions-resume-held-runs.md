---
format: aep.planning-md/3
id: story:approval-decisions-resume-held-runs
kind: story
status: draft
title: b10x-loom run routes its approvals through decision requests and resumes only on a recorded grant
summary: DecisionAuthority, grant record writer at the decided case revision, --escalate-when, grounds checked again before an allow; a final holder's end stops the run with exit 3
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:silence-and-expiry-at-a-host-tick
- depends_on: story:llm-decider
revision: 5
---
## Outcome

`b10x-loom run --state DIR` can route its approvals through decision requests. With the decision
flags (`--autonomy-level`, `--autonomy-presets FILE`, `--decision-classes FILE`,
`--admitted FILE`, `--override CLASS=HOLDER`, `--escalate-when FILE`, `--principals FILE`,
`--decider ROUTE`) the run's authority provider is `DecisionAuthority` (in
`b10x-loom-intake-slice`), over the `FileDecisionStore` in the state directory:

- asked about a capability for an action request, it answers `Allow` when a Decided request for
  that action and capability carries a `produced_record` at the request's case revision whose
  grant is `granted`, `Deny` when that record is `denied`, and otherwise routes the stop
  (`route_stop`) and answers `ApprovalRequired` with a request naming the decision request id. If
  the route decided the request at once (a decider or rule holder) and its record is written, it
  answers from the record instead;
- before it answers from a record, it checks the Decided request's grounds again through
  `HeldCaseGrounds` (`story:grounds-resolve-against-held-cases`): a ground that no longer resolves
  makes it supersede the request, route the stop anew, and answer `ApprovalRequired`. Canon will
  set aside a decided authority entry whose ground moved (`story:decision-grounds`, session
  `canon`, 2026-11-21), and Loom will hand Canon its approvals
  (`story:approvals-supplied-to-canon-evaluation`); until Canon judges grounds on those entries
  (`story:authority-records-bound-and-grounded-in-canon`), Loom applies the rule to its approvals
  itself;
- the slice hands no touched paths to the router, so a run whose `--escalate-when` lists
  `paths_touched` escalates every decider-held approval (`story:escalate-when-conditions`: an
  unknown counts as matching);
- a failure of the store, the router or the writer is a failure to decide, which Commission
  refuses (`crates/loom-commission/src/ports/authority.rs:39`);
- the router sets the request's `expires_at` from the classes document's `expires_after` and the
  run's instant (`story:decision-routing-by-holder`), so `decisions tick --at` over the state
  directory can expire a request a real run opened.

On `RunEnds` (a final holder with no usable result, an `escalate_when` condition under a final
holder, `rule_final`, or a veto under `rule_and_decider_final`) the run ends:

- `DecisionAuthority` answers `Deny` for the stop's capability, and `Deny` for every capability it
  is asked about later in the same run, so nothing more that needs authority is performed;
- when the Commission loop ends, the slice reports `stopped: Refused (decision <id> ended the run:
  <cause>)` and `run` exits 3, the status the slice's existing table gives `Refused`
  (`crates/loom-cli/src/lib.rs:26-35`, `:195-204`); answer 3 of session `sdc` names exit 6 for
  `run-trial.sh`, which is SDC's table, not Loom's;
- no Suspension is held, so `resume` is refused `NothingSuspended`;
- the request stays as the route left it: Decided on the denying option after a veto, with its
  denied grant written; otherwise Open. It is left Open rather than superseded: `Supersede` needs a
  newer request (`~/beyond10x/sdc/ess/domains/decision.yaml:742-745`) and there is none, a listed
  person may still decide it (`~/beyond10x/sdc/docs/decisions.md:110`), and the next run of the
  case finds it by its record key and routes it again instead of opening a second one.

Commission does not await an action a verdict denied (`crates/loom-commission/src/runtime.rs:51-52`),
so an action that needs no capability may still run before the loop ends; no Commission port lets
an authority provider end a run outright.

`GrantRecordWriter` writes an approval request's record as the slice's grant
(`intake.routing.Grant`, `crates/loom-intake-slice/src/grant.rs:116`) with record id
`<capability>@<case_revision>` at the request's decided case revision and the request's principal,
idempotent by that id (session `sdc`, 2026-11-21, answers e and g). `b10x-loom decisions` given
`--state DIR` uses it. `b10x-loom resume` then continues the held run through `resume_after_grant`
(`crates/loom-commission/src/runtime.rs:2374`), which asks `DecisionAuthority` again and performs
the held request once only when the record is there. `b10x-loom approve` on a commission whose
Suspension names a decision request is refused, naming the request and the `decisions decide`
command. Without the decision flags, `run`, `approve` and `resume` behave as today.

`DecisionAuthority` and `GrantRecordWriter` go in their own module of
`crates/loom-intake-slice`, and `--state` on `decisions` in the inbox's module of `crates/loom-cli`.

## Why

Item 3 of the request: resume of the suspended run (`resume_after_grant`) after `Decide`, the
Canon write and `RecordProduced` (`~/beyond10x/sdc/docs/decisions.md:307-312`). Today a grant
comes only from `b10x-loom approve` (`crates/loom-cli/src/lib.rs:235-242`) and `GrantAuthority`
allows only the
pending held request (`grant.rs:358-448`). Resume requires `produced_record` (session `sdc`,
2026-11-21, point 6): the provider allows only on a recorded decision. Both approval options
produce a record (answer d).

## Waits on

`story:silence-and-expiry-at-a-host-tick`: both change `crates/loom-cli`, and a silence
decision is one this resume continues after. `story:llm-decider`: `--decider ROUTE` builds it.
`story:decision-inbox-commands`, `story:decided-record-written-then-recorded` and
`story:escalate-when-conditions` through the first.

## ESS first

`ess/intake/domains/routing.yaml` gains `DecisionRouting` (the decision flags as the slice's
resume context keeps them) and the refusal `HeldForDecisionRequest`. On that commit
`task intake-drift` fails, and `crates/loom-cli/tests/approval_decisions.rs` does not compile.

## Acceptance

With the intake slice's fixture protocol, stub programs, a recorded selector, fixture autonomy
documents and principals, and no model call:

- At `A0` a run that reaches `repository.merge` ends `NeedsAuthority`, holds an `Authority`
  Suspension whose request names `DR-…`, and `decisions list --principal timo --state DIR` shows
  it; `decisions decide DR-… A --as timo --state DIR` writes a granted grant with record id
  `repository.merge@<rev>`; `resume` performs the merge request once.
- `resume` before the decision is refused naming the capability as not granted, and performs
  nothing.
- Deciding `B` (deny) writes a denied grant; `resume` is refused and performs nothing.
- At `A4` with a stub rule whose verdict holds and a recorded decider reply `Take`, the run does
  not stop: the request is Decided by the decider, the grant written, and the merge performed in
  the same run.
- At `A4` with `--escalate-when` listing `paths_touched: ["**"]`, the same run stops with the
  request Escalated to the escalation person.
- At `A5` (an eval run) with a stub rule whose verdict grants and a recorded decider veto, the run
  ends with `stopped: Refused (decision DR-… ended the run: veto)` and exit 3, the merge is not
  performed, no Suspension is held, `resume` is refused `NothingSuspended`, and the request is
  Decided `B` with a denied grant `repository.merge@<rev>`.
- At `A5` with `--escalate-when` listing `decider: escalate` and a recorded decider `Escalate`,
  the run ends with exit 3 naming the request and cause `condition`, nothing is performed, the
  request is Open with one `EscalationSuppressedRecord`, and `decisions list --principal
  decider:stub --state DIR` lists it; a second `run` of the same intent routes that request again and opens no
  other.
- At `A0` with the classes document giving the merge class `expires_after: 3600` and the run's
  instant fixed at 12:00, the request's `expires_at` is 13:00; `decisions tick --at 12:59 --state
  DIR` leaves it Open, `decisions tick --at 13:00 --state DIR` expires it, and `resume` then
  performs nothing and the Suspension stays.
- With the case moved after the decision, `resume` is refused `CaseMoved` and performs nothing.
- A granted request whose ground cites `spec@<rev>`: after `spec` moves and before `resume`, the
  resume is refused, the request is Superseded naming a new Open request, and nothing is
  performed.
- `approve` on a commission held for `DR-…` exits non-zero naming the request and the command.
- A store failure injected into `DecisionAuthority` ends the run with nothing performed.
- `run --mandate … --autonomy-level A0` exits 2 naming both flags and opens nothing.
- Without the decision flags, `crates/loom-cli/tests/resume_after_grant.rs` passes unchanged.
- `task check` exits 0.

## Out of scope

Question and outcome decisions (`story:question-and-outcome-decisions-hold-runs`). Mandate
approvals with decision requests: `--mandate` and the decision flags are refused together.
Supplying the grants to Canon's evaluation (`story:approvals-supplied-to-canon-evaluation`).
Computing touched paths from the change's diff.
