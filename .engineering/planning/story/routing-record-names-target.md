---
format: aep.planning-md/3
id: story:routing-record-names-target
kind: story
status: draft
title: A routing record names the route target
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
revision: 1
---
## Outcome

The `routing_record` evidence the host evidence adapter records for an applied `ticket.route`
write names the route target, the queue or owner the ticket was routed to, as `support-triage@1`
describes the evidence kind ("the record that the ticket was routed to a queue or owner, naming the
target"). Today the record names the step, the admitted request and Connectors' request and audit
ids, but not the target.

## Acceptance

- An applied route write records a `routing_record` whose facts name the target, taken from the
  admitted arguments or from Connectors' answer, never from model output.
- A write whose target cannot be read records no evidence and the adapter says why.

## Source

Adversary review of `story:connectors-governed-write` (finding 4, a judgement note).
