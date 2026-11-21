---
format: aep.planning-md/3
id: review-result:tree-daemon-under-enforced-limits-acceptance-r1
kind: review-result
status: active
title: Plan critic acceptance, round 1, on epic:tree-daemon-under-enforced-limits
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

action-limits-enforced has the one blocker. I read no Loom source beyond the bodies' own citations and ran no build or test.

story:action-limits-enforced — the fork-loop bullet says the step "records the limit as reached", but it names no record or field. Its own Out of scope sends recording a hit to `story:limit-hits-in-run-log`. "The same for" also reads as "step is refused", which contradicts the limit-hits bullet where forks fail, the program exits 0 and "the step is not refused for it" — `.engineering/planning/story/action-limits-enforced.md:54-55` (also `:68`, `story/limit-hits-in-run-log.md:50-51`)

story:action-limits-enforced — the Outcome lists five limits and per-run ceilings on total wall time and total CPU time, but the acceptance has no output-bytes case and no per-run CPU-time ceiling case, so those parts can pass or fail unseen — `.engineering/planning/story/action-limits-enforced.md:22-25` (acceptance `:50-64`)

story:action-limits-enforced — the preamble "Under the `linux` adapter or the Substrate daemon" lets the whole list pass on one path while the other fails. It does not say whether both must hold. The daemon runner is not among this story's `depends_on` — `.engineering/planning/story/action-limits-enforced.md:50`

story:limit-hits-in-run-log — the Outcome lists an `egress bytes` kind and a measured peak "where Substrate reports it", but no acceptance bullet produces an egress-byte `LimitHit` or checks a peak. The egress-byte case is checked only as an applied record in `story:named-egress-per-action`, not as a run-log event — `.engineering/planning/story/limit-hits-in-run-log.md:20-22` (acceptance `:48-59`)

story:executable-allow-list — the bullet "Under an adapter that proves the executable fact, a bound allowed program that tries to exec a denied one fails" has no adapter that can satisfy it on the Linux gate. At 0.21.0 only `macos` proves it, `linux` refuses it, and `upstream-blocker:substrate-executable-allow-list` is open. It is not marked conditional, so the story cannot honestly close while the blocker stays open, though the blocker says it blocks "only the part" — `.engineering/planning/story/executable-allow-list.md:60-62`

story:confinement-level-required — the acceptance names no concrete level or fact. "a level whose facts `linux` does not prove" and "naming the missing fact" can't be turned into a test, and the only level named is `linux`, which is an adapter name. The Why says adapter names are discouraged — `.engineering/planning/story/confinement-level-required.md:48-51`

story:confinement-level-required — the Outcome and ESS add `Pod` and the daemon as openable, but no acceptance bullet shows either. The "qualification record instead" in Out of scope has no check on its content or existence. The `oci` bullet may be ignored, so `task check` can exit 0 with `oci` never exercised — `.engineering/planning/story/confinement-level-required.md:14-16` (acceptance `:50-57`, `:61`)

story:agent-session-through-substrate — the Outcome says the harness runs "under the action's limits and executable policy", but the acceptance checks the wall limit only and has no executable-policy case — `.engineering/planning/story/agent-session-through-substrate.md:22-24` (acceptance `:50-63`)

story:substrate-daemon-runner — the Outcome promises an HTTPS endpoint and "starts, observes, signals and stops" every child. The acceptance runs only a local socket and has no signal or stop bullet and no test-runner bullet. Its first bullet also leaves "the SDK at the pinned release" unnamed — `.engineering/planning/story/substrate-daemon-runner.md:18-22` (acceptance `:41-54`)

story:tree-daemon — the Outcome says `tree open`, `close` and `verify` reach a tree through the daemon, and that a waiting tree uses no run slot. No bullet shows either. The slot bullet says only "the third starts when one waits or ends", which passes if a tree ends and none waits — `.engineering/planning/story/tree-daemon.md:18-20` (acceptance `:51-52`)

epic:tree-daemon-under-enforced-limits — the acceptance is one paragraph of about seven independent outcomes joined by semicolons. "three fixture trees" are unnamed. "no child of the Loom daemon that is not a Substrate client connection" compares a process to a connection, so two observers can read it differently. The second host has no stated test setup — `.engineering/planning/epic/tree-daemon-under-enforced-limits.md:70-76`

What I read: all 11 ids (the epic, 8 stories, 2 upstream-blockers) with `aep plan artifact show` and `kinds`/`lifecycle` for `epic`, `story` and `upstream-blocker`. I also read `story:tree-run-command` and `story:bindings-document` (exit 5 and the `results` table exist), the Substrate 0.21.0 sources the acceptances rely on, and the session-executor spawn code.

What I could not establish:
- **Output-limit event source:** Substrate 0.21.0 shows no refusal code for an output cut, only `OutputSlice.truncated`. The "output" `LimitHit` bullet has no cited Substrate observation, so I could not say if it is feasible.
- **Out of my lane, design or parallel-safety:** `story:limit-hits-in-run-log` does not depend on `story:named-egress-per-action`, though it reports egress-byte hits. `story:agent-session-through-substrate` does not depend on `story:executable-allow-list`, though it claims the executable policy.
- **Out of my lane, scope:** the epic acceptance has no ingress or confinement-level clause, and `story:named-egress-per-action` only refuses ingress. After a SIGKILL of the Loom daemon the Substrate daemon may keep the started child alive, which `story:tree-daemon` does not address.

```findings
- file: .engineering/planning/story/action-limits-enforced.md
  line: 54
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the fork-loop bullet says the step 'records the limit as reached' but names no record or field and recording a hit is this story's own Out of scope; 'The same for' also reads as step refused, contradicting story:limit-hits-in-run-log where the process-limit step is not refused"
- file: .engineering/planning/story/action-limits-enforced.md
  line: 22
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome lists output bytes and per-run ceilings on total wall time and total CPU time, but the acceptance has no output-bytes case and no per-run CPU-time ceiling case"
- file: .engineering/planning/story/action-limits-enforced.md
  line: 50
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'Under the linux adapter or the Substrate daemon' lets the list pass on one path while the other fails and does not say whether both must hold"
- file: .engineering/planning/story/limit-hits-in-run-log.md
  line: 20
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome lists an egress-bytes kind and a measured peak but no acceptance bullet produces an egress-byte LimitHit or checks a peak"
- file: .engineering/planning/story/executable-allow-list.md
  line: 60
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet 'Under an adapter that proves the executable fact ... tries to exec a denied one fails' has no adapter that can satisfy it on the Linux gate at Substrate 0.21.0 and is not marked conditional, so the story cannot close while upstream-blocker:substrate-executable-allow-list is open"
- file: .engineering/planning/story/confinement-level-required.md
  line: 50
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance names no concrete level or fact: 'a level whose facts linux does not prove' and 'naming the missing fact' cannot be turned into a test, and the only level named is the adapter name linux"
- file: .engineering/planning/story/confinement-level-required.md
  line: 14
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome and ESS add Pod and the daemon as openable but no acceptance bullet shows either, the 'qualification record instead' in Out of scope has no check, and the oci bullet may be ignored so the gate can pass without it"
- file: .engineering/planning/story/agent-session-through-substrate.md
  line: 22
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says the harness runs under the action's limits and executable policy, but the acceptance checks the wall limit only and has no executable-policy case"
- file: .engineering/planning/story/substrate-daemon-runner.md
  line: 18
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises an HTTPS endpoint and starting, observing, signalling and stopping every child, but the acceptance runs a local socket only, has no signal, stop or test-runner bullet, and leaves 'the SDK at the pinned release' unnamed"
- file: .engineering/planning/story/tree-daemon.md
  line: 18
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says tree open, close and verify reach a tree through the daemon and a waiting tree uses no run slot, but no bullet shows either and 'the third starts when one waits or ends' passes when none waits"
- file: .engineering/planning/epic/tree-daemon-under-enforced-limits.md
  line: 70
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance is one paragraph of about seven independent outcomes, names no fixture trees, compares a process to a connection in the ps clause, and gives no test setup for the second host"
```
