---
format: aep.planning-md/3
id: review-result:loom-on-llm-and-metaharness-design-r1
kind: review-result
status: active
title: 'Design critic, round 1: epic loom-on-llm-and-metaharness and its drafts'
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
- reviews: story:reasoning-selector-on-llm-model
- reviews: story:delete-harness-port
- reviews: story:metaharness-session-executor
revision: 1
---
needs-revision

story:metering-on-llm-model — its acceptance runs `ReasoningModelSelector` over the meter, and `story:reasoning-selector-on-llm-model` re-points that selector, so the two are one change cut in half: merge them, or put the `run_metering.rs` selector construction in one scope and add a `depends_on` from the other — .engineering/planning/story/metering-on-llm-model.md:35, crates/loom-executor/tests/run_metering.rs:391-393, .engineering/planning/story/reasoning-selector-on-llm-model.md:23

story:reasoning-selector-on-llm-model — its outcome is conditional on the selector `story:chooser-executor-for-plugin-turns` moves ("one of the two is removed"), and no `depends_on` edge runs to that story — .engineering/planning/story/reasoning-selector-on-llm-model.md:25-27

story:judgement-step-any-executor — its scope names `workflow_run_store.rs`, which meters the workflow through `MeteredModelPort` (`workflow_run_store.rs:437`), and the chooser's llm calls can only be metered by the wrapper `story:metering-on-llm-model` produces, with no `depends_on` edge to it — .engineering/planning/story/judgement-step-any-executor.md:74

**What I read.** 19 artifacts, full bodies (18 stories and the upstream-blocker), plus the epic. I ran `aep plan artifact show epic:loom-on-llm-and-metaharness`, `list --kind story --status draft`, `relations` and `graph`. I ran `validate` once: it reports `valid`, with only pre-existing review-result warnings. I walked 7 declared needs-first edges inside the set (6 `depends_on`, 1 `blocks`), plus the `decomposes` and `serves` edges, in the full graph, so I also walked outside the set. I found no cycle and no serialising chain. I traced about 10 acceptance reads to a producer in the set, and 7 of them have an edge. The 3 without one are the findings above. I also read `crates/loom-executor/tests/run_metering.rs`, `workflow_run_store.rs`, `src/workflow.rs` and `crates/loom-commission/src/runtime.rs:133-144`.

**What I could not establish.**
- Out of my lane (scope/acceptance): `story:delete-harness-port` and `story:chooser-executor-for-plugin-turns` accept "no file names `harness::`". `crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs`, `crates/loom-selector-laya/tests/laya_arguments_slice.rs` and `crates/loom-plugin/tests/adversary_2.rs` name it today and are in no story's scope.
- Out of my lane (parallel safety): these files are shared by stories with no ordering edge between them:
  - `runtime.rs`, `outcome.rs` and `responsibility.yaml` (refused-effect, idle-bound, pending-effect, unperformable, executor-reports-usage);
  - `metering.rs` and `run_metering.rs` (executor-reports-usage, metering-on-llm-model);
  - `confinement.rs` (python-test-profile, sdk-confines-any-child, harness-child-egress-profile; the last two are ordered);
  - `host_ports.rs` and `sdk-stability.md` (authority-sees-the-request, agent-executor-host-port, delete-harness-port, metaharness-session-executor).
- takt's session-executor crate does not exist, so I could not check whether `story:metaharness-session-executor` needs `story:executor-reports-usage` or the authority change to build "unchanged".

```findings
- file: .engineering/planning/story/metering-on-llm-model.md
  line: 35
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance runs ReasoningModelSelector over the meter, and story:reasoning-selector-on-llm-model re-points that selector, so the two are one change cut in half: merge them, or put the run_metering.rs selector construction in one scope and add a depends_on from the other"
- file: .engineering/planning/story/reasoning-selector-on-llm-model.md
  line: 25
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its outcome is conditional on the selector story:chooser-executor-for-plugin-turns moves (one of the two is removed), and no depends_on edge runs to that story"
- file: .engineering/planning/story/judgement-step-any-executor.md
  line: 74
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its scope names workflow_run_store.rs, which meters the workflow through MeteredModelPort (workflow_run_store.rs:437), and the chooser's llm calls can only be metered by the wrapper story:metering-on-llm-model produces, with no depends_on edge to it"
```
