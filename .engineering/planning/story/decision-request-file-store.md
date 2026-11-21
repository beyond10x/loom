---
format: aep.planning-md/3
id: story:decision-request-file-store
kind: story
status: draft
title: Decision requests are kept a file each in a store directory and survive a restart
summary: FileDecisionStore in the governor beside the case and suspension stores, with the three views and the records kept beside each request
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decision-request-entity
revision: 3
---
## Outcome

`FileDecisionStore` in `b10x-loom-governor` implements the `DecisionStore` port of
`b10x-loom-decision` over a store directory: one file per request (`decision-<stem>.json`, the
stem rule of the case files), written to a temporary file and renamed under the directory's lock,
so it may share one directory with `FileCaseStore` and `FileSuspensionStore`. A write names the
state it read; a write over a request whose state changed since is refused, naming the request
and both states, and changes no byte. A file that does not decode, or holds another request's id,
is refused as a store failure naming the file, never repaired. The three views of
`loom.decision` read the store. It implements the port's two operations for the records beside a
request (`EscalationSuppressedRecord`, declared with the port in `story:decision-request-entity`):
an append is kept in the request's file under the same lock and the same write, and a read gives
the records in the order they were appended. The store holds the record as the port declares it
and reads nothing into it.

## Why

Item 3 of the request needs an inbox, which is a durable store (coordinator's decision: the inbox
is a store plus CLI and SDK calls, accepted by session `sdc` on 2026-11-21). The governor already
keeps a file per case, per suspension and per workflow run over one lock and one write
(`AGENTS.md:444-456`; `crates/loom-governor/src/locked_dir.rs`, `file_suspension.rs`,
`file_run_store.rs`), with the port in another crate and the file implementation in the governor
(`FileWorkflowRunStore` implements a port of `b10x-loom-executor`, `AGENTS.md:450-455`). The
reuse check of `decision-grounds.sh:68-79` reads Decided requests from a log; in Loom it reads
this store (`story:grounds-resolve-against-held-cases`).

## Waits on

`story:decision-request-entity`: the port with its record operations, the entity, the record type
and the views.

## ESS first

`ess/domains/governor.yaml`: `StoredDecisionRequest` (the request, its state, the records kept
beside it) and the refusal `DecisionStoreConflict` (request, state read, state found). On that
commit `task drift` fails, and `crates/loom-governor/tests/decision_store.rs` does not compile.

## Acceptance

- A request written, the process ended, and the store opened again in a second process reads the
  same request; `crates/loom-governor/tests/decision_store.rs` holds it across a real process
  restart, as `restart_durability.rs` does for a case.
- Two threads that read one Open request and each write a different `Decide` leave one Decided
  request; the second write is refused `DecisionStoreConflict` and the file holds the first.
- Two `EscalationSuppressedRecord`s appended beside a request, the process ended, and the store
  opened again in a second process read back both records in order, with the request unchanged.
- Two threads that each append one record beside the same request, while a third writes a
  `Decide` to it, leave the Decided request and both records in the file; an append beside an
  unknown request is refused naming the request.
- A crash injected at the write, sync and rename steps leaves either the old or the new request,
  each decodable.
- A request id longer than 100 bytes is kept under a `sha256-` stem, as a case id is
  (`crates/loom-governor/src/file_store.rs:133-146`).
- A file holding another request's id, and a truncated file, are each refused naming the file.
- A case file, a suspension file and a decision file in one directory are each read back by
  their own store.
- `OpenDecisionsForPrincipal` over the file store equals the view over the in-memory store for
  the same 20 requests.
- `task check` exits 0.

## Out of scope

A query index or a database. Deleting requests. A store in another process or service.
