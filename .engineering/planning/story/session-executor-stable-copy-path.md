---
format: aep.planning-md/3
id: story:session-executor-stable-copy-path
kind: story
status: active
title: The session executor works in a scratch copy path the host keeps stable across a case's sessions
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:O3
scope:
- confidence: cited
  path: crates/loom-session-executor/src/lib.rs
- confidence: cited
  path: crates/loom-session-executor/tests/stable_copy_path.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 5}
---
## Outcome

A host can give the session executor a stable name for its scratch copy (a new `SessionExecutor`
member, such as `copy_name: Option<String>`), so every session the host runs under that name works
in the same path: `<scratch>/<copy_name>/copy`. The harness puts its working directory into the
context it sends, so successive steps of one case then send the same bytes and the provider's
prompt cache holds past it. Without a name the path is today's unique
`session-<pid>-<nanos>-<n>`. The name is one plain path component (no `/`, `.`, `..`, control
character, empty or over 64 bytes), else the step suspends before any copy. The root is made with
mode 0700 as today; a directory left at that path by an earlier session of the same user (a
directory, not a link, owned by the effective user) is removed first; anything else there suspends
the step before any copy, naming the path. The copy is removed when the call returns, as today.

The metaharness part of the same finding is a later item and not this story: metaharness's own
per-session paths for the child (its scratch root, `TMPDIR`, `CLAUDE_CONFIG_DIR`), b10x-harness
placing `environment.workspace` ahead of the project instructions in its context, and whether
print sessions can use the 5-minute cache class instead of the 1-hour one.

## Why

The trial analysts' report 2, item 6 (C5 in the trial summary; the orchestrator's item D283):
every Claude step's first request read only Claude Code's static prefix from the cache (4,780
tokens, all 83 first requests) and wrote the rest again (460,033 tokens over the runs, about 5.5k a
step, all in the 1-hour class), and every b10x-harness step's first request had
`cached_input_tokens: 0`, although the instruction files were byte-identical. Each step ran in a new
copy (`session-3588784-…-0/copy`, `-1/copy`, `-2/copy`), made by `ScratchCopy::make`
(`crates/loom-session-executor/src/lib.rs:3056-3063`), and b10x-harness's `started.context` hash
changed between steps only in `environment.workspace`. takt builds one executor per step
(takt `crates/takt/src/harness.rs` at 0.24.0, `SessionExecutor::new` per step), so the stable
name has to come from the host (takt would pass its case id), not from the executor.

This is cost, not the run budget: the budget counts cache reads in full (report 2, F6). Whether the
cache holds after this change depends on the metaharness part above; this story removes Loom's
share of the per-step difference and measures it on the recorded `RunSpec.cwd`.

## Acceptance

`crates/loom-session-executor/tests/stable_copy_path.rs`:

- `two_sessions_under_one_copy_name_work_in_the_same_path`: two calls of two executors with the same
  `copy_name` and `scratch` report the same `SessionReport::cwd` and hand metaharness the same
  `RunSpec.cwd`; the copy is gone after each call.
- `without_a_copy_name_each_session_gets_its_own_path`: today's behaviour, two different paths.
- `a_copy_name_that_is_not_one_plain_component_is_refused`: `a/b`, `..`, `.`, empty, a control
  character, 65 bytes: the step suspends `ExternalAvailability` before any copy.
- `a_leftover_copy_of_the_same_user_is_replaced`: a directory left at the path is removed and the
  session runs.
- `a_link_or_foreign_entry_at_the_path_is_refused`: a symbolic link (to a directory outside
  `scratch`) or a regular file at `<scratch>/<copy_name>` suspends the step; the link's target is
  untouched.
- `a_second_session_while_the_first_runs_is_refused`: unchanged (the executor's existing guard).

## ESS first

None: the scratch copy is host configuration `ess/` does not declare.

## Scope

- cited: `crates/loom-session-executor/src/lib.rs`: `struct SessionExecutor` fields and `fn new`
  (about lines 588-720), `ScratchCopy::make` (about line 3056), the call in `fn session` (about
  line 1247)
- cited: `crates/loom-session-executor/tests/stable_copy_path.rs` (new)
- AGENTS.md § Session executor (one paragraph); CHANGELOG.md is the coordinator's
- Shares `src/lib.rs` with `story:session-executor-claude-loopback-endpoint`: this story stays
  outside `fn spec` and `fn check_api_key_secret`.
- Touches the scratch copy's confinement: a security-reviewer pass runs beside the adversary.
