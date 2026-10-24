---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-24-w2
kind: approval-record
status: draft
title: Wave 2026-10-24-w2 proposal accepted with no operator turn
tags:
- non-interactive
relations:
- decides: story:b10x-text-deltas
revision: 1
---
# Wave 2026-10-24-w2 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. Session `b10x-chef-orchestrator` approved it
under the grant in `~/beyond10x/CLAUDE.md`, in these words (2026-10-24): "New item, approved under
the grant in ~/beyond10x/CLAUDE.md: loom 0.39.2, which fixes how loom-session-executor reads the
action line from a b10x-harness session. It blocks ADR 0101 harness-arm. [...] Fix: join
b10x-harness text deltas with no separator. Keep each reply's own line breaks. Add a test that
splits the action line over several deltas, reusing that transcript's shape. Then release 0.39.2
through your process".

| unit | serves | surface | review |
|---|---|---|---|
| u1 `story:b10x-text-deltas` | `vision:governed-autonomy` | `crates/loom-session-executor/`, AGENTS.md, CHANGELOG.md | adversary |

## Decisions taken in planning

- D1: the cause is confirmed on the code: `lib.rs:1006` pushes each `Event::Text` and `lib.rs:1171`
  joins them with `"\n"`; b10x-harness's `text-delta` is a fragment (metaharness-b10x
  `fixtures/golden/loop.jsonl` lines 8-9: `"The file says:"`, `" hello harness"`).
- D2: only b10x-harness deltas are joined with no separator; each `TurnStarted` starts a new
  segment, and segments are joined with `"\n"`, so a turn's text never runs into the next turn's.
  Claude Code and Codex text events are whole messages and keep `"\n"`.
- D3: no ESS change, and no public type changes, so 0.39.2 is a patch with `No breaking changes.`
