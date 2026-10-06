---
format: aep.planning-md/3
id: story:interruption-recovery
kind: story
status: draft
title: Define interruption and recovery
refs:
- provider: taskboard
  reference: L-013
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:session-transcript-streaming
- depends_on: story:selection-revalidation
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:run-pipeline-skeleton
- depends_on: story:harness-loop-port
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/src/recovery.rs
- confidence: inferred
  path: crates/loom-executor/src/session.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary2_run_identity.rs
- confidence: cited
  path: crates/loom-executor/tests/interruption_recovery.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
- confidence: inferred
  path: website/data/ess/loom-run.domain-graph.json
- confidence: cited
  path: website/data/status.json
- confidence: inferred
  path: website/docs/concepts/commission-and-harness.md
- confidence: cited
  path: website/docs/reference/ess/loom-run.md
revision: 21
---
## Outcome

A Loom run can be cancelled at any point and recovered from its filed session, and a run suspended
at an approval resumes from its checkpoint before the exact effect (the Harness approval checkpoint,
`harness-loop/src/approval.rs`, and `LoopStop::Cancelled`, at `798325f0`). Recovery trusts nothing
that was in flight: it re-projects from the frontier current at resume and revalidates
(`story:selection-revalidation`) before returning any `ProposedAction`, so a selection made before
the interruption cannot be proposed against a moved case.

## Shared surface

Behavioural edges: `story:session-transcript-streaming` (recovery resumes the filed session by
id), `story:selection-revalidation` (recovery revalidates before proposing) and
`story:harness-loop-port` (a resumed run re-projects and selects through the wired loop). Depends on
`story:run-pipeline-skeleton` for the `recovery` module. The former edge to
`story:compaction-contract` was ordering-only (both edit `loom.run.Session`) and was dropped on
2026-10-04; the shared `ess/` and `generated/` paths still keep the two in separate waves.
`story:loom-ess-conformance` depends on it. The whole order is in `story:agent-executor` § Shared
surface.

## ESS first

- **First commit:** add the interrupt and resume commands and their outcomes on `loom.run.Session`
  in `ess/domains/run.yaml`; `ess specify validate --path ess` passes; nothing else changes. Whether
  this resume is the resume `story:session-transcript-streaming` declares, extended, or a second
  command is settled before that commit, and the story says which.
- **Red on it:** `task drift` fails, naming the first file of `generated/rust/loom/` that differs
  from the model regenerated from the changed specification.
- **Then:** `task generate`; the test `interruption_recovery`; the implementation that makes it
  pass.

## Domain relations

- `loom.run.Session -> loom.run.Turn` (relation `turns`, owns) and `loom.run.Selection ->
  loom.run.ActionCatalogue` (relation `catalogue`) for the revision a pre-interruption selection was
  made at — inferable from `ess/domains/run.yaml`.

## Depends on, outside this store

`commission:story:run-outcomes` (M-007) for the Run's suspend and resume, and
`commission:story:authority-provider-port` (M-005) for the fake that grants the approval.

## Scope

Derived 2026-10-06 by `story-scoper` at loom `04a1a73`. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-executor` (package `b10x-loom-executor`, library `loom_executor`) — cited
- **Path map:** the body's `crates/loom/` is `crates/loom-executor/` (story:crate-names); Harness `harness-loop/src/approval.rs` at `798325f0` is `crates/loom-executor/src/harness/turn_loop/approval.rs`; `commission:story:run-outcomes` (M-007) is `crates/loom-commission/src/outcome.rs` (`RunStore`) with `SuspendRun`/`ResumeRun` at `ess/commission/domains/responsibility.yaml:643-672`; `commission:story:authority-provider-port` (M-005) is `crates/loom-commission/src/ports/authority.rs` with its fake at `crates/loom-commission-testkit/src/fake_authority.rs` — cited
- **Files:** `crates/loom-executor/src/recovery.rs` (3-line stub, declared at `lib.rs:24`) — cited
- **Files:** `crates/loom-executor/src/lib.rs` (`Loom.runs` at :68; `run_id` at :152, whose doc at :148-151 assigns the cross-`Loom` id collision to this story) — cited
- **Files:** `crates/loom-executor/tests/interruption_recovery.rs` (new) — cited
- **Files:** `crates/loom-executor/tests/adversary2_run_identity.rs:291` (`#[ignore = "story:interruption-recovery; ESS-SYNTH-004"]` on `a_refused_selection_is_never_selected_or_admitted_again`) — cited
- **Files:** `ess/domains/run.yaml`, the only domain `ess/ess-inputs.yaml` lists (`loom.run.Session` :75-104, lifecycle `[Active, Filed]`; `ResumeSession` :385; `SelectAction` :470 and `RequestArguments` :532 for `existing_instance`) — cited
- **Files:** `generated/rust/loom/` (8 files, rewritten by `task generate`, checked by `task drift`) — cited
- **Symbols:** `run_id`, `Loom::runs`, `loom.run.Session`, `loom.run.ResumeSession`, `LoopStop::Cancelled` (`harness/turn_loop/mod.rs:158`), `AgentLoop::resume_approval` (`mod.rs:2200`) — cited
- **Also likely:** `crates/loom-executor/src/session.rs` — inferred: `run_and_file` builds the loop with no cancel handle (`session.rs:38`, `:644`) and files only completed turns, so cancelling a session run and keeping an approval checkpoint lands there
- **Reads, does not edit:** `crates/loom-executor/src/revalidation.rs` (story:selection-revalidation), `crates/loom-executor/src/harness/turn_loop/approval.rs` and `mod.rs`, `crates/loom-commission-testkit/src/fake_governor.rs` and `fake_authority.rs` — cited
- **Documents:** `website/docs/reference/ess/loom-run.md`, generated from `ess/` by `loom-docs` (`crates/loom-docs/src/main.rs:39-43`) and checked by `task docs-check` in `task check` — cited
- **Documents:** `website/data/status.json:31` (row "Harness loop wired to a governed run") and `CHANGELOG.md` Unreleased, by the rule at `AGENTS.md:159-161` — cited
- **Documents:** `website/data/ess/loom-run.domain-graph.json` — inferred, changes only if Session's states or relations change
- **Documents:** `website/docs/concepts/commission-and-harness.md:41` ("interruption and recovery, is planned") — inferred, the page that describes the capability
- **Confidence:** high — the story names every code file and each resolves in the tree after the rename; only `session.rs` and two documents are readings
- **Would collide with:** any unit editing `crates/loom-executor/src/lib.rs`; any unit changing `ess/domains/run.yaml`, which also rewrites `generated/rust/loom/` and `website/docs/reference/ess/loom-run.md` (one such unit per wave); the `status.json` row shared with compaction and budgets; `CHANGELOG.md` Unreleased — cited
- **Would also collide with:** any unit on `crates/loom-executor/src/session.rs`, and any unit on approval suspend/resume across a restart (`story:approval-suspend-resume-slice` area, no edge to this story) — inferred
- **Safety fact:** `run_id` is private to `lib.rs`, and no file outside `crates/loom-executor`, nor `session.rs` or `harness/`, names `SelectionId` or `selection_id`. So giving each `Loom` its own id namespace reaches no session file, wire or other crate. The unit test `run_ids_are_uuids_of_their_kind_frontier_and_run` (`lib.rs:419-442`) pins today's derivation and changes with it — step 2 (`git grep -E 'SelectionId|selection_id' -- crates`), unproven

Not established while scoping, to settle before the story is proposed:

- Whether ESS 0.53.0 still refuses `existing_instance: true` on `SelectAction`/`RequestArguments` with ESS-SYNTH-004.
- Which approval suspension "suspended at the merge approval" means: the Harness checkpoint (`LoopStop::AwaitingApproval`, `resume_approval`) or Commission's `AwaitingApproval` run end (`crates/loom-commission/src/runtime.rs:28-33`), while `lib.rs:13-14` says "Loom never suspends for authority".
- The shape of interrupt: `run.yaml` has no interrupt or cancel command, and Session has only `[Active, Filed]`.
- Whether recovery needs the Run to survive a process restart: Commission's `RunStore` is in memory only (`outcome.rs:9-11`).
- The adversary case `two_looms_give_their_different_selections_on_one_frontier_different_ids` is not in the tree.

## Acceptance

The test `interruption_recovery` in `crates/loom/tests/interruption_recovery.rs` passes. With the
Commission fakes, it checks:

1. A run cancelled after a selection, and resumed by session id after the fake governor advanced
   the case revision, projects its first catalogue at the new revision.
2. That resumed run returns no `ProposedAction` for the pre-interruption selection; the
   revalidation refusal names the stale revision.
3. A run suspended at the merge approval, resumed by session id after the fakes grant the approval
   with the frontier otherwise unchanged, returns a `ProposedAction` for `repository.merge`, and the
   selector is not called again between the suspension and that return.
4. A run suspended at the merge approval, resumed after the fake governor advanced the case
   revision, re-projects at the new revision before returning anything.

## Source

TASKBOARD L-013; Atlas ADR 0071 and 0072; Harness `harness-loop/src/approval.rs` at `798325f0`.

## From wave 2026-10-04-w9 (run-pipeline-skeleton, adversary pass 2)

`SelectAction` and `RequestArguments` declare no `existing_instance:` refusal, so a retried or
replayed `SelectAction` with the id of a refused selection resets it to `Selected`, and revalidation can
admit it. The fix (`existing_instance: true`) makes ESS 0.52.0 refuse the synthesized suite
(ESS-SYNTH-004: the route through `SelectAction/selected` is reached by no input). Needs an ESS change
before recovery can rely on selection identity; the adversary case
`a_refused_selection_is_never_selected_or_admitted_again` is ignored until then.

## Carried from story:argument-generator (wave 2026-10-04-w15)

Selection and argument-request ids are derived from the frontier id and a per-`Loom` run counter
(`run_id` in `crates/loom/src/lib.rs`), so they are unique only within one `Loom`. A recovered
`Loom` restarts the counter at 0: recovery must not reuse a run id an earlier `Loom` gave, for
example by adding a per-`Loom` namespace to the hash. The adversary case that shows the collision
is `two_looms_give_their_different_selections_on_one_frontier_different_ids`.
