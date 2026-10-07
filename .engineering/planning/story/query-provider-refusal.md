---
format: aep.planning-md/3
id: story:query-provider-refusal
kind: story
status: implemented
title: Diagnose intermittent provider refusal during clock query selection
relations:
- informed_by: story:system-query
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-cli/Cargo.toml
- confidence: cited
  path: crates/loom-intake-router/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice
- confidence: cited
  path: ess/intake
- confidence: cited
  path: generated/rust/intake
- confidence: cited
  path: website
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T12:54:54Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"test_result":1}}}
- {from: "proposed", to: "active", at: "2026-10-07T12:54:54Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
- {from: "active", to: "implemented", at: "2026-10-07T13:36:47Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":3}}}
---
## Outcome and root cause

A live reproduction captured the top-level Responses error code server_is_overloaded. llm 0.1.7 classifies it as Refused rather than transient Unavailable. This is provider capacity failure, not a model refusal, request-ceiling failure or clock tool failure. Correct classification belongs to llm; Loom owns the bounded request-attempt policy.

## Design and acceptance

Use llm Error::may_retry and llm-routing RetryPolicy for at most three identical attempts on the same model binding, waiting 1s and 2s (provider delays capped at30s). Retry only before any sink event; stop on cancellation, genuine refusal, auth failures, invalid requests, local request capacity and exhausted attempts. Preserve the last error class and observation and include attempt count after exhaustion. Measure and ceiling-check every attempt. No tool is invoked until arguments succeed and Commission revalidates.

Recorded clock workflows exercise recovery independently at classification, selection and arguments, byte-identical requests and every attempt measured; exhaustion and real refusal produce zero clock reads. Wrapper tests exercise streamed output and cancellation. Re-run the installed exact user command with payload-free context reports.

## ESS first

8037406 changes only ess/intake/domains/context.yaml. task intake-drift is the initial red check for the changed specification digest; its output is retained in .engineering/drafts/query-refusal-spec-red.log. Generate before implementation. No additional domain type is introduced; llm's typed failure and retry policy remain their owner's types.

## Scope

Loom intake request composition, per-attempt metrics and shared model wrapper; adoption of the reviewed llm provider classification fix; targeted workflow tests and user documentation. llm owns provider code handling and output visibility. No model fallback, credential change, effect retry, arbitrary retry of policy refusals or common-system-tool registry.

## Evidence

Baseline installed commit957a8a7: two Completed runs then one exact Refused suspension; selection4127 bytes. Diagnostic runs also fail at argument generation5273 bytes. Diagnostic3 captures server_is_overloaded before any output. Retained baseline and diagnostic reports are under .engineering/drafts/; no source payload is added to context reports.

## Implementation and verification

81d4b42 implements caller retries without changing Model's single-attempt contract. The updated
llm dependency c0e97d620c27a8b413facffb119793d6881599f2 is published in llm PR24 with all CI checks green.
Independent review corrected decoder output tracking in llm and found no remaining blocker in
Loom. All five retry unit tests pass; recorded system-query tests recover at each stage in both
policies and preserve usage from failed attempts. Exhaustion and refusal perform no clock effect.

task check, task plan and task website pass. The gate's1252 distinct test names all appear in
this checkout's inventory. Planning retains only the existing prose-only review warning. Five live
exact-intent bounded queries with --workspace=/tmp/foo completed; they encountered no overload,
so live recovery itself is not claimed. Loopback adapter tests and recorded workflows establish
that path. docs/qualification/2026-10-07-provider-overload.md records the boundary and evidence.

Temporary diagnostic source lives only in ignored .engineering/drafts/debug-scaffolding, never
in the published dependency or installed binary. The dependency is a development commit pin until
a released llm tag includes the correction. Local-main installation follows the already-authorized
dogfooding workflow; neither repository's remote main is merged by this change.
