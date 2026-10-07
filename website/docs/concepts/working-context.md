---
title: Working context
sidebar_position: 10
description: Opt-in bounded context for a single CLI intent, with retrievable history and measurements.
lede: Loom can send current working state and recent events while keeping earlier observations available through references.
source: ess/intake/domains/context.yaml; crates/loom-intake-slice/src/context.rs; crates/loom-intake-slice/src/selector.rs; crates/loom-intake-slice/tests/bounded_context.rs
---

:::caution[Development source]
This capability is implemented after release `0.3.0`. `legacy` remains the default. Recorded
workflows establish behavior and request-byte reduction; matched live evaluations are still needed
to establish task quality and cost. No proportional token-cost saving is assumed.
:::

The `--context-policy bounded` option replaces the rolling transcript with current working state,
a recent-event tail, and retrievable history. The original intent and instructions remain intact.
The briefing derives the latest revision, test exit status and timeout, tested revision, refusal,
and recent artifact references from typed execution reports. File contents that claim tests passed
cannot change this state. A passing test on revision A does not validate a later revision B.

Compact events enter a separate run-local archive from the first action. Bulk contents stay in the
[result store](result-references.md). The archive holds at most 4,096 events and 16 MiB of serialized
event data. Exceeding capacity stops subsequent model requests with an explicit error; completed
edits, test observations and printed effects remain visible. Neither store survives the run.

When an agent request exceeds 48 KiB, or keeping a new event would overflow the 64-entry tail, Loom
retires batches of old events, aiming for 32 KiB. Every bounded request must fit 64 KiB after JSON
serialization, including instructions, schemas and escaped lookup responses. Mandatory content
that cannot fit fails explicitly. Whole lookup responses may be omitted with an error; structured
records are never cut to fit. Classification is also checked against the request ceiling.

Retained events follow a stable intent/checkpoint prefix and precede changing working state and
candidates. New events append between checkpoints. Retirement changes that prefix, so it can
invalidate cached context; request-byte reduction does not imply cache or billing reduction.
This follows the tradeoff described in [Anthropic's context-editing and caching documentation](https://platform.claude.com/docs/en/build-with-claude/context-editing#context-editing-and-prompt-caching).

Both selection and argument generation can use `{$list_history: 0}` and `{$read_history: reference}`,
alongside the existing result lookups. Listings return a `next_offset`; references use the same
digest and whole/byte/line/JSON-pointer selection rules as stored results. Each stage permits eight
total lookups. A ninth argument lookup refuses the step; a ninth selection lookup suspends the run.
Lookups only select data and never grant authority or perform an action. No summarization model
call is added.

`--context-report PATH` writes JSON measurements for either policy: request bytes and elapsed time
by phase, checkpoint and retrieval counts, model calls, total elapsed time, and provider-reported
input, cache-read, cache-write and output counters. Missing counters are `null`; `final_usage`
distinguishes final usage from partial error observations. Reports contain no prompts or source
payloads. Request bytes describe the serialized provider-neutral `TurnRequest`, not provider HTTP
framing or billed tokens. Reports are also written when a started slice fails; CLI setup failures
before the slice starts produce no report.

Embedders can call `run_with_options` with `RunOptions`; `run` and `SliceRequest` remain compatible.
Interactive corrections, durable session memory, training exports and the governed-loop compactor
are outside this policy.
