---
format: aep.planning-md/3
id: story:selector-stop-names-set-aside-record
kind: story
status: draft
title: A selector stop caused by evidence set aside as not independent names the record, its producer and the action
summary: The not-independent rule stays; the refusal carries the set-aside records so a stalled run reads as its cause
tags:
- from-finding
- sdc
relations:
- informed_by: review-result:adv-w20261121w1-u3-pass-1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:deterministic-default-selector
- serves: vision:O1
revision: 1
---
## Outcome

When the deterministic selector finds no useful action because a kind's only records are set aside
by Canon as not independent, the stop says so: it names the set-aside record, its producer, the
independence demand the record failed, and the action that would need an independent producer.
The rule itself does not change: such a kind counts as produced, the selector does not pick the
producing action again, and the run stops.

## Why

Adversary pass 1 on `story:deterministic-default-selector`
(`review-result:adv-w20261121w1-u3-pass-1`, finding 4) showed that a run ends
`SelectorError::NothingAdmissible` with the claim unknown when a review is recorded by a producer
the case does not declare independent. The adversary read it from SDC's protocols:
`sdc-story.protocol.yaml:71-80` (`change.reviewed`) and `sdc-spec` `spec.approved`. Nothing in the
stop names the cause, so a person reads a stalled run. Session `sdc` confirmed the rule on
2026-11-21 ("keep your choice … the run stops, no loop") and asked for this story: "the stop
reason names the set-aside record, its producer, the independence demand it failed and the action
that would need an independent producer (`change.review`, `spec.review`)". SDC's own fix is on its
side: the binding of a review action names another principal, and a review by a non-independent
producer becomes a pending decision.

## Waits on

`story:deterministic-default-selector`: it creates `DeterministicSelector`, `ActionFactsPort` and
`CanonGovernor::action_facts`, which this story extends.

## ESS first

`ess/domains/run.yaml`: `ActionFacts` gains a list of set-aside records (record id, kind,
producer, the independence dimension demanded), filled only for records set aside as not
independent and nothing else. On that commit `task drift` fails, and
`crates/loom-executor/tests/selector_stop_reason.rs` does not compile.

## Acceptance

With a fixture protocol whose `review` action may produce `reviewed`, a claim that demands a
producer independent of the implementing one, stub ports and no model:

- A case holding one `reviewed` record by the implementing producer: the selector's refusal names
  that record's id, its producer, the dimension the match demands and the action `review`. The
  refusal is still a refusal: no action is returned and the same action is not selected.
- The same case with a second `reviewed` record by an independent producer: the claim is true and
  the selector's answer carries no set-aside record.
- A record set aside as expired or for a moved revision is not named: the kind is open and the
  producing action is selected, as before.
- With two set-aside records of two kinds, both are named, in identifier order.
- `FirstAdmissibleSelector` and every existing caller of `ActionSelector` compile unchanged; no
  member is added to `FrontierAction` or `CatalogueEntry`.
- `task check` exits 0.

## Out of scope

Turning a review by a non-independent producer into a pending decision
(`epic:decision-requests-in-the-runtime`, classes `code_review` and `spec_acceptance`). Choosing
another producer. Showing the reason in `tree run` (`story:tree-run-command` prints the stop it is
given).
