---
format: aep.planning-md/3
id: decision-blocker:action-operation-binding
kind: decision-blocker
status: open
title: Nobody has decided who declares which Connector operations a frontier action binds to, or how many
relations:
- blocks: story:connector-action-binding
revision: 2
---
## Question

For a consequential action in a Frontier (a Canon `ActionId`), which artifact declares the Connector
operations it executes through, and how many may it bind to? Candidates the sources leave open: the
action definition in the protocol (Canon or ELS), the Commission composition, a Loom deployment
configuration, or the Connectors catalog. Part of the same question: does the binding name the
Connection, or does Connectors choose it, so that Loom never sees a `connection_ref`?

## Relation

Frontier action -> Connector operation (`instance_id`, `operation_id`).

- Cardinality: UNMAPPED (one-to-one, or one action to many operations; may an action bind to none).
- Ownership: UNMAPPED (who declares and may change a binding).
- Lifecycle coupling: UNMAPPED (does a binding outlive the protocol version or the Connector
  declaration it names).

No `ess/1` document declares this relation: not `ess/domains/run.yaml` (`loom.run`), not
`connectors/ess/domains/*.yaml`. No code in this repository or in `beyond10x/harness` implements it.

## What is settled around it

- The far side is typed: a Connector invocation is one `connectors.mutations.AttemptRecord`, which
  references exactly one `connectors.auth_bindings.Connection` (relation `connection`, cardinality
  one, via `connection_ref`) and one `connectors.declarations.ServiceConfiguration` (relation
  `instance`), with `operation_id` a plain field (connectors `ess/domains/mutations.yaml:120-133`,
  at connectors `c7a9d5b1d`; the ESS-LIMIT note there says operation identity is the pair
  `instance_id`, `operation_id`).
- That a binding exists, not who owns it: build pack
  `docs/integrations/current-beyond10x-boundaries.md:73` ("Connector operations bind to protocol
  actions; Loom never receives unrestricted provider APIs merely because a connection exists"), and
  Atlas `epic:ga-governed-effects` § Outcome ("Connectors bind operations to protocol actions").
- Atlas ADR 0072 § Rule: visible candidates are available integrations intersected with
  protocol-admissible actions and environment capabilities.
- Loom owns neither protocol semantics nor connector credentials (`AGENTS.md:20-21`).
- Related, filed by the `epic:fast-selector` decomposition: `decision-blocker:action-argument-schema`
  lists "the Connector operation it is bound to" as one possible source of an action argument schema,
  so an answer here may settle part of that one.

## What it stops

`story:connector-action-binding` (L-014): its binding lookup, the refusal for an unbound action and
the `loom.run` domain extension that has to precede it (planning guardrail 7).

## Clears when

An accepted decision names the owner of the action-to-operation binding and its cardinality, and the
owning repository declares it as a `relations:` entry in its ESS domain.

## After ADR 0082

Atlas ADR 0082 (operator, 2026-10-04) puts the effect invocation in the Commission runtime: it
rechecks, then invokes through the action's binding. The binding is therefore read by Commission,
not Loom, and the question moves to Commission as an open follow-up (ADR 0082 § Open). It is filed
there as commission `decision-blocker:action-operation-binding`, blocking commission
`story:effect-invocation`. This blocker stays open here because Loom keeps one dependency on the
answer: whether an unbound action is still offered in the catalogue Loom projects from the frontier
(Atlas ADR 0072 § Rule, candidates intersected with available integrations), or filtered out before
Loom sees it. Not decided here.
