---
format: aep.planning-md/3
id: story:session-executor-partial-runspec
kind: story
status: implemented
title: The session executor merges a caller's partial RunSpec over its per-kind defaults
summary: effort, budget, strict version and context reach metaharness; takt's partial-runspec blocker is the consumer
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: charts/loom-agents/crds/loomagents.loom.beyond10x.dev.yaml
- confidence: cited
  path: crates/loom-sdk/src/agent.rs
- confidence: cited
  path: crates/loom-session-executor/src/lib.rs
- confidence: inferred
  path: crates/loom-session-executor/tests
- confidence: cited
  path: ess/domains/agent.yaml
- confidence: cited
  path: generated/rust/loom
revision: 15
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T19:13:18Z", actor: "human:timo", revision: 12}
- {from: "proposed", to: "active", at: "2026-10-28T19:13:18Z", actor: "human:timo", revision: 13}
- {from: "active", to: "implemented", at: "2026-10-28T20:49:47Z", actor: "human:timo", revision: 15, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
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

Derived 2026-10-28 by an `aep:story-scoper` pass for wave 2026-10-28-w1, read at `origin/main`
586b0ed83 and the metaharness 0.22.0 checkout (rev `fd8c9ab`, `Cargo.lock:3784`). Confidence: high
on the cited lines; medium on b10x-harness's handling of effort and budget, which rests on the
absence of a reference in `metaharness-b10x/src` and `metaharness/src` (only `builder.rs:199-202`
names effort).

- `crates/loom-session-executor/src/lib.rs`: `SessionExecutor` (:425), `from_spec` (:545-603, reads
  no `session.budget`, `stop` or `confinement`), `fn spec` (:791, also sets `scratch_cwd`,
  `substrate_embedded` and the b10x token fields) — cited
- `ess/domains/agent.yaml`: `loom.agent.SessionSpec` (:160-171); money is integer micro-USD there
  (`RunBudget.max_cost_micro_usd`, :114) — cited
- `generated/rust/loom/src/agent.rs` (`SessionSpec`, :475-486) and the CRD copy
  `charts/loom-agents/crds/loomagents.loom.beyond10x.dev.yaml`, regenerated only — cited
- metaharness 0.22.0 `RunSpec` (`metaharness-protocol/src/spec.rs`): `effort: Option<String>`
  (:365), `max_budget_usd: Option<String>` (:379), `strict_version: bool` (:463),
  `context: Vec<PathBuf>` (:590); Codex refuses a budget by name (`metaharness-codex/src/launch.rs:928-934`) — cited
- `crates/loom-session-executor/tests/`: the recorder `Script`/`Request` of `from_spec.rs:91-115`
  sees argv, env and config, not the `RunSpec` (`metaharness/src/process.rs:42-93`) — cited
- `crates/loom-sdk/src/agent.rs:40-46` re-exports `SessionSpec` — cited
- AGENTS.md § Session executor, CHANGELOG.md — inferred

Confirmed at the wave's close (the u2 implementor's table, 2026-10-28): every line above right; the
inferred tests, AGENTS.md and CHANGELOG lines right. Also touched:

- `crates/loom-session-executor/tests/partial_runspec.rs` (new), `from_spec.rs`, `adv_q13_metaharness_pin.rs`, `adversary_w20261023w1_u2.rs` — cited
- `crates/loom-sdk/tests/agent_spec.rs`, `deploy/crd/loomagents.loom.beyond10x.dev.yaml` — cited
- README.md (the crate row), `website/data/status.json` — cited
- b10x-harness has no reader for effort or budget (was medium confidence): confirmed; metaharness does not refuse them, so Loom does — cited

## Out of scope

The run budget and stop policy the spec already carries (`loom.agent.RunBudget`, applied by a host
through Commission) and the egress apertures; a takt change.

## Decisions (wave 2026-10-28-w1)

Taken by the wave coordinator inside the item the orchestrator approved
(approval-record:wave-2026-10-28-w1).

- The open question: the budget is integer micro-USD, `Optional<Integer>`, as `RunBudget` and
  `loom.metering` hold money (`ess/domains/agent.yaml:114`); the executor renders it as the decimal
  string metaharness takes (`12500000` is `12.5`). `Decimal` carries the replay caveat of
  `ess/domains/resource.yaml:12-16`.
- The executor takes the overrides as a Loom type of its own (effort, budget in micro-USD, strict
  version, context), never a whole `RunSpec`: an unset `bool` cannot be told from `false` there.
  Each member the caller names wins over the kind's default; nothing else of the `RunSpec` is read
  from the caller.
- A member the kind's launcher does not apply is refused before the copy, never dropped: a budget
  on Codex (metaharness refuses it, `metaharness-codex/src/launch.rs:928-934`) and on b10x-harness,
  and effort on b10x-harness (no reader in `metaharness-b10x/src`). From a spec this is a
  `SpecError`.
- A context path must be one of the workspace files the executor copies, as one plain relative
  path; it reaches the `RunSpec` as its path in the scratch copy. Any other path is refused before
  the copy, naming it.
