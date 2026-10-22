---
format: aep.planning-md/3
id: story:frontier-names-movers
kind: story
status: draft
title: Each open obligation names the actions that can move it, and each claim the actions that decide it
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

Each open obligation on Commission's frontier names the actions that can move it (`moved_by`), and
each claim names the actions that decide it (`decided_by`). The governor derives both from Canon's
compiled protocol. No model supplies them.

## Why

takt's probe on 0.34.0 (takt `dependency-blocker:loom-handed-frontier`, takt `545c591`) showed
that the runtime cannot tell "only a grant moves this obligation" from "a performed action moves
it", because `FrontierData` holds claims, obligations and actions with no relation between them
(`generated/rust/commission/src/responsibility.rs:2152-2167`). takt took its own pre-check instead
(orchestrator, 2026-10-22: "takt takes option C (its own pre-check before each Loom step, no Loom
change)"). This story stays draft until Timo wants Loom's documented stop order changed.

Canon relates the parts only through evidence kinds (Canon 0.6.0 `efcfd04`,
`crates/canon/src/ir/mod.rs`): `Action.may_produce` (:141), `Claim.true_when` over evidence and
claims (:119), and `Obligation.discharged_when` (:126). Nothing links an obligation or claim to an
action directly (:117-145), and `canon-decision/1` names no producer (`model/decision.rs:18-46`).
The join is `may_produce` against the kinds a predicate reads, following claim references. The
public `Predicate::visit` and `order_reads` are enough (`model/predicate.rs:192,233`). Canon's own
`evidence_reached` and `kinds_read` are `pub(super)` (`check/space.rs:1194-1222`).

On `software.change/1`, `repository.merge` produces nothing (engineering-protocols
`protocols/software-change/1.yaml:256-265`), and no action produces `code_review`,
`build_provenance`, `operational_observation` or `security_review`, so those obligations name no
mover. That evidence is external.

## Acceptance

- `loom-governor/tests/frontier_movers.rs::an_open_obligation_names_the_actions_whose_evidence_it_reads`
- `loom-governor/tests/frontier_movers.rs::a_claim_no_action_produces_evidence_for_names_none`
- The SDK stability page and `crates/loom-sdk/tests/host_ports.rs:668` move with `FrontierData`.

## ESS first

`ess/commission/domains/responsibility.yaml`: `FrontierObligation` gains `moved_by: List<String>`,
and `FrontierClaim` gains `decided_by: List<String>` (:821-838). The first commit changes only
`ess/`. `task commission:drift` fails on it, which is the red.

## Depends on

Nothing.

## Not in scope

Any change to the stop order (`story:needs-authority-before-host-stop`,
`story:unperformable-names-the-claim`). The field goes on obligations and claims, not
`FrontierAction`: that would mean 221 `FrontierAction {` literals in 116 files against 42
obligation and claim literals in 28. This story supersedes `story:frontier-action-names-its-claims`,
which is retired when this one is accepted.

## Scope

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`
- `crates/loom-governor/src/lib.rs` (`project`, :1874-1924; frontier built :1288-1330)
- 13 `FrontierClaim {` and 29 `FrontierObligation {` literals; 30 `impl Governor for`
  (testkit `fake_governor.rs:213`, conformance `governor.rs:84`, `loom-sdk/tests/host_ports.rs:291`)
- AGENTS.md:378, `responsibility.yaml:481`, `crates/loom-commission/src/runtime.rs:45` and
  `unperformable_names.rs:11` say "the frontier relates no action to a claim". Breaking.
