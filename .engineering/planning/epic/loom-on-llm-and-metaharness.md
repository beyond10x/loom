---
format: aep.planning-md/3
id: epic:loom-on-llm-and-metaharness
kind: epic
status: active
title: Loom calls llm for its decisions and metaharness for agent work, and holds no port of harness
summary: 'Atlas ADRs 0096, 0098, 0099: delete the harness port, meter at llm Model, host the session executor, close what takt needs from loom-sdk.'
relations:
- serves: vision:governed-autonomy
- serves: vision:O3
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T12:21:08Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-21T12:21:08Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":1}}}
---
## Outcome

Loom is the governed runtime and nothing below it (Atlas ADRs 0096 and 0098, accepted 2026-10-20).
It makes its own short decisions (classification, routing, action selection, argument generation)
through llm's `Model` directly, and it reaches agent work (a session that edits code over many tool
calls) through metaharness, in a scratch copy of the workspace, with the session's change returning
as a proposal Commission rechecks and applies. The port of harness under
`crates/loom-executor/src/harness/` (26,826 lines, `find … | xargs wc -l`, 2026-10-20) is deleted,
and the README's sentence that Loom "speaks no provider wire" (`README.md:33-34`) becomes true.

takt becomes an application on loom-sdk (Atlas ADR 0099). What loom-sdk 0.26.0 lacks for that move
is a Loom story under this epic; takt does not work around it.

## Why

Provider wires and an agent loop live three times across the stack: in llm, in harness, and in
Loom's port of harness. ADR 0096 gives each layer one owner: llm the model call, harness and the
vendor harnesses the agent loop, metaharness the harness interface, Loom the governed runtime. ADR
0098 decides Loom's side: decisions to llm, agent work to metaharness, the port deleted.

The port has callers inside Loom and none outside it (read 2026-10-20): plugin turns run on its
governed loop (`crates/loom-plugin/src/turn.rs:48-50`), a workflow's judgement step is hard-wired
to it (`crates/loom-executor/src/workflow.rs:110`, `:1302`), the run meter counts at its
`ModelPort` (`crates/loom-executor/src/metering.rs:51-52`), the reasoning-model selector takes its
`ModelPort` (`crates/loom-executor/src/selection.rs:26`), and loom-sdk ships two ports over it
(`LlmModelPort`, `crates/loom-sdk/src/llm.rs:19`; `ClaudeCodePort`,
`crates/loom-executor/src/wire/claude_code.rs`). The SDK stability page lists eight of its traits
among the ports a host implements (`website/docs/reference/sdk-stability.md:52-79`). Deleting it is
therefore a breaking loom-sdk release, and those callers move first.

## Acceptance

- No file under `crates/` names `harness::`, and `crates/loom-executor/src/harness/` does not exist.
- `b10x-loom plugin run` and a workflow's judgement steps run without the port, their existing
  tests passing.
- The release that deletes the port says `Breaking:` and names every removed port, each name
  checked as `story:delete-harness-port` says.
- Every story below reaches `implemented` with its own acceptance met.

## Order

Stories that block takt first, then the deletion path in the order it must land. Where two stories
share a file, the later one carries a `depends_on` edge or says so in its Scope.

1. Blocks takt's first story (loom-sdk pinned with evaluation parity), on which every later takt
   story depends: `story:engineering-protocols-090`.
2. Blocks takt's governor story: `story:governor-exposes-canon-decision` (after 1),
   `story:authority-sees-the-request`.
3. Blocks takt's run-loop story, in this order because each changes Commission's run outcomes or
   idle rule in the same files: `story:refused-effect-ends-run`, `story:idle-bound-outcome`,
   `story:unperformable-names-actions`, `story:pending-effect`, `story:executor-reports-usage`.
4. Blocks takt's confined-harness-child story: `story:sdk-confines-any-child`, then
   `story:harness-child-egress-profile`.
5. Asked by takt, blocking none of its stories: `story:agent-executor-host-port`,
   `story:python-test-profile` (takt keeps its confinement off by default until it lands).
6. The deletion path: `story:chooser-executor-for-plugin-turns`, then
   `story:metering-on-llm-model` (the meter and the reasoning-model selector together), then
   `story:judgement-step-any-executor`, then `story:delete-harness-port`.
7. `story:metaharness-session-executor`, once takt's standalone session-executor crate exists
   (`upstream-blocker:takt-session-executor-crate`).

## Decisions taken in this plan

- Plugin turns and judgement steps move to the chooser executor over llm's `Model`, not to a
  metaharness session (ADR 0098 point 4 leaves it to this plan). A plugin turn's reads are each
  admitted, revalidated and performed by Loom's `DataSourceEffects`; metaharness 0.9.3 lets a caller
  allow, deny or rewrite a tool call but not perform it (ADR 0098, Context), so through metaharness
  the unit of governance would be the whole session. The chooser path already runs the intake slice
  over llm (`crates/loom-intake-slice/src/selector.rs:3-4`), and each of its steps is one
  Commission iteration, revalidated.
- The port's compaction, session transcript, interruption recovery and wire credentials go with it:
  ADR 0096 puts them in the agent loop's layer, which Loom no longer holds.

## Not in scope

- Changes inside takt, metaharness, harness or llm. takt records, in its own plan, which Loom
  release unblocked each of its stories.
- Governing a session per tool call instead of per session: that waits on metaharness (ADR 0098,
  Consequences).
- swarm's move onto loom-sdk (ADR 0096, Open).
