---
format: aep.planning-md/3
id: story:approval-decisions-resume-held-runs
kind: story
status: draft
title: b10x-loom run routes its approvals through decision requests and resumes only on a recorded grant
summary: DecisionAuthority, grant record writer at the decided case revision, --escalate-when, grounds checked again before an allow; no rule on the command line; a final holder's end stops the run with exit 3
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:silence-and-expiry-at-a-host-tick
- depends_on: story:llm-decider
- depends_on: story:decision-suspension-reason
revision: 7
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
  `HeldCaseGrounds` (`story:grounds-resolve-against-held-cases`), built with the run's
  `--workspace` as the root of `path_line` citations: a ground that no longer resolves makes it
  supersede the request, route the stop anew, and answer `ApprovalRequired`. In a Loom-held case
  an `artifact_revision` ground cannot move alone: moving the artifact raises the case revision
  (`crates/loom-governor/src/lib.rs:1042`), and `resume_after_grant` refuses `CaseMoved` before it
  asks the provider (`crates/loom-commission/src/runtime.rs:2410`). The grounds that can go stale
  while the case stays are `path_line`, `aep_artifact` and `sdc_decisions` citations, and this
  check is what catches them. Canon will set aside a decided authority entry whose ground moved
  (`story:decision-grounds`, session `canon`, 2026-11-21); until Canon judges grounds on those
  entries (`story:authority-records-bound-and-grounded-in-canon`), Loom applies the rule itself,
  here and, once the approvals reach Canon, where they are supplied
  (`story:approvals-supplied-to-canon-evaluation`);
- no flag supplies a rule: Loom ships no rule (`story:decider-port-and-grounding-check`). On the
  command line a `rule`, `rule_final`, `rule_and_decider` or `rule_and_decider_final` holder has
  an absent rule port, which routing treats as a failed rule result
  (`story:decision-routing-by-holder`): `rule` and `rule_and_decider` escalate to the escalation
  person, `rule_final` and `rule_and_decider_final` end the run. A host with a rule hands it to
  `DecisionAuthority` as a `RuleDecider` through `loom_sdk::intake::slice`;
- the slice hands no touched paths to the router, so a run whose `--escalate-when` lists
  `paths_touched` escalates every decider-held approval (`story:escalate-when-conditions`: an
  unknown counts as matching);
- a failure of the store, the router or the writer is a failure to decide, which Commission
  refuses (`crates/loom-commission/src/ports/authority.rs:39`);
- the router sets the request's `expires_at` from the classes document's `expires_after` and the
  run's instant (`story:decision-routing-by-holder`), so `decisions tick --at` over the state
  directory can expire a request a real run opened, or decide it by `silence` with its default.

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
`--state DIR`, `decisions tick` included, uses it as the writer of `canon_authority` records;
`story:question-and-outcome-decisions-hold-runs` adds the writers for outcome and question records.
`b10x-loom resume` then continues the held run through `resume_after_grant`
(`crates/loom-commission/src/runtime.rs:2374`), which asks `DecisionAuthority` again and performs
the held request once only when the record is there. `b10x-loom approve` on a commission whose
Suspension names a decision request is refused, naming the request and the `decisions decide`
command; that refusal is in `approve`'s function in `crates/loom-intake-slice/src/grant.rs`
(`:285-316`), which reads the held Suspension. Without the decision flags, `run`, `approve` and
`resume` behave as today.

`DecisionAuthority` and `GrantRecordWriter` go in their own module of
`crates/loom-intake-slice`, and `--state` on `decisions` in the inbox's module of `crates/loom-cli`.

## Why

Item 3 of the request: resume of the suspended run (`resume_after_grant`) after `Decide`, the
Canon write and `RecordProduced` (`~/beyond10x/sdc/docs/decisions.md:307-312`). Today a grant
comes only from `b10x-loom approve` (`crates/loom-cli/src/lib.rs:235-242`) and `GrantAuthority`
allows only the pending held request (`grant.rs:358-448`). Resume requires `produced_record`
(session `sdc`, 2026-11-21, point 6): the provider allows only on a recorded decision. Both
approval options produce a record (answer d).

## Waits on

- `story:decision-suspension-reason`: it makes the Suspension's `action_request_id`, `action`,
  `arguments` and `expected_case_revision` optional, which forces an edit in
  `crates/loom-intake-slice/src/grant.rs` (`:293`, `:300`, `:315`, `:332`, `:402-403`), the file
  whose `approve` this story changes. `story:question-and-outcome-decisions-hold-runs` and
  `story:approvals-supplied-to-canon-evaluation` are ordered after it through this story.
- `story:silence-and-expiry-at-a-host-tick`: both change `crates/loom-cli`, and a silence
  decision is one this resume continues after.
- `story:llm-decider`: `--decider ROUTE` builds it.
- `story:decision-inbox-commands`, `story:decided-record-written-then-recorded` and
  `story:escalate-when-conditions` through `story:silence-and-expiry-at-a-host-tick`.

## ESS first

`ess/intake/domains/routing.yaml` gains `DecisionRouting` (the decision flags as the slice's
resume context keeps them) and the refusal `HeldForDecisionRequest`. On that commit
`task intake-drift` fails, and `crates/loom-cli/tests/approval_decisions.rs` does not compile.

## Acceptance

On the command line (`crates/loom-cli/tests/approval_decisions.rs`), with the intake slice's
fixture protocol, stub programs, a recorded selector, fixture autonomy documents and principals, a
recorded decider route, and no model call:

- At `A0` a run that reaches `repository.merge` ends `NeedsAuthority`, holds an `Authority`
  Suspension whose request names `DR-…`, and `decisions list --principal timo --state DIR` shows
  it; `decisions decide DR-… A --as timo --state DIR` writes a granted grant with record id
  `repository.merge@<rev>`; `resume` performs the merge request once.
- `resume` before the decision is refused naming the capability as not granted, and performs
  nothing.
- Deciding `B` (deny) writes a denied grant; `resume` is refused and performs nothing.
- At `A2` with a classes document that puts `repository.merge` in class `context` (holder
  `decider`) and a recorded decider reply `A` whose grounds resolve, the run does not stop: the
  request is Decided by `decider:stub`, the grant written, and the merge performed in the same run.
- The same `A2` run with `--escalate-when` listing `paths_touched: ["**"]` stops
  `NeedsAuthority` with the request Escalated to the escalation person and holder `person`.
- At `A4` (merge `rule_and_decider`) with no rule, the run stops `NeedsAuthority` and the request
  is Escalated to the escalation person with no recommendation and the reason
  `no rule configured`.
- At `A5` (merge `rule_and_decider_final`) with no rule, the run ends with
  `stopped: Refused (decision DR-… ended the run: failure)` and exit 3, nothing is performed, and
  the request is Open with one `EscalationSuppressedRecord` of cause `failure` whose reasons name
  `no rule configured`.
- At `A5` with `repository.merge` in class `context` (`decider_final`), `--escalate-when` listing
  `decider: escalate` and a recorded decider `Escalate` with `best: A`, the run ends with exit 3
  naming the request and cause `condition`, nothing is performed, the request is Open with one
  `EscalationSuppressedRecord`, and `decisions list --principal decider:stub --state DIR` lists
  it; a second `run` of the same intent routes that request again and opens no other.
- At `A0` with the classes document giving the merge class `expires_after: 3600` and the run's
  instant fixed at 12:00, the request's `expires_at` is 13:00; `decisions tick --at 12:59 --state
  DIR` leaves it Open, `decisions tick --at 13:00 --state DIR` expires it, and `resume` then
  performs nothing and the Suspension stays.
- At `A1` with the same expiry and a recorded decider reply `A`, strong, with a default on silence,
  `decisions tick --at 13:00 --state DIR` decides the request by `silence`, writes the granted
  grant `repository.merge@<rev>` and records it; `resume` then performs the merge once.
- With the case moved after the decision (the test raises an artifact through `update_revision`),
  `resume` is refused `CaseMoved` naming both case revisions, `DecisionAuthority` is not asked (a
  counting store records no read), nothing is performed, and the request stays Decided.
- At `A1` a request whose recorded decider recommendation `A`, strong, has a `path_line` ground on lines 1-3
  of `notes.md` in the workspace, taken with `decisions take DR-… --as timo --state DIR`: after
  those lines are rewritten and not committed, the case revision is unchanged, `resume` is refused
  naming `repository.merge` as not granted (not `CaseMoved`), the request is Superseded naming a
  new Open request, and nothing is performed.
- `approve` on a commission held for `DR-…` exits non-zero naming the request and the command.
- A store failure injected into `DecisionAuthority` ends the run with nothing performed.
- `run --mandate … --autonomy-level A0` exits 2 naming both flags and opens nothing.
- Without the decision flags, `crates/loom-cli/tests/resume_after_grant.rs` passes, unchanged by
  this story.

As a library test (`crates/loom-intake-slice/tests/decision_authority_rule.rs`) that drives the
slice through `loom_sdk::intake::slice` with `DecisionAuthority` given a `RuleDecider` over a
fixture function, a recorded decider and no model call:

- At `A4` with a rule verdict that grants and a recorded decider reply `Take`, the run does not
  stop: the request is Decided by the decider through `TakeRecommendation`, the grant written, and
  the merge performed in the same run.
- At `A5` with a rule verdict that grants and a recorded decider veto, the slice reports
  `stopped: Refused (decision DR-… ended the run: veto)`, the merge is not performed, no
  Suspension is held, and the request is Decided `B` with a denied grant `repository.merge@<rev>`.
- At `A4` with `escalate_when: [{decider: escalate}]` and a recorded decider `Escalate`, the request
  is Escalated to the escalation person; at `A5` the same run reports
  `stopped: Refused (decision DR-… ended the run: condition)`, and the request is Open with one
  `EscalationSuppressedRecord` of cause `condition`.
- `task check` exits 0.

## Out of scope

Question and outcome decisions (`story:question-and-outcome-decisions-hold-runs`). Mandate
approvals with decision requests: `--mandate` and the decision flags are refused together.
Supplying the grants to Canon's evaluation (`story:approvals-supplied-to-canon-evaluation`).
Computing touched paths from the change's diff. A command-line flag that supplies a rule.
