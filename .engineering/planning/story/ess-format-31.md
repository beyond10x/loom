---
format: aep.planning-md/3
id: story:ess-format-31
kind: story
status: active
title: Loom's main ESS system declares ess/31 on the pinned ess 0.86.0, with the raise's resource-command changes taken
summary: ess/system.yaml from ess/25 to ess/31, regenerated; five loom.resource commands gain an existence check and a refusal order, with tests and Breaking notes; the intake and commission systems and the toolchain unchanged
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-resource/tests/record.rs
- confidence: cited
  path: crates/loom-session-executor/src/lib.rs
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: cited
  path: ess/system.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:17Z", actor: "agent:sdc", revision: 7, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T07:20:39Z", actor: "agent:sdc", revision: 8, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w4"}
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

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `ess/system.yaml` (line 1, `format: ess/25` to `ess/31`) and the regenerated `generated/rust/loom/` — cited, story Outcome and ESS first
- **Files:** `ess/system.yaml:1` — cited
- **Generated (listed for this story only):** `generated/rust/loom/src/behaviour.rs` (+31/-3), `src/run.rs` (+1/-1), `plan.json` (+1/-1), `PLAN.md` (+1/-1) — cited, probe and regeneration at `sweep31`
- **Test file:** `crates/loom-resource/tests/record.rs` — inferred. `a_resource_the_record_does_not_hold_is_not_found` (`:393-415`) already covers three of the five; `ObserveScheduledRun` has no absent-row test anywhere
- **Symbols:** `DeleteLoomAgent`, `ObserveDeployment`, `ObserveRun`, `ObserveScheduledRun`, `RecordCheck`, `LoomAgentRecord` — cited. `LoomAgentRecord` is `crates/loom-resource/src/lib.rs:155,179,214,242,268`, the wrapper both callers go through
- **Documents:** `AGENTS.md:993`, `AGENTS.md:1663` — cited. Neither reason holds at `ess/31` as written (see defects)
- **Confidence:** high on the file set (the story and the diff name every file). The semantic claim is contradicted by the code (see Safety fact)
- **Would collide with, `ess/system.yaml`:**
  - `story:decision-request-entity` — cited. It has `depends_on` this story, so it is ordered
  - `story:autonomy-grant-table` — cited. Ordered through `decision-request-entity`
  - `story:hash-chained-run-log` — cited. No `depends_on` path either way
  - `story:tree-run-command` — cited. No `depends_on` path either way
  - `story:engine-configuration-document` — inferred. Prose only, no scope entry, no path
  - Each of the last three edits the `domains:` list, not line 1, so the merge is mechanical
- **Would collide with, `generated/rust/loom/{plan.json,PLAN.md,src/behaviour.rs,src/run.rs}`:** any story that edits `ess/domains/*.yaml` — inferred. `story:egress-by-host`, `story:secret-files-several-secrets`, `story:boundary-admits-declared-http-egress`, `story:command-home-survives-pod-replacement`, `story:agent-node-run-records` and `story:revalidation-membership-conformance` have no `depends_on` path to or from this story; `story:flow-limits-recorded-as-events` is ordered after it. Whoever lands second regenerates
- **Would collide with, `crates/loom-resource/tests/record.rs`:** no other open story scopes it — cited, `.engineering/planning/story/*.md` scope lines
- **Callers of the five commands, and whether the new check or order can change what each observes (all none):**
  - `crates/loom-resource/src/lib.rs:155,179,214,242,268` (wrappers). The match arms for `Unknown` do nothing (`:167,203,256,277`) — cited, none
  - `crates/loom-operator/src/reconcile.rs:281` (`delete_loom_agent`, `let _ =`; the row may be absent) — cited, none
  - `reconcile.rs:372` (`forget`, `let _ =`; callers `:1395`, `:1674`, `tests/reconcile.rs:845`) — cited, none
  - `reconcile.rs:1429` (`?`; row present, guarded by `recorded` at `:1416`) — cited, none
  - `reconcile.rs:1297` (`observe_scheduled_run`, `?`), `:1460` (`record_check`, `?`; after `apply`, `Stale` returns at `:1445`), `:1540`, `:1566`, `:1600` (`observe_run`, `?`), `:1590` (`observe_deployment`, `?`). The row is applied earlier in the same reconcile; no outcome is read — cited, none
  - `crates/loom-operator/tests/*` (118 reconcile calls in `reconcile.rs`, plus the `adv*` files) reach the commands only through `reconcile`/`forget` — inferred, none
  - `crates/loom-conformance/src/lib.rs:234-243` dispatches; `src/resource.rs:121,215,281,370,412` map `Unknown` to the scenario answer `unknown` (`:134,225,294,383,426`) — cited, none
  - `ess/scenarios/resource-schedule/runs-reported-in-turn.yaml:15,19,23` and `a-run-past-its-deadline-is-reported-failed.yaml:15,19` use `RecordCheck` and `ObserveScheduledRun` after `ApplyLoomAgent`, so the row is present — cited, none
  - `crates/loom-resource/tests/record.rs:123,137,260-267,393-415,483-487,497+` (the five commands, 38 hits): the absent-row asserts at `:396,:402,:408,:484` expect `Unknown`, as today — cited, none
  - `crates/loom-cli`, `crates/loom-sdk`: no call — cited. `loom-cli` depends on `loom-resource` only for `stage_files`
- **Safety fact (existence check):** the code before the raise already answers `Unknown { LoomAgentNotFound }` for an absent row, as the first branch of each command: `generated/rust/loom/src/behaviour.rs:499` (`DeleteLoomAgent`), `:517`, `:598`, `:674`, `:734`. `behaviour.rs` at `HEAD` is byte-identical to `orig`. The regenerated up-front `is_none()` check returns the same value before any other branch, so no outcome or stored row changes — walked, level 3, unproven
- **Safety fact (refusal order):** the only other `behaviour.rs` hunks are `RecordSelection` (`:848`) and `RequestArguments` (`:884`), both in `loom.run`. `RecordSelection` moves `let _ = &related;` and adds an empty `{}` block. `RequestArguments` changes one comment. The generated `RequestArguments` is not used: `crates/loom-executor/src/arguments.rs:302` is hand-written. `RecordSelection` runs generated at `arguments.rs:294` and has no present-related refusal, so it is behaviour-identical — walked, level 3, unproven
- **`SelectAction`:** the contract text gains a clause (`run.rs:1326`, `plan.json:1815`, `PLAN.md:341`). Its implementor is hand-written (`crates/loom-conformance/src/lib.rs:11`, `executor::selection::select_action`) and is not touched — cited
