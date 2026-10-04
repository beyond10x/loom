---
format: aep.planning-md/3
id: story:session-transcript-streaming
kind: story
status: draft
title: Preserve Harness session, transcript and streaming behaviour
refs:
- provider: taskboard
  reference: L-011
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:harness-module-map
- depends_on: story:agent-executor
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

Loom keeps the Harness behaviour for sessions, transcripts and streaming, through the disposition
`story:harness-module-map` gives each crate (depend at a pinned revision, or port): turns are
stateless and replayed whole; opaque provider items are stored verbatim and refused across wires; a
session is filed outside the workspace, `0700`, with no credential and no instruction text, written
whether the run answered or died, and resumable by id; streamed reasoning reaches the stream sink as
it arrives and is not stored.

## ESS first

Extend `loom.run.Session` and `loom.run.Turn` with the filing and resume commands and their outcomes,
including the cross-wire refusal; validate with `ess specify validate --path ess`; regenerate the
synthesized model.

## Domain relations

- `loom.run.Session -> loom.run.Turn`, one-to-many; the session owns its turns and a turn does not
  outlive it — inferable from `ess/domains/run.yaml`, entity `loom.run.Session`, relation `turns`
  (owns, many, via `session_id`).
- A resume continues the same session rather than opening a new one — inferable (inferred from
  `harness/crates/harness-cli/src/transcript.rs:184` and `:219`, `Session::save` and `Session::load`
  by id, at harness `798325f0`; no ess/1 document declares it).

## Constraints

`beyond10x/harness` is not changed; its consumers keep their pinned revisions.

## Acceptance

A `b10x-loom` test over a provider-emulated endpoint shows a run streaming its deltas to the stream
sink as they arrive and filing a session outside the workspace with mode `0700` and no credential or
instruction text, which a second run resumes by id and replays item for item.

## Source

TASKBOARD L-011; Atlas ADR 0071; Harness README § Sessions, resume and chat, and AGENTS.md
invariants 4 and 5 and § Safety envelope, at `798325f0`.
