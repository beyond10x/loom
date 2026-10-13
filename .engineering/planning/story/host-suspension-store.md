---
format: aep.planning-md/3
id: story:host-suspension-store
kind: story
status: implemented
title: A host keeps a commission's Suspension in a directory (production file store)
relations:
- serves: vision:O1
- informed_by: story:approval-suspend-resume-slice
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission-testkit/src/file_suspension.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/approval_suspend_resume.rs
- confidence: inferred
  path: crates/loom-governor/src/file_suspension.rs
- confidence: inferred
  path: crates/loom-governor/src/lib.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-13T07:10:02Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-10-13T07:35:42Z", actor: "human:timo", revision: 8}
- {from: "active", to: "implemented", at: "2026-10-13T10:08:51Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A host on a released Loom keeps a commission's Suspension in a directory: a production
file-backed implementation of Commission's suspension port lives in `loom-governor`, beside
`FileCaseStore`, sharing its lock and atomic-rename code.

## Why

Since 0.17.0 the only file-backed suspension store is the testkit's
(`crates/loom-commission-testkit/src/file_suspension.rs:4` says it is for tests), so no host can
keep a Suspension across a restart. Decided 2026-10-13 (`decision-blocker:host-suspend-resume`):
the store lives in `loom-governor` beside `FileCaseStore` (`crates/loom-governor/src/file_store.rs`).

## ESS first

No new noun: the Suspension and its commands are declared in `ess/commission/` (0.17.0). The
unit's first commit is the red test: `approval_suspend_resume.rs` driven against the production
store fails to compile until the store exists. If a durable-format noun is needed, it goes into
`ess/domains/governor.yaml` first.

## Acceptance

`crates/loom-commission-testkit/tests/approval_suspend_resume.rs`, its two-process restart
included, and both adversary files pass against the production store in `loom-governor`; the
testkit's `FileSuspensionStore` re-exports it or is removed.

## Scope

- `crates/loom-governor/src/file_suspension.rs` (new), `crates/loom-governor/src/lib.rs` (inferred)
- `crates/loom-commission-testkit/src/file_suspension.rs` (cited)
- `crates/loom-commission-testkit/tests/approval_suspend_resume.rs` (cited)
- `CHANGELOG.md` (inferred)

Build: about 7G with headroom (the last unit's target reached 7.1G).
