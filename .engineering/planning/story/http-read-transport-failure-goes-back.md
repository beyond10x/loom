---
format: aep.planning-md/3
id: story:http-read-transport-failure-goes-back
kind: story
status: active
title: An http read that fails in transport is retried once, then goes back to the model as a refusal
tags:
- from-consumer-report
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:O1
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T09:24:08Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-03T09:24:08Z", actor: "human:timo", revision: 3}
---
## Outcome

When an `http` read fails in transport (connect, send, timeout, or a body that breaks off; not an
HTTP status the binding maps), the read is retried once. If the retry also fails, the failure goes
back to the model as a refused read within the step budget, the same way a refused argument does:
"the read of `<source>` (path `<path>`) found the endpoint unavailable, also when retried:
<error>". The run ends only when the turn stops on its budgets, and the stop detail then names the
endpoint and the error through its last refusal. Nothing else is retried: a refused read (path,
method, bearer, redirect, failing status) is not, and neither is any effect other than an `http`
read (`reply.send`, Connectors reads).

## Why

A downstream adopter's scheduled run on 0.55.1 ended with exit 1 (item D412, 2026-11-03): "the
read of `logs` found the endpoint unavailable: the request failed: error sending request". The
adopter ruled out NetworkPolicy, DNS and a backend restart, and infers a transient connect error.
At 0.55.1 the port turns `HttpReadError::Unavailable` into an effect error
(`crates/loom-plugin/src/effects.rs`, `read_http`), and that ends the whole run, so one failed read
loses the item.

## ESS first

No change to `ess/`: `loom.agent.AdmitHttpRead` and `loom.agent.BoundHttpAnswer` decide what is
refused before and after a request, and a transport failure is neither. The red test is
`crates/loom-plugin/tests/http_read_transport.rs`, run on the base.

## Acceptance

- `http_read_transport.rs`: a read whose first attempt fails in transport and whose retry answers
  is performed, and the reader was called twice.
- `http_read_transport.rs`: a read that fails twice goes back to the model as a refusal naming
  the source, the path and the error, and a corrected read or an answer ends the turn.
- `http_read_transport.rs`: a refused read (a failing status) is not retried.

## Out of scope

Retrying a Connectors read, or retrying more than once.
