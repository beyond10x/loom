---
format: aep.planning-md/3
id: story:hash-chained-run-log
kind: story
status: implemented
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
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:45Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":4}}}
- {from: "proposed", to: "active", at: "2026-11-21T04:23:20Z", actor: "agent:sdc", revision: 8, decided_on: {"recorded":{"review_outcome":4}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w3"}
- {from: "active", to: "implemented", at: "2026-11-21T07:04:54Z", actor: "agent:sdc", revision: 12, decided_on: {"recorded":{"test_result":1,"review_outcome":6,"verification":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w3"}
---
## Outcome

A new crate, `b10x-loom-runlog` (`crates/loom-runlog`), keeps one run log per case: the file
`runlog-<stem>.jsonl` beside the case file `<stem>.json` in the store directory, one JSON line
per event. Each line carries `seq` (0, 1, 2, ...), `prev` (the SHA-256 of the previous line's
bytes, absent on line 0), the instant, the event kind and its members. An append is atomic and
made under the store's lock: after a crash at any point the file holds every earlier line
unchanged and either all of the new line or none of it. Lines are never rewritten. A reader
function checks the chain and refuses a log with a cut last line, a missing line, a changed byte
or a `seq` out of order, naming the first line that fails. A change confined to the last line is
not detected, as a cut at a line boundary is not: nothing after the last line holds its digest and
the log is not signed (stated on 2026-11-21 during wave 2026-11-21-w3, after the adversary's first
pass; the chain never promised more). An `action finished` line holds an exit or a refusal,
exactly one, and a line with both or neither is refused. The crate declares the events and
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

Rewritten 2026-11-21 at the close of wave 2026-11-21-w3 from the unit's commits (`git diff --stat 39283f1b 942bceab`: 62 files, 3595 insertions, 107 deletions). Every line is cited from that diff.

- **Primary surface:** `crates/loom-runlog` (new crate `b10x-loom-runlog`) — cited, 5 of the 62 changed files are under it (manifest, `src/lib.rs`, three new test files); the governor (3 files) and `crates/loom-docs` (3 files) carry the rest of the source change
- **Files (ESS sources):** `ess/domains/runlog.yaml` (183 lines, new: domain `loom.runlog`, 15 types), `ess/system.yaml` (1 line, `loom.runlog`), `ess/ess-inputs.yaml` (1 line, `domains/runlog.yaml`) — cited
- **Files (generated):** `docs/contracts/run-log-v1.json` (182 lines, `task docs-generate`), `generated/rust/loom/**` (23 files: `src/runlog.rs` 179 lines new, `PLAN.md`, `plan.json`, `Cargo.toml`, and the other modules, a 4-line change each except `src/lib.rs` with 5, `task generate`), `website/docs/reference/ess/**` (16 files: `loom-runlog.md` 142 lines new, `index.mdx`, 14 pages with 2 lines each), `website/data/ess/loom-runlog.domain-graph.json` (12 lines, new), `website/docs/reference/crates.md` (3 lines: the crate row and the count 29 to 30) — cited
- **Files (manifests):** `crates/loom-runlog/Cargo.toml` (20 lines, new; depends on `b10x-loom-governor`, `loom`, `sha2 0.11.0`), `Cargo.lock` (11 lines, the new package) — cited
- **Files (source):** `crates/loom-runlog/src/lib.rs` (708 lines, new: `RunLog`, the append under the governor's lock, `verify`, `encode`, the crash steps), `crates/loom-docs/src/run_log.rs` (149 lines, new: the contract writer for `loom.runlog`), `crates/loom-docs/src/main.rs` (30 lines: `mod run_log`, `run_log_contract`, called at the end of `generate` beside `run_events_contract`), `crates/loom-docs/src/run_events.rs` (4 insertions, 4 deletions: `declared`, `field_names`, `variants` and `field_type` become `pub(crate)`), `crates/loom-governor/src/file_store.rs` (20 insertions, 12 deletions: the stem rule moves out of `case_path` into `case_file_stem`), `crates/loom-governor/src/locked_dir.rs` (14 insertions, 6 deletions: `LockedDir` and four methods public, doc comments), `crates/loom-governor/src/lib.rs` (2 insertions, 1 deletion: two re-exports) — cited
- **Files (new tests):** `crates/loom-runlog/tests/run_log.rs` (721 lines, 15 cases), `crates/loom-runlog/tests/adversary_w20261121w3_u1.rs` (753 lines, 12 cases) and `crates/loom-runlog/tests/adversary_w20261121w3_u1_pass2.rs` (210 lines, 3 cases), the two adversary passes' cases — cited
- **Files (existing tests):** `crates/loom-executor/tests/crate_names.rs` (2 insertions, 1 deletion: `EXPECTED` goes from 29 to 30 with the `b10x-loom-runlog` entry) — cited
- **Files (documents):** `AGENTS.md` (35 insertions, 1 deletion: the generated-files row, `LockedDir` public, a new § Run log), `CHANGELOG.md` (1 line), `README.md` (1 line, the crate table), `website/data/status.json` (1 line, the item "Run log per case") — cited
- **Symbols (`b10x_loom_runlog`, new):** `PREFIX`, `EXTENSION`, `log_file_name`, `digest`, `AppendStep` (`Created`, `HalfWritten`, `Written`, `Synced`, `Renamed`, `DirectorySynced`; `ALL`, `renamed`), `RunLogError` (`Refused`, `Store`), `RunLog` (`open`, `dir`, `path`, `read`, `append`, `append_until`), `verify`, `encode` — cited
- **Symbols (`loom.runlog` types, generated into `loom::runlog`):** `RunLogLine`, `RunLogEvent` (ten variants: `CaseOpened`, `Evaluated`, `ActionStarted`, `ActionFinished`, `Discarded`, `ChildOpened`, `ChildClosed`, `AuthorityRecorded`, `Stopped`, `Terminated`), `RevisionMove`, `RunLogRefusal` (`CutLine`, `BrokenChain`, `SequenceGap`), `RunLogRefused` — cited
- **Symbols (`b10x_loom_governor`):** added `case_file_stem` (the stem of `case_path`, `FileCaseStore::case_path` now calls it); `LockedDir` and its `open`, `path`, `locked`, `temporary` go from `pub(crate)` or private to `pub`; both exported from `lib.rs`. Nothing removed — cited
- **Symbols (`loom-docs`, crate-private):** `run_log::{CONTRACT, SYSTEM, contract, text}`, `run_log_contract`, and the four helpers of `run_events.rs` widened to `pub(crate)` — cited
- **Not predicted and touched:** `crates/loom-runlog/tests/adversary_w20261121w3_u1.rs` and `crates/loom-runlog/tests/adversary_w20261121w3_u1_pass2.rs` (inside the predicted crate directory, not named by the stored Scope section); the widening of `LockedDir::open`, `path`, `locked` and `temporary`, which the story's "changes nothing else in it" does not name (the stored Scope section's safety fact names `open`, `locked` and `replace`; `replace` stays private and `temporary` is the method made public instead)
- **Predicted and not touched:** `crates/loom-docs/Cargo.toml` (inferred, "only if the schema version is read from the runlog crate")
- **Review:** `review-result:hash-chained-run-log-adversary-1` (head 85133ac4, red: 12 cases added, one red, `ActionFinished` taking both or neither of exit and refusal; the unchecked change to the last line named in four texts; five surviving mutants) and `review-result:hash-chained-run-log-adversary-2` (head 600b9fdd, nothing found: 27 to 30 cases, red 0, the exactly-one rule enforced in `decode`; one note, "the log is not signed" twice in `status.json:24`)
