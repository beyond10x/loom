---
format: aep.planning-md/3
id: review-result:tree-daemon-under-enforced-limits-scope-r1
kind: review-result
status: active
title: Plan critic scope, round 1, on epic:tree-daemon-under-enforced-limits
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

- `story:action-limits-enforced` — the Outcome promises a per-run ceiling for each of five limits, but only total wall time and total CPU time are counted per run, and nothing records that per-run memory, process count and output were dropped — `.engineering/planning/story/action-limits-enforced.md:19`
- `epic:tree-daemon-under-enforced-limits` — promise "a daemon serving `local`, `oci` or `pod`" is listed as missing in Substrate, but no story or upstream-blocker claims it, so the `oci` and `pod` levels in `story:confinement-level-required` can only run in process. That contradicts "Every process a run starts … is started and stopped by a Substrate daemon". Nothing records the narrowing, and the natural owner is a new upstream-blocker on `story:confinement-level-required` — `.engineering/planning/epic/tree-daemon-under-enforced-limits.md:48`
- `story:named-egress-per-action` — the optional per-aperture byte ceiling, with a step ended past it, is traceable to no sentence of the parent or the request. `story:limit-hits-in-run-log` carries it on as the `egress bytes` limit kind — `.engineering/planning/story/named-egress-per-action.md:20`
- `epic:tree-daemon-under-enforced-limits` — the Outcome adds "output size" to the limit list, but the request, quoted in the same epic's Why, names memory, CPU, wall time and process count only. The stories carry output bytes into the engine ceilings, the `limit:` result keys and the acceptance — `.engineering/planning/epic/tree-daemon-under-enforced-limits.md:22`
- `story:tree-daemon` — restart recovery of every tree from store and run log, `tree stop` and `tree stop --now`, and a recorded approval continuing a tree without a command are traceable to no sentence of the parent or the quoted request. The request asks for a daemon that runs many trees at once and that the `tree` commands talk to — `.engineering/planning/story/tree-daemon.md:25`

**What you read:** 11 artifacts, all with `aep plan artifact show`: the epic first, then 8 stories and 2 upstream-blockers. I also ran `aep plan artifact graph` and listed the `upstream-blocker` directory. I extracted 12 promises from the epic and the request and traced all 12 to an item or a named blocker.
- The 12 are: many trees at once; every process through Substrate, including the `arguments_from` program and the agent harness; remote placement by endpoint; per-action limits; per-run limits; allow-list; deny-list; confinement level for local, linux, oci and pod; egress; ingress; limit hits as events; and the UI showing them.
- Two are traced only in part. Per-run limits are covered for wall time and CPU only. The `oci` and `pod` levels are covered only in process.
- Ingress is claimed by `story:named-egress-per-action` as a load-time refusal plus `upstream-blocker:substrate-ingress`. The epic's Not in scope names the narrowing, so it is not a finding.

**What you could not establish:**
- `story:confinement-level-required` says it needs `story:sdk-confinement-requires`, which is named in the epic's Upstream section. No upstream-blocker artifact or `depends_on` edge records it. I could not tell whether a named Upstream entry meets the epic's "filed as upstream blockers" line (`epic:81`), so I made it no finding.
- Out of lane for `plan-critic-acceptance`: the epic's `ps` check and the second-host run have no story acceptance (`story:substrate-daemon-runner` tests only a local socket).
- Out of lane for `plan-critic-design`: `story:tree-daemon` has no dependency on `story:substrate-daemon-runner`, and `story:limit-hits-in-run-log` has none on `story:named-egress-per-action` for the egress-bytes line.
- Not a finding: the advisory-limits and advisory-network switches on `local` sit awkwardly beside "hard limits". Both are drafter decisions that the request does not forbid.

```findings
- file: .engineering/planning/story/action-limits-enforced.md
  line: 19
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises a per-run ceiling for each of five limits, but only total wall time and total CPU time are counted per run, and nothing records that per-run memory, process count and output were dropped"
- file: .engineering/planning/epic/tree-daemon-under-enforced-limits.md
  line: 48
  category: scope
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "promise 'a daemon serving local, oci or pod' is listed as missing in Substrate but no story or upstream-blocker claims it, so the oci and pod levels in story:confinement-level-required can only run in process, contradicting 'Every process a run starts ... is started and stopped by a Substrate daemon'; nothing records the narrowing, and the natural owner is a new upstream-blocker on story:confinement-level-required"
- file: .engineering/planning/story/named-egress-per-action.md
  line: 20
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the optional per-aperture byte ceiling, with a step ended past it, is traceable to no sentence of the parent or the request, and story:limit-hits-in-run-log carries it on as the egress bytes limit kind"
- file: .engineering/planning/epic/tree-daemon-under-enforced-limits.md
  line: 22
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome adds output size to the limit list, but the request quoted in the same epic's Why names memory, CPU, wall time and process count only, and the stories carry output bytes into the engine ceilings, the limit: result keys and the acceptance"
- file: .engineering/planning/story/tree-daemon.md
  line: 25
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "restart recovery of every tree from store and run log, tree stop and tree stop --now, and a recorded approval continuing a tree without a command are traceable to no sentence of the parent or the quoted request, which asks for a daemon that runs many trees at once and that the tree commands talk to"
```
