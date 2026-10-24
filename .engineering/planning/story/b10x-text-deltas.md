---
format: aep.planning-md/3
id: story:b10x-text-deltas
kind: story
status: implemented
title: A b10x-harness action line streamed in several text deltas is read as one line
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-24T02:18:54Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-24T02:18:54Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-24T02:59:14Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

A b10x-harness session whose reply streams the action line in several `text-delta` events is read
as that one line: the executor joins a turn's deltas with no separator, keeping the line breaks
the deltas carry, and separates turns by a line break. Claude Code and Codex sessions are read as
before.

## Why

takt-trial cycle 0118 (takt 0.11.0, Loom 0.39.1, b10x-harness 0.18.0, the openai wire) failed
with "the session ended without the JSON line naming its action". The step's transcript holds 31
`text-delta` events in the last turn which, concatenated, are exactly the line
`{"action": "tests.write", "summary": "..."}` (118 bytes, no newline). `SessionExecutor::session`
pushes each `Event::Text` into `said` (`crates/loom-session-executor/src/lib.rs:1006`) and reads
`reply(&said.join("\n"))` (`lib.rs:1171`), so the line arrives split over 31 lines and `reply`
(`lib.rs:2113`) finds none. Every b10x-harness test sends the action as one delta
(`tests/kinds.rs:222`, `tests/endpoint_secret.rs:135`). It blocks takt's ADR 0101 harness arm.

## Acceptance

`crates/loom-session-executor/tests/b10x_text_deltas.rs`:

- `an_action_line_split_over_deltas_is_read_as_one_line`: a b10x-harness session whose last turn
  streams the action line in several deltas, as the cycle 0118 transcript does, yields a proposal
  of that action and summary.
- `a_turns_own_line_breaks_are_kept`: prose, a newline inside a delta, then the action line split
  over deltas still reads the action.
- `turns_are_separated_by_a_line_break`: an earlier turn's text that ends without a newline does
  not run into the next turn's action line.
- Unchanged: every existing Claude Code, Codex and b10x-harness test.

## ESS first

None. How the executor reads a session's text is not declared in `ess/`; the red run is the new
test file on a commit that adds only it.

## Not in scope

A change to metaharness's event mapping; Claude Code or Codex text handling.

## Scope

- `crates/loom-session-executor/src/lib.rs` (`SessionExecutor::session`)
- `crates/loom-session-executor/tests/b10x_text_deltas.rs`
- AGENTS.md § Session executor, CHANGELOG.md
