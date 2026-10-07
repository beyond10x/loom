---
format: aep.planning-md/3
id: story:query-provider-refusal
kind: story
status: active
title: Diagnose intermittent provider refusal during clock query selection
relations:
- informed_by: story:system-query
- serves: vision:O1
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T12:54:54Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"test_result":1}}}
- {from: "proposed", to: "active", at: "2026-10-07T12:54:54Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
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
