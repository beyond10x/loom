---
format: aep.planning-md/3
id: review-result:loom-native-harness-acceptance-r1
kind: review-result
status: active
title: loom-native-harness decomposition — acceptance critic, round 1
relations:
- reviews: epic:loom-native-harness
- reviews: story:action-selector
- reviews: story:agent-executor
- reviews: story:argument-generator
- reviews: story:compaction-contract
- reviews: story:frontier-projection
- reviews: story:harness-module-map
- reviews: story:interruption-recovery
- reviews: story:loom-ess-conformance
- reviews: story:selection-revalidation
- reviews: story:session-transcript-streaming
revision: 1
---
needs-revision

story:compaction-contract — the acceptance reads the same whether or not compaction carries a stale catalogue forward, because it never says the frontier moved between the old catalogue and the first post-compaction request; it also leaves "priced and recorded" unobserved — .engineering/planning/story/compaction-contract.md:40
story:session-transcript-streaming — the acceptance joins three independent outcomes (streaming deltas, 0700 filing with no credential or instruction text, resume replaying item for item) in one sentence, so one can pass while the others fail — .engineering/planning/story/session-transcript-streaming.md:49
story:session-transcript-streaming — the cross-wire refusal that ESS first adds as a command, the session written when the run dies, and reasoning not being stored have no observable acceptance — .engineering/planning/story/session-transcript-streaming.md:21
story:agent-executor — the acceptance observes neither the closed `commission_run` marker, nor the `task check` regenerate-and-diff gate, nor the replaced bootstrap types, and "using only model types from the committed synthesized crate" names no check; `story:loom-ess-conformance` assigns the drift gate to this story, so no acceptance owns it — .engineering/planning/story/agent-executor.md:56
story:loom-ess-conformance — the scope says running the suite is not in this story and cites `decision-blocker:rust-conformance-target` as holding it, but that blocker is cleared with "Loom's I-006 does the same" (a Rust target on `ess-conformance`), so the epic's "`task check` runs the Loom ESS conformance suite" clause has no acceptance anywhere — .engineering/planning/story/loom-ess-conformance.md:37
story:loom-ess-conformance — the acceptance joins the drift-gate failure and the scenario-per-command count with "and", and omits the Outcome's "every UNMAPPED marker left in `ess/` is held by an open decision-blocker" — .engineering/planning/story/loom-ess-conformance.md:43
story:argument-generator — the acceptance joins two independent outcomes with "and" (the generator is handed exactly the selected action, and its JSON output reaches `ProposedAction` through a synthesized `ArgumentRequest`) — .engineering/planning/story/argument-generator.md:42
story:selection-revalidation — the Outcome and ESS first name three refusals (not in frontier, stale revision, authority denied), but the acceptance observes only two, so the "asks for authority and obeys the answer" clause can be done or undone without the check noticing — .engineering/planning/story/selection-revalidation.md:45
story:interruption-recovery — the Outcome promises that a run suspended at an approval resumes from its checkpoint before the exact effect, but the acceptance covers only cancel after selection and resume after the case moved — .engineering/planning/story/interruption-recovery.md:38
story:harness-module-map — the acceptance joins two independent outcomes with "and": one row per crate with one disposition, and every § Owns responsibility named by a row or listed as new — .engineering/planning/story/harness-module-map.md:50

Read: 10 of 10 ids that decompose `epic:loom-native-harness` (agent-executor, frontier-projection, harness-module-map, action-selector, argument-generator, selection-revalidation, session-transcript-streaming, compaction-contract, interruption-recovery, loom-ess-conformance), via `aep plan artifact list`, `show`, `kinds` and `lifecycle story`, plus the epic, `decision-blocker:rust-conformance-target`, `ess/domains/run.yaml` and `crates/loom/src/lib.rs`. I also checked `beyond10x/harness` at `798325f0` (14 crates under `crates/`; the compaction constants, `environment.rs:47` and `transcript.rs:184` and `:219` match their citations). `story:frontier-projection` and `story:action-selector` have no finding. The other stories in the store belong to `epic:fast-selector` and `epic:effect-bindings` and were not judged.

Not established:
- Whether the ESS-first commands will pass `ess specify validate`. I did not run `ess`.
- Whether Commission M-004, M-005, M-008 and M-009 deliver what these acceptances assume. I did not read the Commission stories beyond their titles.

Out of my lane:
- `story:frontier-projection` cites Canon's bootstrap `Frontier.actions: Vec<ActionCandidate>` at canon `cf29c4b` (frontier-projection.md:47-49). Canon's wave 1 removes `Frontier`, `ActionId`, `ActionStatus` and `ActionCandidate` and Commission will define `Frontier`, so that citation goes stale.
- `story:agent-executor`, `story:action-selector` and `story:argument-generator` rest on those same types in `crates/loom/src/lib.rs`. This belongs to `plan-critic-design` and `plan-critic-scope`.

```findings
[
  {"file": ".engineering/planning/story/compaction-contract.md", "line": 40, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance reads the same whether or not compaction carries a stale catalogue forward, because it never says the frontier moved between the old catalogue and the first post-compaction request; it also leaves 'priced and recorded' unobserved"},
  {"file": ".engineering/planning/story/session-transcript-streaming.md", "line": 49, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins three independent outcomes (streaming deltas, 0700 filing with no credential or instruction text, resume replaying item for item) in one sentence, so one can pass while the others fail"},
  {"file": ".engineering/planning/story/session-transcript-streaming.md", "line": 21, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the cross-wire refusal that ESS first adds as a command, the session written when the run dies, and reasoning not being stored have no observable acceptance"},
  {"file": ".engineering/planning/story/agent-executor.md", "line": 56, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance observes neither the closed commission_run marker, nor the task check regenerate-and-diff gate, nor the replaced bootstrap types, and 'using only model types from the committed synthesized crate' names no check; story:loom-ess-conformance assigns the drift gate to this story, so no acceptance owns it"},
  {"file": ".engineering/planning/story/loom-ess-conformance.md", "line": 37, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the scope says running the suite is not in this story and cites decision-blocker:rust-conformance-target as holding it, but that blocker is cleared with 'Loom's I-006 does the same' (a Rust target on ess-conformance), so the epic's 'task check runs the Loom ESS conformance suite' clause has no acceptance anywhere"},
  {"file": ".engineering/planning/story/loom-ess-conformance.md", "line": 43, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins the drift-gate failure and the scenario-per-command count with 'and', and omits the Outcome's 'every UNMAPPED marker left in ess/ is held by an open decision-blocker'"},
  {"file": ".engineering/planning/story/argument-generator.md", "line": 42, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins two independent outcomes with 'and' (the generator is handed exactly the selected action, and its JSON output reaches ProposedAction through a synthesized ArgumentRequest)"},
  {"file": ".engineering/planning/story/selection-revalidation.md", "line": 45, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the Outcome and ESS first name three refusals (not in frontier, stale revision, authority denied), but the acceptance observes only two, so the 'asks for authority and obeys the answer' clause can be done or undone without the check noticing"},
  {"file": ".engineering/planning/story/interruption-recovery.md", "line": 38, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the Outcome promises that a run suspended at an approval resumes from its checkpoint before the exact effect, but the acceptance covers only cancel after selection and resume after the case moved"},
  {"file": ".engineering/planning/story/harness-module-map.md", "line": 50, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins two independent outcomes with 'and': one row per crate with one disposition, and every Owns responsibility named by a row or listed as new"}
]
```
