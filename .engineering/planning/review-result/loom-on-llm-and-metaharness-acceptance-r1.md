---
format: aep.planning-md/3
id: review-result:loom-on-llm-and-metaharness-acceptance-r1
kind: review-result
status: active
title: 'Acceptance critic, round 1: epic loom-on-llm-and-metaharness and its drafts'
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

story:engineering-protocols-090 — the Canon bullet is conditional on an answer the story says it does not have ("Whether it does: I don't know") and names no result, and "any protocol behaviour 0.9.0 changes is named in `CHANGELOG.md`" has no list of what changed to check against; the acceptance should state the `cargo tree --locked -i b10x-canon` result — .engineering/planning/story/engineering-protocols-090.md:35-39
story:governor-exposes-canon-decision — the acceptance never names the method, so "The method is reachable through `loom_sdk::governor`, and the stability page lists it" cannot be checked. Neither the Outcome nor the acceptance gives its name, and the page has a row per type or function, not per method — .engineering/planning/story/governor-exposes-canon-decision.md:40-41
story:refused-effect-ends-run — "The intake slice and `b10x-loom` map the outcome" and "the run's `LoopEnd` records the request and the effect" name no test, event field or exit status. The acceptance does not say what a host sees that it did not see before — .engineering/planning/story/refused-effect-ends-run.md:51-54
story:idle-bound-outcome — "The intake slice and `b10x-loom` map the new outcome" names no test, event or exit status, so the host-visible half of the transition (a host telling the idle bound from an empty frontier) has no check — .engineering/planning/story/idle-bound-outcome.md:51-52
story:python-test-profile — the Outcome promises "writes only where the profile allows", but no acceptance bullet names a writable path or a refused write. Today only `target` is writable (Outcome), so the Python change is unobserved — .engineering/planning/story/python-test-profile.md:36-39
story:reasoning-selector-on-llm-model — the Outcome says that where the moved llm-backed selector already holds the contract, "one of the two is removed", but no acceptance bullet observes that exactly one selector survives — .engineering/planning/story/reasoning-selector-on-llm-model.md:25-27
story:metaharness-session-executor — the acceptance never observes the Outcome's behaviour (a session in a scratch copy of the workspace returns its change as a `ProposedAction` that Commission revalidates and an effect applies). It names only a build, a graph check and an unnamed trace test — .engineering/planning/story/metaharness-session-executor.md:29-31,44-52
story:metaharness-session-executor — "builds in Loom's workspace from takt's source with no change to its public items" names no takt revision to diff against, and "its tests pass on recorded sessions" names no test, so neither bullet is checkable — .engineering/planning/story/metaharness-session-executor.md:44-45
story:delete-harness-port — the Outcome requires `Breaking:` to name the eight ports, every other `loom_sdk::loom::harness` path and the two model ports, but the acceptance only says `release-check` "passes with the entry". `release-check` refuses `Breaking: none.` and nothing else, so the names go unchecked (the epic's acceptance has the same sentence) — .engineering/planning/story/delete-harness-port.md:41-45,64
story:delete-harness-port — the Outcome deletes compaction, the session transcript, interruption recovery and wire credentials, but the acceptance checks only the `harness/` directory and the `harness::` grep. The `src/{compaction,credentials,recovery,session}.rs` modules in scope may survive and pass — .engineering/planning/story/delete-harness-port.md:37-39,56
story:delete-harness-port — "its model-call paragraphs describe llm's `Model` only" is a judgement with no grep or test behind it, unlike its neighbour about porting Harness — .engineering/planning/story/delete-harness-port.md:66-67
upstream-blocker:takt-session-executor-crate — the body is the title repeated, with no "Waiting on" or "Clears when". Nothing says what ends the block (which takt repository and ref, what "standalone" means), so it can only be asserted cleared. Compare `upstream-blocker:llm-catalog-model-port` — .engineering/planning/upstream-blocker/takt-session-executor-crate.md:11

What I read: 19 of 19 artifacts (the 18 stories and `upstream-blocker:takt-session-executor-crate`), plus the epic. I ran `aep plan artifact kinds`, `lifecycle story`, `lifecycle upstream-blocker`, `list --kind story --status draft`, and `aep plan artifact show` for the epic and the first three stories. I read the other files whole. I also read AGENTS.md § ESS, the `host_ports.rs` test bodies, and the cited sites (`README.md:33-34`, `plugin.rs:111`, `TURN_MODEL_BUDGET`, the `loom-executor/src` module list).

What I could not establish:
- Whether the new test files named in the acceptances can fail first. They do not exist, so I could not check them.
- Whether Canon moves with engineering-protocols 0.9.0. The story itself says it does not know, and nothing here shows it.
- Epic acceptance bullet 3 asks takt stories to "each name the Loom release that unblocked them", while "Changes inside takt" is out of scope; I judged neither as part of this set.
- Out of my lane, not counted in the verdict: `story:agent-executor-host-port` does not say whether `AgentExecutor` leaves the "Types and functions an embedding calls" table when it joins the ports table (`website/docs/reference/sdk-stability.md:138`), and `host_ports.rs` compares both tables exactly. That is for the design critic.

```findings
- file: .engineering/planning/story/engineering-protocols-090.md
  line: 35
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Canon bullet is conditional on an answer the story says it does not have and names no result, and 'any protocol behaviour 0.9.0 changes is named in CHANGELOG.md' has no list of what changed to check against; the acceptance should state the cargo tree -i b10x-canon result"
- file: .engineering/planning/story/governor-exposes-canon-decision.md
  line: 40
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance never names the method, so 'The method is reachable through loom_sdk::governor, and the stability page lists it' cannot be checked; the page has a row per type or function, not per method"
- file: .engineering/planning/story/refused-effect-ends-run.md
  line: 51
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'The intake slice and b10x-loom map the outcome' and 'the run's LoopEnd records the request and the effect' name no test, event field or exit status, so what a host sees that it did not see before is unobserved"
- file: .engineering/planning/story/idle-bound-outcome.md
  line: 51
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'The intake slice and b10x-loom map the new outcome' names no test, event or exit status, so the host-visible half of the transition has no check"
- file: .engineering/planning/story/python-test-profile.md
  line: 36
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises 'writes only where the profile allows' but no acceptance bullet names a writable path or a refused write, so the Python change to the writable set is unobserved"
- file: .engineering/planning/story/reasoning-selector-on-llm-model.md
  line: 25
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says one of the two llm-backed selectors is removed where the moved one already holds the contract, but no acceptance bullet observes that exactly one survives"
- file: .engineering/planning/story/metaharness-session-executor.md
  line: 44
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance never observes the Outcome's behaviour (a session in a scratch copy returns its change as a ProposedAction that Commission revalidates and an effect applies); it names only a build, a dependency-graph check and an unnamed trace test"
- file: .engineering/planning/story/metaharness-session-executor.md
  line: 44
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'builds in Loom's workspace from takt's source with no change to its public items' names no takt revision to diff against and 'its tests pass on recorded sessions' names no test, so neither bullet is checkable"
- file: .engineering/planning/story/delete-harness-port.md
  line: 64
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome requires Breaking: to name the eight ports, every other loom_sdk::loom::harness path and the two model ports, but the acceptance only says release-check passes with the entry, and release-check refuses only 'Breaking: none.', so the names go unchecked"
- file: .engineering/planning/story/delete-harness-port.md
  line: 56
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome deletes compaction, the session transcript, interruption recovery and wire credentials but the acceptance checks only the harness/ directory and the harness:: grep, so the src/{compaction,credentials,recovery,session}.rs modules could survive and pass"
- file: .engineering/planning/story/delete-harness-port.md
  line: 66
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'its model-call paragraphs describe llm's Model only' is a judgement with no grep or test behind it"
- file: .engineering/planning/upstream-blocker/takt-session-executor-crate.md
  line: 11
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the body is the title repeated, with no 'Waiting on' or 'Clears when', so nothing says what ends the block (which takt repository and ref, what standalone means) and it can only be asserted cleared"
```
