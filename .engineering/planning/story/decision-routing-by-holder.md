---
format: aep.planning-md/3
id: story:decision-routing-by-holder
kind: story
status: draft
title: A run's stop on a decision opens a request and routes it by its holder, escalating or suppressing
summary: Class from the host's classes, principals from the host, expires_at from the class, eight holder rules with an absent decider or rule as a failed result, suppressed escalations appended through the store port, an open request reused per record id
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decider-port-and-grounding-check
revision: 7
---
## Outcome

`route_stop` in `b10x-loom-decision` turns a run's stop on a decision into a decision request and
routes it by its holder. It is given a `DecisionStop` (case and its current revision, what it
unblocks, the subject artifact at its revision and its path, the question; the options for a
question), the run's grant table (`story:autonomy-grant-table`), the host's classes document, the
host's `EligiblePrincipals` (per class the persons, in the order they are asked, and the person an
escalation goes to), a decider and a rule (`Decider` ports, either absent), the decision store, a
citation resolver, and the instant. Loom lists no principal itself.

Opening:

- The class is the one whose `actions` list the stop's action; an action no class lists is
  refused `UnclassifiedAction`, naming it, and nothing is opened.
- Options: an approval gets `A` grant (consequence `grant: granted`) and `B` deny (`grant:
  denied`), both with `produces: canon_authority`; an outcome gets `A` take the decision, with
  `produces: canon_decisions`, and `B` leave it untaken, with no `produces`; a question takes the
  host's options, each with `produces: artifact_revision`
  (`~/beyond10x/sdc/ess/domains/decision.yaml:95`, `:204-208`). The second option of an outcome
  is "leave it untaken" because Canon's `pending_decisions` offers only `[taken]`
  (`story:pending-decisions`, session `canon`, 2026-11-21) and a request needs two options
  (`decision.yaml:405-408`); session `sdc` accepted it on 2026-11-21 (answer d), and the contract
  now says so (`decision.yaml:67`).
- The denying option, wherever a rule below names it, is the one whose `consequence.grant` is
  `denied` (session `sdc`, 2026-11-21, third message; `decision.yaml:553-555`). Only an approval
  has one; an outcome and a question have none.
- `question_key` is `<case>|<action>|<question id or empty>` and `request_id` is
  `DR-<case>-<action or question id>-<n>`, `n` one more than the requests already stored under the
  key (`decision.yaml:13`, `:227`).
- `record_key` is the record id the decision would produce at the stop's case revision:
  `<capability>@<case_revision>` for an approval, `<outcome>/<decision>@<case_revision>` for an
  outcome (session `sdc`, 2026-11-21, answer g), and `question:<question id>@<subject revision>`
  for a question, whose record revision is not known before the answer.
- `expires_at` is the instant plus `expires_after(class)` from the grant table, where the classes
  document gives the class one; otherwise the request has none and only a person, a decider or a
  rule ends it.
- When the store holds an Open or Escalated request for the `record_key`, the router reuses it: it
  routes that request and opens none. A `Request` for the key while one is open would be refused
  `RecordKeyOpen` (`story:decision-request-entity`); the router reads the store first and passes
  what it found as `open_for_record`, so a race between two routers ends in that refusal and the
  loser reuses the winner's request (answer g, the coordinator's choice). The contract states this
  rule in prose only (`~/beyond10x/sdc/docs/decisions.md:225-226`); Loom enforces it.
- An Open or Escalated request with the same `question_key` at another subject revision is
  superseded by the new request. A Decided request without a `produced_record` whose
  `decided_case_revision` is no longer the case's revision is superseded by the new request too:
  its record cannot be written any more (answer e, `decisions.md:226-228`,
  `story:decided-record-written-then-recorded`).
- `eligible` is the class's persons plus the decider's or rule's principal the holder involves;
  `addressed_to` is the first person under `person` and `advised`, the decider under `decider*`
  and `rule_and_decider*`, the rule under `rule*`.

Routing, per holder (`decision.yaml:182-184`, `decisions.md:94-108`); every proposal passes
`check_grounds` first and is recorded through `Recommend`, verified or withheld. An escalation is
`Escalate` to the escalation person, which sets the holder to `person` (`decision.yaml:720-724`).

An absent decider or rule port is a failed result of that port. The contract is final and has no
cause for a missing port; it names a "missing, failed or withheld" decider result under the final
holders (`decision.yaml:184`, `~/beyond10x/sdc/docs/autonomy-levels.md:277-279`), so every row
below treats an absent port as that port's failure: where a row escalates, the Escalate's reason is
`no decider configured` or `no rule configured`; where it records an `EscalationSuppressedRecord`,
the cause is `failure` and the reasons carry the same words; where it waits on a person, the
request's `no_recommendation_reason` carries them. A decider failure (`Unavailable`,
`InvalidReply`, or no decider) counts as an `Escalate` with no `best`
(`story:decider-port-and-grounding-check`), and under `rule_and_decider*` that is a veto.

| holder | what the router does |
|---|---|
| `person` | no decider call; a reusable prior decision is recorded as the recommendation; the request waits on the person |
| `advised` | the decider proposes; the request waits on the person with the recommendation or the reason for none (a failure included) |
| `decider` | a verified proposal is decided by the decider (`Decide`); an `Escalate`, a failure or a withheld proposal is escalated |
| `decider_final` | a verified proposal is decided by the decider; `Escalate` is refused `EscalationSuppressed`; an `Escalate` with `best` decides `best`; an `Escalate` with no `best`, a failure or a withheld proposal decides the denying option and the route answers `Decided`; where no option denies (an outcome, a question), the request stays Open and the route answers `RunEnds` |
| `rule` | a verified rule proposal is decided by the rule; anything else (an `Escalate`, a failure, a withheld proposal) is escalated |
| `rule_final` | as `rule`, except that the escalation is suppressed: the request stays Open and the route answers `RunEnds` |
| `rule_and_decider` | the rule's verified proposal is recorded as a strong recommendation proposed by `rule`; the decider takes it (`TakeRecommendation`) or vetoes (a failure included), and a veto is escalated with the recommendation kept; a rule proposal the check withholds, or a rule failure, is no recommendation, and the request is escalated with no decider call |
| `rule_and_decider_final` | as `rule_and_decider`, except: a veto is a `Decide` by the decider on the denying option and the route answers `RunEnds` as refused; a decider failure decides the denying option and the route answers `Decided`; where no option denies, a veto or a decider failure leaves the request Open and the route answers `RunEnds`; a withheld rule proposal or a rule failure leaves the request Open and the route answers `RunEnds` |

The `decider_final` rule is session `sdc`'s answer b of 2026-11-21, now in the contract
(`decision.yaml:184`; `decisions.md:104-106`; `autonomy-levels.md:277-279`), and the rule that a
stop with no denying option ends the run applies to `rule_and_decider_final` too (the same
lines). The rule for a withheld rule proposal under `rule_and_decider*` is answer c
(`decisions.md:106-108`, `autonomy-levels.md:280-282`).

The holder alone decides whether an escalation goes to a person or is suppressed: the request
stores its holder and the guards read it, not the level (`decisions.md:89-91`), and an Escalate
always sets the holder to `person` (`decision.yaml:720-724`). There is no run-wide escalation
mode (`story:autonomy-grant-table`): the document's `escalation.mode` (`person` at A2 to A4,
`final` at A5, `autonomy-levels.md:385-387`) is what the presets' holders already say. So a
lowering at A5 to `decider` escalates to the person, and a raise at A1 to `decider` (within an
admitted ceiling) escalates to the escalation person though A0 and A1 have no decider escalation
in the document.

A decider `Propose` of another option than the rule's counts as a veto. The route answers
`AwaitsPrincipal` (request, principal), `Decided` (request), or `RunEnds` (request, cause, and
whether the run ends as refused). What a host does on `RunEnds` is each host story's
(`story:approval-decisions-resume-held-runs`, `story:question-and-outcome-decisions-hold-runs`,
`story:tree-waits-open-decision-requests`). Every suppressed escalation (under `decider_final`,
`rule_final` and `rule_and_decider_final`) appends one `EscalationSuppressedRecord` beside the
request through the `DecisionStore` port (`story:decision-request-entity`): the class, the subject
at its revision, the reasons, `best` where there is one, and the cause, one of `escalate`,
`failure`, `withheld`, `veto` or `rule_escalate` (`autonomy-levels.md:460-462`; the cause is answer
b, `autonomy-levels.md:279`).

Its code goes in its own module of `crates/loom-decision`; `lib.rs` gains one `mod` line.

## Why

Item 3 of the request (routing to eligible principals, escalation to a named person) and the
recording of `escalation_suppressed` of item 2 (`decisions.md:304-312`). The decision rules per
holder are those of `decisions.md:94-108`. Session `sdc` accepted on 2026-11-21 that an Escalate
moves the existing request and that the host hands in the eligible principals. The expiry source
is the coordinator's decision: the classes document's per-class duration, no CLI flag. An absent
port as a failed result is the coordinator's decision too: the contract is final, so no cause is
added.

## Waits on

`story:decider-port-and-grounding-check`: the port and the check, and both change
`ess/domains/decision.yaml` and `crates/loom-decision`. `story:autonomy-grant-table` (with
`expires_after`) and `story:decision-request-entity` (the port's record operations and the record
type) through it.

## ESS first

`ess/domains/decision.yaml` gains `DecisionStop`, `EligiblePrincipals`, `RouteOutcome`
(`AwaitsPrincipal`, `Decided`, `RunEnds`) and the refusal `UnclassifiedAction`. On that commit
`task drift` fails, and `crates/loom-decision/tests/routing.rs` does not compile.

## Acceptance

With fixture presets and classes copying the table of `autonomy-levels.md:242-257`, a stub decider
and a stub rule that answer fixed replies, a `PathLineResolver` over fixture files, and the
in-memory store:

- At `A0` an approval of `roadmap.approve` on case `C` opens one request `DR-C-roadmap.approve-1`
  with `question_key` `C|roadmap.approve|`, addressed to the first person, makes no decider call
  (the stub counts calls), and answers `AwaitsPrincipal`; after it is superseded by a stop at a
  new subject revision, the new request is `DR-C-roadmap.approve-2`.
- At `A0` with a Decided prior request on the same `question_key` and subject revision whose
  grounds resolve, the new request carries a strong recommendation proposed by `prior_decision`
  whose first ground cites the prior, makes no decider call, and waits on the person.
- The approval request has options `A` (`grant: granted`) and `B` (`grant: denied`), both
  `produces: canon_authority`; an outcome stop `close/taken` gives `A` with `produces:
  canon_decisions` and `B` "leave it untaken" with no `produces`; a question stop with host
  options `A`, `B`, `C` keeps them, each `produces: artifact_revision`.
- With `roadmap: {expires_after: 86400}` and the instant `2026-11-21T12:00:00Z`, the request's
  `expires_at` is `2026-11-22T12:00:00Z`; a `vision` request has no `expires_at`.
- At `A1` the decider is called once and the request waits on the person with the verified
  recommendation; with a reply citing a moved revision it waits with `no_recommendation_reason`.
- At `A1` with no decider, the request waits on the person with no recommendation and
  `no_recommendation_reason` `no decider configured`.
- At `A3` a verified decider reply on `roadmap` is Decided by `decider:stub`; an `Escalate` reply
  leaves the request Escalated, addressed to the escalation person, with holder `person`.
- At `A3` with no decider on `roadmap` (`decider`), and with no rule on `rollback` (`rule`), each
  request is Escalated to the escalation person with holder `person` and the reason
  `no decider configured` or `no rule configured`; no `EscalationSuppressedRecord` is appended.
- At `A3` on `rollback` (`rule`), a verified rule proposal is Decided by `rule:stub`; a rule
  `Escalate` leaves the request Escalated to the escalation person with holder `person`, and makes
  no decider call.
- At `A5` an `Escalate` reply with `best: A` appends one `EscalationSuppressedRecord` with class,
  subject, revision, reasons, `A` and cause `escalate`, and the request is Decided `A`; with no
  `best` it is Decided `B` (the denying option); a decider failure is Decided `B` with cause
  `failure`; a reply citing a moved revision is Decided `B` with cause `withheld`.
- At `A5` on `roadmap` (`decider_final`) with no decider, and on `merge` (`rule_and_decider_final`)
  with a rule verdict that grants and no decider, each request is Decided `B` with one record of
  cause `failure` whose reasons name `no decider configured`, and the route answers `Decided`.
- At `A5` on a question whose options have no denying option, a decider failure appends a record
  with cause `failure`, leaves the request Open, and answers `RunEnds`; an outcome stop `close`
  under `decider_final` with no decider does the same.
- At `A5` on an outcome stop classed `rule_and_decider_final`, a decider veto, and a decider
  failure, each leave the request Open with one record of cause `veto` or `failure` and answer
  `RunEnds`.
- At `A4` on `merge`, the rule's verdict is recorded strong and proposed by `rule`; a decider
  `Take` is Decided by the decider through `TakeRecommendation`; a veto leaves it Escalated; a rule
  verdict citing a moved revision leaves no recommendation and the request Escalated. At `A5` a
  veto is Decided `B` with cause `veto` and the route answers `RunEnds` as refused; a withheld rule
  verdict leaves the request Open with cause `withheld` and answers `RunEnds`.
- At `A4` on `merge` with no decider, the rule's strong recommendation is recorded and the request
  is Escalated to the escalation person keeping it, with the reason `no decider configured`.
- At `A4` on `merge` with no rule, the request carries no recommendation and is Escalated to the
  escalation person with the reason `no rule configured`, and the decider is not called.
- At `A5` on `budget_overrun` (`rule_final`), a rule `Escalate` appends a record with cause
  `rule_escalate`, leaves the request Open, and answers `RunEnds`; with no rule, and on `merge`
  (`rule_and_decider_final`) with no rule, the request stays Open with one record of cause
  `failure` whose reasons name `no rule configured`, and the route answers `RunEnds`.
- At `A5` with the override `roadmap: decider` (a lowering), a decider `Escalate` leaves the request
  Escalated to the escalation person with holder `person` and appends no
  `EscalationSuppressedRecord`.
- At `A1` with the override `roadmap: decider` and an admitted entry `roadmap: decider` (a raise),
  a decider `Escalate` leaves the request Escalated to the escalation person with holder `person`.
- The same stop routed twice opens one request: the second routing finds the open request for the
  record key and reuses it. A stop at a new subject revision supersedes the open request, and the
  new request names it in `supersedes`.
- Two routings of one stop that race (the store fixture interleaves them) leave one request: the
  second `Request` is refused `RecordKeyOpen` and that routing reuses the first request.
- A Decided request without a record whose decided case revision moved is superseded when the stop
  is routed again at the new revision.
- An action that no class lists is refused `UnclassifiedAction` and the store is unchanged.
- Two routings of the same inputs from fresh stores give byte-identical stores.
- `task check` exits 0.

## Out of scope

Writing the Canon record and resuming (`story:decided-record-written-then-recorded`,
`story:approval-decisions-resume-held-runs`). What a host does on `RunEnds` (the three host
stories). A person's Escalate, Decide or Take (`story:decision-inbox-commands`). Firing expiry
(`story:silence-and-expiry-at-a-host-tick`). `escalate_when` (`story:escalate-when-conditions`).
Building a stop from Canon's `pending_decisions` (`story:requests-from-canon-pending-decisions`).
A stop for a review recorded by a producer that is not independent
(`story:non-independent-review-becomes-decision-request` routes it through this router). A new
cause for a missing port.
