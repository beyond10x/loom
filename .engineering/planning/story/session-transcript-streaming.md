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
- depends_on: story:harness-loop-port
scope:
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: inferred
  path: crates/loom/src/session.rs
- confidence: inferred
  path: crates/loom/tests/session_transcript_streaming.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
revision: 4
---
## Outcome

Loom keeps the Harness behaviour for sessions, transcripts and streaming, through the disposition
`story:harness-module-map` gives each crate (depend at a pinned revision, or port) and on top of the
loop `story:harness-loop-port` carries in:

- turns are stateless and replayed whole;
- opaque provider items are stored verbatim and refused across wires: a session recorded on one
  wire is refused by name, naming both wires, before anything is sent on another;
- a session is filed outside the workspace, directory `0700` and file `0600`, with no credential
  and no instruction text, written whether the run answered or died, and resumable by id;
- streamed reasoning text (`reasoning_summary_text` and `thinking_delta`) reaches the stream sink as
  it arrives and is not stored; the opaque item the turn ends with is what a session holds.

## Shared surface

Link 7 of the `epic:loom-native-harness` chain over `ess/domains/run.yaml`, `generated/rust/loom/`
and `crates/loom/src/lib.rs`. It depends on `story:harness-loop-port`, and
`story:compaction-contract` depends on it. The whole order is in `story:agent-executor` § Shared
surface. It extends `loom.run.Session` and `loom.run.Turn`, which `story:compaction-contract` and
`story:interruption-recovery` extend after it.

## ESS first

Extend `loom.run.Session` and `loom.run.Turn` with the filing and resume commands and their outcomes,
including the cross-wire refusal; validate with `ess specify validate --path ess`; regenerate with
`task generate`.

## Domain relations

- `loom.run.Session -> loom.run.Turn`, one-to-many; the session owns its turns and a turn does not
  outlive it — inferable from `ess/domains/run.yaml`, entity `loom.run.Session`, relation `turns`
  (owns, many, via `session_id`).
- A resume continues the same session rather than opening a new one — inferable (inferred from
  `harness/crates/harness-cli/src/transcript.rs:184` and `:219`, `Session::save` and `Session::load`
  by id, at harness `798325f0`; no ESS document declares it).

## Scope

- `crates/loom/src/session.rs` (new), `crates/loom/src/lib.rs`
- `crates/loom/tests/session_transcript_streaming.rs` (new)
- `ess/domains/run.yaml`, `generated/rust/loom/` (chain surface)

## Constraints

`beyond10x/harness` is not changed; its consumers keep their pinned revisions.

## Acceptance

The test `session_transcript_streaming` in `crates/loom/tests/session_transcript_streaming.rs`
passes. Over a provider-emulated endpoint, it checks:

1. Each streamed delta reaches the stream sink before the endpoint emits the next one.
2. After a run that answered, its session is filed outside the workspace directory, the session
   directory has mode `0700` and the file `0600`.
3. The filed session contains neither the credential the run used nor its instruction text.
4. The filed session contains none of the streamed reasoning text, and does contain the opaque
   reasoning item the turn ended with, byte for byte.
5. A second run resuming that session by id sends, as its replayed history, the stored items in the
   stored order, byte for byte.
6. Resuming that session on the other wire is refused before any request is sent, and the refusal
   names both wires.
7. A run whose endpoint drops the connection mid-stream still files its session, holding every turn
   completed before the drop.

## Source

TASKBOARD L-011; Atlas ADR 0071; Harness README § Sessions, resume and chat, and AGENTS.md
invariants 4 and 5 and § Safety envelope, at `798325f0`; `harness-cli/src/transcript.rs:176` and
`:480-496` (modes).
