---
format: aep.planning-md/3
id: story:decision-routing-by-holder
kind: story
status: proposed
title: A run's stop on a decision opens a request and routes it by its holder, escalating or suppressing
summary: Class from the host's classes, principals from the host, expires_at through the host's ExpiryInstant, eight holder rules with an absent port as decider:none or rule:none failing, suppressed escalations appended through the store port, an open or decided request reused per record id, a racing loser reusing after StoreConflict
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decider-port-and-grounding-check
scope:
- confidence: cited
  path: crates/loom-decision/src/lib.rs
- confidence: inferred
  path: crates/loom-decision/src/routing.rs
- confidence: cited
  path: crates/loom-decision/tests/routing.rs
- confidence: cited
  path: ess/domains/decision.yaml
revision: 19
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:20Z", actor: "agent:sdc", revision: 17, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`route_stop` in `b10x-loom-decision` turns a run's stop on a decision into a decision request and
routes it by its holder. It is given a `DecisionStop` (case and its current revision, what it
unblocks, the subject artifact at its revision and its path, the question; the options for a
question), the run's grant table (`story:autonomy-grant-table`), the host's classes document
(`DecisionClasses`), the host's `EligiblePrincipals` (per class the persons, in the order they are
asked, and the person an escalation goes to), a decider and a rule (`Decider` ports, either
absent), the decision store, a citation resolver, the instant, and an `ExpiryInstant` port
(`fn after(&self, seconds: u64) -> Timestamp`), through which the host turns a class's
`expires_after` into an instant. Loom lists no principal itself.

The host gives the expiry instant because the contract takes `expires_at` as an input of `Request`
(`~/beyond10x/sdc/ess/domains/decision.yaml:413` at `00e3557`, `:442`), and `Timestamp` is a string
(`generated/rust/loom/src/primitives.rs:25`): `crates/loom-decision` does no time arithmetic and
its `Cargo.toml` gains no dependency. Each host implements the port with its own clock: the slice
in `story:approval-decisions-resume-held-runs`, whose implementation
`story:question-and-outcome-decisions-hold-runs` passes too, and the tree in
`story:tree-waits-open-decision-requests`. `ExpiryInstant` has an ESS home, in
`ess/domains/decision.yaml`, by the epic's rule for host ports: its answer becomes a command input
(`expires_at` of `Request`), as `CaseRevisions`' answer becomes `case_revision`
(`story:decision-inbox-commands`).

A host calls `complete_pending` (`story:decided-record-written-then-recorded`) before it routes
any stop, so a record a crash left written but unrecorded is recorded before the supersede rule
below reads the request.

This story declares three stop kinds, approval, outcome and question.
`story:non-independent-review-becomes-decision-request` adds a fourth, review, to `DecisionStop`
in this story's surface (`ess/domains/decision.yaml` and the routing module of
`crates/loom-decision`), with its options, its `question_key` and `record_key`, and a second
reason for an absent decider.

Opening:

- The stop's `action` (`Unblocks.action`, `decision.yaml:75`): for an approval, the protocol action
  that is approval-required, with its `capability` and, when the requirement has one, its scope
  (`DecisionStop.scope`, an optional map of key to value, read only by `record_key` below); for
  an outcome, the outcome id, with
  `Unblocks.outcome` set to it and the decision in the stop (`close` for the outcome `close`,
  decision `taken`); for a question, the protocol action whose writer handed the question off,
  with `question_id` set (`decision.yaml:77`).
- The class is the one whose `actions` list the stop's action: the router scans the entries of the
  classes document itself, so the autonomy module gains no lookup. An action no class lists is
  refused `UnclassifiedAction`, naming it, and nothing is opened.
- A built-in class (`budget_overrun`, `limit_hit`) lists no actions: the classes document accepts
  one only as `{kind: control}` (`story:autonomy-grant-table`). A stop gets it from
  `DecisionStop.builtin_class`, an optional member the host that raises the stop sets; with it the
  router takes that class and scans no `actions`. A value other than `budget_overrun` or
  `limit_hit` is refused `UnclassifiedAction`, naming it. No story of this epic raises such a stop;
  `story:budget-and-limit-stops-raise-decision-requests` of `epic:tree-daemon-under-enforced-limits`
  does.
- Options: an approval gets `A` grant (consequence `grant: granted`) and `B` deny (`grant:
  denied`), both with `produces: canon_authority`; an outcome gets `A` take the decision, with
  `produces: canon_decisions`, and `B` leave it untaken, with no `produces`; a question takes the
  host's options, each with `produces: artifact_revision`
  (`decision.yaml:95`, `:206-210`). The second option of an outcome
  is "leave it untaken" because Canon's `pending_decisions` offers only `[taken]`
  (`story:pending-decisions`, session `canon`, 2026-11-21) and a request needs two options
  (`decision.yaml:420-423`); session `sdc` accepted it on 2026-11-21 (answer d), and the contract
  now says so (`decision.yaml:67`).
- The denying option, wherever a rule below names it, is the one whose `consequence.grant` is
  `denied` (session `sdc`, 2026-11-21, third message; `decision.yaml:593-595`). Only an approval
  has one; an outcome and a question have none.
- `question_key` is `<case>|<action>|<question id or empty>` and `request_id` is
  `DR-<case>-<action or question id>-<n>`, `n` one more than the requests already stored under the
  key (`decision.yaml:13`, `:229`).
- `record_key` is the record id the decision would produce at the stop's case revision, the case
  revision written `r<N>` as the governor stamps it (`crates/loom-governor/src/lib.rs:1075`).
  This is the one statement of its form; `story:requests-from-canon-pending-decisions` reads it.
  - An approval's key is built by program from the stop's fields, in this order: the capability,
    then the scope's pairs in key order when the stop has a scope, then `r<N>`.
    - With no scope it is `<capability>@r<N>`.
    - With a scope it is `<capability>[<key>=<value>,<key>=<value>…]@r<N>`. The pairs are joined
      by `,` and sorted by key, keys compared by code point, as Canon orders a scope
      (`~/beyond10x/canon/crates/canon/src/eval/pending.rs:15-18` at `49ff2ce`). An empty scope
      is no scope.
    - In the capability, in each key and in each value, the characters `%`, `[`, `]`, `=`, `,`
      and `@` are written `%25`, `%5B`, `%5D`, `%3D`, `%2C` and `%40`. No other character is
      escaped. A capability that holds none of them, such as `repository.merge`, is written as
      it is.
    - Two stops that differ in capability, scope or case revision never give one key.
    - The record writer parses a key back from the right. The last `@` starts `r<N>`. A key
      whose part before that `@` ends in `]` holds a scope, from its first `[`. Pairs split at
      `,`, a key from its value at `=`, and each part is unescaped.
  - An outcome's key is `<outcome>/<decision>@r<N>` (session `sdc`, 2026-11-21, answer g). The
    contract has no field for an outcome's decision name, so the record writer parses it back out
    of `record_key` (`story:decided-record-written-then-recorded`). An outcome request decided
    "leave it untaken" keeps the outcome's `record_key`, though it produces no record.
  - A question's key is `question:<question id>@<subject revision>`, because its record revision
    is not known before the answer.
- `expires_at` is `ExpiryInstant::after(expires_after(class))`, where the classes document gives
  the class an `expires_after`; otherwise the request has none and only a person, a decider or a
  rule ends it.
- Store lookups: the router uses the port's "find the Open or Escalated request for a record key"
  and "list requests" (`story:decision-request-entity`); the port has no lookup of a Decided
  request by record key or of any request by `question_key`, so the reuse of a Decided request,
  the prior below and both supersede rules scan `list`.
- When the store holds an Open or Escalated request for the `record_key`, the router reuses it: it
  routes that request and opens none. Otherwise it opens one with `Request`, passing no
  `open_for_record`. Two routers that both read the store before either wrote compute the same
  `request_id`; the loser's write is refused by the port as `StoreConflict`
  (`story:decision-request-entity`), and the loser re-finds the open request for the `record_key`
  and reuses it (answer g, the coordinator's choice). `RecordKeyOpen` is the entity's refusal for a
  caller that passes the open request it found as `open_for_record`; the router never does, since
  it reuses that request. The contract states the one-open-request rule in prose only
  (`~/beyond10x/sdc/docs/decisions.md:225-226`); Loom enforces it.
- When the store holds a Decided request for the `record_key` that is not superseded (below), the
  router opens none and answers `Decided` with it: the host answers from its record, completes it,
  or, for "leave it untaken", goes on with no record. So an outcome left untaken opens no new
  request at that case revision, as the slice and the tree read it.
- An Open or Escalated request with the same `question_key` at another subject revision is
  superseded by the new request. A Decided request without a `produced_record` whose
  `decided_case_revision` is no longer the case's revision is superseded by the new request too:
  its record cannot be written any more (answer e, `decisions.md:226-228`,
  `story:decided-record-written-then-recorded`). In a Loom-held case moving the subject artifact
  raises the case revision (`crates/loom-governor/src/lib.rs:1042`), so an approval stop at a new
  subject revision has a new `record_key` and supersedes; a stop at the same case revision reuses.
- An absent decider port has the principal `decider:none` and an absent rule port `rule:none`,
  which the contract's principal grammar allows (`decider:<kind>`, `rule:<name>`,
  `decision.yaml:55`, `:192`). `eligible` is the class's persons plus the principal of the decider
  or rule the holder involves, the fixed one where the port is absent; `addressed_to` is the first
  person under `person` and `advised`, the decider's principal under `decider*` and
  `rule_and_decider*`, the rule's under `rule*`. Where an absent port's failure decides the denying
  option, `decided_by` is that fixed principal.

Routing, per holder (`decision.yaml:184-186`, `decisions.md:94-108`); every proposal passes
`check_grounds` first and is recorded through `Recommend`, verified or withheld. An escalation is
`Escalate` to the escalation person, which sets the holder to `person` (`decision.yaml:760-764`).

Under `person` the router calls no decider. It scans `list` for a Decided, not superseded request
with the same `question_key` and subject revision; if there is one, it builds a reply from it, a
`Propose` of its decided option with strength `strong`, source `prior_decision`, its first ground
citing the prior request and the prior's grounds after it, and passes that reply to
`check_grounds` with the store, which resolves the prior's `sdc_decisions` ground itself
(`story:decider-port-and-grounding-check`). A verified one is the recommendation; with no prior, or a withheld one, the
request's `no_recommendation_reason` is `no prior decision` or the check's reason.

Under `rule_and_decider*` the rule's verified proposal is recorded with strength `strong`, whatever
strength the rule's reply gives: the contract makes the rule's verdict the strong recommendation
the decider takes (`decision.yaml:632`), and `TakeRecommendation` takes only a strong one
(`:660`).

An absent decider or rule port is a failed result of that port. The contract is final and has no
cause for a missing port; it names a "missing, failed or withheld" decider result under the final
holders (`decision.yaml:186`, `~/beyond10x/sdc/docs/autonomy-levels.md:277-279`), so every row
below treats an absent port as that port's failure. Its reason is a `DeciderAbsence`, here only
`NotConfigured`, rendered `no decider configured` or `no rule configured`: where a row escalates,
it is the Escalate's reason; where it records an `EscalationSuppressedRecord`, the cause is
`failure` and the reasons carry it; where it waits on a person, the request's
`no_recommendation_reason` carries it. A decider failure (`Unavailable`, `InvalidReply`, or no
decider) counts as an `Escalate` with no `best` (`story:decider-port-and-grounding-check`). Under
`rule_and_decider` that is handled as a veto: the request is escalated. Under
`rule_and_decider_final` it is not a veto: like any no-usable-result under a final holder it
decides the denying option with the route answering `Decided`, where a veto would end the run as
refused (`decisions.md:104-106`).

| holder | what the router does |
|---|---|
| `person` | no decider call; a reusable prior decision is recorded as the recommendation (above); the request waits on the person |
| `advised` | the decider proposes; the request waits on the person with the recommendation or the reason for none (a failure included) |
| `decider` | a verified proposal is decided by the decider (`Decide`); an `Escalate`, a failure or a withheld proposal is escalated |
| `decider_final` | a verified proposal is decided by the decider; `Escalate` is refused `EscalationSuppressed`; an `Escalate` with `best` decides `best`; an `Escalate` with no `best`, a failure or a withheld proposal decides the denying option and the route answers `Decided`; where no option denies (an outcome, a question), the request stays Open and the route answers `RunEnds` |
| `rule` | a verified rule proposal is decided by the rule; anything else (an `Escalate`, a failure, a withheld proposal) is escalated |
| `rule_final` | as `rule`, except that the escalation is suppressed: the request stays Open and the route answers `RunEnds` |
| `rule_and_decider` | the rule's verified proposal is recorded as a strong recommendation proposed by `rule`; the decider takes it (`TakeRecommendation`) or vetoes, and a veto, or a decider failure handled as one, is escalated with the recommendation kept; a rule proposal the check withholds, or a rule failure, is no recommendation, and the request is escalated with no decider call |
| `rule_and_decider_final` | as `rule_and_decider`, except: a veto is a `Decide` by the decider on the denying option and the route answers `RunEnds` as refused; a decider failure decides the denying option and the route answers `Decided`; where no option denies, a veto or a decider failure leaves the request Open and the route answers `RunEnds`; a withheld rule proposal or a rule failure leaves the request Open and the route answers `RunEnds` |

The `decider_final` rule is session `sdc`'s answer b of 2026-11-21, now in the contract
(`decision.yaml:186`; `decisions.md:104-106`; `autonomy-levels.md:277-279`), and the rule that a
stop with no denying option ends the run applies to `rule_and_decider_final` too (the same
lines). The rule for a withheld rule proposal under `rule_and_decider*` is answer c
(`decisions.md:106-108`, `autonomy-levels.md:280-282`).

The holder alone decides whether an escalation goes to a person or is suppressed: the request
stores its holder and the guards read it, not the level (`decisions.md:90-92`), and an Escalate
always sets the holder to `person` (`decision.yaml:760-764`). There is no run-wide escalation
mode (`story:autonomy-grant-table`): the document's `escalation.mode` (`person` at A2 to A4,
`final` at A5, `autonomy-levels.md:385-387`) is what the presets' holders already say, every A5
cell being a final holder (`autonomy-levels.md:253`). So in a run that is not an eval run a
lowering at A5 to `decider` escalates to the person, and a raise at A1 to `decider` (within an
admitted ceiling) escalates to the escalation person though A0 and A1 have no decider escalation
in the document; an eval run refuses an override to a non-final holder (`story:autonomy-grant-table`).

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

Its code goes in its own module of `crates/loom-decision`; `lib.rs` gains one `mod` line. The new
types are types only, with no command, so `crates/loom-conformance` and the scenarios are
unchanged.

## Why

Item 3 of the request (routing to eligible principals, escalation to a named person) and the
recording of `escalation_suppressed` of item 2 (`decisions.md:304-312`). The decision rules per
holder are those of `decisions.md:94-108`. Session `sdc` accepted on 2026-11-21 that an Escalate
moves the existing request and that the host hands in the eligible principals. The expiry source
is the coordinator's decision: the classes document's per-class duration, no CLI flag; the host
turns it into an instant, as the contract takes `expires_at` as an input. An absent port as a
failed result is the coordinator's decision too: the contract is final, so no cause is added, and
its principal is a fixed one of the contract's grammar.

## Waits on

`story:decider-port-and-grounding-check`: the port and the check, and both change
`ess/domains/decision.yaml` and `crates/loom-decision`. `story:autonomy-grant-table` (with
`expires_after`) and `story:decision-request-entity` (the port's record operations, its
`StoreConflict`, and the record type) through it.
`story:non-independent-review-becomes-decision-request` comes after this story and edits the same
two files.

## ESS first

`ess/domains/decision.yaml` gains `DecisionStop` (with the optional `builtin_class` and `scope`),
`EligiblePrincipals`, `RouteOutcome` (`AwaitsPrincipal`, `Decided`, `RunEnds`), `DeciderAbsence`
(`NotConfigured`), the port `ExpiryInstant` and the refusal `UnclassifiedAction`. On that commit `task drift` fails, and
`crates/loom-decision/tests/routing.rs` does not compile.

## Acceptance

In `crates/loom-decision/tests/routing.rs`, with fixture presets and classes copying the table of
`autonomy-levels.md:242-257`, plus one fixture class the table lacks, `gated`, of kind `outcome`
with action `ship`, held `rule_and_decider` at `A4` and `rule_and_decider_final` at `A5` (the
table's only outcome, `operate: close`, is `D*` at `A5`, `:254`); a stub decider and a stub rule
that answer fixed replies; a fixture resolver that delegates `path_line` to `PathLineResolver`
over fixture files and answers the subject's `artifact_revision` from a table; a fixture
`ExpiryInstant` that
records the seconds it is asked for and answers `2026-11-22T12:00:00Z`; and the in-memory store.
The outcome stop `close/taken` has action `close`, outcome `close` and decision `taken`; cases are
at `r3` unless a bullet moves them.

- At `A0` an approval of `roadmap.approve` on case `C` opens one request `DR-C-roadmap.approve-1`
  with `question_key` `C|roadmap.approve|`, addressed to the first person, makes no decider call
  (the stub counts calls), and answers `AwaitsPrincipal`; after the subject artifact moves (the
  case goes to `r4`) and the stop is routed again, the new request is `DR-C-roadmap.approve-2`.
- At `A0` with a Decided prior request on the same `question_key` and subject revision whose
  grounds resolve, the new request carries a strong recommendation proposed by `prior_decision`
  whose first ground cites the prior, makes no decider call, and waits on the person.
- At `A0` with no prior request, the new request has no recommendation and
  `no_recommendation_reason` `no prior decision`.
- The approval request has options `A` (`grant: granted`) and `B` (`grant: denied`), both
  `produces: canon_authority`, and `record_key` `owner.approve@r3`; the outcome stop `close/taken`
  gives `A` with `produces: canon_decisions` and `B` "leave it untaken" with no `produces`, and
  `record_key` `close/taken@r3`; a question stop with host options `A`, `B`, `C` keeps them, each
  `produces: artifact_revision`.
- The approval stop of `owner.approve` with scope `{namespace: b, cluster: a}` at `r3` gets the
  `record_key` `owner.approve[cluster=a,namespace=b]@r3`. With scope `{team: "x,y"}` it gets
  `owner.approve[team=x%2Cy]@r3`, and with an empty scope `owner.approve@r3`.
- With `roadmap: {expires_after: 86400}`, the `ExpiryInstant` fixture is asked for 86400 seconds
  and the request's `expires_at` is `2026-11-22T12:00:00Z`; a `vision` request has no `expires_at`
  and the fixture is not asked.
- At `A1` the decider is called once and the request waits on the person with the verified
  recommendation; with a reply citing a moved revision it waits with `no_recommendation_reason`.
- At `A1` with no decider, the request waits on the person with no recommendation and
  `no_recommendation_reason` `no decider configured`.
- At `A3` a verified decider reply on `roadmap` is Decided by `decider:stub`; an `Escalate` reply
  leaves the request Escalated, addressed to the escalation person, with holder `person`.
- At `A3` with no decider on `roadmap` (`decider`), the request is opened with `decider:none` in
  `eligible` and as `addressed_to`, then Escalated to the escalation person with holder `person`
  and the reason `no decider configured`; no `EscalationSuppressedRecord` is appended.
- At `A3` with no rule on `rollback` (`rule`), the request is opened with `rule:none` in `eligible`
  and as `addressed_to`, then Escalated to the escalation person with holder `person` and the
  reason `no rule configured`; no `EscalationSuppressedRecord` is appended.
- At `A3` on `rollback` (`rule`), a verified rule proposal is Decided by `rule:stub`; a rule
  `Escalate` leaves the request Escalated to the escalation person with holder `person`, and makes
  no decider call.
- At `A5` an `Escalate` reply with `best: A` appends one `EscalationSuppressedRecord` with class,
  subject, revision, reasons, `A` and cause `escalate`, and the request is Decided `A`; with no
  `best` it is Decided `B` (the denying option); a decider failure is Decided `B` with cause
  `failure`; a reply citing a moved revision is Decided `B` with cause `withheld`.
- At `A5` on `roadmap` (`decider_final`) with no decider, the request is Decided `B` by
  `decider:none`, with one record of cause `failure` whose reasons name `no decider configured`,
  and the route answers `Decided`.
- At `A5` on `merge` (`rule_and_decider_final`) with a rule verdict that grants and no decider, the
  request is Decided `B` by `decider:none`, with one record of cause `failure` whose reasons name
  `no decider configured`, and the route answers `Decided`.
- At `A5` on a question of class `scope_changing_answer` (`decider_final`), whose options have no
  denying option, a decider failure appends a record with cause `failure`, leaves the request
  Open, and answers `RunEnds`.
- At `A5` the outcome stop `close/taken` (`decider_final`) with no decider appends a record with
  cause `failure`, leaves the request Open addressed to `decider:none`, and answers `RunEnds`.
- At `A5` on the outcome stop `ship/taken` (class `gated`, `rule_and_decider_final`), a decider
  veto leaves the request Open with one record of cause `veto` and answers `RunEnds`.
- At `A5` on `ship/taken`, a decider failure leaves the request Open with one record of cause
  `failure` and answers `RunEnds`.
- At `A4` on `merge`, a rule reply of strength `moderate` is recorded with strength `strong` and
  proposed by `rule`; a decider `Take` is Decided by the decider through `TakeRecommendation`; a
  veto leaves it Escalated; a rule verdict citing a moved revision leaves no recommendation and the
  request Escalated. At `A5` a veto is Decided `B` with cause `veto` and the route answers
  `RunEnds` as refused; a withheld rule verdict leaves the request Open with cause `withheld` and
  answers `RunEnds`.
- At `A4` on `merge` with no decider, the rule's strong recommendation is recorded and the request
  is Escalated to the escalation person keeping it, with the reason `no decider configured`.
- At `A4` on `merge` with no rule, the request carries no recommendation and is Escalated to the
  escalation person with the reason `no rule configured`, and the decider is not called.
- A stop with action `step` and `builtin_class: budget_overrun`, at `A5` (`rule_final`), routed
  with a classes document that lists `step` under no class: a rule `Escalate` appends a record with cause
  `rule_escalate`, leaves the request Open, and answers `RunEnds`; with no rule, and on `merge`
  (`rule_and_decider_final`) with no rule, the request stays Open addressed to `rule:none` or the
  decider's principal, with one record of cause `failure` whose reasons name `no rule configured`,
  and the route answers `RunEnds`.
- At `A5` in a run that is not an eval run, with the override `roadmap: decider` (a lowering), a
  decider `Escalate` leaves the request Escalated to the escalation person with holder `person`
  and appends no `EscalationSuppressedRecord`.
- At `A1` with the override `roadmap: decider` and an admitted entry `roadmap: decider` (a raise),
  a decider `Escalate` leaves the request Escalated to the escalation person with holder `person`.
- The same stop routed twice at `r3` opens one request: the second routing finds the open request
  for the record key and reuses it.
- After the subject artifact moves and the case goes to `r4`, the same approval stop opens a new
  request with `record_key` `owner.approve@r4` that names the `r3` request in `supersedes`, and
  the `r3` request is Superseded.
- An outcome stop `close/taken` whose request was Decided `B` ("leave it untaken") at the same
  case revision, routed again, opens no request and answers `Decided` with that request; the store
  is unchanged.
- Two routings of one stop that race (the store fixture lets both read before either writes) leave
  one request: the second write is refused `StoreConflict`, that routing re-finds the open request
  for the record key and reuses it, and no `RecordKeyOpen` is raised.
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
The review stop kind and the not-independent reason
(`story:non-independent-review-becomes-decision-request`). A new cause for a missing port. Time
arithmetic in `crates/loom-decision`. A port lookup by `question_key`.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-decision` (crate `b10x-loom-decision`, created by `story:decision-request-entity`) plus `ess/domains/decision.yaml` — cited (story Outcome "Its code goes in its own module of `crates/loom-decision`; `lib.rs` gains one `mod` line"; ESS first)
- **Files:** `ess/domains/decision.yaml` — cited (ESS first: gains `DecisionStop`, `EligiblePrincipals`, `RouteOutcome`, `DeciderAbsence`, `UnclassifiedAction`)
- **Files:** `crates/loom-decision/src/lib.rs` — cited (one `mod` line)
- **Files:** `crates/loom-decision/tests/routing.rs` — cited (ESS first: "does not compile" on the spec commit)
- **Files:** `crates/loom-decision/src/routing.rs` — inferred: the story says "its own module" and `story:non-independent-review-becomes-decision-request` calls it "the routing module of `crates/loom-decision`"; the file name is a guess
- **Symbols:** `route_stop`, `DecisionStop`, `EligiblePrincipals`, `RouteOutcome` (`AwaitsPrincipal`, `Decided`, `RunEnds`), `DeciderAbsence::NotConfigured`, `UnclassifiedAction`, `EscalationSuppressedRecord`, `SuppressionCause` — cited (story; none exists in the tree yet, the crate is not there)
- **Symbols it consumes:** `DecisionStore`, `Decider`, `check_grounds`, `CitationResolver`, `PathLineResolver`, `expires_after`, `holder` — cited (the three stories it builds on; not in the tree)
- **Also likely:** `crates/loom-decision/Cargo.toml` — inferred. `Timestamp` is a `String` newtype (`generated/rust/loom/src/primitives.rs:25`). `expires_at` = instant + seconds needs RFC 3339 arithmetic. Loom's only parser is `crates/loom-commission/src/rfc3339.rs:12` (`pub(crate)`), and `b10x-loom-decision` may not name Commission (`story:decision-request-entity` Acceptance). `crates/loom-cli/Cargo.toml:46`, `crates/loom-agent/Cargo.toml:18` and `crates/loom-a2a/Cargo.toml:17` already use `chrono = "0.4"`.
- **Documents:** none beyond the house-rule files — cited (the story changes no `workflows/`, `docs/` or `protocols/` file)
- **Confidence:** high on the surface (the story names both files and the test), medium on the module file name and on `Cargo.toml`
- **Would collide with (earlier, already ordered):** `story:decision-request-entity`, `story:autonomy-grant-table` and `story:decider-port-and-grounding-check` all edit `ess/domains/decision.yaml`, `crates/loom-decision/src/lib.rs` and, inferred, `crates/loom-decision/Cargo.toml` — cited (their `## Waits on` and `## ESS first`). A `depends_on` path orders all three before this story (routing, decider, grant-table, entity: `aep plan artifact graph --format json`).
- **Would collide with (later, already ordered):** `story:escalate-when-conditions` (adds `touched_paths` to `DecisionStop`; direct `depends_on`), `story:non-independent-review-becomes-decision-request` (adds the review stop in `decision.yaml` and the routing module; direct `depends_on`), `story:decided-record-written-then-recorded` (via `escalate-when-conditions`) and `story:silence-and-expiry-at-a-host-tick` (via `decided-record-written-then-recorded` and `decision-inbox-commands`) — cited (their bodies; the graph orders each after this story).
- **Would collide with (unordered):** none. `story:llm-decider` (`crates/loom-executor`), `story:decision-request-file-store` and `story:grounds-resolve-against-held-cases` (`crates/loom-governor`) and `story:decision-suspension-reason` (Commission) have no `depends_on` path to this story. None names `decision.yaml` or `crates/loom-decision` as a file it edits, so no collision was found — cited (their Outcome and ESS first).
- **Consumers only (no shared file):** `story:budget-and-limit-stops-raise-decision-requests`, `story:approval-decisions-resume-held-runs`, `story:question-and-outcome-decisions-hold-runs`, `story:tree-waits-open-decision-requests` and `story:requests-from-canon-pending-decisions` call `route_stop` from other crates; all are after this story in the graph — inferred from their bodies
- **Safety fact:** the ESS change only adds types, so no existing caller changes. `crates/loom-decision` has no consumer before this story: `loom-executor` (`llm-decider`) and `loom-governor` (`file-store`) depend on the port, not on routing — step 2 (the stories' Outcome sections), unproven
- **Contradicted by the code:** nothing in the story contradicts the code. `crates/loom-decision`, `DecisionStore` and `Decider` do not exist at `61fd500b` (`ls crates` shows no `loom-decision`), and the epic says so — cited
