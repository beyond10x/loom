---
format: aep.planning-md/3
id: review-result:tree-daemon-under-enforced-limits-acceptance-r2
kind: review-result
status: active
title: Plan critic acceptance, round 2, on epic:tree-daemon-under-enforced-limits
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

All 11 round-1 findings are resolved except one, which is carried in part. Seven findings are open, all warnings. The revision introduced six of them. The seventh is the carried one, on `agent-session-through-substrate`.

**Part 1: round-1 findings**

- `action-limits-enforced:54` (blocker, fork loop): resolved. The bullet says the forks fail, the program exits 0 and the step takes its result from that exit. It no longer says "records" and no longer reads as a refusal.
- `action-limits-enforced:22` (output bytes, per-run CPU case): resolved. Output bytes left the epic and every story. The per-run CPU bullet is split into `Daemon` and in-process `linux`, where the in-process case is refused `CpuTimeUnreported`.
- `action-limits-enforced:50` (linux or daemon): resolved. Every bullet runs under both paths unless it names one. `confinement-level-required` and `substrate-daemon-runner` are now in the `depends_on` chain.
- `limit-hits-in-run-log:20` (egress bytes, peak): resolved. Egress bytes are dropped. The memory peak is checked against the exec's `memory_peak_bytes`, and the process peak against the forks bullet.
- `executable-allow-list:60` (descendants bullet cannot run on the gate): resolved. It moved to `story:executable-descendants-held`, which the blocker holds. The kept story's bullets all run today.
- `confinement-level-required:50` (no concrete level or fact): resolved. The levels are named family sets, and the bullets name `filesystem_view`, `limits`, `stdin_pipe` and `exec_subpaths`.
- `confinement-level-required:14` (Pod and daemon unchecked): resolved. `oci` and `pod` moved to the held story. The decoder refusal and the `Daemon` bullet are in, and the ignorable test and the qualification record are gone.
- `agent-session-through-substrate:22` (limits and executable policy): carried in part. The executable-policy case is added and resolved. The limits part still holds, see the first finding below.
- `substrate-daemon-runner:18` (HTTPS, signal, stop, test runner, SDK): resolved. There are bullets for `signal`, the test runner, the lease kill and the HTTPS endpoint refused before any child starts. The SDK is named by `rev` and `version`.
- `tree-daemon:18` (open/close/verify, waiting slot): resolved. The open, close and verify bullet and the `W` Waiting bullet now exist, and the slot bullet can no longer pass when nothing waits.
- `epic:70` (one paragraph, fixtures, second host): resolved. It is now separate bullets with named fixtures and a second managed daemon. This produced one new defect, the CPU wording below.

**Part 2: findings on the revised set**

story:agent-session-through-substrate — carried: the Outcome says the harness runs "under the action's limits", but the acceptance checks the wall limit only, so a memory, CPU or process-count stop of the harness is never shown (the executable-policy half is fixed) — .engineering/planning/story/agent-session-through-substrate.md:23 (acceptance `:75`)

epic:tree-daemon-under-enforced-limits — the CPU bullet says the run "stops with `BudgetSpent` naming `cpu_millis` before the step that would pass it", but the stories stop only once the total "has reached a ceiling" before the next step, so the stated stop point cannot be checked against what is built — .engineering/planning/epic/tree-daemon-under-enforced-limits.md:120-121 (`story/action-limits-enforced.md:31`)

story:confinement-level-required — no bullet says what an engine configuration with no `default_level` does, though the Outcome makes it "required of every action" and `story:engine-configuration-document` and the `local` bullet of `story:action-limits-enforced` run files holding only the `confinement` section — .engineering/planning/story/confinement-level-required.md:26 (acceptance `:70-90`)

story:limit-hits-in-run-log — the Outcome gives every `LimitHit` an "enforced" member, but no bullet produces or checks `enforced: false` (or `true`), so it can be present or wrong unseen — .engineering/planning/story/limit-hits-in-run-log.md:21 (acceptance `:63-80`)

story:executable-allow-list — the relative-`argv[0]` bullet resolves against "the engine's `PATH`", which no section of the engine configuration or `ExecutablesSection` defines, so a test cannot set it up — .engineering/planning/story/executable-allow-list.md:64

story:confinement-level-on-oci-and-pod — the bullet "refused at `tree open` `LevelUnmet`, naming the first family or fact it lacks" asks for a fact, but `LevelUnmet` carries (action, level, family) and this story's ESS first adds no fact member — .engineering/planning/story/confinement-level-on-oci-and-pod.md:63 (`story/confinement-level-required.md:65`)

story:confinement-level-on-oci-and-pod — the Outcome says "the daemon refuses an exec it cannot serve at admission", but the only `requires` bullet checks that the exec observation echoes it, and none starts an exec the daemon cannot serve — .engineering/planning/story/confinement-level-on-oci-and-pod.md:23 (acceptance `:55-67`)

**What I read**

I read 17 artifacts with `aep plan artifact show` (the epic, 12 stories, 4 blockers). I also read the round-1 report, `.engineering/drafts/e2r2-changes.md` and the upstream-blocker lifecycle. I ran no build or test and did not open Substrate sources.

**What I could not establish**

- Whether the Linux gate's in-process `linux` probe serves `limits` and whether `ManagedDaemon` starts there. The work order's Open section names the same doubt. Many unheld bullets run on it.
- Out of my lane, not counted in the verdict:
  - `story:agent-session-through-substrate` `## Waits on` says the daemon "stops" agent sessions with a tree, while `story:tree-daemon` `tree stop` only lets the current step finish.
  - `upstream-blocker:substrate-executable-allow-list` clears only when `oci` and `pod` also prove the lists, though `story:executable-descendants-held` needs `linux` only.

```findings
- file: .engineering/planning/story/agent-session-through-substrate.md
  line: 23
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says the harness runs under the action's limits, but the acceptance checks the wall limit only"
- file: .engineering/planning/epic/tree-daemon-under-enforced-limits.md
  line: 120
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the CPU bullet says the run stops with BudgetSpent naming cpu_millis before the step that would pass it, but the stories stop only once the total has reached the ceiling before the next step, so the stated stop point cannot be checked against what is built"
- file: .engineering/planning/story/confinement-level-required.md
  line: 26
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet says what an engine configuration with no default_level does, though the Outcome makes it required of every action and other stories' bullets run engine files that hold only the confinement section"
- file: .engineering/planning/story/limit-hits-in-run-log.md
  line: 21
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome gives every LimitHit an enforced member, but no bullet produces or checks enforced false or true, so it can be present or wrong unseen"
- file: .engineering/planning/story/executable-allow-list.md
  line: 64
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the relative argv[0] bullet resolves against the engine's PATH, which no section of the engine configuration defines, so a test cannot set it up"
- file: .engineering/planning/story/confinement-level-on-oci-and-pod.md
  line: 63
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet 'refused at tree open LevelUnmet, naming the first family or fact it lacks' asks for a fact, but LevelUnmet carries action, level and family and this story's ESS first adds no fact member"
- file: .engineering/planning/story/confinement-level-on-oci-and-pod.md
  line: 23
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says the daemon refuses an exec it cannot serve at admission, but the only requires bullet checks that the exec observation echoes it and none starts an exec the daemon cannot serve"
```
