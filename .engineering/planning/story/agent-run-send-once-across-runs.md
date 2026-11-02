---
format: aep.planning-md/3
id: story:agent-run-send-once-across-runs
kind: story
status: draft
title: A scheduled agent run does not resend a reply whose write came back unknown
relations:
- decomposes: epic:loom-agent-command-variant
revision: 1
---
## Outcome

A scheduled `agent run` does not send the same reply again on the next run after a write that failed or came back `unknown`.

## Why

Out-of-scope row of review-result:adv-w20261102w2-a-pass-1: such a write is not counted, and each run is a new item with a new idempotency key, so the next scheduled run may send the same reply again.

## Acceptance

- Two runs on the same input whose first write came back `unknown` send at most once, and the second run says why it sent nothing.
