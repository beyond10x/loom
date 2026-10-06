---
format: aep.planning-md/3
id: story:compaction-contract
kind: story
status: draft
title: Define the compaction contract
refs:
- provider: taskboard
  reference: L-012
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:session-transcript-streaming
- depends_on: story:frontier-projection
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:run-pipeline-skeleton
- depends_on: story:harness-loop-port
scope:
- confidence: cited
  path: crates/loom-executor/src/compaction.rs
- confidence: inferred
  path: crates/loom-executor/src/harness/turn_loop/
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: inferred
  path: crates/loom-executor/src/session.rs
- confidence: cited
  path: crates/loom-executor/tests/compaction_contract.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
- confidence: inferred
  path: website/data/ess/loom-run.domain-graph.json
- confidence: inferred
  path: website/docs/reference/ess/loom-run.md
revision: 16
---
## Outcome

A Loom compaction contract. When a session crosses its trigger (Harness: 80 % of a declared context
window, freeing to 50 %, otherwise the byte rule `MAX_CONVERSATION_BYTES` / `COMPACTED_TARGET_BYTES`,
`harness-loop/src/lib.rs:885-937` at `798325f0`), the transcript prefix is compacted and the
compaction is priced and recorded on the session. Compaction never carries a stale catalogue
forward: the first request after it carries the catalogue projected from the frontier current at
that request, and nothing the model wrote is promoted to trusted context.

## Shared surface

Behavioural edges: `story:session-transcript-streaming` (the compaction record lives on the filed
session), `story:frontier-projection` (the first request after compaction carries a fresh
projection) and `story:harness-loop-port` (acceptance item 2 reads the tool list of the wired loop's
requests). Depends on `story:run-pipeline-skeleton` for the `compaction` module.

It still edits `loom.run.Session` in `ess/domains/run.yaml` and regenerates `generated/rust/loom/`
itself (the compaction record and usage shapes are not settled for the skeleton), and so does
`story:interruption-recovery`. The two have no behavioural edge between them; the former
ordering-only edge from `story:interruption-recovery` to this story was dropped on 2026-10-04, and
the shared `ess/` and `generated/` paths keep them in separate waves. The whole order is in
`story:agent-executor` § Shared surface.

## ESS first

- **First commit:** add the compaction command and its outcome on `loom.run.Session` in
  `ess/domains/run.yaml`, the outcome carrying the compaction's usage; `ess specify validate --path
  ess` passes; nothing else changes.
- **Red on it:** `task drift` fails, naming the first file of `generated/rust/loom/` that differs
  from the model regenerated from the changed specification.
- **Then:** `task generate`; the test `compaction_contract`; the implementation that makes it pass.

## Domain relations

- `loom.run.Session -> loom.run.Turn` (relation `turns`, owns): compaction rewrites turns the session
  owns — inferable from `ess/domains/run.yaml`, entity `loom.run.Session`.

## Scope

Derived 2026-10-06 by `story-scoper` at `04a1a73`. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Path map:** the body's `crates/loom/` is now `crates/loom-executor/` (package `b10x-loom-executor`, library `loom_executor`), renamed by `story:crate-names`; `crates/loom` does not exist at `04a1a73` — cited
- **Primary surface:** `crates/loom-executor` — cited
- **Files:** `crates/loom-executor/src/compaction.rs` (was `crates/loom/src/compaction.rs`; a 3-line stub that says this story builds it) — cited
- **Files:** `crates/loom-executor/src/lib.rs` (was `crates/loom/src/lib.rs`; `pub mod compaction;` at :21) — cited
- **Files:** `crates/loom-executor/tests/compaction_contract.rs` (was `crates/loom/tests/compaction_contract.rs`; new, absent today) — cited
- **Files:** `ess/domains/run.yaml` (`loom.run.Session` at :75, the session commands at :290-430, outcomes from :609) — cited
- **Files:** `generated/rust/loom/` (rewritten by `task generate`, checked by `task drift`, `Taskfile.yml:59-76`) — cited
- **Symbols:** `loom.run.Session`, `MAX_CONVERSATION_BYTES`, `COMPACTED_TARGET_BYTES`; the Harness `harness-loop/src/lib.rs:885-937` the body cites is now `turn_loop/mod.rs:897-953` — cited
- **Symbols:** `AgentLoop::compact_run` (`turn_loop/mod.rs:2925`, runs before each request at :2551), `LoopEvent::Compacted` (`turn_loop/event.rs:254`, carries no usage), `SessionFile` (`session.rs:103`, has no compaction field) — inferred, the trigger, its record and the filed session the outcome must reach
- **Also likely:** `crates/loom-executor/src/harness/turn_loop/` — inferred: the 80 % / 50 % trigger and the summary turn already run there, and the summary turn's usage goes only into the run totals (`RunState::absorb_usage`, mod.rs:3071), not into any compaction record
- **Also likely:** `crates/loom-executor/src/session.rs` — inferred: "recorded on the session" means the filed `SessionFile`
- **Documents:** `website/docs/reference/ess/loom-run.md`, `website/data/ess/loom-run.domain-graph.json` — inferred: `task check` runs `docs-check` (`Taskfile.yml:31`), which rebuilds both from `ess/` (`crates/loom-docs/src/main.rs:43-45`); commit 9b9a09a, which last edited `loom.run.Session`, changed both
- **Test dependency, not changed:** the "Commission fake governor" is `FakeGovernor` in `b10x-loom-commission-testkit` (`fake_governor.rs:128`). It is already a dev-dependency of the executor and scripts one revision and frontier per call, so acceptance item 2 needs no testkit change — inferred
- **Confidence:** medium — all five surfaces the story names exist after the rename, but where compaction hooks in (`turn_loop/` vs `lib.rs`) and the session-file change are read from the tree, not named by the story
- **Would collide with:** any unit editing the loom run specification `ess/domains/run.yaml` (and so `generated/rust/loom/`), or `crates/loom-executor/src/lib.rs` — cited
- **Would collide with:** any unit in the ported loop under `crates/loom-executor/src/harness/` (where the loop-to-projection wiring lands), the filed session format in `crates/loom-executor/src/session.rs`, or the two `loom-run` reference pages — inferred
- **Safety fact:** filed sessions keep resuming only if the compaction record goes into `SessionFile` as a `#[serde(default)]` field, or the change bumps `SESSION_VERSION`. The file is `deny_unknown_fields` at version 2 (`session.rs:98-102`) and any other version is refused by name (`session.rs:490-500`). A build that writes the field also makes its files unreadable to an older build — inferred, step 2, unproven

Not established while scoping: whether the fresh catalogue is wired in `lib.rs`, `turn_loop/` or both (`lib.rs:207` projects once per run and never calls the turn loop; `story:harness-loop-port` does that wiring); the usage shape (`run.yaml` declares none; the only `Usage` is `crates/loom-executor/src/harness/wire/turn.rs:304`); whether the compaction record bumps `SESSION_VERSION` (a design decision); whether the provider-emulated endpoint in `tests/session_transcript_streaming.rs:632` is shared or copied.

## Acceptance

The test `compaction_contract` in `crates/loom/tests/compaction_contract.rs` passes. Over a
provider-emulated endpoint with a declared context window, with the Commission fake governor, it
runs a session past the trigger and checks:

1. After compaction the session is at or below 50 % of the declared window.
2. Between the last request before compaction and the first after it, the fake governor moves the
   frontier to a new case revision whose admissible set differs. The tool list of the first request
   after compaction equals the catalogue projected from the new frontier, and differs from the tool
   list of the last request before it.
3. The filed session holds exactly one compaction record, and its usage equals the usage the
   endpoint reported for the compaction request.
4. The instruction text of the first request after compaction equals that of the last request
   before it, byte for byte; the compacted summary appears only as model-authored content.

## Source

TASKBOARD L-012; Atlas ADR 0071 and 0072; Harness `harness-loop/src/lib.rs:885-937` at `798325f0`.
