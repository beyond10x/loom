---
format: aep.planning-md/3
id: story:ess-format-31
kind: story
status: implemented
title: Loom's main ESS system declares ess/31 on the pinned ess 0.86.0, with the raise's resource-command changes taken
summary: ess/system.yaml from ess/25 to ess/31, regenerated (four generated files, +34/-6); five loom.resource commands gain an up-front existence check that answers as their first branch did; one regression test, no Breaking note; the intake and commission systems and the toolchain unchanged
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-resource/tests/adversary_w20261121w4_u2.rs
- confidence: cited
  path: crates/loom-resource/tests/record.rs
- confidence: cited
  path: crates/loom-session-executor/src/lib.rs
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: cited
  path: ess/system.yaml
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:17Z", actor: "agent:sdc", revision: 7, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T07:20:39Z", actor: "agent:sdc", revision: 8, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w4"}
- {from: "active", to: "implemented", at: "2026-11-21T09:11:56Z", actor: "agent:sdc", revision: 12, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w4"}
---
## Outcome

Loom's main ESS system, `ess/system.yaml`, declares `format: ess/31` instead of `ess/25`, on the
toolchain Loom already pins (`requires: ess 0.86.0`, `ess/ess-inputs.yaml:2`; CI installs the same
release, `.github/workflows/check.yml:58-72`). `generated/rust/loom/` is regenerated
(`task generate`). No command's outcome and no stored row changes. `ess/intake/system.yaml`
(`ess/26`) and `ess/commission/system.yaml` (`ess/22`) stay as they are, and no ESS toolchain
moves.

The story has two commits: the first changes `ess/system.yaml` only; the second, the regeneration
commit, carries the regenerated files, the new regression test, the corrected comments and the
documents below.

What the regeneration changes, read from the `ess/31` regeneration of the loom system
(`diff -r` against `generated/rust/loom/` at `61fd500b`):

- Four generated files differ, +34/-6 lines: `generated/rust/loom/src/behaviour.rs` (+31/-3),
  `generated/rust/loom/src/run.rs`, `generated/rust/loom/plan.json` and
  `generated/rust/loom/PLAN.md` (one line each).
- In `behaviour.rs`, `DeleteLoomAgent`, `ObserveDeployment`, `ObserveRun`, `ObserveScheduledRun`
  and `RecordCheck` gain an up-front check that the addressed row exists, answering
  `Unknown { LoomAgentNotFound }` when it does not. The code before the raise already answers
  exactly that as the first branch of each command (`let Some(held) = … else { return …Unknown … }`,
  `generated/rust/loom/src/behaviour.rs:499`, `:517`, `:598`, `:674`, `:734`), so the outcome for
  an absent row and for a present one is the same before and after.
- In `loom.run`, `RecordSelection` (`behaviour.rs:848-853`) moves one statement
  (`let _ = &related;`) and gains an empty block with a comment, and `RequestArguments`
  (`:884-890`) moves the same statement and changes one comment. Neither changes a branch.
- `run.rs`, `plan.json` and `PLAN.md` gain one clause in the `SelectAction` contract text
  (`run.rs:1326`); its implementor is hand-written and is not touched.

The five commands are reached through `LoomAgentRecord` in `crates/loom-resource`
(`crates/loom-resource/src/lib.rs:155`, `:179`, `:214`, `:242`, `:268`), which the operator
(`crates/loom-operator/src/reconcile.rs`) and the conformance adapter
(`crates/loom-conformance/src/resource.rs`) call. `crates/loom-resource/tests/record.rs` already
tests the absent row for `RecordCheck`, `ObserveDeployment` and `DeleteLoomAgent`
(`a_resource_the_record_does_not_hold_is_not_found`, `:393-415`) and for `ObserveRun`
(`:483-487`); `ObserveScheduledRun` has no such test, and this story adds one as a regression test
that passes before and after the raise.

Four places give `ess/25` as a reason; each is corrected in the regeneration commit:

- `AGENTS.md:993`: "on ess/25 a subject-fact guard (a stale generation) cannot combine with the
  subject-state guards" becomes: the specification does not combine a subject-fact guard with the
  subject-state guards (`ess/27` would allow it), so the controller compares the Job's generation
  itself.
- `AGENTS.md:1663`: "`ess/25` having no empty-list literal" becomes: the specification leaves
  `files` undetermined (it does not store an empty list there, which `ess/31` would allow).
- `ess/domains/resource.yaml:864`, a comment: "on `format: ess/25` a guard on a subject field … and
  guards on the held state cannot share a command" becomes the same statement as `AGENTS.md:993`.
  No declaration changes; this comment is corrected.
- `crates/loom-session-executor/src/lib.rs:2193`, a doc comment: "the specification's format
  (`ess/25`) has no empty-list literal" becomes the same statement as `AGENTS.md:1663`.

## Why

`story:decision-request-entity` copies SDC's decision domain, which stores `[]` into required
lists in `sets:` and calls `coalesce(...)` on the left of a comparison. `ess` 0.86.0 accepts the
first from `ess/31` and the second from `ess/29`, and refuses both at `ess/25` to `ess/30` (the
probe; the format history entries `ess/29` and `ess/31` in `~/beyond10x/ess`, tag `0.86.0`,
`crates/specify/ess-domain/src/system.rs:303-308` and `:328-333`; `SUPPORTED_FORMATS` up to `34`
at `:53-56`). Session `sdc` chose on 2026-11-21 to raise `ess/system.yaml` to `ess/31` in its own
story before the entity story, keeping `[]` and `coalesce` as SDC writes them
(`.engineering/drafts/e3-sdc-answers-6.md`). `ess/31` is the lowest format with both, and 0.86.0
accepts up to `ess/34`. The combination `AGENTS.md:993` called impossible is allowed from `ess/27`
(`system.rs:275` at tag `0.86.0`), which is why the four reasons are corrected rather than kept.

## Waits on

Nothing in this epic. `story:decision-request-entity` waits on this story.
`crates/loom-session-executor/src/lib.rs` is also changed by `story:session-executor-b10x-ask`,
`story:agent-node-run-records` and `story:agent-session-through-substrate`, outside this epic and
with no edge; this story changes one doc comment there (`:2193`), and whichever lands second
rebases.

## ESS first

`ess/system.yaml`: `format: ess/25` becomes `format: ess/31`; no other declaration in `ess/` changes, and
the comment at `ess/domains/resource.yaml:864` is corrected in the regeneration commit. On
that commit `task drift` fails, naming the four generated files.

## Acceptance

- `ess/system.yaml` declares `format: ess/31`; `ess/intake/system.yaml` still declares `ess/26`,
  `ess/commission/system.yaml` `ess/22`, and `ess/ess-inputs.yaml` `requires: ess 0.86.0`.
- After `task generate`, `task drift` exits 0, and the regeneration commit's diff under
  `generated/` touches only the four files named in the Outcome.
- `task conform` exits 0 with no scenario and no assertion edited.
- `cargo test -p b10x-loom-resource`, `cargo test -p b10x-loom-operator` and
  `cargo test -p b10x-loom-conformance` exit 0 with no assertion edited.
- A new test in `crates/loom-resource/tests/record.rs` drives `ObserveScheduledRun` on a record
  with no row for the name and asserts `Unknown { LoomAgentNotFound }` and an unchanged record. It
  is a regression test: it passes on the code before the raise and after it.
- `CHANGELOG.md` under `[Unreleased]` has one `### Changed` bullet naming the raise of
  `ess/system.yaml` to `ess/31` and this story, and no `**Breaking:**` bullet for it.
- `AGENTS.md:993` and `:1663`, the comment at `ess/domains/resource.yaml:864` and the doc comment at
  `crates/loom-session-executor/src/lib.rs:2193` no longer give `ess/25` as a reason (a grep for
  `ess/25` in those files finds none).
- `task check` exits 0.

## Out of scope

`ess/intake/system.yaml` and `ess/commission/system.yaml`. Any change of the ESS toolchain
(`ess` 0.86.0 stays). `ordered:` enums, which need `ess/36` and `ess` 0.91.1. The decision domain
itself (`story:decision-request-entity`). Any change of a command's outcome.

## Scope

Rewritten 2026-11-21 at the close of wave 2026-11-21-w4 from the unit's commits (`git diff --stat 63668a97 4544b9a8`: 11 files, 386 insertions, 21 deletions). Every line is cited from that diff.

- **Primary surface:** `ess/` and its regeneration `generated/rust/loom/` — cited, 6 of the 11 changed files are under them (`ess/system.yaml`, `ess/domains/resource.yaml`, and the four generated files); the other five are `crates/loom-resource` (2), `crates/loom-session-executor` (1), `AGENTS.md` and `CHANGELOG.md`
- **Files (ESS sources):** `ess/system.yaml` (1 line: `format: ess/25` to `format: ess/31`), `ess/domains/resource.yaml` (7 lines, 4 added and 3 removed: the comment above the command that takes no generation no longer gives `ess/25` as its reason; no declaration changes) — cited
- **Files (generated, one line):** `generated/rust/loom/{PLAN.md,plan.json,src/behaviour.rs,src/run.rs}` by `task generate` from `ess/`, +34/-6 lines (`behaviour.rs` 34 changed lines, 31 added and 3 removed; the other three 1 line each) — cited
- **Files (source):** `crates/loom-session-executor/src/lib.rs` — cited, 8 lines (4 added, 4 removed): the doc comment on `impl RecordFailedSessionBehavior for SessionRecord` no longer gives `ess/25` as its reason; no code changes
- **Files (new tests):** `crates/loom-resource/tests/adversary_w20261121w4_u2.rs` (280 lines, 3 cases), the adversary pass's cases, committed unchanged — cited
- **Files (existing tests):** `crates/loom-resource/tests/record.rs` (57 lines, 55 added and 2 removed: the import list gains `ObserveScheduledRun` and `ObserveScheduledRunOutcome`, and one case is added, `a_scheduled_run_of_a_resource_the_record_does_not_hold_is_not_found`, taking the file from 7 to 8 `#[test]`; no assertion edited) — cited
- **Files (documents):** `AGENTS.md` (12 lines, 7 added and 5 removed: the reason the observe command takes no generation, and the reason `RecordFailedSession` leaves `files` undetermined, at `:1014` and `:1719` where the story said `:993` and `:1663`), `CHANGELOG.md` (1 line: one `### Changed` bullet naming `ess/31` and the story, no `**Breaking:**`) — cited
- **Symbols:** no public symbol added, changed or removed — cited; `src/` changes only a doc comment in `loom-session-executor`, and the generated `behaviour.rs` changes only inside existing trait-impl bodies (an up-front existence check in `DeleteLoomAgent`, `ObserveDeployment`, `ObserveRun`, `ObserveScheduledRun`, `RecordCheck`, and a moved statement and an empty block in `RecordSelection` and `RequestArguments`); the `SelectAction` contract text in `run.rs` gains a clause
- **Not predicted and touched:** `CHANGELOG.md`, `crates/loom-resource/tests/adversary_w20261121w4_u2.rs`; `ess/domains/resource.yaml` and `crates/loom-session-executor/src/lib.rs` are in the typed scope but not in the stored Scope section (the Outcome names them)
- **Predicted and not touched:** none; the stored Scope's callers list (`crates/loom-resource/src/lib.rs`, `crates/loom-operator/src/reconcile.rs`, `crates/loom-conformance/src/resource.rs`, `ess/scenarios/resource-schedule/*`) was read, not predicted to change
- **Review:** `review-result:ess-format-31-adversary-1` — adversary pass 1 on head `8b69b0b6`: green, no finding; 3 cases added (46 passed in `b10x-loom-resource`, 43 before), the four corrected `ess/25` reasons probed true, no `**Breaking:**` owed
