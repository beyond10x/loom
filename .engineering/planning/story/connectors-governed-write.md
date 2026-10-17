---
format: aep.planning-md/3
id: story:connectors-governed-write
kind: story
status: active
title: A host performs a governed write through ConnectorsCli
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-17T07:17:00Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-17T07:17:01Z", actor: "human:timo", revision: 4}
---
## Outcome

A host performs a governed write through Connectors from Loom. `loom_connectors::cli::ConnectorsCli`
gains `invoke_write(&AdmittedRequest, operation, input, &WriteApproval)` beside `invoke_read`. The
admitted request is the Canon decision admitting the step (only the runtime makes one); the
`WriteApproval` names the path of the person's confirmation, the proof `connectors approvals issue`
wrote. The call passes that path as `--approval-file` and an idempotency key derived from the
admitted request id as `--idempotency-key`, and never reads, writes or stores the proof. Today
`ConnectorsCli` has only `invoke_read` and refuses an operation whose profile is `mutation`, so a
host's write (for example routing a ticket) ends `refused`.

A host evidence adapter turns the applied result Connectors reports (`mutation`, `request_id`,
`source_audit`) into the protocol's evidence for the step through `submit_evidence`: for
`support-triage@1`, `routing_record` naming `ticket.route`. `support-triage@1` already admits
`ticket.route` only after an approved `classification.review` and a known requester; it is not
changed. The service invoker (`ConnectorsInvoker`) is unchanged.

## Acceptance

- Specification first: the write approval, the write result (with Connectors' mutation
  classification `not_attempted`, `refused`, `applied`, `unknown`) and the write refusal are
  declared in `ess/domains/datasource.yaml`, validated and regenerated before code.
- An invoke-write with an admitted request and a proof path runs `operations describe` and one
  `operations invoke` with `--approval-file <path>` and `--idempotency-key <derived from the
  request id>`, and returns the classification, `request_id` and audit record; a stub Connectors
  binary sees both flags and the input on stdin, never in argv.
- An invoke-write without a proof path, or with a path that is not absolute, is refused before
  Connectors is started; an operation `describe` does not report as a write is refused before the
  invoke.
- The same admitted request yields the same idempotency key; a result Connectors classifies
  `unknown` is returned as unknown and the call invokes nothing again.
- `invoke_read` still refuses a `mutation` operation.
- A `support-triage@1` case admits `ticket.route` only after `classification.review` approved and
  the requester is known; an applied route write, through the host evidence adapter, records a
  `routing_record` evidence naming the step, and a write classified `refused`, `not_attempted` or
  `unknown` records none.
- The SDK stability page lists the new paths, and `CHANGELOG.md` names them under Unreleased.

## Scope

- `ess/domains/datasource.yaml`, generated `generated/rust/loom/src/datasource.rs` (regenerated,
  never hand-edited).
- `crates/loom-connectors/src/cli.rs` (`invoke_write`, `WRITE_PROFILES`, `describe_for`, `run`),
  a new host evidence adapter module in `crates/loom-connectors/src/`.
- `crates/loom-connectors/tests/connectors_cli.rs` or a new `connectors_cli_write.rs`, a
  `support-triage@1` test in `crates/loom-connectors/tests/`.
- `website/docs/reference/sdk-stability.md`, `crates/loom-sdk/tests/host_ports.rs`, `CHANGELOG.md`,
  `AGENTS.md` § Checks that hold the rules (one row).

## Decision

decision-blocker:connectors-governed-write (cleared, 2026-10-17).
