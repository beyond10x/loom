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
- decides: story:refused-selection-stays-refused
revision: 5
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

## Change of order (2026-10-24)

Session `b10x-chef-orchestrator` first approved, under the same grant, "ess 0.69.0 is tagged
[...] 1. Release 0.39.2 (the text-delta join fix) first [...] 2. Then item 3, loom 0.40.0: move
the ess pin to 0.69.0, re-test the selection-guard story with host-stop option B as you recorded,
and release", and then replaced it: "Change of order, from noto for Timo. This replaces my last two
messages: there is no 0.39.2. Make one release, loom 0.40.0, containing: (a) the ess 0.69.0 pin
move; (b) the selection-guard story with host-stop option B; (c) the text-delta join fix and its
split-action test. [...] Build against tag 0.69.0 (e1cf58c11), and tag 0.40.0 only after I confirm
ess 0.69.0 is verified. If the 0.69.0 move is blocked for more than 2 hours, tell me at once: the
fallback is 0.39.2 with only the fix."

- The wave now carries three units and releases 0.40.0, not 0.39.2: u1 `story:b10x-text-deltas`
  (c), u2 the ess 0.69.0 pin (a), u3 `story:refused-selection-stays-refused` (b).
- The 0.40.0 tag waits for the orchestrator's confirmation that ESS 0.69.0 is verified.
- D3 changes with it: the release is 0.40.0; whether it breaks anything is decided by what u2 and
  u3 change.

## Decisions taken while the wave ran

- D4: (b) is read as `story:refused-selection-stays-refused`, the selection guard ESS 0.69.0 was
  awaited for. `story:needs-authority-before-host-stop` (option B) depends on
  `story:frontier-names-movers`, whose new `FrontierObligation.moved_by` and
  `FrontierClaim.decided_by` break every consumer's struct literal of them, which the grant does not
  cover. It stays `draft`, as `approval-record:needs-authority-before-host-stop-option-b` records;
  the orchestrator was asked (2026-10-24) whether 0.40.0 should carry it with Timo's word on the
  break.

- D4 answered (2026-10-24): session `b10x-chef-orchestrator` wrote "D: A. loom 0.40.0 carries the
  ess 0.69.0 pin, story:refused-selection-stays-refused and the text-delta join fix with its
  split-action test. Host-stop option B stays draft. It breaks consumers' FrontierObligation and
  FrontierClaim literals, which is on the never-list, and the goal does not need it." and "Build
  and gate against the tag now, and tag 0.40.0 only after I confirm."

- D5: `model::run::SelectActionOutcome` gains `SelectionExists`, which breaks a consumer's
  exhaustive match. It is a generated model type, which `website/docs/reference/sdk-stability.md`
  places outside its promise, so it breaks no documented contract; the CHANGELOG names it in a
  `**Breaking:**` bullet and the release moves the minor version (0.40.0), as the orchestrator's
  order set.
- D6: `upstream-blocker:ess-0-69-existing-selection-guard` is cleared: on ESS 0.69.0 the `ess/`
  copy with `selection-exists` synthesizes `30 scenario(s) (0 authored), 0 refusal(s)`.
