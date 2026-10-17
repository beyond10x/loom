---
format: aep.planning-md/3
id: story:connectors-governed-write
kind: story
status: draft
title: A host performs a governed write through ConnectorsCli
relations:
- decomposes: epic:effect-bindings
revision: 1
---
## Outcome

A host performs a governed write through Connectors from Loom. `loom_connectors::cli::ConnectorsCli`
gains an invoke-write call beside `invoke_read`: it takes the operation, its input and an
approval, which is the person's confirmation together with the Canon decision admitting the step.
It passes Connectors' `--approval-file`. Today `ConnectorsCli` has only `invoke_read` and refuses an
operation whose profile is `mutation`, so a host's write (for example routing a ticket) ends
`refused`.

The protocol side is defined too: how Canon admits `ticket.route` after `classification.review` in
`support-triage@1`, so that the write records Canon evidence for the step it performed.

## Acceptance

- Specification first: the approval and the write call are declared in Loom's ESS specification,
  validated and regenerated before code.
- An invoke-write with a confirmation and an admitting decision runs the Connectors operation with
  `--approval-file` and returns its result. A stub Connectors binary sees the approval file.
- An invoke-write without a confirmation, or without an admitting decision, is refused before
  Connectors is started.
- `invoke_read` still refuses a `mutation` operation.
- A `support-triage@1` run admits `ticket.route` only after `classification.review`, and the write
  records Canon evidence that names the step.
- The SDK stability page lists the new paths.
