---
format: aep.planning-md/3
id: story:llm-decider
kind: story
status: draft
title: An LLM decider answers one forced tool call over an llm Model, never outside the offered options and sources
summary: ModelDecider in loom-executor; recorded responses only
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decider-port-and-grounding-check
revision: 2
---
## Outcome

`ModelDecider` in `b10x-loom-executor` implements the `Decider` port of `b10x-loom-decision` over
an llm `Model`, as `ModelSelector` implements `ActionSelector`
(`crates/loom-executor/src/model_selector.rs:1-27`). Per decision it sends one request that
forces one call of one named tool, `decide`, whose schema is `DeciderReply`; the model is told the
question, the options with their consequences, the subject at its revision, the recorded
recommendation where there is one, and the offered sources, each under an id. The reply is taken
only as the tool call: text beside it is dropped. A reply is `InvalidReply` when it names an option
the request does not have, cites a source that was not offered, answers `Take` where no
recommendation is recorded, or carries calls with different arguments. The decider's principal is
`decider:<kind>`, `<kind>` given by the host; the model never supplies a principal, an instant, a
case revision or a strength criterion. The model's usage is returned beside the reply. The model
comes from the host's routes as any other Loom model (`epic:model-routes`).

## Why

Item 1 of the request: "A decider executor port: given a pending decision and its inputs, return
an option, reasons and grounds (rule or LLM)" (`~/beyond10x/sdc/docs/decisions.md:301-303`).
SDC's decider is `tools/decider.sh` with Haiku (`~/beyond10x/sdc/docs/autonomy-levels.md:203`).
An LLM decider lives where `llm-core` is already a dependency: `b10x-loom-executor`
(`crates/loom-executor/Cargo.toml:11`), never Commission (`AGENTS.md:96-97`). Every answer goes
through the grounding check; the model is not trusted context (`AGENTS.md` § Rules).

## Waits on

`story:decider-port-and-grounding-check`: the port and `DeciderReply`.

## ESS first

No new noun: the tool's schema is the projection of `loom.decision.DeciderReply`, declared by
`story:decider-port-and-grounding-check`. The red test is
`crates/loom-executor/tests/llm_decider.rs`, which does not compile before `ModelDecider` exists.

## Acceptance

Every test uses a recorded `Model` that replays fixed turns; none calls a model or the network.

- A recorded `Propose` of option `A` with two grounds citing offered sources gives that reply,
  and the request sent carries exactly one tool, `decide`, with `ToolChoice::Named`.
- A recorded call naming option `D` of a request with `A` to `C`, one citing a source id that was
  not offered, and one answering `Take` with no recorded recommendation are each `InvalidReply`
  naming what was wrong.
- A recorded answer with two `decide` calls carrying different arguments is `InvalidReply`; two
  identical calls are one reply.
- Text beside the call ("I approve") changes nothing in the reply.
- A recorded `Escalate` with `best: B` gives `Escalate` with `best` `B`.
- A model failure is `Unavailable` and never a reply.
- A recorded reply whose ground cites an offered source gets that source's citation from the
  offer, not from the model's text: a reply that rewrites the offered path or line range is
  `InvalidReply`.
- `task check` exits 0.

## Out of scope

Decider prompts per class (SDC data, `autonomy-levels.md:481`). Confining the decider. Retrying
a failed call (the host retries, as for the selector). Recording the call in a run log.
