---
format: aep.planning-md/3
id: review-result:adv-w11-u1-pass-1
kind: review-result
status: active
title: Adversary review, loom w11 U1 run model meter, pass 1
relations:
- reviews: story:dw-run-model-meter
revision: 1
---
needs-revision, then fixed in a follow-up unit

Adversarial pass on story:dw-run-model-meter (`crates/loom-executor/src/metering.rs`), cases in
`crates/loom-executor/tests/adversary_w11_u1_metering.rs`: 10 cases, 4 red.

- A `ModelPort::turn` that unwinds was never recorded, so the reported calls fell below the turns
  the port saw (reachable where a delegate's panic is contained and the run goes on). Fixed: the
  call is counted before the turn.
- Under a rate card, a failed turn left its step and the run at cost 0. Decided: a failed turn's
  cost is unknown, so the cost is absent, as for an answer without usage. Fixed so.
- Overlapping executor calls on one meter put turns in the wrong steps, and nested metered
  executors opened two steps for one call. Nothing in the repository does either. The contract is
  documented (calls on one meter do not overlap; one `MeteredExecutor` per executor), and both
  cases are ignored, naming story:meter-overlapping-calls.

Attacked and held: forked and fork-of-fork turns, answers without usage, a step mixing priced and
unlisted models, calls outside an executor call in step 0, a step closing on unwind, saturation at
`u64::MAX`.

```findings
- file: crates/loom-executor/src/metering.rs
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: a turn that unwinds was never counted; fixed by counting before the turn
- file: crates/loom-executor/src/metering.rs
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: a failed turn under a rate card reported cost 0 instead of absent; fixed
- file: crates/loom-executor/src/metering.rs
  category: concurrency
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: overlapping executor calls on one meter count into the wrong steps; contract documented, story:meter-overlapping-calls drafted
- file: crates/loom-executor/src/metering.rs
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: nested metered executors open two steps for one call; contract documented, story:meter-overlapping-calls drafted
```
