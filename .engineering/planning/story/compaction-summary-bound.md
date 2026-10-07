---
format: aep.planning-md/3
id: story:compaction-summary-bound
kind: story
status: active
title: A compaction never leaves the session above its target
relations:
- decomposes: epic:loom-native-harness
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-executor/src/harness/turn_loop/mod.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w3_compaction_contract.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T02:51:19Z", actor: "human:timo", revision: 6, executor: "agent:loom", correlation: "wave/2026-10-07-w2"}
- {from: "proposed", to: "active", at: "2026-10-07T02:51:19Z", actor: "human:timo", revision: 7, executor: "agent:loom", correlation: "wave/2026-10-07-w2"}
---
## Outcome

A compaction never leaves the session larger than before it, and never above the target it
compacts to. The ported loop folds any non-empty summary
(`crates/loom-executor/src/harness/turn_loop/mod.rs:3092` at `b9efe09`), so a summary longer than
the items it replaces grows the conversation: adversary pass 1 of wave 2026-10-06-w3 measured
14,596 bytes before and 17,170 after, 4,292 tokens of a 4,000-token window, and the next request was
still sent (`review-result:adversary-w3-loom-compaction-contract-pass-1`). The same guard and fold
are in the base the wave started from (`268d37b`), so the defect predates `story:compaction-contract`.

## Acceptance

`crates/loom-executor/tests/adversary_w3_compaction_contract.rs`
`adversary_w3_a_summary_longer_than_the_target_leaves_the_session_above_half_the_window` today
asserts the current outcome (above half the window) and names this story; it is flipped to assert
that after compaction the session is at or below 50 % of the declared window, and passes.

## Options (to settle when the story is proposed)

Settled 2026-10-07, before proposing: the first option. A summary that does not shrink the items it
would replace is treated as a failed summary (`Summarised::Failed(reported)`): the items are elided
instead, as the byte rule does without a window, and the usage the endpoint reported for the summary
request is still recorded. Only this option bounds the result in every case; capping the summary
request's output tokens (the second option) bounds tokens, not the bytes the fold adds, so it is not
taken.

## Reach

No caller or configuration shown to produce it: the summary request is limited only by
`max_output_tokens_per_turn`, which defaults to `None`, and needs a model that writes more summary
than it was asked to fold.

## ESS first

No specification change. The compaction record (`loom.run.RecordCompaction`, `loom.run.ReportedUsage`
in `ess/domains/run.yaml`) keeps its shape; what changes is the ported loop's fold rule, which no ESS
declaration states. The red test is the existing adversary case, flipped first:
`adversary_w3_a_summary_longer_than_the_target_leaves_the_session_above_half_the_window` in
`crates/loom-executor/tests/adversary_w3_compaction_contract.rs` asserts the bound and fails on that
commit; the fix in `crates/loom-executor/src/harness/turn_loop/mod.rs` makes it pass.
