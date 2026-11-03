---
format: aep.planning-md/3
id: story:forced-call-asks-for-one-call
kind: story
status: implemented
title: Every forced selection and argument request asks the model for one call
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:O1
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T06:23:11Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-03T06:23:11Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-03T06:41:45Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

Every forced request of the model selector and argument generator (`select_action`,
`action_arguments`, built by `model_selector::request`) asks the model for one call:
`controls.parallel_tool_calls` is `Some(false)`, which llm's Messages codec sends as
`tool_choice.disable_parallel_tool_use: true`. An answer that still calls the tool more than once
is taken when every call carries the same arguments; calls whose arguments differ stay unusable,
and none of them is taken.

## Why

A downstream adopter's first live agent on Loom 0.55.0 (`inbound-answer@1`, a `messages` route to
an Anthropic model through llm-gateway) ended with exit 4, "the model called `action_arguments`
more than once", on every run with one model and on runs with a second (item D409, 2026-11-03).
Cause, verified in code: `request()` set `ToolChoice::Named` and left `parallel_tool_calls`
unset, so llm 0.8.1's Messages codec (`crates/llm-messages/src/codec.rs:194-199`) sends no
`disable_parallel_tool_use`; llm-gateway passes the control through on a Messages route to an
invoke target and drops it only for Converse. Whether the model's two calls in those runs carried
the same arguments is not recorded.

## ESS first

No change to `ess/`: the forced request and the reading of its answer are executor code that no
ESS command declares. The red test is
`crates/loom-executor/tests/forced_single_call.rs` (4 tests, 0 passed on the base 00dc7fe8).

## Acceptance

- `forced_single_call.rs::a_forced_request_asks_for_one_call`
- `forced_single_call.rs::the_selection_and_the_arguments_requests_each_ask_for_one_call`
- `forced_single_call.rs::calls_that_carry_the_same_arguments_are_one_answer`
- `forced_single_call.rs::calls_that_differ_are_unusable_and_none_is_taken`

## Out of scope

A live run against a provider; the Converse wire, which drops the control in llm-gateway.
