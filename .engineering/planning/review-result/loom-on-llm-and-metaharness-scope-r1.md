---
format: aep.planning-md/3
id: review-result:loom-on-llm-and-metaharness-scope-r1
kind: review-result
status: active
title: 'Scope critic, round 1: epic loom-on-llm-and-metaharness and its drafts'
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

epic:loom-on-llm-and-metaharness — the acceptance "takt's stories that loom-sdk blocked … each name the Loom release that unblocked them" is claimed by no item. The epic also excludes "Changes inside takt", so the takt-side naming has no home in Loom's set. Either each unblocking story's acceptance should make its release or `CHANGELOG.md` entry name the takt story it unblocks (most naturally `story:governor-exposes-canon-decision`, `story:refused-effect-ends-run`, `story:sdk-confines-any-child` and their siblings), or the epic should say takt records this itself when the release lands. — .engineering/planning/epic/loom-on-llm-and-metaharness.md:48-49

**What I read.** 21 artifacts: the epic, 18 stories and `upstream-blocker:takt-session-executor-crate` through `aep plan artifact show` and the files in the worktree, plus `aep plan artifact graph` and `aep plan artifact list --kind story`. I also read ADRs 0096, 0098 and 0099 in `~/beyond10x/atlas/architecture/adr/`. I read takt's `takt-on-loom-sdk` epic and its Blocked-by sections (`~/beyond10x/takt/.engineering/planning/`) to check that all ten of takt's Loom-blocking gaps are claimed. I ran `git grep 'harness::'` over `crates/`.

**Extracted versus traced.** I extracted 16 promises from the epic and ADR 0098 and traced 15 to an item. The one untraced promise is the finding above. I found no reach beyond the parent and no two items claiming one outcome. Every story traces to the epic's Order list or Decisions section. The two nearest overlaps, `story:chooser-executor-for-plugin-turns` with `story:reasoning-selector-on-llm-model`, and `story:metering-on-llm-model` with `story:executor-reports-usage`, each name the other in "Not in scope" or conditionalise the removal.

**What I could not establish.**
- ADR 0098's gap list: takt's plan says it is "held outside this repository". I checked the blocking gaps in takt's three blocked stories and its first story instead. They map one to one onto 12 Loom stories. I cannot confirm the count ADR 0098 gives.
- Out of my lane, did not set the verdict: three test files outside any story's Scope list use `harness::`. They are `crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs`, `crates/loom-selector-laya/tests/laya_arguments_slice.rs` and `crates/loom-plugin/tests/adversary_2.rs`. Only the generic "no file under `crates/`" line in `story:delete-harness-port` reaches them. That belongs to the acceptance and parallel-safety critics.
- `upstream-blocker:takt-session-executor-crate` has a title-only body. That is an acceptance or design matter, not coverage.

```findings
- file: .engineering/planning/epic/loom-on-llm-and-metaharness.md
  line: 48
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance \"takt's stories that loom-sdk blocked ... each name the Loom release that unblocked them\" is claimed by no item and the epic excludes changes inside takt; either each unblocking story's acceptance makes its release or CHANGELOG entry name the takt story it unblocks, or the epic says takt records the naming itself"
```
