---
format: aep.planning-md/3
id: story:decision-request-file-store
kind: story
status: active
title: Decision requests are kept a file each in a store directory and survive a restart
summary: FileDecisionStore in the governor beside the case and suspension stores, with the three views and the records kept beside each request
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decision-request-entity
scope:
- confidence: inferred
  path: crates/loom-governor/Cargo.toml
- confidence: inferred
  path: crates/loom-governor/src/file_decision.rs
- confidence: inferred
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/decision_store.rs
- confidence: cited
  path: ess/domains/governor.yaml
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:18Z", actor: "agent:sdc", revision: 8, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-22T00:22:59Z", actor: "agent:sdc", revision: 10, decided_on: {"recorded":{"review_outcome":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w9"}
---
## Outcome

`FileDecisionStore` in `b10x-loom-governor` (its own module, `crates/loom-governor/src/file_decision.rs`,
with one `mod` line and one `pub use` in `lib.rs`) implements the `DecisionStore` port of
`b10x-loom-decision` over a store directory, through the directory discipline the governor's file
stores share (`LockedDir`, `crates/loom-governor/src/locked_dir.rs:1-17`): one file per request,
`decision-<stem>.json`, its stem named as the case store names a case file
(`FileCaseStore::case_path`, `crates/loom-governor/src/file_store.rs:133-146`): the id's hex for an
id up to 100 bytes (`HEX_NAMED`, `:82`), `sha256-<hex of its SHA-256>` for a longer one. A longer
id is kept, not refused, as the suspension and run stores refuse theirs. Each file is written to a
temporary file, synced and renamed under the directory's lock, so the store may share one
directory with `FileCaseStore` and `FileSuspensionStore`.

A write names the state it read. The read, the comparison and the write run in one `locked(...)`
operation, as `FileSuspensionStore` does for hold and take. A write over a request whose state
changed since, or of a new request whose id was stored since, is refused with the port's
`StoreConflict` (`story:decision-request-entity`), naming the request id, the state read and the
state found, and changes no byte. A file that does not decode, or holds another request's id, is
refused with the port's `StoreFailure` naming the file, never repaired. The three views of
`loom.decision` read the store.

It implements the port's two operations for the records beside a request
(`EscalationSuppressedRecord`, declared with the port in `story:decision-request-entity`): an
append is kept in the request's file under the same lock and the same write, and a read gives the
records in the order they were appended. An append beside an unknown request is refused
`NoSuchRequest`. The store holds the record as the port declares it and reads nothing into it.

## Why

Item 3 of the request needs an inbox, which is a durable store (coordinator's decision: the inbox
is a store plus CLI and SDK calls, accepted by session `sdc` on 2026-11-21). The governor already
keeps a file per case, per suspension and per workflow run over one lock and one write
(`AGENTS.md` § Governor; `crates/loom-governor/src/locked_dir.rs`, `file_suspension.rs`,
`file_run_store.rs`), with the port in another crate and the file implementation in the governor
(`FileWorkflowRunStore` implements a port of `b10x-loom-executor`, `AGENTS.md` § Governor). The
reuse check of `decision-grounds.sh:68-79` reads Decided requests from a log; in Loom it reads
this store (`story:grounds-resolve-against-held-cases`).

## Waits on

`story:decision-request-entity`: the port with its record operations and its `StoreConflict` and
`StoreFailure` results, the entity, the record type and the views.

## ESS first

`ess/domains/governor.yaml`: `StoredDecisionRequest` (the request, its state, the records kept
beside it), whose request and record fields are typed by `loom.decision`, as `governor.yaml:125`
types a field by `loom.evaluation`. The conflict is the port's `StoreConflict`, so the governor
domain declares no conflict type of its own. On that commit `task drift` fails, and
`crates/loom-governor/tests/decision_store.rs` does not compile.

## Acceptance

- A request written, the process ended, and the store opened again in a second process reads the
  same request; `crates/loom-governor/tests/decision_store.rs` holds it across a real process
  restart, as `restart_durability.rs` does for a case.
- Two threads that read one Open request and each write a different `Decide` leave one Decided
  request; the second write is refused `StoreConflict` naming the request, and the file holds the
  first.
- Two threads that each write a new request with the same id from an empty read leave one request;
  the second is refused `StoreConflict`.
- Two threads that each write a new Open request with a different id and the same `record_key`
  from an empty read leave one request; the second is refused `StoreConflict` naming the holder
  in `record_key_held_by`, and `Request` over the file store answers `RecordKeyOpen` naming it.
  The check for an Open or Escalated request under the record key and the write are one
  `locked(...)` operation: the rule is an obligation of `DecisionStore::write`
  (`story:decision-request-entity`, "As built"; security pass 1, F3,
  `review-result:decision-request-entity-security-1`).
- Two `EscalationSuppressedRecord`s appended beside a request, the process ended, and the store
  opened again in a second process read back both records in order, with the request unchanged.
- Two threads that each append one record beside the same request, while a third writes a
  `Decide` to it, leave the Decided request and both records in the file; an append beside an
  unknown request is refused `NoSuchRequest`.
- A write into a store directory made read-only after the store opened (as
  `restart_durability.rs:195` does for a case) is refused `StoreFailure`, and the request file
  keeps every byte it had.
- A `.tmp-<process id>-<n>` file holding half a request, left in the directory under a process id
  no live process has, is removed when the store opens (`locked_dir.rs:9-12`), and the request
  reads its earlier state.
- A request id longer than 100 bytes is kept under a `decision-sha256-` file name, and an id of
  100 bytes under its hex, as a case id is (`crates/loom-governor/src/file_store.rs:133-146`).
- A file holding another request's id, and a truncated file, are each refused `StoreFailure`
  naming the file.
- A case file, a suspension file and a decision file in one directory are each read back by
  their own store.
- `OpenDecisionsForPrincipal` over the file store equals the view over the in-memory store for
  the same 20 requests.
- `task check` exits 0.

## Out of scope

A query index or a database. Deleting requests. A store in another process or service. A
fault-injection seam in `LockedDir`.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-governor` (a new file store beside `file_store.rs`, `file_suspension.rs`, `file_run_store.rs`) — cited (story Outcome: "`FileDecisionStore` in `b10x-loom-governor`")
- **Files:** `crates/loom-governor/tests/decision_store.rs` (new; Acceptance and ESS first) — cited
- **Files:** `ess/domains/governor.yaml` (gains `StoredDecisionRequest` and `DecisionStoreConflict`; the file exists and declares `StoredCase`, `StoredCaseId` at :23,:107) — cited
- **Files:** `crates/loom-governor/src/file_decision.rs` (new module; name follows `file_suspension.rs` and `file_run_store.rs`) — inferred
- **Files:** `crates/loom-governor/src/lib.rs:175-182` (one `mod file_decision;` line and one `pub use`, beside the existing `mod file_run_store;` ... `pub use file_suspension::...`) — inferred
- **Files:** `crates/loom-governor/Cargo.toml` (a path dependency on `b10x-loom-decision`, which the entity story creates; the manifest has none today) — inferred
- **Symbols (reused, not changed):** `LockedDir::{open,locked,replace,path}`, `read_if_present`, `failed` (`crates/loom-governor/src/locked_dir.rs:35,52-70,126,193`), `FileStoreError` (`lib.rs:181`), `HEX_NAMED = 100` (`file_store.rs:82`), `FileCaseStore::case_path` (`file_store.rs:133-146`) — cited
- **Symbols (new):** `FileDecisionStore` — cited; `DecisionStore` and `EscalationSuppressedRecord` (`b10x-loom-decision`, declared by `story:decision-request-entity`; not in the tree yet, `git grep` finds no `DecisionStore`) — cited from the story, absent from code
- **Documents:** none outside the house-rule files. `AGENTS.md` (Governor section) is a house-rule file. — cited
- **Confidence:** high on the surface (the story names the crate, the type, the ESS file and the test file). Medium on the exact module and file names (inferred from the sibling stores). — cited for the surface, inferred for the names
- **Would collide with** (all on `crates/loom-governor/src/lib.rs`, `crates/loom-governor/Cargo.toml` or `ess/domains/governor.yaml`; this story's edits there are one `mod`/`pub use` line pair, one dependency line, one ESS block):
  - `story:decision-request-entity` — no shared file in the body (it owns `ess/domains/decision.yaml`, `crates/loom-decision`); ordered by direct `depends_on` — cited
  - `story:grounds-resolve-against-held-cases` (`lib.rs`, `governor.yaml`) — ordered by direct `depends_on` — cited
  - `story:decided-record-written-then-recorded`, `story:approvals-supplied-to-canon-evaluation` (`lib.rs`, `governor.yaml`) — ordered through `grounds-resolve-against-held-cases` — cited
  - `story:requests-from-canon-pending-decisions` (`lib.rs`, `governor.yaml`, `Cargo.toml:15`), `story:decided-records-carry-grounds-to-canon`, `story:authority-records-bound-and-grounded-in-canon` (`lib.rs`, `governor.yaml`) — ordered through `approvals-supplied-to-canon-evaluation` — cited
  - `story:decision-suspension-reason` edits `crates/loom-governor/src/file_suspension.rs`, a file this story only reads; no collision, and no `depends_on` path — cited
  - UNORDERED, outside the epic, none with a `depends_on` path to or from this story (checked in the graph JSON): `story:canon-0-9-0-pin` (`Cargo.toml`, `lib.rs`; status active), `story:engineering-protocols-dependency-removed` (`Cargo.toml`), `story:engineering-registry-calls-replaced` (`lib.rs`), `story:hash-chained-run-log` (`lib.rs`, `file_store.rs`, `locked_dir.rs`), `story:child-case-runtime` (`lib.rs`, `file_store.rs`, `governor.yaml`), `story:child-case-own-workspace` (`governor.yaml`), `story:record-copy-between-cases` and `story:bound-artifact-revisions` (`lib.rs`), `story:frontier-names-movers`, `story:standing-tree-rests-quiescent` (`lib.rs`) — cited from their scope entries or bodies. These are line-adjacent edits, not same-line edits, so the textual conflict is small, but the planner sees a shared file.
- **Safety fact:** a new module and a new file prefix (`decision-…`) change no reader of the case, suspension or run files: each store opens only its own prefix (`case-`, `suspension-`, `workflow-run-`, `locked_dir.rs` module doc), and all four share one lock through `LockedDir` — step 2 (`crates/loom-governor/src/locked_dir.rs:1-22`), unproven. The lock is held for each operation and not across two, so the story's read-then-write conflict check has to be one `locked(...)` closure that reads the state, compares it and writes; `FileSuspensionStore` does this for hold and take (`file_suspension.rs:1-25`) — step 2, unproven.
- **Contradicted by the code:** see Story defects.

Edges:

- Shares the sync sites of `crates/loom-governor/src/locked_dir.rs` and `file_store.rs` with
  `story:tree-run-tests-fit-the-budget`. A sync this story adds goes through that story's
  `durable` door of `b10x-loom-governor`, and its test `no_sync_outside_the_door` holds.
