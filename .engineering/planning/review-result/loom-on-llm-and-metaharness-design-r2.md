---
format: aep.planning-md/3
id: review-result:loom-on-llm-and-metaharness-design-r2
kind: review-result
status: active
title: 'Design critic, round 2: epic loom-on-llm-and-metaharness and its drafts'
relations:
- reviews: epic:loom-on-llm-and-metaharness
- reviews: upstream-blocker:takt-session-executor-crate
- reviews: story:engineering-protocols-090
- reviews: story:governor-exposes-canon-decision
- reviews: story:authority-sees-the-request
- reviews: story:refused-effect-ends-run
- reviews: story:idle-bound-outcome
- reviews: story:pending-effect
- reviews: story:unperformable-names-actions
- reviews: story:executor-reports-usage
- reviews: story:sdk-confines-any-child
- reviews: story:harness-child-egress-profile
- reviews: story:agent-executor-host-port
- reviews: story:python-test-profile
- reviews: story:chooser-executor-for-plugin-turns
- reviews: story:judgement-step-any-executor
- reviews: story:metering-on-llm-model
- reviews: story:delete-harness-port
- reviews: story:metaharness-session-executor
revision: 1
---
approve

The set holds together as a set. No finding is raised.

**Round-1 findings (`review-result:loom-on-llm-and-metaharness-design-r1`)**

| Round-1 finding | State now | Source |
|---|---|---|
| The meter and the reasoning selector were one change cut in half | Merged. `story:reasoning-selector-on-llm-model` is archived, and `story:metering-on-llm-model` carries both. | `.engineering/planning/story/metering-on-llm-model.md:37-48`, `.engineering/planning/story/reasoning-selector-on-llm-model.md` (`status: archived`) |
| The selector story had no `depends_on` to the chooser story | Closed by the merge. The merged story depends on `story:chooser-executor-for-plugin-turns`. | `.engineering/planning/story/metering-on-llm-model.md:10` |
| `story:judgement-step-any-executor` had no edge to the metering story | Edge added. | `.engineering/planning/story/judgement-step-any-executor.md:11`, `:67` |

**What I read and walked**
- I read the epic, all 17 draft stories in full, the archived story's header and `upstream-blocker:takt-session-executor-crate`.
- I ran `aep plan artifact relations`, `graph` and `validate`. `validate` reports `valid`; its only warnings are old review-result ones.
- I walked 20 `depends_on` edges and 1 `blocks` edge, plus 17 `decomposes` and 17 `serves` edges, in the whole-store graph. That walk went outside the set. I found no cycle.
- I traced 14 acceptance reads of state a sibling produces. All 14 have an edge, directly or through one already recorded. Of the 14, 12 are direct edges and 2 run through a recorded chain (the judgement step's budget, `metaharness-session-executor` through `delete-harness-port`).
- The reads I traced:
  - the byte-equality tests in `story:governor-exposes-canon-decision` on the protocol set `story:engineering-protocols-090` moves
  - the `host_ports.rs` and stability-page lines that `story:agent-executor-host-port` and `story:delete-harness-port` read from `story:authority-sees-the-request`
  - the `metering.rs` and `run_metering.rs` lines that `story:metering-on-llm-model` reads from `story:executor-reports-usage`
  - the `ActionSelector` grep in `story:metering-on-llm-model`, which counts the selector `story:chooser-executor-for-plugin-turns` moves
  - the `harness::` greps and metering lines in `story:delete-harness-port` and `story:judgement-step-any-executor`, which read what the chooser, metering and judgement stories produce
  - the `diff -r` against takt's crate in `story:metaharness-session-executor`, which reads what `upstream-blocker:takt-session-executor-crate` clears with
- I also grepped `crates/` for `harness::`, `LlmModelPort` and `ClaudeCodePort`. Every file that names them is in the scope of some story that handles it.

**Trade-off, not a finding: the chain is long but every edge has its reason written**
- The longest chain has 10 stories: `story:authority-sees-the-request`, `story:refused-effect-ends-run`, `story:idle-bound-outcome`, `story:unperformable-names-actions`, `story:pending-effect`, `story:executor-reports-usage`, `story:metering-on-llm-model`, `story:judgement-step-any-executor`, `story:delete-harness-port`, `story:metaharness-session-executor`.
- Each edge records its reason in the story:
  - the five Commission outcome stories share `crates/loom-commission/src/runtime.rs`, `ess/commission/domains/responsibility.yaml` and `generated/rust/commission`
  - the usage and metering stories share `crates/loom-executor/src/metering.rs` (`.engineering/planning/story/executor-reports-usage.md:74-75`)
  - the deletion story shares `host_ports.rs` and the stability page
- The deletion path, which blocks nothing for takt, therefore waits behind the five outcome stories through `story:executor-reports-usage`.
- If the drafter wants that path shorter, the two options are to merge the five outcome stories or to split `metering.rs` so the usage change and the meter move stop colliding. I do not ask for either.

**Out of my lane**
- `story:metaharness-session-executor` (acceptance): it requires a new `loom.session` domain in Loom's `ess/` and a `diff -r` of the crate's `src/` against takt's with no change but package name and path dependencies. A generated type used only after the move would break that diff. Acceptance critic.
- Parallel safety: these pairs share files with no ordering edge.
  - `story:harness-child-egress-profile` and `story:python-test-profile` share `crates/loom-intake-slice/src/confinement.rs`.
  - `story:governor-exposes-canon-decision` and `story:authority-sees-the-request` share `crates/loom-sdk/tests/host_ports.rs`.
  - `story:engineering-protocols-090`, `story:authority-sees-the-request` and `story:sdk-confines-any-child` share `website/docs/reference/sdk-stability.md`, in different rows.
  - Several stories share `AGENTS.md` and `CHANGELOG.md`.

```findings
[]
```
