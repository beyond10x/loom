---
format: aep.planning-md/3
id: decision-blocker:connectors-governed-write
kind: decision-blocker
status: open
title: Nobody has decided how a write through ConnectorsCli is approved, admitted and recorded as evidence
relations:
- blocks: story:connectors-governed-write
revision: 1
---
## Question

How does a host perform a write through `ConnectorsCli` under governance: what the approval is,
what proves the step was admitted, how the write becomes Canon evidence, and what a write whose
outcome Connectors reports as `unknown` allows next?

## Why it is open

`story:connectors-governed-write` asks for an invoke-write call that takes an approval (the
person's confirmation plus the Canon decision admitting the step) and passes Connectors'
`--approval-file`. Four questions are not settled by any accepted record:

1. Approval type. The confirmation is a Connectors proof issued by `connectors approvals issue
   --approve-subject <digest> --proof-output <path>`, beside Commission's own approval and
   suspension (`SuspensionPort`, `AuthorityVerdict`). Is the admitting decision an
   `AdmittedRequest` (`crates/loom-commission/src/ports/effect.rs`, constructor crate-private, so
   only the runtime makes one) or a new Loom struct?
2. Evidence. Atlas ADR 0074: a trace is not evidence, and an effect outcome reaches the governor as
   an observation. Who turns an applied write into the protocol's evidence (for `ticket.route`,
   `routing_record`) naming the step?
3. At-most-once. Connectors classifies a write `not_attempted`, `refused`, `applied` or `unknown`
   and dedupes by `--idempotency-key`. Who supplies the key, and may anything retry after `unknown`?
4. Transport. `ConnectorsInvoker` (`crates/loom-connectors/src/lib.rs`) already performs admitted
   writes over the Connectors service; the command line is a second transport.

## Options

- A (recommended). `invoke_write(&AdmittedRequest, operation, input, &WriteApproval)`, where
  `WriteApproval` names the Connectors proof file path the host obtained from the person's
  confirmation; Loom passes the path and never reads, writes or stores the proof. A missing
  admitted request cannot be built; a missing proof path is refused before Connectors starts.
  The idempotency key is derived from the admitted request id, so the same admitted step can only
  observe, never apply twice; `unknown` is returned as unknown and never retried by Loom.
  Evidence: the write's applied result, as Connectors reports it (`mutation`, `request_id`,
  `source_audit`), is the trusted source; a host-side evidence adapter submits the protocol's
  evidence kind (`routing_record` for `ticket.route`) naming the action and the admitted request,
  through `submit_evidence`; nothing a model says is evidence. The command line is the transport
  for local hosts; the service invoker is unchanged.
- B. A Loom-defined approval struct without `AdmittedRequest`, checked by the call itself: works
  outside a run, but the call would then decide admission itself, which Loom's rules leave to the
  runtime and the governor.
- C. Route command-line writes through `ConnectorsInvoker`'s binding path only (no new call):
  needs Connectors' service for local hosts, which the assistant does not run.

## What it stops

`story:connectors-governed-write`.

## Source

Scoping of `story:connectors-governed-write`; Atlas ADR 0074; Connectors `docs/local-gitlab-merge.md`.
