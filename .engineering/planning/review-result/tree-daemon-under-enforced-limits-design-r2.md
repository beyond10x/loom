---
format: aep.planning-md/3
id: review-result:tree-daemon-under-enforced-limits-design-r2
kind: review-result
status: active
title: Plan critic design, round 2, on epic:tree-daemon-under-enforced-limits
relations:
- reviews: epic:tree-daemon-under-enforced-limits
- reviews: story:substrate-daemon-runner
- reviews: story:engine-configuration-document
- reviews: story:executable-descendants-held
- reviews: story:ingress-routes-per-action
- reviews: story:confinement-level-on-oci-and-pod
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

**Round 1 findings.** All 11 are resolved. None is carried.

1. `epic:…:55` (no owner of the engine configuration): resolved. `story:engine-configuration-document` owns `loom.engine/1`, its loader and `--engine`, and `story:substrate-daemon-runner` depends on it directly (`.engineering/planning/story/engine-configuration-document.md:14-27`).
2. `story:executable-allow-list:59` (one `blocks` edge over two parts): resolved. The descendants half is `story:executable-descendants-held`. The blocker now blocks only that story, and its body says the program check "does not wait on this" (`upstream-blocker/substrate-executable-allow-list.md:25-27`).
3. `story:named-egress-per-action:57` (ingress blocker on the whole story): resolved. `upstream-blocker:substrate-ingress` now blocks `story:ingress-routes-per-action`. Named-egress keeps only the load-time refusal (`named-egress-per-action.md:26-27`).
4. `story:confinement-level-required:19` (daemon with no edge): resolved. `depends_on story:substrate-daemon-runner` exists, `upstream-blocker:substrate-sdk-confinement-requires` exists, and the `oci` and `pod` levels moved to a held story.
5. `story:action-limits-enforced:31` (a second wall limit): resolved. `limits` has no wall member and `timeout_seconds` is the wall limit, capped by the engine (`action-limits-enforced.md:19-21`).
6. `story:action-limits-enforced:59` (`RunLimitSpent` against `BudgetSpent`): resolved. `RunLimitSpent` is dropped and the count is per invocation. One residue is raised as a new finding below.
7. `story:action-limits-enforced:25` (a mechanism keyed by adapter name): resolved. It reads the family mapping (`action-limits-enforced.md:32-35`) and has a direct edge to `story:confinement-level-required`.
8. `story:action-limits-enforced:55` (the process-count record claimed twice): resolved. The bullet says the forks fail and the step exits 0 (`action-limits-enforced.md:93-94`).
9. `story:limit-hits-in-run-log:21` (egress bytes with no edge): resolved. Egress bytes are dropped (`limit-hits-in-run-log.md:81-82`).
10. `story:limit-hits-in-run-log:24` (`limit:wall-time` beside `timeout`): resolved. `timeout` stays the wall key and no `limit:wall-time` key exists (`limit-hits-in-run-log.md:22-23`).
11. `story:agent-session-through-substrate:23` (executable policy with no edge): resolved. The chain orders `story:executable-allow-list` before it, through `story:named-egress-per-action`, and `## Waits on` says so (`agent-session-through-substrate.md:49`).

**Revised set.**
- **Cycles:** none. The `depends_on` chain runs `agent-node-run-records`, then the 12 stories in the epic's order, and every edge points backward in it.
- **Hidden reads:** every acceptance read of a sibling's output has a `depends_on` path to its producer. I found no missing edge.
- **Chain:** the serial chain is not a finding. Each edge has its reason in `## Waits on`.
- **Cross-epic ownership:** nothing else is owned by both epics. No story of `epic:protocol-tree-on-bindings` depends on a story of this epic. The one shared surface I found is the `BudgetSpent` naming, below.

Three findings remain. All are warnings.

story:action-limits-enforced — it conditionally owns the `TreeStop::BudgetSpent` naming ("added here if `story:tree-run-command` did not add it"), and its acceptance names a `wall_seconds` member that no `TreeBudget` declaration provides: `tree-run-command.md:113-115` names no members and this story's ESS first adds only `cpu_millis`. The body should add `wall_seconds` and the naming unconditionally, because the "request on the other epic" lives only in `.engineering/drafts/e2r2-changes.md` and in no artifact. — .engineering/planning/story/action-limits-enforced.md:82-84,104; .engineering/planning/story/tree-run-command.md:113-115,150

story:ingress-routes-per-action — its `depends_on story:executable-descendants-held` makes `upstream-blocker:substrate-executable-allow-list` hold this story, and through `story:confinement-level-on-oci-and-pod` that story too, yet `upstream-blocker:substrate-ingress` says it blocks this story and nothing says so. The recorded reason also names a shared `ess/domains/engine.yaml` edit, but descendants-held states "no new type" there. Name the trade-off in both blocker bodies (keep the edge and say what it also holds), or drop the edge among the three held stories and leave the launch path in `confinement.rs` to merge order. — .engineering/planning/story/ingress-routes-per-action.md:13,42-45; .engineering/planning/story/executable-descendants-held.md:51; .engineering/planning/upstream-blocker/substrate-ingress.md:19-20

story:confinement-level-on-oci-and-pod — two whole-story `blocks` edges hold more than waits on them. `upstream-blocker:substrate-sdk-confinement-requires` says it blocks only "the part … that sends a required level's families", and the `Oci` and `Pod` levels with the `LevelUnmet` refusal under a `Host` daemon (acceptance bullet 2) need neither blocker. This is the round-1 shape of `story:executable-allow-list`. Move the levels and that refusal into an unblocked story, or narrow the blocker bodies to what really waits. — .engineering/planning/story/confinement-level-on-oci-and-pod.md:41-42,57-61; .engineering/planning/upstream-blocker/substrate-sdk-confinement-requires.md:26-27

**What you read:**
- 17 artifacts in full: the epic, 12 stories and 4 blockers.
- Context read: `story:tree-run-command`, `story:bindings-document`, `story:confinement-opens-from-engine-config`, the Scope section of `epic:protocol-tree-on-bindings`, and `review-result:…-design-r1`.
- Commands run: `aep plan artifact show`, `relations`, `graph --format json`, `validate` (it reports `valid`), `blocked`, and `.engineering/drafts/e2r2-changes.md`.

**Edge walk.**
- I walked 234 `depends_on` edges over the whole graph, outside the set included, and found no cycle. I did not walk the `blocks` edges.
- I traced 21 in-set acceptance reads to a producer. All 21 have a `depends_on` path: 8 by a direct edge and 13 through the chain.
- I computed the transitive closures of `story:agent-node-run-records`, `story:engine-configuration-document` and `story:tree-daemon`. They contain every first-epic story whose output this epic reads.

**What you could not establish:**
- Out of my lane, acceptance: `story:engine-configuration-document` acceptance bullet 3 (`LocalReasonMissing`) names a refusal that its ESS first does not list. Its `EngineRefusal` has only `FormatUnknown` and `MemberUndeclared` (`engine-configuration-document.md:63,78`).
- Out of my lane, acceptance: `story:executable-allow-list` resolves a relative `argv[0]` against "the engine's `PATH`", and no section of `loom.engine/1` declares a `PATH` (`executable-allow-list.md:60`).
- Out of my lane, scope: `story:confinement-level-on-oci-and-pod` is outside the epic's acceptance checks.
- Unease, not a finding: `story:executable-descendants-held` names its edge to `story:tree-daemon` as ordering on `confinement.yaml` and `confinement.rs`, which `story:tree-daemon` does not edit. The edge costs nothing, since both stories are unheld by the time it matters.
- Unease, not a finding: `story:named-egress-per-action` adds `ingress` to `BoundAction` only to refuse it, and `story:ingress-routes-per-action` serves it. I judged that staging acceptable because the first story is demonstrable alone.

```findings
- file: .engineering/planning/story/action-limits-enforced.md
  line: 82
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it conditionally owns the TreeStop::BudgetSpent naming ('added here if story:tree-run-command did not add it') and its acceptance names a wall_seconds member that no TreeBudget declaration provides (tree-run-command.md:113-115 names none, this ESS first adds only cpu_millis), so the body should add wall_seconds and the naming unconditionally, since the request on the other epic is recorded in no artifact"
- file: .engineering/planning/story/ingress-routes-per-action.md
  line: 42
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its depends_on edge to story:executable-descendants-held makes upstream-blocker:substrate-executable-allow-list hold this story and, through story:confinement-level-on-oci-and-pod, that one too, which no blocker body says, and the recorded reason names an engine.yaml edit that descendants-held says it does not make; state the trade-off in both blocker bodies or drop the edge among the three held stories and leave the shared launch path in confinement.rs to merge order"
- file: .engineering/planning/story/confinement-level-on-oci-and-pod.md
  line: 41
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "two whole-story blocks edges hold more than waits on them: upstream-blocker:substrate-sdk-confinement-requires says it blocks only the part that sends the required level, and the Oci and Pod levels with the LevelUnmet refusal under a Host daemon (acceptance bullet 2) need neither blocker; move them into an unblocked story or narrow the blocker bodies"
```
