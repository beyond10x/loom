---
format: aep.planning-md/3
id: review-result:adversary-w26-loom-runtime-merge-pass-1
kind: review-result
status: active
title: Wave 2026-10-05-w26 adversary, loom story:runtime-merge, pass 1
relations:
- reviews: story:runtime-merge
revision: 1
---
```
unit: loom/runtime-merge — impl/runtime-merge at 6998522 plus the uncommitted phase-2 tree
verdict: red
cases: executed 998→1002, red 4
origin: introduced 4 / pre-existing 0 / undecided 0
wrote-outside-worktree: ~/.cache/ga-wave-2026-10-05-w26/loom-runtime-merge/scratch/{adversary1-*.log, gate-case.rs, base-9f06e0b/}; cargo clean -p of workspace packages in the brief build dir after a base build polluted it
needs-coordinator: yes
```

Cases added: `crates/intake-cli/tests/slice_run.rs` (`adversary_the_gate_stops_before_the_budget_on_the_last_step`), `crates/commission-testkit/tests/adversary_runtime_effect.rs` (3 cases). Suite: 998 passed, 4 failed (these cases), 4 ignored.

| # | file:line | verdict / origin | severity | measured | reaches it |
|---|---|---|---|---|---|
| 1 | crates/commission/src/runtime.rs:391 | NEEDS-CHANGE / introduced | blocker | last budgeted step leaving the merge gate unchanged gives `stopped: StepBudget`; base 9f06e0b gives `ApprovalRequired` | `b10x-intake run --max-steps N`; exit 0 becomes 3 |
| 2 | crates/commission/src/runtime.rs:520 | NEEDS-CHANGE / introduced | warning | a port with `performs("deploy") == false` is handed an admitted `deploy` | any port trusting `performs` |
| 3 | crates/commission/src/runtime.rs:422 | CONFIRMED / introduced | note | a Refused effect plus a foreign move 5→9: the runtime adopts 9 and invokes again | nothing found in-tree |
| 4 | crates/commission/src/runtime.rs:492 | CONFIRMED / introduced | note | without a budget, an effect that moves the case keeps the loop going (25 executor calls at the test cap) | nothing found: the slice passes `Some(max_steps)` |
| 5 | AGENTS.md:137 | NEEDS-CHANGE / introduced | note | AGENTS.md edited outside typed scope | brief rule |

Not broken: `AdmittedRequest` construction outside the runtime; repository.merge never runs; `EffectError` not swallowed; evidence only from `TestResultVerifier`; no-Canon and deny guards; budget 0; no call between revalidate and invoke; adapted tests not weakened.

```findings
[
{"file":"crates/commission/src/runtime.rs","line":391,"category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The step budget is checked before the approval gate, so a last budgeted step that leaves the gate unchanged stops StepBudget (exit 3) instead of ApprovalRequired (exit 0), breaking the 7dc84ef gate-stop rule the acceptance requires."},
{"file":"crates/commission/src/runtime.rs","line":520,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"An admitted request is handed to EffectPort::invoke even when the port's performs() says it does not perform that action."},
{"file":"crates/commission/src/runtime.rs","line":422,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"After any effect, a Refused one included, the runtime adopts whatever revision the case is at, so a foreign move is taken as the Run's own and a second request is invoked at it."},
{"file":"crates/commission/src/runtime.rs","line":492,"category":"property","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Without a step budget, an effect that moves the case makes every identical proposal a new request, so the idle bound never ends the loop; no production caller passes None."},
{"file":"AGENTS.md","line":137,"category":"judgement","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"Phase 2 edits AGENTS.md, which is outside the story's typed scope; the brief routes such a change to a scratch patch."}
]
```
