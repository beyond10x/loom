---
format: aep.planning-md/3
id: review-result:loom-on-llm-and-metaharness-acceptance-r2
kind: review-result
status: active
title: 'Acceptance critic, round 2: epic loom-on-llm-and-metaharness and its drafts'
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

story:engineering-protocols-090 — the second bullet, "`git grep -n '0\.8\.0'` finds no engineering-protocols reference left in `crates/`, `website/`, `AGENTS.md` or `README.md`", cannot be met as scoped: the grep also hits four doc comments the story never lists, `crates/loom-governor/tests/canon_decision_bytes.rs:8`, `governed_case.rs:9`, `incident_response_slice.rs:23` and `website/docs/guides/move-to-loom.md:49`. It also hits Loom's own release number (`README.md:69`) and `website/package-lock.json`, so the bullet needs the exact allowed hits or a narrower pattern — .engineering/planning/story/engineering-protocols-090.md:61
story:pending-effect — no bullet says what the intake slice does with the new "waiting" outcome. The slice's `RunOutcome` match has no wildcard arm (`crates/loom-intake-slice/src/run.rs:866-916`), so the outcome must map to some `StopReason`. The sibling stories `story:refused-effect-ends-run` and `story:idle-bound-outcome` name that mapping and a unit test for it, and this story's budget-reached bullet checks only the outcome the runtime returns — .engineering/planning/story/pending-effect.md:53
story:delete-harness-port — the Outcome requires `Breaking:` to name the eight ports, "the other `loom_sdk::loom::harness` paths the page names" and the two model ports. The acceptance bullet "For each of the ten names above, `grep -F` finds it" counts only eight plus two. The other paths (the `turn_loop` and `wire` rows at `website/docs/reference/sdk-stability.md:143-163`) are never checked in the entry, so the Outcome's third group is unobserved — .engineering/planning/story/delete-harness-port.md:84
story:metaharness-session-executor — the bullet "No package in metaharness's resolved graph is a Loom package (`cargo metadata` in metaharness)" is read in another repository and gives the same answer before and after this story. No bullet observes the Outcome's own transition, that Loom's graph gains metaharness (`Cargo.lock` has 0 matches today); a `cargo tree --locked -i` of metaharness in Loom's workspace would show it — .engineering/planning/story/metaharness-session-executor.md:70

What I read: all 19 ids (the 17 draft stories, `upstream-blocker:takt-session-executor-crate` and the epic), each file in full. I also ran `aep plan artifact lifecycle upstream-blocker`, `list --kind story --status draft`, and `git grep` on the named tests, paths and greps (the cited test names and Cargo.lock's Canon 0.6.0 resolve).

What I could not establish:
- The new test files named in the acceptances do not exist yet, so I could not check that they fail first.
- Whether the Laya selector is covered by `git grep -n 'impl.*ActionSelector for'` in `story:metering-on-llm-model`. It lives outside the two directories searched, and I treated that as intended.
- Out of my lane, not counted in the verdict: `story:chooser-executor-for-plugin-turns` and `story:metering-on-llm-model` both edit the selector under `crates/loom-executor/src/`. That is for the parallel-safety critic.

Round 1 findings that no longer hold:
- The Canon bullet now names its `cargo tree` result.
- The governor method is named, with three tests.
- The refused-effect and idle-bound mappings are named.
- The Python writable set is observed.
- The metering story carries the selector, and the one-selector grep shows the change.
- The compaction, session, recovery and credentials modules are named in the deletion check.
- The upstream blocker has a "Clears when".

```findings
- file: .engineering/planning/story/engineering-protocols-090.md
  line: 61
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet 'git grep -n 0\\.8\\.0 finds no engineering-protocols reference left in crates/, website/, AGENTS.md or README.md' cannot be met as scoped: the grep also hits four doc comments the story does not list (crates/loom-governor/tests/canon_decision_bytes.rs:8, governed_case.rs:9, incident_response_slice.rs:23, website/docs/guides/move-to-loom.md:49) and Loom's own release number (README.md:69) and website/package-lock.json, so the bullet needs the exact allowed hits or a narrower pattern"
- file: .engineering/planning/story/pending-effect.md
  line: 53
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet says what the intake slice does with the new waiting outcome, although its RunOutcome match has no wildcard arm (crates/loom-intake-slice/src/run.rs:866-916); the sibling stories name the StopReason mapping and a unit test, and this story's budget-reached bullet checks only the runtime's outcome"
- file: .engineering/planning/story/delete-harness-port.md
  line: 84
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome requires Breaking: to name the eight ports, the other loom_sdk::loom::harness paths the page names and the two model ports, but 'For each of the ten names above, grep -F finds it' counts only eight plus two, so the other paths (website/docs/reference/sdk-stability.md:143-163) are never checked in the entry"
- file: .engineering/planning/story/metaharness-session-executor.md
  line: 70
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet 'No package in metaharness's resolved graph is a Loom package (cargo metadata in metaharness)' is read in another repository and is the same before and after this story; no bullet observes the Outcome's transition that Loom's graph gains metaharness (Cargo.lock has 0 matches today)"
```
