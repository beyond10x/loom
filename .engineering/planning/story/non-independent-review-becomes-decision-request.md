---
format: aep.planning-md/3
id: story:non-independent-review-becomes-decision-request
kind: story
status: draft
title: A review recorded by a producer that is not independent becomes a decision request for a person or a decider, never a re-run
summary: tree run routes a set-aside review as a Review stop (declared in routing's decision.yaml and module) for code_review or spec_acceptance; ReviewEvidenceWriter; NotIndependent is an escalation reason on the three conditions of session sdc's fourth message; ModelDecider reports model and adapter
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:tree-waits-open-decision-requests
- depends_on: story:selector-stop-names-set-aside-record
- depends_on: story:decision-routing-by-holder
revision: 3
---
## Outcome

When `tree run` finds a case stopped because its only review records are set aside by Canon as not
independent, it opens a decision request for that review instead of stopping stuck, and the
review is then decided by a person or a decider, never re-run by the producer that recorded it.

In the routing story's surface (`ess/domains/decision.yaml` and the routing module of
`crates/loom-decision`, `story:decision-routing-by-holder`) this story adds:

- a fourth `DecisionStop` variant, `Review`: the set-aside record (id, kind, producer, the
  independence dimension it failed), the review action, and the subject at the revision the
  record is about;
- its options: `A` approve and `B` reject, both `produces: canon_evidence`, the evidence record the
  review action's evidence entry in the bindings file declares (kind, subject), with `result`
  `approved` or `rejected`, `subject_revision` the request's subject revision, and `producer` the
  request's `decided_by`. Neither option has `grant: denied`, so the request has no denying option,
  and under `decider_final` or `rule_and_decider_final` a decider with no usable result ends the
  case's run (`story:decision-routing-by-holder`);
- its keys: `question_key` `<case>|<review action>|`, and `record_key`
  `review:<set-aside record id>@<subject revision>`, so a second routing of the same stop finds
  the Open request by that key and reuses it, as for the other kinds;
- a second `DeciderAbsence` variant, `NotIndependent` (the condition failed: `principal`,
  `write access` or `produced a revision`), rendered `decider not independent: <condition>`. It is
  an escalation reason, not a refusal: the router treats a decider that fails a condition as an
  absent decider, a failed result, and the holder's row applies (escalated under `decider`,
  suppressed with cause `failure` under the final holders).

In `tree run` (`crates/loom-tree`):

- The selector's stop names the set-aside records, their producers, the dimension each failed and
  the action that would need an independent producer
  (`story:selector-stop-names-set-aside-record`). When that action is listed by a class of kind
  `review` in the host's classes document (`story:autonomy-grant-table`; SDC's classes
  `code_review` and `spec_acceptance`, `~/beyond10x/sdc/docs/autonomy-levels.md:404-406`), `tree
  run` routes a `Review` stop. An action no `review` class lists stops the case as before.
- `ReviewEvidenceWriter` writes the decided option's record into the case's evidence, record id
  `review-<request id>`, idempotent by id, through the path `tree approve --action` uses to write a
  `person` executor's record (`story:tree-run-command`), and is the `canon_evidence` writer of
  `decisions … --tree STORE` (`story:tree-waits-open-decision-requests`).
- The review action is not performed again for that case at that revision: the selector still does
  not pick it, and `tree run` never falls back to its bound executor.

A decider's review is independent under `independent_on: [different_principal]` when its
principal differs from the producer of the reviewed work: Canon knows principals only, and Loom
adds no hidden second test (session `sdc`, 2026-11-21, fourth message). `tree run` checks three
conditions before it offers the request to a decider:

1. `principal`: the decider's principal names its role and configuration, `decider:<kind>`
   (`decider:requester` is the kind `requester`), and is neither the set-aside record's producer
   nor the producer of the subject revision under review, as the case's run log records it;
2. `write access` and `produced a revision`: the decider has no write access to the reviewed work
   (its principal is bound in the bindings file as the principal of no action that revises the
   subject) and produced no revision of it in this case (no run-log line of the case records an
   earlier revision of the subject by it). The `Decider` port hands a decider no workspace
   (`story:decider-port-and-grounding-check`), and `ModelDecider` holds only a `Model`
   (`story:llm-decider`);
3. the producer's model and adapter are recorded beside the principal: the decider identity of
   the port gains optional `model` and `adapter`; `ModelDecider` reports the serving model id its
   route resolves to and the adapter `llm:<route alias>`, in its module of `crates/loom-executor`;
   the `record produced` run-log line of a review evidence carries principal, model and adapter.
   Session `sdc` accepted on 2026-11-21 that the run log counts as beside the principal (fifth
   message, point 4).

A person decides through `decisions decide … --tree`, with the person's name as producer.

What it cannot do yet:

- Record the model and adapter in the evidence itself: `canon-evidence/1` has no member for them
  (`~/beyond10x/canon/website/docs/reference/documents.md:47-58`). Session `sdc` asks Canon for
  none (fifth message, point 4).
- Demand more than `different_principal` (for example a different model): that is a protocol
  dimension Canon would have to declare; SDC's protocols demand only `different_principal`
  (fourth message). No story holds it.
- Ship a `decider:requester`: no story builds that decider; a host hands one in through the
  `Decider` port.
- Run in the slice: only `tree run` reads the selector's set-aside records.
- Start before the tree: it waits on `story:tree-waits-open-decision-requests`, which waits on
  `story:tree-run-command` and through it on `story:canon-0-9-0-pin`. That pin is unblocked
  (engineering-protocols `0.16.0` pins Canon `0.9.0`) and not yet delivered.

## Why

Session `sdc`, 2026-11-21, third message: "a review recorded by a non-independent producer becomes
a pending decision for a person or decider (classes `code_review`, `spec_acceptance`), never a
re-run by the same producer". The fourth message answered that a decider's review evidence is
independent under `different_principal` when its principal differs from the producing one, and
asked for the three conditions in this story's acceptance. Whether a review by the same model
family is good enough is the holder of those classes in the host's autonomy table, bounded by the
admitted ceiling, not independence (fourth message;
`~/beyond10x/sdc/docs/autonomy-levels.md:483-486`). Today such a case stops with the set-aside
record named and nothing to answer it (`story:selector-stop-names-set-aside-record` § Why).

## Waits on

- `story:decision-routing-by-holder`: `DecisionStop`, the option rules, the keys and
  `DeciderAbsence`, which this story extends in `ess/domains/decision.yaml` and the routing module
  of `crates/loom-decision`.
- `story:tree-waits-open-decision-requests`: decision requests in `tree run`, `decisions --tree`,
  the run-log decision events, and `crates/loom-tree`.
- `story:selector-stop-names-set-aside-record` (`epic:protocol-tree-on-bindings`): the set-aside
  records in the selector's stop, which this story reads.
- `story:llm-decider` (through the first two): `ModelDecider` in `crates/loom-executor`, which
  this story extends with its identity report.

Files: `ess/domains/decision.yaml`, `ess/domains/runlog.yaml`, `ess/domains/tree.yaml`, the
routing and decider-port modules of `crates/loom-decision`, the `ModelDecider` module of
`crates/loom-executor`, and a new module of `crates/loom-tree`.

## ESS first

`ess/domains/decision.yaml`: `DecisionStop` gains `Review` (the set-aside record, the review
action, the subject at its revision); `DeciderAbsence` gains `NotIndependent` (condition); the
decider identity gains optional `model` and `adapter`. `ess/domains/runlog.yaml`: the
`record produced` event gains optional `producer_model` and `producer_adapter`. On that commit
`task drift` fails, and `crates/loom-decision/tests/review_routing.rs` and
`crates/loom-tree/tests/review_decisions.rs` do not compile.

## Acceptance

Routing (`crates/loom-decision/tests/review_routing.rs`), with the in-memory store and a stub
decider:

- A `Review` stop opens a request with options `A` (`result: approved`) and `B`
  (`result: rejected`), both `produces: canon_evidence`, `question_key` `C|review|` and
  `record_key` `review:<record id>@<rev>`; routed again, it opens no other request.
- At `A3` (`decider`) with the stub decider passed as not independent (`principal`), the decider
  is not called and the request is Escalated with the reason `decider not independent: principal`;
  at `A5` (`decider_final`) the request stays Open with one record of cause `failure` whose
  reasons carry the same words, and the route answers `RunEnds`.

`ModelDecider` (`crates/loom-executor/tests/model_decider_identity.rs`), with a recorded `Model`:

- A `ModelDecider` built on route alias `decider` whose serving model is `fixture-model` reports
  principal `decider:stub`, model `fixture-model` and adapter `llm:decider`.

`tree run` (`crates/loom-tree/tests/review_decisions.rs`), with a fixture tree whose `review`
action may produce `reviewed`, a claim demanding a producer independent of the implementing one
(`independent_on: [different_principal]`), a fixture classes document listing `review` in class
`code_review` (kind `review`), a recorded decider `decider:stub` with model `fixture-model` and
adapter `recorded`, and no model call:

- A case holding one `reviewed` record by the implementing producer `llm-node:impl`: at `A0`,
  `tree run` opens one request of class `code_review` on the record's subject at its revision,
  exits 3 naming it, and the bound `review` executor is called no second time (a counting stub).
- `decisions decide DR-… A --as timo --tree STORE` writes one `reviewed` record with `result`
  `approved` and `producer` `timo`; the next `tree run` finds the claim true and goes on.
- At `A3` (`code_review` held by `decider`), a recorded decider reply `A` whose grounds resolve
  decides the request in the same `tree run`; the written record's `producer` is `decider:stub`,
  and the case's `record produced` line carries `decider:stub`, `fixture-model` and `recorded`.
- At `A3` with the set-aside `reviewed` record produced by `decider:stub`, the decider is not
  called, the request is Escalated to the escalation person with the reason
  `decider not independent: principal`, and `tree run` exits 3.
- At `A3` with the subject revision under review recorded in the run log as produced by
  `decider:stub` (and the set-aside record by `llm-node:impl`), the decider is not called and the
  request is Escalated with the reason `decider not independent: principal`.
- At `A3` with `decider:stub` bound in the bindings file as the principal of the action that
  revises the reviewed subject, the decider is not called and the request is Escalated with the
  reason `decider not independent: write access`.
- At `A3` with an earlier revision of the subject, not the one under review, recorded in the run
  log as produced by `decider:stub`, the decider is not called and the request is Escalated with
  the reason `decider not independent: produced a revision`.
- At `A5` (`decider_final`) with a decider failure, the case stops `Stuck` with exit 4, the request
  is Open with one `escalation suppressed` line of cause `failure`, and no `reviewed` record is
  written.
- A case stopped by a set-aside record whose action no `review` class lists opens no request and
  stops as `story:selector-stop-names-set-aside-record` says.
- A second `tree run` over a case whose review request is Open opens no other request.
- `task check` exits 0.

## Out of scope

Choosing another producer and re-running the review with it. A `different_model` demand. Model and
adapter members on Canon's evidence. The slice. Learning from review decisions
(`beyond10x/decider`).
