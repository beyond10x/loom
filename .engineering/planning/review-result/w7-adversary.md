---
format: aep.planning-md/3
id: review-result:w7-adversary
kind: review-result
status: active
title: Adversary review, wave 2026-10-21-w7, passes 1 and 2 at 4bfacfd and a083809
relations:
- reviews: story:stop-policy-gate-and-idle-bound
- reviews: story:run-budget-tokens-and-deadline
- reviews: story:pending-answer-ends-run
- reviews: story:first-declared-completion
revision: 1
---
Two adversary passes on wave 2026-10-21-w7 were green: 36 cases on the four units at 4bfacfd, then 21 cases on the deadline read after an executor call at a083809. Pass 1 found that a proposal from a call ending past the deadline was still performed; the wave fixed it (3e2712a) and pass 2 attacked the fix.

```
unit: wave 2026-10-21-w7 (story:stop-policy-gate-and-idle-bound, story:run-budget-tokens-and-deadline, story:pending-answer-ends-run, story:first-declared-completion)
verdict: PASS (pass 1 at 4bfacfd, pass 2 at a083809)
cases: pass 1 36 green, 0 red (fc0da5d8, merged c29004fe); pass 2 21 green, 0 red (2be0ffd, merged ad63859)
origin: introduced 9, pre-existing 3
```

## Cases

| File | Cases | Pass |
|---|---|---|
| `crates/loom-commission-testkit/tests/adversary_w20261021w7_runtime.rs` | 30 | 1 |
| `crates/loom-governor/tests/adversary_w20261021w7_completion.rs` | 6 | 1 |
| `crates/loom-commission-testkit/tests/adversary_w20261021w7_deadline_after_call.rs` | 21 | 2 |

## Reviewed and not faulted

- With the approval gate off, an approval-required action the provider withholds, errors on or panics on is never invoked: a fan-out, a pending request asked again, `resume_after_grant`, a kept step.
- The RFC 3339 parser: 16 malformed texts refused with `InvalidDeadline`, and offsets, fractions, leap days and leap seconds, pre-epoch and year 0000 and 9999 accepted at the right second.
- `max_tokens` sums executor and effect tokens and saturates; a charge equal to the limit is no breach.
- `DiscardStep` keeps the pre-wave behaviour; `ends_run` ordering and the Suspension release on resume.
- `FirstDeclared` follows declaration order per protocol, survives a `FileCaseStore` restart and leaves `evaluate` on `ExactlyOne`.
- Pass 2: the after-call deadline read on a step that ends the run, a fan-out, a pending request, against a kept spending breach, a reading that is not RFC 3339, `resume_after_grant` and a failed observation; no extra clock read without a deadline.

## Mutants killed

Found by reading the code, not run; each survives the units' own tests and is killed by an adversary case.

| Mutant | Location | Killed by |
|---|---|---|
| `now < deadline` to `<=` | runtime.rs:3261 | `adversary_a_deadline_equal_to_the_reading_is_reached` |
| deadline compared on whole seconds | runtime.rs:3261 | `adversary_a_sub_second_deadline_compares_its_fraction` |
| `charged <= max` to `<` | runtime.rs:3150 | `adversary_a_charge_equal_to_max_tokens_is_no_breach` |
| wrapping add in the token sum | runtime.rs `add_usage` | `adversary_max_tokens_saturates_near_the_maximum` |
| `Deadline` or `MaxTokens` row dropped from `LIMITS` | file_suspension.rs:308 | `adversary_the_new_limits_survive_the_file_suspension_store` |
| `kept_breach.is_none()` dropped | runtime.rs:1509 | deadline_after_call.rs:1207 |
| `observed.is_ok()` dropped | runtime.rs:1508 | deadline_after_call.rs:1567 |
| after-call check moved below the executor-`Suspended` branch | runtime.rs:1520 | deadline_after_call.rs:908 |

## Outcome

- The deadline finding of pass 1 was taken into the wave: spec at 82d3561, red at da5c818 (`a_deadline_passed_during_an_executor_call_discards_its_step`, invoked 1, expected 0), fixed at 3e2712a, merged a083809.
- The `**Breaking:**` bullet naming `StopPolicy.on_breach` was dropped from the CHANGELOG: the type is new in the same release.
- The other findings are documented semantics or pre-existing and are recorded, not changed.

```findings
[
{"file":"crates/loom-commission/src/runtime.rs","line":1473,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"pass 1: the deadline was read only before each executor call, so a proposal or fan-out from a call ending past it was still performed; fixed in 3e2712a"},
{"file":"crates/loom-commission/src/runtime.rs","line":1503,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"pass 1: under KeepStep, an executor step past a spending limit that returns Suspended ends with the executor's reason; the breach is not reported"},
{"file":"crates/loom-commission/src/runtime.rs","line":1724,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"pass 1: a pending read with ends_run beside a stale read ends the run CaseMovedOn; the pending read is named nowhere in the outcome"},
{"file":"crates/loom-commission/src/runtime.rs","line":1708,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"pass 1: in one fan-out a final refusal beside an ends_run pending read ends EffectRefused, not Waiting"},
{"file":"CHANGELOG.md","line":25,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"pass 1: the Breaking bullet for StopPolicy.on_breach named a type new in the same release; the bullet was dropped"},
{"file":"crates/loom-commission/src/runtime.rs","line":1270,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"pass 2: under KeepStep a step that proposes nothing on a case completed during the call ends suspended for the kept breach, not Completed; the same holds for a kept spending breach and the step budget"},
{"file":"crates/loom-commission/src/runtime.rs","line":1657,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"pass 2: while a deadline breach is kept the effect's spending check is skipped, so an effect that then passes max_tokens is reported as Deadline; nothing more is performed"},
{"file":"crates/loom-commission/src/runtime.rs","line":1506,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"pass 2: under DiscardStep the after-call deadline breach replaces the executor's own Suspended reason and a completion during the call, as a spending breach does"},
{"file":"crates/loom-commission/src/runtime.rs","line":3261,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the <= and whole-second mutants of deadline_reached survived the units' tests by inspection; killed by the equal-reading and sub-second cases"},
{"file":"crates/loom-commission/src/runtime.rs","line":3150,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the < mutant (breach exactly at max_tokens) survived the units' tests by inspection; killed by adversary_a_charge_equal_to_max_tokens_is_no_breach"},
{"file":"crates/loom-commission/src/runtime.rs","line":1509,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"dropping kept_breach.is_none() or observed.is_ok(), or moving the after-call check below the executor-Suspended branch, survived the fix's tests by inspection; killed by deadline_after_call.rs cases at 1207, 1567 and 908"},
{"file":"crates/loom-commission/src/runtime.rs","line":1482,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"pass 1: an executor reporting usage None is never charged, so max_tokens cannot stop it; a resumed request skips the trial-limit check; a deadline breach reports max and charged in whole seconds"}
]
```
