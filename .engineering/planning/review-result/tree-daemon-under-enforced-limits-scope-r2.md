---
format: aep.planning-md/3
id: review-result:tree-daemon-under-enforced-limits-scope-r2
kind: review-result
status: active
title: Plan critic scope, round 2, on epic:tree-daemon-under-enforced-limits
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

- `epic:tree-daemon-under-enforced-limits` — the plain `tree stop` command, the `Stopped` state and the `Stop` command of `story:tree-daemon` are in no sentence of the epic or the request, and the epic's `## Why` gives reasons only for restart recovery and approval continuation — `.engineering/planning/story/tree-daemon.md:31`

**What you read:** 17 artifacts: the epic first (its promises written down before any story), the round-1 report, `.engineering/drafts/e2r2-changes.md`, 12 stories and 4 upstream blockers, all with `aep plan artifact show`. I also ran `aep plan artifact graph` and grepped the store files for the dropped items. Those items are output bytes, egress bytes, `RunLimitSpent` and `limit:wall-time`. I found no leftover mention beyond the Out-of-scope lines and the fixed-cap lines. I also grepped `story:tree-run-command` for `tree stop`.

Part 1, round-1 findings (5):

- **Resolved: `story:action-limits-enforced:19` (per-run ceilings for five limits).**
  - `story:action-limits-enforced` Outcome now sets per-run ceilings for wall time and CPU time only, and says "Substrate bounds memory and process count per process start, so neither has a per-run sum" (`.engineering/planning/story/action-limits-enforced.md:28`).
  - The epic's `## Not in scope` repeats it (`epic:tree-daemon-under-enforced-limits`, `.engineering/planning/epic/tree-daemon-under-enforced-limits.md:144`).
- **Resolved: `epic:48` (no owner for a daemon serving `oci` or `pod`).**
  - `upstream-blocker:substrate-daemon-oci-pod` now blocks the new `story:confinement-level-on-oci-and-pod` (epic lines 93 and 177).
  - `story:confinement-level-required` is narrowed to `local` and `linux` and says why (`.engineering/planning/story/confinement-level-required.md:49-52`).
  - `upstream-blocker:substrate-sdk-confinement-requires` is filed and blocks the same story.
- **Resolved: `named-egress:20` (per-aperture byte ceiling).** It is gone from `story:named-egress-per-action`, whose Out of scope names it ("A byte ceiling per aperture"), and from `story:limit-hits-in-run-log`. The epic records it at line 145.
- **Resolved: `epic:22` (output size).** Output size is out of the epic's Outcome. The decision is at epic line 75-78, "output bytes stay the fixed 1 MiB cap", and `## Not in scope` has it at line 143. The story mentions in `story:action-limits-enforced` (:54, :119) only describe the fixed cap.
- **Partly resolved, carried: `tree-daemon:25`.**
  - Restart recovery and approval continuation each have a reason in the epic's `## Why` (epic lines 38-44).
  - `tree stop --now` is dropped and named in epic `## Not in scope` line 150.
  - Plain `tree stop` is not covered. No `tree stop` exists in `story:tree-run-command` (grep found none).
  - The epic's Outcome, Why and Acceptance never mention it; `Stopped` and `Stop` first appear in `story:tree-daemon` (`.engineering/planning/story/tree-daemon.md:31`, `:63`, `:94`).
  - The only trace is the epic's Not-in-scope entry for `--now`, which implies a plain stop without justifying one.
  - The fix is a sentence in the epic's `## Why`, or dropping the command and the state.

Part 2, promises traced (this was a count of 12 in round 1): I extracted 12 and traced all 12.

- **Many trees at once** — `story:tree-daemon`.
- **Every process through Substrate, and remote placement by endpoint** — `story:substrate-daemon-runner`, `story:agent-session-through-substrate` and `story:engine-configuration-document`.
- **Per-action memory, CPU, wall and process count** — `story:action-limits-enforced`.
- **Per-run limits** — `story:action-limits-enforced`, wall and CPU only. The narrowing is recorded in the epic at line 144.
- **Executable allow-list and deny-list** — `story:executable-allow-list` for the program Loom starts. `story:executable-descendants-held` covers descendants, held by `upstream-blocker:substrate-executable-allow-list`.
- **Confinement level** — `story:confinement-level-required` for `local` and `linux`. `story:confinement-level-on-oci-and-pod` covers `oci` and `pod`, held by two blockers.
- **Egress** — `story:named-egress-per-action`.
- **Ingress** — `story:ingress-routes-per-action`, held by `upstream-blocker:substrate-ingress`. `story:named-egress-per-action` refuses it at load until then.
- **Limit hits as events the UI can show** — `story:limit-hits-in-run-log`, through the `LimitHit` event, the `tree status` count and `docs/contracts/run-log-v1.json`.

No promise is claimed twice. Reach beyond the parent, besides the finding above:

- The new `story:engine-configuration-document` is justified. The epic's Why, at line 78, names its single owner, and every limit, list and aperture needs a place to live.
- Run slots and the queue in `story:tree-daemon` are not asked for by the request. The epic's own Acceptance carries "three run slots" (line 112), so I made this no finding. The epic does not say why the cap exists.
- The advisory switches, the lease kill and the `oci`/`pod` image-digest requirement are drafter decisions and trace to the request's limits, remote and level promises. I made them no finding.

**What I could not establish (out of my lane, not set against the verdict):**

- The three held stories each name "a pin story of its own" that comes before them (`story:executable-descendants-held:71`, `story:ingress-routes-per-action:67`, `story:confinement-level-on-oci-and-pod:71`). None is drafted and the epic does not list one. This is for `plan-critic-design`.
- Under in-process `linux`, only the wall-time hit is recorded. Memory, CPU and process-count hits wait on Substrate stories that the epic's `## Upstream` names (epic line 160), so I counted them as recorded rather than a silent narrowing. `story:limit-hits-in-run-log` states it only in an acceptance bullet (:76).
- No acceptance bullet shows an `arguments_from` program running through the daemon, and no epic fixture is named for one. This is for `plan-critic-acceptance`.
- Under `Daemon`, an agent session is checked only up to the daemon's refusal of the slot (`story:agent-session-through-substrate`). The Why says so. Whether that meets "every process through Substrate" is the acceptance critic's question.

```findings
- file: .engineering/planning/story/tree-daemon.md
  line: 31
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "restart recovery and approval continuation now have their reasons in the epic's Why and tree stop --now is dropped, but the plain tree stop command, the Stopped state and the Stop command of story:tree-daemon are traceable to no sentence of the parent or the quoted request, and the epic's Why gives no reason for them; the epic should say why or the story should drop them"
```
