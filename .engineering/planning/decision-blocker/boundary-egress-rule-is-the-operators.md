---
format: aep.planning-md/3
id: decision-blocker:boundary-egress-rule-is-the-operators
kind: decision-blocker
status: open
title: The operator has not decided whether a credential-free pod may reach an HTTP source in another namespace
summary: 'Holds story:boundary-admits-declared-http-egress: option A (operator list, empty by default), B (a fourth relay) or C (keep refusing); asked 2026-11-21'
tags:
- ai-agent-platform
relations:
- blocks: story:boundary-admits-declared-http-egress
revision: 2
---
## The decision that is open

Whether a credential-free agent pod (`spec.boundary`) may reach an HTTP source in another
namespace at all, and by which rule. It is a security decision and the operator's: session `loom`
put it to him on 2026-11-21 with three options and has no answer yet.

Today admission refuses any namespace egress beside a boundary
(`crates/loom-operator/src/admission.rs:1473-1480`, `Reason::BoundaryEgressNamespace`). Session
`ai-agent-platform` needs it for reads of Loki and Prometheus.

## The options

- A (drafted as `story:boundary-admits-declared-http-egress`, recommended by session `loom`):
  admission admits a namespace-and-selector egress beside a boundary only when its namespace,
  selector and port equal an entry of an operator list `allowedBoundaryHttpEgress`, empty by
  default. What stays exposed: an allowed port also accepts unauthenticated writes where the
  service has them (Loki push, Prometheus admin); an allowed peer can forward with its own
  credential; the sidecar's reach; the list applies to every agent resource in the namespace the
  controller watches, with no per-resource field.
- B: a fourth relay in the pod that holds per-destination methods and paths. Cost: a new relay
  component and its stories; the gap stays open until then.
- C: admission keeps refusing. Cost: no HTTP source under a boundary.

## What it holds

`story:boundary-admits-declared-http-egress`. The stories
`story:send-input-in-the-declared-shape` and `story:agent-selects-the-connectors-relay` depend on
it by shared files (`ess/ess-inputs.yaml`, `crates/loom-operator/src/admission.rs`,
`crates/loom-operator/tests/render.rs`), so they wait with it; `story:connectors-relay-client`
does not.

## What clears it

The operator's answer in session `loom`. On A the story stands as drafted; on B it is replaced by
relay stories; on C it is rejected and the two dependent stories are re-cut without it.
