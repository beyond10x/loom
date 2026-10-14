---
format: aep.planning-md/3
id: decision-blocker:dw-incident-sources
kind: decision-blocker
status: cleared
title: Nobody has decided which incident data sources the troubleshooting workflow reads first
relations:
- blocks: epic:deterministic-workflows
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-14T02:50:22Z", actor: "human:timo", revision: 3}
---
## Question

Which data sources does the incident troubleshooting workflow read first? The configured Connectors read adapters are a tracker, chat, support, a wiki, a forge and web search; none reads call records or traces.

## Options

- A. The configured adapters first, call and trace sources once Connectors has adapters for them.
- B. Wait for call-record and trace adapters in Connectors.
- C. Fixtures for call and trace sources now, live later.

Recommendation: A with C.

## Decision (2026-10-14)

A with C. The fan-out runs live over the configured tracker, chat, support and forge adapters; call-record and trace sources run against fixtures (the fake connectors) until Connectors has read adapters for them. The workflow does not wait on Connectors.
