---
format: aep.planning-md/3
id: story:selector-stop-names-set-aside-record
kind: story
status: draft
title: A selector stop names its cause, a record set aside as not independent or an action performed at this revision that produced nothing
summary: Both rules stay; DeterministicSelector answers a SelectorStop naming the set-aside records and the spent actions, so a stalled run reads as its cause
tags:
- from-finding
- sdc
relations:
- informed_by: review-result:adv-w20261121w1-u3-pass-1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:deterministic-default-selector
- serves: vision:O1
revision: 4
---
## Outcome

When the deterministic selector finds no useful action, its stop names why, for two causes:

- a kind whose only records Canon set aside as not independent: the stop names the set-aside
  record, its producer, the independence demand the record failed, and the action that would need
  an independent producer;
- an action that was performed at this case revision and produced nothing (the requester's
  wording when it decided the rule): an admissible action, not due, for which
  `StepHistory::performed_at` is true at the revision the facts were read at while a kind it may
  produce is still open. The stop names the action, that case revision and its open kinds.

The rules themselves do not change. A kind whose only records are set aside as not independent
counts as produced, so the selector does not pick the producing action again for it. An action
already performed at the current case revision is not useful again unless the host reports it
due (`retry_when`, a pending copy); it becomes useful again when an artifact moves. In both cases
the run stops.

The reason is a value, `loom.run.SelectorStop`, that `DeterministicSelector` answers from the same
two ports and the same revision as `select`, through a new inherent method. `SelectorError` and
`ActionSelector` stay as they are: `NothingAdmissible` remains a unit variant
(`crates/loom-executor/src/selection.rs:58-66`), which Loom still turns into
`NoUsefulAction(Unit(true))` (`crates/loom-executor/src/lib.rs:507-511`). A host that sees that
stop asks the selector for the reason.

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

The delivered selector added the second cause. After adversary pass 2 (finding 1) the requester
decided rule B (session `sdc`, 2026-11-21): "An action already performed at the current case
revision is not useful again unless the host reports it due (`retry_when`, a pending copy); it
becomes useful again when an artifact moves; `max_runs` stays a ceiling in the effect"
(`approval-record:loom-wave-2026-11-21-w1`). It is delivered at `ba343770`:

- `DeterministicSelector::useful` answers `due` first, then `performed_at` at the facts' revision,
  then open evidence (`crates/loom-executor/src/selection.rs:284-306`).
- `select` keeps the useful admissible candidates and answers `NothingAdmissible` when none is
  left (`:310-344`); `StepHistory` (`:224-243`) is the history port.
- `CanonGovernor::action_facts` (`crates/loom-governor/src/lib.rs:1166-1202`) fills each action's
  open evidence from `counted_kinds` (`:1889-1925`). That function counts a record Canon set aside
  only as not independent, as its doc says at `:1888` and `action_facts`'s at `:1155-1160`.

An action stopped by rule B, for example a writer whose program ran and wrote no record, leaves
the same unexplained stop as a set-aside review.

## Waits on

`story:deterministic-default-selector` (`implemented` in wave 2026-11-21-w1): it created
`DeterministicSelector`, `ActionFactsPort`, `StepHistory` and `CanonGovernor::action_facts`, which
this story extends.

## ESS first

`ess/domains/run.yaml`: `ActionFacts` gains a list of set-aside records (record id, kind,
producer, the independence dimension demanded), filled only for records set aside as not
independent and nothing else. A new `SelectorStop`: the set-aside records, each with the action
that may produce its kind, and the spent actions, each with the case revision and its open kinds.
On that commit `task drift` fails, and `crates/loom-executor/tests/selector_stop_reason.rs` does
not compile.

## Acceptance

With a fixture protocol whose `review` action may produce `reviewed`, a claim that demands a
producer independent of the implementing one, stub ports and no model:

- A case holding one `reviewed` record by the implementing producer: the selector's stop names
  that record's id, its producer, the dimension the match demands and the action `review`. It is
  still a stop: no action is returned and the same action is not selected.
- The same case with a second `reviewed` record by an independent producer: the claim is true and
  the selector's stop, when asked, carries no set-aside record.
- A record set aside as expired or for a moved revision is not named: the kind is open and the
  producing action is selected, as before.
- With two set-aside records of two kinds, both are named, in identifier order.
- A case with no `reviewed` record where `review` was performed at the current case revision
  (`performed_at` true, `due` false): the selector selects nothing, and its stop names `review`,
  that case revision and the open kind `reviewed`, and no set-aside record.
- The same case after an artifact moved: `review` is selected and nothing is named. With `due`
  true, `review` is selected and nothing is named.
- An action with no `may_produce` that was performed at the current case revision is not named as
  spent.
- With two spent actions, both are named, in identifier order. A stop with a set-aside record and
  a spent action names both.
- `SelectorError` and `ActionSelector` are unchanged; `FirstAdmissibleSelector` and every existing
  caller of `ActionSelector` compile unchanged; no member is added to `FrontierAction` or
  `CatalogueEntry`.
- `docs/contracts/loom-action-selection.md` names both causes of the stop where it states the two
  rules.
- `task check` exits 0.

## Out of scope

Turning a review by a non-independent producer into a pending decision
(`epic:decision-requests-in-the-runtime`, classes `code_review` and `spec_acceptance`). Choosing
another producer. Changing either rule. Showing the reason in `tree run`
(`story:tree-run-command` prints the stop it is given).
