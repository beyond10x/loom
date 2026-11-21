---
format: aep.planning-md/3
id: story:relay-client-reads-a-0-73-host
kind: story
status: draft
title: The relay client is held to the answers of the Connectors host the requester deploys (0.73.0)
summary: 'Backlog draft from the boundary epic''s confirming pass: v0.62.0 decoding refuses source_audit on invoke answers; the 0.73.0 host probably sends none, not proven; test form and pin move not designed'
tags:
- ai-agent-platform
relations:
- decomposes: epic:boundary-agent-reaches-connectors-and-http-sources
- depends_on: story:connectors-relay-client
- serves: vision:O1
revision: 2
---
## Outcome

Loom knows whether its relay client, built on `connectors-core` `v0.62.0` types, reads the
`/v1/describe` and `/v1alpha2/invoke` answers of the Connectors host the requester deploys
(`ghcr.io/beyond10x/connectors` tag `0.73.0`), and a test fails when it does not.

## Why

`story:connectors-relay-client` keeps the `v0.62.0` pin and puts a contract test against a newer
host out of scope. At `v0.62.0` the keyed decoder refuses an invoke answer that carries
`source_audit` on this binding, so such an answer makes every relay write `Unknown` and every read
`upstream-protocol` (`e6r4-changes.md` § Open 19).

Session `ai-agent-platform`, 2026-11-21, read from files with no live host
(`.engineering/drafts/e6-requester-answer-1.md`): its chart pins the host at `0.73.0`; the served
v1alpha2 handler sets `source_audit: None` where it builds the invoke answer, so that answer
carries no such key; whether a served instance with standing grants routes a keyed write through
the local owner path, which does write `source_audit`, was not traced. Its words: "probable, not
proven, until a contract test runs against a 0.73.0 host with a standing grant."

## Not designed yet

This draft records the gap; it has not been before a drafter, the critic panel or a scoper. Open
before it can be accepted:

- The form of the test under Loom's rule that no test makes a network call: answer bodies built
  from the `0.73.0` types and read by the `v0.62.0` decoder, a recorded set of bodies with their
  source named, or a run against a host on a loopback socket if the repository's precedent for the
  Connectors fake allows a real host binary.
- Whether the result is a test only, or a move of Loom's `connectors-core` pin (its own story,
  with its Breaking notes).
- Who traces the standing-grant path in Connectors `0.73.0`: session `ai-agent-platform` or a
  Loom agent reading `~/beyond10x/connectors` at the tag.

## Acceptance

Not written. To be drafted with the design above.

## Waits on

`story:connectors-relay-client` (the client it tests).
