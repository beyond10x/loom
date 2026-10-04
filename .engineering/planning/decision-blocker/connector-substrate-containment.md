---
format: aep.planning-md/3
id: decision-blocker:connector-substrate-containment
kind: decision-blocker
status: open
title: Nobody has decided whether Substrate is the Connector provider an effect invokes or the confinement a Connector invocation runs in
relations:
- blocks: story:substrate-execution-binding
revision: 1
---
## Question

`epic:effect-bindings` says a consequential action executes "through a Connector operation inside
Substrate". Which of the two is the container?

- Reading A: Substrate is the Connector provider. The bound Connector operation targets a Substrate
  daemon enrolled as a Connection; Connectors governs the call and Substrate runs the effect.
- Reading B: Substrate confines the invocation. The Connector invocation itself (the adapter doing the
  provider call) runs as a confined Substrate workload.

## Relation

`connectors.mutations.AttemptRecord` <-> `substrate.operations.AcceptedOperation`. Direction,
cardinality, ownership and lifecycle coupling: all UNMAPPED. Neither connectors `ess/domains/` nor
substrate `spec/domains/operations.yaml` (at substrate `aeeca78d`) declares a relation to the other.

## Evidence

- For A: substrate `docs/VISION.md:104` ("Each enrolled daemon is a Connection; grants admit
  operations from the risk vocabulary this spec declares"), and `docs/VISION.md:30` (connectors
  "governs invocation but must never contain the thing invoked").
- For B: substrate `docs/VISION.md:97` (connectors future "supervised client runtime" tier exists as a
  Substrate workload), and the epic wording "a Connector operation run inside Substrate".
- Both readings keep the Substrate invariants: argv-only, named refusal instead of silent
  degradation (substrate `AGENTS.md` invariant 3), no sibling-component implementation dependency
  (invariant 2).

## What it stops

`story:substrate-execution-binding` (L-015): what Loom (or the invoker) hands to Substrate, and which
operation the Substrate ledger records for the effect.

## Clears when

An accepted decision picks one reading (or a third) and the owners declare the relation in their ESS
domains.
