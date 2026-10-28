---
format: aep.planning-md/3
id: story:session-transcript-withholds-stderr-credentials
kind: story
status: draft
title: The session transcript and the event observer hold a harness child's stderr to the executor's credential walk
relations:
- decomposes: epic:loom-on-llm-and-metaharness
revision: 1
---
## Outcome

The session executor's transcript (`SessionReport::transcript`) and the event observer never carry a
harness child's standard error that the executor's credential walk would withhold: the
`NO_TERMINAL_RECORD` warning metaharness raises is held to the same walk as the failure record, or
its stderr text is left out of the transcript.

## Why

The adversary's judgement on wave 2026-10-28-w1, u1 (review-result:adversary-w20261028w1-u1-pass-1,
no case): "Metaharness's `NO_TERMINAL_RECORD` warning carries the whole unredacted stderr. It still
reaches `SessionReport::transcript`, which takt keeps on disk at mode 0600, and the event observer.
The crate docs say a withheld line is \"never repeated\"." Origin undecided: the adversary did not run
the base. Not measured: whether any harness child in use writes a credential on stderr.

## Acceptance

- A scripted child that writes a token on stderr and ends without its action line leaves neither the
  transcript nor the observed lines holding the token, and the failure record keeps its tail as today.

## Scope

Inferred 2026-10-28, not yet scoped by a story-scoper.

- `crates/loom-session-executor/src/lib.rs` (the poll loop's transcript append and observer call) — inferred
- `crates/loom-session-executor/tests/` — inferred
