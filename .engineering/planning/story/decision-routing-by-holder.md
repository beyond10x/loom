---
format: aep.planning-md/3
id: story:decision-routing-by-holder
kind: story
status: draft
title: A run's stop on a decision opens a request and routes it by its holder, escalating or suppressing
summary: Class from the host's classes, principals from the host, expires_at from the class, eight holder rules, suppressed escalations appended through the store port, an open request reused per record id
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decider-port-and-grounding-check
revision: 5
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
  has one.
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
`Escalate` to the escalation person, which sets the holder to `person` (`decision.yaml:720-724`):

| holder | what the router does |
|---|---|
| `person` | no decider call; a reusable prior decision is recorded as the recommendation; the request waits on the person |
| `advised` | the decider proposes; the request waits on the person with the recommendation or the reason for none |
| `decider` | a verified proposal is decided by the decider (`Decide`); an `Escalate`, a failure or a withheld proposal is escalated |
| `decider_final` | a verified proposal is decided by the decider; `Escalate` is refused `EscalationSuppressed`; an `Escalate` with `best` decides `best`; an `Escalate` with no `best`, a failure or a withheld proposal decides the denying option, and where no option denies, the request stays Open and the route answers `RunEnds` |
| `rule` | a verified rule proposal is decided by the rule; anything else is escalated |
| `rule_final` | as `rule`, except that the escalation is suppressed: the request stays Open and the route answers `RunEnds` |
| `rule_and_decider` | the rule's verified proposal is recorded as a strong recommendation proposed by `rule`; the decider takes it (`TakeRecommendation`) or vetoes, and a veto is escalated; a rule proposal the check withholds is no recommendation, and the request is escalated |
| `rule_and_decider_final` | as `rule_and_decider`, except that a veto is a `Decide` by the decider on the denying option and the route answers `RunEnds` as refused; a withheld rule proposal leaves the request Open and the route answers `RunEnds` |

The `decider_final` rule is session `sdc`'s answer b of 2026-11-21, now in the contract
(`decision.yaml:184`; `decisions.md:104-106`; `~/beyond10x/sdc/docs/autonomy-levels.md:277-279`).
The rule for a withheld rule proposal under `rule_and_decider*` is answer c (`decisions.md:106-108`,
`autonomy-levels.md:280-282`).

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
holder are those of `decisions.md:94-102`. Session `sdc` accepted on 2026-11-21 that an Escalate
moves the existing request and that the host hands in the eligible principals. The expiry source
is the coordinator's decision: the classes document's per-class duration, no CLI flag.

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
- At `A3` a verified decider reply on `roadmap` is Decided by `decider:stub`; an `Escalate` reply
  leaves the request Escalated, addressed to the escalation person, with holder `person`.
- At `A3` on `rollback` (`rule`), a verified rule proposal is Decided by `rule:stub`; a rule
  `Escalate` leaves the request Escalated to the escalation person with holder `person`, and makes
  no decider call.
- At `A5` an `Escalate` reply with `best: A` appends one `EscalationSuppressedRecord` with class,
  subject, revision, reasons, `A` and cause `escalate`, and the request is Decided `A`; with no
  `best` it is Decided `B` (the denying option); a decider failure is Decided `B` with cause
  `failure`; a reply citing a moved revision is Decided `B` with cause `withheld`.
- At `A5` on a question whose options have no denying option, a decider failure appends a record
  with cause `failure`, leaves the request Open, and answers `RunEnds`.
- At `A4` on `merge`, the rule's verdict is recorded strong and proposed by `rule`; a decider
  `Take` is Decided by the decider through `TakeRecommendation`; a veto leaves it Escalated; a rule
  verdict citing a moved revision leaves no recommendation and the request Escalated. At `A5` a
  veto is Decided `B` with cause `veto` and the route answers `RunEnds` as refused; a withheld rule
  verdict leaves the request Open with cause `withheld` and answers `RunEnds`.
- At `A5` on `budget_overrun` (`rule_final`), a rule `Escalate` appends a record with cause
  `rule_escalate`, leaves the request Open, and answers `RunEnds`.
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
A stop for a review recorded by a producer that is not independent (classes `code_review`,
`spec_acceptance`): no story of this epic builds it yet.
