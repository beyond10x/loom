---
format: aep.planning-md/3
id: review-result:tree-daemon-under-enforced-limits-design-r1
kind: review-result
status: active
title: Plan critic design, round 1, on epic:tree-daemon-under-enforced-limits
relations:
- reviews: epic:tree-daemon-under-enforced-limits
- reviews: story:substrate-daemon-runner
- reviews: story:action-limits-enforced
- reviews: story:limit-hits-in-run-log
- reviews: story:executable-allow-list
- reviews: story:confinement-level-required
- reviews: story:named-egress-per-action
- reviews: story:agent-session-through-substrate
- reviews: story:tree-daemon
revision: 1
---
needs-revision

Findings, one line each:

epic:tree-daemon-under-enforced-limits — five stories each add a fragment of "the engine configuration" (`EngineLimits`, `ExecutablePolicy`, `EngineApertures`, the level default, the model-endpoint aperture and credential slot) and `story:executable-allow-list` tests that it "is refused when it is loaded", but no story in the set or in `epic:protocol-tree-on-bindings` owns that configuration as a document or loader, or says how `tree run` and `tree serve` receive it. The epic's Stories list should add one owner that the five depend on, or name one of them as the owner. — .engineering/planning/epic/tree-daemon-under-enforced-limits.md:55; .engineering/planning/story/executable-allow-list.md:55; .engineering/planning/story/confinement-opens-from-engine-config.md:49

story:executable-allow-list — it holds two things behind one whole-story `blocks` edge: Loom's own program check, which the blocker body says "does not wait on this", and the descendants hold, which does. `aep plan artifact blocked` lists the whole story as stopped. The descendants half (the "adapter that proves the executable fact" acceptance bullet) should be split into its own story that carries the `blocks` edge, so the program check can land. — aep plan artifact blocked (row `upstream-blocker:substrate-executable-allow-list`); .engineering/planning/upstream-blocker/substrate-executable-allow-list.md:26-27; .engineering/planning/story/executable-allow-list.md:59-62

story:named-egress-per-action — `upstream-blocker:substrate-ingress` `blocks` the whole story. But the story only refuses `ingress` at load (`IngressUnserved`), which needs nothing from Substrate, and the blocker body says "Its egress half does not wait". The edge stops a story whose acceptance no Substrate release gates, and the ingress work that is blocked has no story of its own. The edge should move to the later story that serves ingress, or be dropped. — aep plan artifact blocked (row `upstream-blocker:substrate-ingress`); .engineering/planning/upstream-blocker/substrate-ingress.md:23-24; .engineering/planning/story/named-egress-per-action.md:57

story:confinement-level-required — its outcome has the engine open "a Substrate daemon", the `Daemon` variant that `story:substrate-daemon-runner` adds to the same `EngineConfinement`, and the epic says the story "needs" `story:sdk-confinement-requires` for it. No `depends_on` edge to the daemon-runner story exists, and no upstream-blocker records the SDK request. The body should either take the daemon out of its outcome, or gain that edge and a blocker artifact. — .engineering/planning/story/confinement-level-required.md:19-20; .engineering/planning/story/substrate-daemon-runner.md:34; .engineering/planning/epic/tree-daemon-under-enforced-limits.md:105-106

story:action-limits-enforced — it adds a per-action wall limit (`limits`, wall seconds) beside `timeout_seconds` (1 to 3600), which the active `story:bindings-document` already owns. Neither body says which one wins or how they relate, so both epics own "wall time ends this action". — .engineering/planning/story/action-limits-enforced.md:31,40-45; .engineering/planning/story/bindings-document.md:53,97

story:action-limits-enforced — its per-run total-wall-time ceiling and `RunLimitSpent` (exit 5, before the next step) claim the same stop as `--max-wall-seconds` and `TreeStop::BudgetSpent` of `story:tree-run-command`. Nothing says whether the flag is capped by the ceiling or is a second budget, or what `RunLimitSpent` means across `tree run` invocations. — .engineering/planning/story/action-limits-enforced.md:59-60; .engineering/planning/story/tree-run-command.md:150

story:action-limits-enforced — it decides "cannot enforce a limit" by adapter name (`the local adapter`). `story:confinement-level-required` owns the mapping from a level to the facts an adapter proves, with "limits" and "no egress" among them. Nothing records that the limit, egress and executable stories read facts through it, and none of the three depends on it, so there are two mechanisms. This story should depend on `story:confinement-level-required` and read the facts through its mapping. — .engineering/planning/story/action-limits-enforced.md:25,61-63; .engineering/planning/story/confinement-level-required.md:21-24

story:action-limits-enforced — "the step records the limit as reached" for the process-count hit is also what `story:limit-hits-in-run-log` delivers as a `LimitHit` line of kind `processes`, so each story claims half of recording one hit. The acceptance here should say only that the forks fail and the step completes, and leave the record to the limit-hits story. — .engineering/planning/story/action-limits-enforced.md:54-55; .engineering/planning/story/limit-hits-in-run-log.md:50-51

story:limit-hits-in-run-log — its outcome and summary promise `egress bytes` events, but the byte ceiling and the stop are created by `story:named-egress-per-action`, and no `depends_on` edge runs from this story to it. The edge should be added, or egress bytes dropped from the outcome. — .engineering/planning/story/limit-hits-in-run-log.md:7,21; .engineering/planning/story/named-egress-per-action.md:61-62

story:limit-hits-in-run-log — its result-table keys `limit:wall-time` and `limit:memory` sit "beside `timeout`", the key `story:bound-program-result-table` looks up when a program times out. Both claim the same wall-time hit and the body does not say which key is looked up first or whether `timeout` stays. — .engineering/planning/story/limit-hits-in-run-log.md:24; .engineering/planning/story/bound-program-result-table.md:73

story:agent-session-through-substrate — its outcome runs the harness "under the action's limits and executable policy", but the policy is created by `story:executable-allow-list` and no edge records that order. The edge should be added, or "executable policy" dropped from the outcome. — .engineering/planning/story/agent-session-through-substrate.md:23; .engineering/planning/story/executable-allow-list.md:41

**What you read:** all 11 artifacts of the set in full. Context read: the bodies of `epic:protocol-tree-on-bindings`, `story:tree-run-command`, `story:confinement-opens-from-engine-config`, `story:agent-node-run-records`, `story:bound-commands-run-confined`, `story:bound-program-result-table` and parts of `story:bindings-document` and `story:bound-agent-node`; `crates/loom-intake-slice/src/confinement.rs` (the `run_child`/`spawn_child` signatures). Commands run: `aep plan artifact show`, `relations`, `graph --format json`, `validate`, `waves`, `blocked`.

**Edge walk:**
- I walked 224 `depends_on` edges across the whole 665-artifact graph, outside the set included, and found no cycle. The `blocks` and `decomposes` edges are not `depends_on` and were not walked.
- There is no serialising chain. The deepest path is `tree-run-command` → `action-limits-enforced` → `agent-session-through-substrate` → (proposed) `limit-hits-in-run-log`.
- I traced 4 acceptance-line reads to a producer inside the set, and all 4 have an edge. They are `limit-hits` reading the daemon runner and the limits, and `agent-session` reading the limits and the apertures.
- Three outcome-level reads have no edge. They are egress bytes, executable policy, and `Daemon` (the findings above).
- The edges on the first epic's stories (`confinement-opens-from-engine-config`, `bound-commands-run-confined`, `bound-program-result-table`) are all inherited through `tree-run-command`, so no finding is needed there.
- `aep plan artifact validate` reports `valid`; the warnings about review-results with no recorded outcome are not mine.

**What you could not establish:**
- Out of my lane, scope: no story's acceptance runs a tree inside the daemon together with Substrate-daemon limit hits and the `ps` check, so the epic's acceptance has no owner.
- Out of my lane, scope: the harness's limit stop (`agent-session` acceptance) has no `LimitHit` event owner, though the epic outcome says every process.
- Out of my lane, acceptance: the executable-allow-list bullet "under an adapter that proves the executable fact" cannot be checked, because Loom can open no adapter that proves it (`macos` is out of scope in the first epic).
- Out of my lane, parallel-safety: `ess/domains/tree.yaml`, `ess/domains/bindings.yaml` and `BoundAction` are edited by five stories that depend only on `tree-run-command`.
- Unease, not a finding: `story:tree-daemon` depends on `story:agent-node-run-records` with no stated reason, since its acceptance reads nothing that story creates.
- Unease, not a finding: `tree stop --now` needs to interrupt a blocking `run_child`. No acceptance in the set shows a stop through the runner (`substrate-daemon-runner` has none), and I did not trace how it would work.

```findings
- file: .engineering/planning/epic/tree-daemon-under-enforced-limits.md
  line: 55
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "five stories each add a fragment of 'the engine configuration' (EngineLimits, ExecutablePolicy, EngineApertures, level default, model aperture and credential slot) and executable-allow-list tests that it is 'refused when it is loaded', but no story in the set or in epic:protocol-tree-on-bindings owns that configuration as a document or loader, or says how tree run and tree serve receive it; the Stories list should add one owner the five depend on, or name one of them as the owner"
- file: .engineering/planning/story/executable-allow-list.md
  line: 59
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it holds two things behind one whole-story blocks edge, Loom's own program check (which the blocker body says does not wait) and the descendants hold (which does), so aep plan artifact blocked lists the whole story as stopped; the descendants half should be split into its own story that carries the blocks edge"
- file: .engineering/planning/story/named-egress-per-action.md
  line: 57
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "upstream-blocker:substrate-ingress blocks the whole story, but the story only refuses ingress at load (IngressUnserved), which needs nothing from Substrate, and the blocker body says its egress half does not wait; the blocks edge should move to the later story that serves ingress, or be dropped"
- file: .engineering/planning/story/confinement-level-required.md
  line: 19
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its outcome has the engine open a Substrate daemon, the Daemon variant story:substrate-daemon-runner adds to the same EngineConfinement, and the epic says it needs story:sdk-confinement-requires for it, yet no depends_on edge to the daemon-runner story and no upstream-blocker records either; either take the daemon out of the outcome or add the edge and a blocker artifact"
- file: .engineering/planning/story/action-limits-enforced.md
  line: 31
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it adds a per-action wall limit beside timeout_seconds (1 to 3600) that the active story:bindings-document owns, and neither body says which one wins or how they relate, so both epics own 'wall time ends this action'"
- file: .engineering/planning/story/action-limits-enforced.md
  line: 59
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its per-run total-wall-time ceiling and RunLimitSpent (exit 5, before the next step) claim the same stop as --max-wall-seconds and TreeStop::BudgetSpent of story:tree-run-command, and nothing says whether the flag is capped by the ceiling or is a second budget, or what RunLimitSpent means across tree run invocations"
- file: .engineering/planning/story/action-limits-enforced.md
  line: 25
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it decides 'cannot enforce a limit' by adapter name (the local adapter) while story:confinement-level-required owns the mapping from a level to the facts an adapter proves, and no depends_on edge records that the limit, egress and executable stories read facts through it, so there are two mechanisms; this story should depend on story:confinement-level-required and read the facts through its mapping"
- file: .engineering/planning/story/action-limits-enforced.md
  line: 55
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'the step records the limit as reached' for the process-count hit is also what story:limit-hits-in-run-log delivers as a LimitHit line of kind processes, so each story claims half of recording one hit; the acceptance here should say only that the forks fail and the step completes"
- file: .engineering/planning/story/limit-hits-in-run-log.md
  line: 21
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its outcome and summary promise egress bytes events, but the byte ceiling and the stop are created by story:named-egress-per-action and no depends_on edge runs from this story to it; add the edge or drop egress bytes from the outcome"
- file: .engineering/planning/story/limit-hits-in-run-log.md
  line: 24
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its result-table keys limit:wall-time and limit:memory sit beside the key timeout that story:bound-program-result-table already looks up when a program times out, so both claim the same wall-time hit and the body does not say which key is looked up first or whether timeout stays"
- file: .engineering/planning/story/agent-session-through-substrate.md
  line: 23
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its outcome runs the harness under the action's limits and executable policy, but the policy is created by story:executable-allow-list and no depends_on edge records that order; add the edge or drop executable policy from the outcome"
```
