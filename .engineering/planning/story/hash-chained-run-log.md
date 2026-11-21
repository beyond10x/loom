---
format: aep.planning-md/3
id: story:hash-chained-run-log
kind: story
status: active
title: Each case has a run log appended one line per event, each line naming the digest of the one before
summary: seq and prev per line, appended under the store lock, a reader that refuses a cut or altered log
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bindings-document
scope:
- confidence: inferred
  path: crates/loom-docs/Cargo.toml
- confidence: inferred
  path: crates/loom-docs/src/main.rs
- confidence: inferred
  path: crates/loom-docs/src/run_events.rs
- confidence: inferred
  path: crates/loom-docs/src/run_log.rs
- confidence: cited
  path: crates/loom-executor/tests/crate_names.rs
- confidence: cited
  path: crates/loom-governor/src/file_store.rs
- confidence: inferred
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/src/locked_dir.rs
- confidence: cited
  path: crates/loom-runlog
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: inferred
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: inferred
  path: website/data/status.json
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:45Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":4}}}
- {from: "proposed", to: "active", at: "2026-11-21T04:23:20Z", actor: "agent:sdc", revision: 8, decided_on: {"recorded":{"review_outcome":4}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w3"}
---
## Outcome

A new crate, `b10x-loom-runlog` (`crates/loom-runlog`), keeps one run log per case: the file
`runlog-<stem>.jsonl` beside the case file `<stem>.json` in the store directory, one JSON line
per event. Each line carries `seq` (0, 1, 2, ...), `prev` (the SHA-256 of the previous line's
bytes, absent on line 0), the instant, the event kind and its members. An append is atomic and
made under the store's lock: after a crash at any point the file holds every earlier line
unchanged and either all of the new line or none of it. Lines are never rewritten. A reader
function checks the chain and refuses a log with a cut last line, a missing line, a changed byte
or a `seq` out of order, naming the first line that fails. The crate declares the events and
writes what it is given; the run that emits them is `story:tree-run-command`. Events: case
opened, evaluated (case revision, frontier id, digest of the decision), action started, action
finished (exit or refusal, blob digest, evidence ids, revision moves), discarded, child opened,
child closed, authority recorded (capability, decision, principal), stopped (the stop and the
next decisions), terminated.

## Why

Loom's `EventSink` is in memory (`crates/loom-commission/src/ports/events.rs:26-35`:
`NoEvents` and `Vec<RuntimeEvent>`), and the `run-events-v1` lines of `run --output jsonl` carry
no sequence number and no digest of the previous line
(`docs/contracts/run-events-v1.json`, `crates/loom-cli/src/events.rs`). The SDC's live view
(`~/beyond10x/sdc/ui/server.mjs`) reads a per-case `events.jsonl`, and
`~/beyond10x/sdc/docs/driver.md` gaps 14 and 15 name what the driver's log still lacks: its
reader silently drops an unparsable last line and never checks `prev`. Canon is not an event
store (`boundary.md` row 6); the log is the runtime's. The append-by-rewrite under the directory
lock that `mandate-approvals.jsonl` uses (`crates/loom-intake-slice/src/mandate_log.rs:10-16`) is
the precedent for an atomic append in this store.

## ESS first

A new domain `loom.runlog` in `ess/domains/runlog.yaml`, declared in `ess/system.yaml`:
`RunLogLine` (`seq`, `prev`, `at`, `event`), `RunLogEvent` with one variant per event above, and
`RunLogRefusal` (`CutLine`, `BrokenChain`, `SequenceGap`). On that commit `task drift` fails,
and `crates/loom-runlog/tests/run_log.rs` does not compile. The contract file
`docs/contracts/run-log-v1.json` is generated from the domain by `task docs-generate`. This
story adds a line to `ess/system.yaml` after `story:bindings-document` did, which is why it
depends on it.

The store's lock and the case-file stem are `crates/loom-governor`'s (`LockedDir`,
`crates/loom-governor/src/locked_dir.rs:45`, `pub(crate)` today; the stem rule,
`crates/loom-governor/src/file_store.rs:133-146`). This story makes both public items of that
crate and changes nothing else in it. `story:bound-artifact-revisions` and
`story:child-case-runtime` change the same crate later; `story:bound-artifact-revisions` depends
on this story for that order.

## Acceptance

- After 50 events appended to one case, the log verifies, `seq` is gapless from 0, and each
  `prev` equals the SHA-256 of the line before it.
- Changing one byte of line 3 is refused `BrokenChain` naming line 4; deleting line 3 and
  swapping lines 3 and 4 are each refused `SequenceGap` naming line 3; truncating the last line
  in the middle is refused `CutLine` naming it.
- An append to a log that does not verify is refused with the reader's refusal and changes no
  byte of the file.
- Two processes that each append 200 lines to one store leave 400 lines, each a complete JSON
  object, and the log verifies.
- A crash injected at each write, sync and rename step of one append leaves a file that verifies
  and holds either n or n+1 lines.
- An append takes the same lock file as a case write: a test holds the governor's `LockedDir` on
  the store and an append from a second thread does not return until it is released.
- The log file of a case whose id is longer than 100 bytes sits beside its `case-sha256-…` file
  under the same stem.
- `docs/contracts/run-log-v1.json` exists, is generated, and `task docs-check` holds it.
- `task check` exits 0.

## Out of scope

Emitting the events, their order relative to store writes, and what a `tree` command prints
(`story:tree-run-command`). Agent and model-call events (`story:agent-node-run-records`). Folding
the log to rebuild a case. Signing the log. Changing `run-events-v1`.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-runlog` (new crate `b10x-loom-runlog`, tests in `crates/loom-runlog/tests/run_log.rs`) — cited, the story's Outcome and ESS-first paragraph
- **Files:** `ess/domains/runlog.yaml` (new), `ess/system.yaml` (one line added to `domains:`) — cited, the story's ESS-first paragraph
- **Files:** `crates/loom-governor/src/locked_dir.rs:45` (`LockedDir`, `pub(crate)` today), `crates/loom-governor/src/file_store.rs:136-147` (`FileCaseStore::case_path`, the stem rule) — cited, read in the tree and the story's ESS-first paragraph
- **Files:** `crates/loom-executor/tests/crate_names.rs:18` — cited, read in the tree. `EXPECTED` is `[...; 28]`, and the test requires the workspace package set to equal it exactly (header lines 4-5), so one entry `("b10x-loom-runlog", "loom-runlog", Some(..))` and `; 29` are needed
- **Symbols:** `LockedDir`, `FileCaseStore::case_path`, `HEX_NAMED`, `push_hex` (`file_store.rs:82-92`) — cited
- **Also likely:** `crates/loom-governor/src/lib.rs:175-179` — inferred, `mod locked_dir;` is private there, so a `pub use locked_dir::LockedDir` and an export of the stem function are needed
- **Also likely:** `crates/loom-docs/src/main.rs` — inferred. It holds the generator of `docs/contracts/run-events-v1.json`: `run_events_contract` (line 461), called at line 608 at the end of `generate`. A `run-log-v1.json` writer and its `--check` branch would be added beside that call
- **Also likely:** `crates/loom-docs/src/run_log.rs` (new module, modelled on `run_events.rs`, which holds `CONTRACT`, `SYSTEM`, `contract()` and `differences()`) — inferred
- **Also likely:** `crates/loom-docs/src/run_events.rs` — inferred, only if the new module reuses its private `stated_type` / `field_type` / `declared` helpers
- **Also likely:** `crates/loom-docs/Cargo.toml` — inferred, only if the schema version is read from the runlog crate as `b10x_loom_cli::events::SCHEMA_VERSION` is today
- **Also likely:** `ess/ess-inputs.yaml` — inferred. Its `specification:` list names every `domains/*.yaml`, so `domains/runlog.yaml` would be listed there. I did not run `ess` to prove a missing entry fails
- **Also likely:** `website/data/status.json` — inferred, hand-maintained, and `AGENTS.md` says a change that ships a capability updates it
- **Documents:** `docs/contracts/run-log-v1.json` is generated, so it is a house-rule file, not a hand-written document. No hand-written document is named by the story.
- **House-rule files:** `CHANGELOG.md`, `AGENTS.md` (the generated-files table at line 301, the crate and invariant sections), `README.md` (crate table near line 726), `Cargo.lock`, `generated/rust/loom/` (`task generate`), `website/docs/reference/ess/` (`loom-runlog` pages), `website/data/ess/loom-runlog.domain-graph.json`, `website/docs/reference/crates.md`, `docs/contracts/run-log-v1.json`.
- **Not touched:** root `Cargo.toml` (`members = ["crates/*"]` is a glob, nothing to edit), `crates/loom-xtask` (no domain or crate list in its sources; `drift` / `generate` run over all of `ess/`), `crates/loom-conformance` (`loom.metering` is the precedent of a types-only domain, so no conformance work follows), `.github/workflows/*`, `Taskfile.yml`, and `crates/loom-xtask/src/release.rs` `STABLE_SURFACES` (the story does not make `run-log-v1.json` a stable surface) — cited, by grep of the tree
- **Confidence:** medium — the story names the crate, domain and governor items exactly, and I checked the crate-list test and the generator in the tree. The `loom-docs` change shape and the `ess-inputs.yaml` / `status.json` lines are readings
- **Would collide with:** `crates/loom-governor/src` (`locked_dir.rs`, `file_store.rs`, `lib.rs`), which `bound-artifact-revisions` and `child-case-runtime` also change. Also `ess/system.yaml` and `ess/ess-inputs.yaml`, which `bindings-document` and any unit adding a domain change. Also `crates/loom-docs/src/main.rs`, `crates/loom-executor/tests/crate_names.rs` and `website/data/status.json`, which any unit adding a crate, a contract or a status item touches.
- **Safety fact:** making the `LockedDir` type `pub` is not enough to use it from another crate. Its `open` (`locked_dir.rs:52`), `locked` (line 105) and `replace` (line 121, the temp-file, sync and rename step) are `pub(crate)`, and the stem is not a function. It is fused with `.json` inside the private `case_path` and reads private `HEX_NAMED` and `push_hex`. "Changes nothing else in it" therefore means widening those methods and splitting out a stem function, with `case_path` calling it so the case file name stays byte for byte as it is — `file_store.rs:136-147`, unproven. A second fact to prove: `LockedDir::open` deletes every file in the store directory whose name starts with `TEMPORARY` (`locked_dir.rs:91-100`), so a runlog temporary must use that prefix, and `runlog-*.jsonl` is not touched by it — unproven
