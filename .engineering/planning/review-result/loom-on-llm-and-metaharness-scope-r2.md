---
format: aep.planning-md/3
id: review-result:loom-on-llm-and-metaharness-scope-r2
kind: review-result
status: active
title: 'Scope critic, round 2: epic loom-on-llm-and-metaharness and its drafts'
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
needs-revision

story:judgement-step-any-executor — the epic promises that "a workflow's judgement steps run without the port, their existing tests passing", but this story does not claim two existing suites that use the port. `crates/loom-executor/tests/adversary_w11_u2.rs` (7 cases, the `WorkflowExecutor` governance rules) and `crates/loom-executor/tests/adversary_w11_u3.rs` (25 cases, typed step outputs) build `WorkflowExecutor::new(…, LoopConfig …)` over a `ModelPort` double. They are neither in the story's Scope nor in its Acceptance, and no other story's Scope names them. `story:delete-harness-port` then deletes them, because its acceptance sweeps "the others `git grep -l 'harness::' crates/loom-executor/tests` lists today" as port-only suites. They test workflow behaviour, not the port, so 32 cases are dropped with nothing recording the decision. The story should migrate them to the supplied or chooser executor, as it does for `workflow_executor.rs` and `workflow_step_outputs.rs`. Alternatively the delete story should name them as retained. — `.engineering/planning/epic/loom-on-llm-and-metaharness.md:46`, `.engineering/planning/story/judgement-step-any-executor.md:12-27`, `.engineering/planning/story/delete-harness-port.md:80`

**What I read.** The epic, ADRs 0096, 0098 and 0099, all 17 drafted stories, `upstream-blocker:takt-session-executor-crate` and the round-1 scope finding. I used `aep plan artifact show` and `aep plan artifact graph`, and checked `git grep` over `crates/` in the worktree. The set was read as it stands now, after the archive and merge of `story:reasoning-selector-on-llm-model` into `story:metering-on-llm-model`.

**Extracted versus traced.** I extracted 18 promises from the epic and ADR 0098, and traced 17 to an item. The untraced one is the finding above. It is the existing-tests clause of the judgement-step promise, which `story:judgement-step-any-executor` claims only in part.

**Round 1.** The round-1 finding is closed. The epic no longer lists "takt's stories … name the Loom release" as an acceptance line. Its Not in scope section now says takt records which Loom release unblocked each story in its own plan.

**Other checks.**
- Nothing reaches beyond the epic. Every story traces to the Order list or the Decisions section.
- No two stories claim one outcome. `story:chooser-executor-for-plugin-turns` moves the llm-backed selector and `story:metering-on-llm-model` removes `ReasoningModelSelector` only if the moved selector already holds its contract. That is stated in the story and is not a double claim.
- The three files that were unclaimed in round 1 are now claimed: `crates/loom-plugin/tests/adversary_2.rs` by the chooser story, `crates/loom-selector-laya/tests/laya_arguments_slice.rs` by the metering story, and `crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs` by the delete story.

**Out of my lane, did not set the verdict.**
- `website/docs/guides/embed-the-runtime.md:115-117` and `website/docs/guides/move-to-loom.md:80-84` still describe `MeteredModelPort`, `ModelPort` and the `harness::` paths. `ess/domains/metering.yaml:3` and `website/data/status.json:32` do the same. Only `website/data/status.json` is in the delete story's Scope. This is a docs-completeness matter for the acceptance and design critics.
- `story:metering-on-llm-model` changes the public `MeteredModelPort` and `RateCard` location but has no `Breaking:` acceptance line. That belongs to the acceptance critic.

```findings
- file: .engineering/planning/story/judgement-step-any-executor.md
  line: 12
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the epic promises judgement steps run without the port with their existing tests passing, but crates/loom-executor/tests/adversary_w11_u2.rs and adversary_w11_u3.rs (WorkflowExecutor governance and typed step outputs, 32 cases, built on LoopConfig and a ModelPort double) are claimed by no story's Scope, and story:delete-harness-port's sweep of every remaining harness:: suite would delete them; migrate them in this story or name them as retained"
```
