---
format: aep.planning-md/3
id: story:frontier-action-names-its-claims
kind: story
status: draft
title: Each frontier action names the claims it may produce, and NoPerformableAction names each action's open claims
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:unperformable-names-actions
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission/src/outcome.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: generated/rust/commission
- confidence: cited
  path: website/docs/reference/commission
revision: 2
---
## Outcome

Each action the frontier lists names the claims it may produce, and `NoPerformableAction` names, for
each action no effect performs, its own open claims instead of the frontier's open claims as a
whole. Today `commission.responsibility.FrontierAction` carries only `action`, `status`,
`capability` and `reasons` (`ess/commission/domains/responsibility.yaml:582-592`), so the outcome
`story:unperformable-names-actions` shipped names the unperformed actions and, separately, the
frontier's claims whose value is not true, in frontier order. The protocol does state which action
may produce which claim (for example `may_produce` in the engineering protocols'
`incident-response/1.yaml`), but only Canon's IR carries that link, and the governor does not pass
it into the frontier.

## Why

takt reports an unperformable run as a claim and the performer it lacks. With frontier-level claims
it must relate them itself from the protocol it already reads; with the link in the frontier, the
outcome says it.

## Acceptance

- `FrontierAction` declares the claims the action may produce, and the Canon governor fills them
  from the protocol; a test over a protocol with two actions producing different claims shows each
  action naming only its own.
- `NoPerformableAction` names each unperformed action with its open claims, in frontier order; the
  testkit test `the_outcome_names_each_unperformed_action_and_its_claim` checks the per-action
  pairing.
- `task check` exits 0.

## ESS first

The new `FrontierAction` field and the changed `NoPerformableAction` payload are declared in
`ess/commission/domains/responsibility.yaml` first; the red step is the synthesized conformance
scenario.

## Depends on

`story:unperformable-names-actions`, which ships the frontier-level form this story refines.

## Not in scope

Pulled into a wave only when takt asks for the per-action form; until then the frontier-level form
holds.

## Scope

Crates: `loom-commission`, `loom-commission-testkit`, `loom-commission-conformance`, `loom-governor`,
and every crate that builds a `FrontierAction` literal (about 187 literals in 99 files, per the
implementor of `story:unperformable-names-actions`, 2026-10-20).

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`,
  `website/docs/reference/commission/` (regenerated)
- `crates/loom-commission/src/runtime.rs`, `crates/loom-commission/src/outcome.rs`
- `crates/loom-governor/src/`
- `CHANGELOG.md`, `AGENTS.md`
