---
format: aep.planning-md/3
id: review-result:loom-on-llm-and-metaharness-parallel-r2
kind: review-result
status: active
title: 'Parallel-safety critic, round 2: epic loom-on-llm-and-metaharness and its drafts'
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

- story:chooser-executor-for-plugin-turns — moving `crates/loom-intake-slice/src/selector.rs` leaves `crate::selector::Briefing` imported by `bindings.rs:43`, `clock.rs:21`, `effect.rs:42`, `handoff.rs:48` and `intent.rs:44` (cited, `git grep -n 'selector::' crates`). The body does not say whether `Briefing` moves. If it does, those files change, and four of them hold the `EffectPort` impls `story:pending-effect` edits, with no edge between the two and neither body naming the other. The scope also omits `crates/loom-executor/Cargo.toml`, which has no `llm-core` dependency today, so it needs one for the moved selector, and the `Cargo.lock` lines that change with it. Remedy A is an ordering edge from this story to `story:pending-effect` that records the files. Remedy B is to leave `Briefing` in the slice, so the five importers stay untouched. — .engineering/planning/story/chooser-executor-for-plugin-turns.md:96-97, crates/loom-executor/Cargo.toml:10-11
- story:pending-effect — its scope names `crates/loom-intake-slice/src`, `crates/loom-plugin/src` and `crates/loom-connectors/src` as whole directories (inferred). That makes it collide with `story:chooser-executor-for-plugin-turns` (`turn.rs`, `lib.rs`, `run.rs`, `selector.rs`) and with `story:sdk-confines-any-child`, `story:harness-child-egress-profile` and `story:python-test-profile` (`confinement.rs`), none of which it is ordered against or names. `git grep -nE 'impl.* EffectPort for' crates` names the files it needs: `crates/loom-connectors/src/lib.rs`, `crates/loom-intake-slice/src/{bindings,clock,effect,handoff}.rs` and `crates/loom-plugin/src/effects.rs`. The scope should list those instead of the directories. — .engineering/planning/story/pending-effect.md:20-25, .engineering/planning/story/pending-effect.md:77-78
- story:agent-executor-host-port — it edits `crates/loom-sdk/tests/host_ports.rs` beside `story:governor-exposes-canon-decision` and `website/docs/reference/sdk-stability.md` beside `story:engineering-protocols-090` and `story:sdk-confines-any-child` (cited, their scopes). It names only `story:delete-harness-port` and `story:metaharness-session-executor`, and has no edge to the three. It is the later story, so it carries the edge or the admission. Remedy A is an ordering edge naming the files. Remedy B is to split the page rows and test regions. — .engineering/planning/story/agent-executor-host-port.md:62-63, .engineering/planning/story/governor-exposes-canon-decision.md:79
- story:delete-harness-port — it edits `crates/loom-sdk/tests/host_ports.rs` (the governor story adds a call there), `crates/loom-sdk/src/lib.rs` and the stability page (`story:sdk-confines-any-child` adds a re-export and a row to each), and the page's pin line (`story:engineering-protocols-090`). Its edges and its "Depends on" paragraph name none of the three stories, and its closing paragraph names only `story:metaharness-session-executor`. `story:sdk-confines-any-child` may also declare a type in `ess/domains/run.yaml`, which this story edits. Remedy A is edges to the three stories that record the files. Remedy B is to split the surface. — .engineering/planning/story/delete-harness-port.md:99-104, .engineering/planning/story/delete-harness-port.md:125-126, .engineering/planning/story/sdk-confines-any-child.md:57-59, .engineering/planning/story/sdk-confines-any-child.md:72-73
- story:metaharness-session-executor — it edits `host_ports.rs`, `crates/loom-sdk/src/lib.rs` and the stability page, and `Cargo.lock`. The governor story, `story:sdk-confines-any-child` and `story:engineering-protocols-090` edit those files too, and its only ordering is the edge to `story:delete-harness-port`. The `host_ports.rs` and `lib.rs` overlaps are named for `story:delete-harness-port` only, and none of the three is named. The tool lists the `Cargo.lock` and page overlap with `story:engineering-protocols-090` as a collision. Remedy A is edges recording the files. Remedy B is to split the surface. — .engineering/planning/story/metaharness-session-executor.md:82, .engineering/planning/story/metaharness-session-executor.md:93-97
- story:sdk-confines-any-child — its ESS paragraph says a Loom-side request type "is declared in `ess/domains/run.yaml` first", which regenerates `generated/rust/loom`. `story:executor-reports-usage` and `story:delete-harness-port` edit both, and neither is ordered against this story. The scope lists neither file. Add them as a conditional surface with an ordering edge, or commit to Substrate's wire types so the file is not touched. — .engineering/planning/story/sdk-confines-any-child.md:57-59
- story:harness-child-egress-profile — its ESS paragraph says an egress list that is a Loom type "is declared in `ess/domains/run.yaml` first", the same conditional surface as above. `story:executor-reports-usage` and `story:delete-harness-port` edit it and `generated/rust/loom`, and no edge orders them. The scope lists neither. — .engineering/planning/story/harness-child-egress-profile.md:45-48

What I read: 18 artifacts: the epic, the 17 draft stories and `upstream-blocker:takt-session-executor-crate`. I ran `aep plan artifact show epic:loom-on-llm-and-metaharness`, `aep plan artifact list --kind story --status draft`, `aep plan artifact waves --kind story --status draft` and `aep plan artifact relations`. I read every body in full, and `git grep` for `selector::`, `EffectPort for`, `AuthorityProvider for` and `harness::`. A script compared every pair of scopes, excluding `CHANGELOG.md` and `AGENTS.md`, against the transitive `depends_on` closure. Surfaces established: cited 17 stories, inferred-only 0, unplaced 0. Inferred parts remain in `story:pending-effect` (three directories), `story:chooser-executor-for-plugin-turns` (the moved module's name) and `story:metaharness-session-executor` (the new crate and the session domain). Acceptance reads traced to a producer in the set: 20, and a `depends_on` edge records all 20. The proposed first wave holds: `story:engineering-protocols-090` first, then `story:governor-exposes-canon-decision` (edge) and `story:authority-sees-the-request` once it has merged. Their shared stability page, `host_ports.rs` and `AGENTS.md` regions are named in both bodies. All findings above are warnings, and none lies inside that first wave.

What I could not establish:
- Whether `Briefing` moves with the selector. The body does not say, so the first finding is conditional.
- Whether the `host_ports.rs` and `crates/loom-sdk/src/lib.rs` edits of different stories land in distinct hunks, so they may merge mechanically. I compared files, not hunks.
- Out of my lane, so not in the verdict: whether "wave-one stories" in `story:sdk-confines-any-child` (line 78) names a defined set. The epic's Order has no such label. That is for the design and scope critics.

```findings
- file: .engineering/planning/story/chooser-executor-for-plugin-turns.md
  line: 96
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "moving crates/loom-intake-slice/src/selector.rs leaves crate::selector::Briefing imported by bindings.rs:43, clock.rs:21, effect.rs:42, handoff.rs:48 and intent.rs:44 (cited, git grep), and the body does not say whether Briefing moves; if it does, those files change, four of them hold the EffectPort impls story:pending-effect edits, with no edge and no mention in either body; the scope also omits crates/loom-executor/Cargo.toml (no llm-core dependency today) and the Cargo.lock lines that change with it; remedies are an ordering edge to story:pending-effect recording the files, or leaving Briefing in the slice"
- file: .engineering/planning/story/pending-effect.md
  line: 20
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its scope names crates/loom-intake-slice/src, crates/loom-plugin/src and crates/loom-connectors/src as whole directories (inferred), so it collides with story:chooser-executor-for-plugin-turns and with story:sdk-confines-any-child, story:harness-child-egress-profile and story:python-test-profile (confinement.rs), none ordered against it or named; git grep for impl EffectPort for names crates/loom-connectors/src/lib.rs, crates/loom-intake-slice/src/{bindings,clock,effect,handoff}.rs and crates/loom-plugin/src/effects.rs, which the scope should list instead; the finding rests on the inferred directory entries"
- file: .engineering/planning/story/agent-executor-host-port.md
  line: 62
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it edits crates/loom-sdk/tests/host_ports.rs beside story:governor-exposes-canon-decision and website/docs/reference/sdk-stability.md beside story:engineering-protocols-090 and story:sdk-confines-any-child (cited, their scopes), names only story:delete-harness-port and story:metaharness-session-executor, and has no edge to the three; as the later story it carries the edge or the admission; remedies are an ordering edge naming the files, or splitting the page rows and test regions"
- file: .engineering/planning/story/delete-harness-port.md
  line: 125
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it edits host_ports.rs (story:governor-exposes-canon-decision adds a call), crates/loom-sdk/src/lib.rs and the stability page (story:sdk-confines-any-child adds a re-export and a row) and the page's pin line (story:engineering-protocols-090), its edges and Depends on paragraph name none of the three and its closing paragraph names only story:metaharness-session-executor; story:sdk-confines-any-child may also declare a type in ess/domains/run.yaml, which this story edits; remedies are edges recording the files, or splitting the surface"
- file: .engineering/planning/story/metaharness-session-executor.md
  line: 82
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it edits host_ports.rs, crates/loom-sdk/src/lib.rs, the stability page and Cargo.lock, which story:governor-exposes-canon-decision, story:sdk-confines-any-child and story:engineering-protocols-090 also edit, and its only ordering is the edge to story:delete-harness-port, which it names for host_ports.rs and lib.rs only; none of the three is named; remedies are edges recording the files, or splitting the surface"
- file: .engineering/planning/story/sdk-confines-any-child.md
  line: 59
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its ESS paragraph declares a Loom-side request type in ess/domains/run.yaml first, which regenerates generated/rust/loom, and story:executor-reports-usage and story:delete-harness-port edit both with no edge to this story, while the scope lists neither; add the conditional surface with an ordering edge, or commit to Substrate's wire types"
- file: .engineering/planning/story/harness-child-egress-profile.md
  line: 47
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its ESS paragraph declares an egress list that is a Loom type in ess/domains/run.yaml first, the same conditional surface story:executor-reports-usage and story:delete-harness-port edit with generated/rust/loom, with no edge, and the scope lists neither file"
```
