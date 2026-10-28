---
format: aep.planning-md/3
id: story:session-executor-partial-runspec
kind: story
status: draft
title: The session executor merges a caller's partial RunSpec over its per-kind defaults
summary: effort, budget, strict version and context reach metaharness; takt's partial-runspec blocker is the consumer
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A caller of the session executor (`b10x-loom-session-executor`, reached through `b10x-loom-sdk`)
names a partial `metaharness::RunSpec` (effort, budget in US dollars, strict version and context
files) and the executor merges it over its per-kind defaults: each member the caller names wins,
each member it leaves unset keeps today's per-kind value, and a member Loom sets for its own
guarantees (credentials, working directory, scratch copy, hermetic mode, decision mode, the
b10x-harness endpoint and secret route) is never taken from the caller.

## Why

takt's dependency-blocker:loom-session-executor-partial-runspec (open, 2026-10-28) blocks takt's
story:harness-section-is-metaharness-session-request and story:harness-flags-spell-metaharness-names:
"takt's Loom 0.45.0 `SessionExecutor` has no effort field and hard-codes the `RunSpec` per kind".
`SessionExecutor::spec` (`crates/loom-session-executor/src/lib.rs`, `fn spec`) builds the
`RunSpec` from `RunSpec::new(self.kind)` and sets only credentials, cwd, prompt, model, decisions,
hermetic mode, max turns and the b10x endpoint. metaharness 0.22.0's `RunSpec` carries `effort`
(`Option<String>`), `max_budget_usd` (`Option<String>`), `strict_version` (`bool`) and `context`
(`Vec<PathBuf>`) (`metaharness-protocol` `src/spec.rs`), so the executor drops them.

Consumer: takt (dependency-blocker:loom-session-executor-partial-runspec). Drafted at the request
of session `b10x-chef-orchestrator`, 2026-10-28.

## ESS first

`loom.agent.SessionSpec` (`ess/domains/agent.yaml`) gains optional members for the four overrides,
so a `loom.agent/1` spec can carry them and `SessionExecutor::from_spec` applies them. The named
test that fails on the ESS-only commit is the story's own
`a_partial_runspec_overrides_the_kind_defaults` (below). Open question for scoping: whether the
budget is `Optional<String>`, as metaharness holds it, or a typed amount ESS admits.

## Acceptance

1. `a_partial_runspec_overrides_the_kind_defaults` (`crates/loom-session-executor/tests/`): for
   Claude Code, Codex and b10x-harness, a caller naming effort, budget, strict version and context
   sees each in the `RunSpec` handed to metaharness (recorded runner), and a caller naming none
   gets the same `RunSpec` as before, member for member.
2. `a_partial_runspec_cannot_override_what_loom_guarantees`: a caller's credentials, cwd, scratch,
   hermetic mode or decision mode never reach the `RunSpec`; the executor's own values stand.
3. A context path outside the workspace's copied files is refused before the copy is made, naming
   the path.
4. `SessionExecutor::from_spec` applies the spec's optional overrides, and a spec without them
   builds the executor it builds today.
5. `task check` passes; CHANGELOG names the new public members under `### Added`.

## Scope

Inferred 2026-10-28 from `origin/main` 78a6d278a, not yet scoped by a story-scoper.

- `crates/loom-session-executor/src/lib.rs` (`SessionExecutor`, `fn spec`, `from_spec`) — cited
- `ess/domains/agent.yaml` (`loom.agent.SessionSpec`), `generated/rust/loom/` — inferred
- `crates/loom-session-executor/tests/`, `crates/loom-sdk` re-exports — inferred
- AGENTS.md § Session executor, CHANGELOG.md, `website/data/status.json` — inferred

## Out of scope

The run budget and stop policy the spec already carries (`loom.agent.RunBudget`, applied by a host
through Commission) and the egress apertures; a takt change.
