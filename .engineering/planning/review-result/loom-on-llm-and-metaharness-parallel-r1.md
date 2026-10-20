---
format: aep.planning-md/3
id: review-result:loom-on-llm-and-metaharness-parallel-r1
kind: review-result
status: active
title: 'Parallel-safety critic, round 1: epic loom-on-llm-and-metaharness and its drafts'
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

The set cannot be worked at once. Each pair below shares a file or a generated output, and neither body admits it. I excluded the `CHANGELOG.md` and `AGENTS.md` append collisions as instructed. I kept the `AGENTS.md` collision where two stories edit the same section.

- story:engineering-protocols-090 — its scope lists four manifests and "site pages naming the tag (inferred)", but the `0.8.0` pin also sits in `crates/loom-governor/src/lib.rs:127`, `crates/loom-protocols/src/lib.rs:153`, `website/docs/reference/sdk-stability.md:48` and `AGENTS.md:324` (§ Governor). `story:governor-exposes-canon-decision` edits the first and last of these, and it and `story:authority-sees-the-request` both edit the stability page. This collides inside the proposed first wave. The hits are found by `git grep 0.8.0` (cited); the story's own list is inferred. Remedy A is an ordering edge that records the shared files. Remedy B is to split the surface — .engineering/planning/story/engineering-protocols-090.md:66, .engineering/planning/story/governor-exposes-canon-decision.md:72-76
- story:governor-exposes-canon-decision — its acceptance runs byte-equality tests on `software-change@1`, which `story:engineering-protocols-090` says it may change ("any protocol behaviour 0.9.0 changes"). No `depends_on` edge runs from the reader to that producer; the finding names the missing edge, or merging the two — .engineering/planning/story/governor-exposes-canon-decision.md:43-47, .engineering/planning/story/engineering-protocols-090.md:54-55
- story:authority-sees-the-request — it and `story:governor-exposes-canon-decision` both edit `website/docs/reference/sdk-stability.md`, and the first wave holds both. Neither body says so (cited, both scopes). Remedy A is an ordering edge naming the page. Remedy B is to split the page rows — .engineering/planning/story/authority-sees-the-request.md:97, .engineering/planning/story/governor-exposes-canon-decision.md:74
- story:authority-sees-the-request — it changes the two `check_authority` calls in `crates/loom-commission/src/runtime.rs` and `crates/loom-intake-slice/src/run.rs`. `story:refused-effect-ends-run`, `story:idle-bound-outcome`, `story:unperformable-names-actions`, `story:pending-effect` and `story:executor-reports-usage` edit the same files, and the body names none of them (cited) — .engineering/planning/story/authority-sees-the-request.md:91-95, .engineering/planning/story/refused-effect-ends-run.md:73-75
- story:refused-effect-ends-run — it shares `ess/commission/domains/responsibility.yaml`, `generated/rust/commission`, `runtime.rs`, `outcome.rs`, `run.rs`, `crates/loom-cli/src/events.rs`, `docs/contracts/run-events-v1.json` and `website/docs/reference/commission` with `story:idle-bound-outcome` and `story:unperformable-names-actions`. It also redefines what counts as idle, as `story:pending-effect` and `story:idle-bound-outcome` do. It names only `story:pending-effect`, as "not in scope", and none of the shared files. Remedy A is `depends_on` edges that record the shared files. Remedy B is to split the surface — .engineering/planning/story/refused-effect-ends-run.md:68-77, .engineering/planning/story/idle-bound-outcome.md:64-73
- story:idle-bound-outcome — it adds a `RunOutcome` variant in `responsibility.yaml` and regenerates `generated/rust/commission`. Those are the same declarations and the same generated output that `story:refused-effect-ends-run`, `story:unperformable-names-actions` and `story:pending-effect` change, so regeneration conflicts on merge. The body names none of them — .engineering/planning/story/idle-bound-outcome.md:57-73, .engineering/planning/story/unperformable-names-actions.md:67-72
- story:pending-effect — it edits `runtime.rs`, `responsibility.yaml`, `generated/rust/commission` and `website/docs/reference/commission`, as do `story:refused-effect-ends-run`, `story:idle-bound-outcome` and `story:unperformable-names-actions`. Its idle-counting rule is the one the other two redefine, and the body names none of them but `story:refused-effect-ends-run` as "not in scope" — .engineering/planning/story/pending-effect.md:64-69, .engineering/planning/story/refused-effect-ends-run.md:66
- story:unperformable-names-actions — it edits `runtime.rs`, `outcome.rs`, `run.rs`, `events.rs`, `run-events-v1.json` and `responsibility.yaml`, all also edited by `story:refused-effect-ends-run` and `story:idle-bound-outcome`. The body names neither (cited) — .engineering/planning/story/unperformable-names-actions.md:67-72, .engineering/planning/story/idle-bound-outcome.md:64-73
- story:executor-reports-usage — it rewrites `crates/loom-executor/src/metering.rs` and `crates/loom-executor/tests/run_metering.rs`, which `story:metering-on-llm-model` also rewrites. It also edits `runtime.rs`, `responsibility.yaml` and `generated/rust/commission` with the five commission stories. It names `story:metering-on-llm-model` only as "not in scope", with no edge and no order — .engineering/planning/story/executor-reports-usage.md:74-77, .engineering/planning/story/metering-on-llm-model.md:56
- story:chooser-executor-for-plugin-turns — moving `crates/loom-intake-slice/src/selector.rs` forces edits to `crates/loom-intake-slice/src/run.rs:156` (it imports `crate::selector::…`) and `crates/loom-intake-slice/src/lib.rs:44`. The scope lists neither, and `run.rs` is also edited by `story:authority-sees-the-request`, `story:refused-effect-ends-run`, `story:idle-bound-outcome` and `story:unperformable-names-actions` (cited by `git grep selector::` in the tree) — .engineering/planning/story/chooser-executor-for-plugin-turns.md:79
- story:reasoning-selector-on-llm-model — its outcome removes one of two selectors depending on what `story:chooser-executor-for-plugin-turns` moves into `crates/loom-executor/src/` (new home inferred). It reads state that story produces, and no `depends_on` edge records it. The finding names the missing edge, or merging the two — .engineering/planning/story/reasoning-selector-on-llm-model.md:23-27, .engineering/planning/story/chooser-executor-for-plugin-turns.md:79
- story:python-test-profile — it edits `crates/loom-intake-slice/src/confinement.rs` (`:555-565`, `:26`), which `story:sdk-confines-any-child` (`:33-36`) and `story:harness-child-egress-profile` (`:513`) also edit. It has no edge to either and names neither (cited, three scopes). Remedy A is an ordering edge that records the file. Remedy B is to split it — .engineering/planning/story/python-test-profile.md:58, .engineering/planning/story/sdk-confines-any-child.md:68
- story:sdk-confines-any-child — it edits `crates/loom-sdk/src/lib.rs` and `website/docs/reference/sdk-stability.md` beside `story:governor-exposes-canon-decision`. `crates/loom-sdk/src/lib.rs` is also edited by `story:chooser-executor-for-plugin-turns`. The body names none of them (cited) — .engineering/planning/story/sdk-confines-any-child.md:70, .engineering/planning/story/governor-exposes-canon-decision.md:74
- story:agent-executor-host-port — it edits `crates/loom-sdk/tests/host_ports.rs` and the stability page, which `story:authority-sees-the-request` also edits. `story:delete-harness-port` and `story:metaharness-session-executor` edit them too. The test `the_stability_page_lists_exactly_the_ports_this_test_implements` ties page rows to test impls, so these stories must land in one order. The body names none of them — .engineering/planning/story/agent-executor-host-port.md:50, .engineering/planning/story/authority-sees-the-request.md:96-97
- story:delete-harness-port — it deletes `crates/loom-sdk/tests/sdk_model_ports.rs` and `host_ports.rs` rows that `story:authority-sees-the-request` edits. It regenerates `generated/rust/loom` and removes `metering.rs` surface that `story:executor-reports-usage` edits. It shares `README.md` and `crates/loom-sdk` with `story:metaharness-session-executor`. Its edges cover only four stories, so none of these pairs is ordered — .engineering/planning/story/delete-harness-port.md:92-99, .engineering/planning/story/authority-sees-the-request.md:96
- story:metaharness-session-executor — it names `crates/loom-sdk` and `ess/` as whole directories and `README.md`, so it collides with every story that touches either. That includes `story:delete-harness-port`, which names `crates/loom-sdk` and `README.md`. Narrow it to the files it touches — .engineering/planning/story/metaharness-session-executor.md:74-78

What I read: 19 artifacts (the epic, 18 stories, 1 upstream-blocker). I ran `aep plan artifact waves --kind story --status draft`, `aep plan artifact list` and `aep plan artifact relations`, then read each body in full. I checked the surfaces with `git grep` for `0.8.0`, `AuthorityProvider`, `selector::` and `harness::` in the worktree. Surfaces established: cited 18, inferred 0 as a whole story, unplaced 0. Parts of four stories rest on inferred paths: `story:engineering-protocols-090` (README and site pages), `story:pending-effect` (the `EffectPort` impls), `story:chooser-executor-for-plugin-turns` (the new home) and `story:metaharness-session-executor` (the crate name). Acceptance reads traced to a producer in the set: 9. Six of those are recorded by an edge (`story:judgement-step-any-executor` to `story:chooser-executor-for-plugin-turns`; `story:harness-child-egress-profile` to `story:sdk-confines-any-child`; `story:delete-harness-port` to its four prerequisites). Three are not: `story:governor-exposes-canon-decision` to `story:engineering-protocols-090`, `story:reasoning-selector-on-llm-model` to `story:chooser-executor-for-plugin-turns`, and `story:executor-reports-usage` to `story:metering-on-llm-model`.

What I could not establish:
- Whether `crates/loom-sdk/src/lib.rs` edits by different stories land in distinct regions. I only compared files. They may merge mechanically.
- Whether the 0.9.0 tag changes `software-change@1`; the story itself says "I don't know".
- Out of my lane, so not in the verdict: `upstream-blocker:takt-session-executor-crate` has a heading and no body, and the epic's acceptance line on takt's stories naming a Loom release maps to no story. Both are for the scope and design critics.

```findings
- file: .engineering/planning/story/engineering-protocols-090.md
  line: 66
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its scope lists four manifests and 'site pages naming the tag (inferred)', but the 0.8.0 pin also sits in crates/loom-governor/src/lib.rs:127, crates/loom-protocols/src/lib.rs:153, website/docs/reference/sdk-stability.md:48 and AGENTS.md:324 (§ Governor), which story:governor-exposes-canon-decision and story:authority-sees-the-request also edit, and no body says so; the hits are found by git grep (cited), the story's own list is inferred; remedies are an ordering edge recording the shared files, or splitting the surface"
- file: .engineering/planning/story/governor-exposes-canon-decision.md
  line: 43
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance runs byte-equality tests on software-change@1, which story:engineering-protocols-090 says it may change, and no depends_on edge runs from this reader to that producer; the finding names the missing edge, or merging the two"
- file: .engineering/planning/story/authority-sees-the-request.md
  line: 97
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it and story:governor-exposes-canon-decision both edit website/docs/reference/sdk-stability.md and the proposed first wave holds both, and neither body says so (cited); remedies are an ordering edge naming the page, or splitting the page rows"
- file: .engineering/planning/story/authority-sees-the-request.md
  line: 91
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it changes the two check_authority calls in crates/loom-commission/src/runtime.rs and crates/loom-intake-slice/src/run.rs, which story:refused-effect-ends-run, story:idle-bound-outcome, story:unperformable-names-actions, story:pending-effect and story:executor-reports-usage also edit, and the body names none of them (cited); remedies are ordering edges recording the files, or splitting the surface"
- file: .engineering/planning/story/refused-effect-ends-run.md
  line: 72
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it shares responsibility.yaml, generated/rust/commission, runtime.rs, outcome.rs, run.rs, events.rs, run-events-v1.json and website/docs/reference/commission with story:idle-bound-outcome and story:unperformable-names-actions and redefines idle counting as story:pending-effect and story:idle-bound-outcome do, and names only story:pending-effect, as not in scope; remedies are depends_on edges recording the files, or splitting the surface"
- file: .engineering/planning/story/idle-bound-outcome.md
  line: 57
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it adds a RunOutcome variant in responsibility.yaml and regenerates generated/rust/commission, the same declarations and generated output that story:refused-effect-ends-run, story:unperformable-names-actions and story:pending-effect change, so regeneration conflicts on merge, and the body names none of them"
- file: .engineering/planning/story/pending-effect.md
  line: 64
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it edits runtime.rs, responsibility.yaml, generated/rust/commission and website/docs/reference/commission as do story:refused-effect-ends-run, story:idle-bound-outcome and story:unperformable-names-actions, its idle-counting rule is the one the other two redefine, and it names only story:refused-effect-ends-run, as not in scope"
- file: .engineering/planning/story/unperformable-names-actions.md
  line: 67
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it edits runtime.rs, outcome.rs, run.rs, events.rs, run-events-v1.json and responsibility.yaml, all also edited by story:refused-effect-ends-run and story:idle-bound-outcome, and the body names neither (cited)"
- file: .engineering/planning/story/executor-reports-usage.md
  line: 74
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it rewrites crates/loom-executor/src/metering.rs and crates/loom-executor/tests/run_metering.rs which story:metering-on-llm-model also rewrites, and edits runtime.rs, responsibility.yaml and generated/rust/commission with the five commission stories, naming story:metering-on-llm-model only as not in scope, with no edge and no order"
- file: .engineering/planning/story/chooser-executor-for-plugin-turns.md
  line: 79
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "moving crates/loom-intake-slice/src/selector.rs forces edits to crates/loom-intake-slice/src/run.rs:156 and crates/loom-intake-slice/src/lib.rs:44, which the scope omits, and run.rs is also edited by story:authority-sees-the-request, story:refused-effect-ends-run, story:idle-bound-outcome and story:unperformable-names-actions (found by git grep in the tree)"
- file: .engineering/planning/story/reasoning-selector-on-llm-model.md
  line: 23
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its outcome removes one of two selectors depending on what story:chooser-executor-for-plugin-turns moves into crates/loom-executor/src (new home inferred), so it reads state that story produces and no depends_on edge records it; the finding names the missing edge, or merging the two"
- file: .engineering/planning/story/python-test-profile.md
  line: 58
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it edits crates/loom-intake-slice/src/confinement.rs (:555-565, :26) which story:sdk-confines-any-child and story:harness-child-egress-profile also edit, with no edge to either and neither named; remedies are an ordering edge recording the file, or splitting it"
- file: .engineering/planning/story/sdk-confines-any-child.md
  line: 70
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it edits crates/loom-sdk/src/lib.rs and website/docs/reference/sdk-stability.md beside story:governor-exposes-canon-decision, and crates/loom-sdk/src/lib.rs is also edited by story:chooser-executor-for-plugin-turns, and the body names none of them (cited)"
- file: .engineering/planning/story/agent-executor-host-port.md
  line: 50
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it edits crates/loom-sdk/tests/host_ports.rs and the stability page which story:authority-sees-the-request, story:delete-harness-port and story:metaharness-session-executor also edit, and the test the_stability_page_lists_exactly_the_ports_this_test_implements ties page rows to test impls so they must land in one order, and the body names none of them"
- file: .engineering/planning/story/delete-harness-port.md
  line: 92
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it deletes crates/loom-sdk/tests/sdk_model_ports.rs and host_ports.rs rows that story:authority-sees-the-request edits, regenerates generated/rust/loom and removes metering.rs surface that story:executor-reports-usage edits, and shares README.md and crates/loom-sdk with story:metaharness-session-executor, while its edges cover only four stories so none of these pairs is ordered"
- file: .engineering/planning/story/metaharness-session-executor.md
  line: 74
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it names crates/loom-sdk and ess/ as whole directories and README.md, so it collides with every story touching either, including story:delete-harness-port; narrow it to the files it touches"
```
