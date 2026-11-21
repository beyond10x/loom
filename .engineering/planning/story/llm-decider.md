---
format: aep.planning-md/3
id: story:llm-decider
kind: story
status: proposed
title: An LLM decider answers one forced tool call over an llm Model, never outside the offered options and sources
summary: ModelDecider in loom-executor; citations only as offered; scripted Model in the test file
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decider-port-and-grounding-check
scope:
- confidence: inferred
  path: crates/loom-executor/Cargo.toml
- confidence: inferred
  path: crates/loom-executor/src/lib.rs
- confidence: inferred
  path: crates/loom-executor/src/model_decider.rs
- confidence: inferred
  path: crates/loom-executor/src/model_selector.rs
- confidence: cited
  path: crates/loom-executor/tests/llm_decider.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:20Z", actor: "agent:sdc", revision: 8, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`ModelDecider` in `b10x-loom-executor` implements the `Decider` port of `b10x-loom-decision` over
an llm `Model`, as `ModelSelector` implements `ActionSelector`
(`crates/loom-executor/src/model_selector.rs:1-28`, `:187-235`). Its code goes in its own module,
`model_decider`, of `crates/loom-executor`. Per decision it sends one request that forces one call
of one named tool, `decide`, whose schema is `DeciderReply`; the model is told the question, the
options with their consequences, the subject at its revision, the recorded recommendation where
there is one, and the offered sources, each with its citation. The reply is taken only as the tool
call: text beside it is dropped. Each ground of the model's reply carries a citation; `ModelDecider`
accepts a ground only when its citation equals an offered source's citation, and the reply's ground
carries the offer's citation (with its digest), never the model's text. A reply is `InvalidReply`
when it names an option the request does not have, cites a citation that was not offered (a
rewritten path or line range included), answers `Take` where no recommendation is recorded, or
carries calls with different arguments. The `Take` check is the decider's: the grounding check
never sees a `Take` without a recommendation from a `ModelDecider`
(`story:decider-port-and-grounding-check` keeps its refusal for other deciders). The decider's
principal is `decider:<kind>`, `<kind>` given by the host; the model never supplies a principal,
an instant, a case revision or a strength criterion. The model comes from the host's routes as any
other Loom model (`epic:model-routes`). The decider's cost on its answer is
`story:escalate-when-conditions`'s; this story returns the reply only.

## Why

Item 1 of the request: "A decider executor port: given a pending decision and its inputs, return
an option, reasons and grounds (rule or LLM)" (`~/beyond10x/sdc/docs/decisions.md:301-303`).
SDC's decider is `tools/decider.sh` with Haiku (`~/beyond10x/sdc/docs/autonomy-levels.md:203`).
An LLM decider lives where `llm-core` is already a dependency: `b10x-loom-executor`
(`crates/loom-executor/Cargo.toml:11`), never Commission (`AGENTS.md:96-97`). Every answer goes
through the grounding check; the model is not trusted context (`AGENTS.md:91`).

## Waits on

`story:decider-port-and-grounding-check`: the port and `DeciderReply`.

## ESS first

No new noun: the tool's schema is the projection of `loom.decision.DeciderReply`, declared by
`story:decider-port-and-grounding-check`. The red test is
`crates/loom-executor/tests/llm_decider.rs`, which does not compile before `ModelDecider` exists.

## Acceptance

Every test uses a scripted `Model` the test file defines, which replays fixed turns and keeps the
requests it was sent, as `crates/loom-executor/tests/forced_single_call.rs:25-62` does; none calls
a model or the network.

- A scripted `Propose` of option `A` with two grounds citing offered sources gives that reply,
  and the request sent carries exactly one tool, `decide`, with `ToolChoice::Named`.
- A scripted call naming option `D` of a request with `A` to `C`, one citing a citation that was
  not offered, and one answering `Take` with no recorded recommendation are each `InvalidReply`
  naming what was wrong.
- A scripted answer with two `decide` calls carrying different arguments is `InvalidReply`; two
  identical calls are one reply.
- Text beside the call ("I approve") changes nothing in the reply.
- A scripted `Escalate` with `best: B` gives `Escalate` with `best` `B`.
- A model failure is `Unavailable` and never a reply.
- A scripted reply whose ground repeats an offered citation gets that offer's citation and digest;
  one whose ground rewrites the offered line range is `InvalidReply`.
- `task check` exits 0.

## Out of scope

Decider prompts per class (SDC data, `autonomy-levels.md:481`). Confining the decider. Retrying
a failed call (the host retries, as for the selector). Recording the call in a run log. The
decider's cost and usage (`story:escalate-when-conditions`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-executor`, a new decider module plus one test file. Cited: the story names `b10x-loom-executor` and `crates/loom-executor/tests/llm_decider.rs`.
- **Files, new:** `crates/loom-executor/tests/llm_decider.rs`. Cited: the story's "ESS first" red test.
- **Files, new:** `crates/loom-executor/src/model_decider.rs`. Inferred: the story gives no module path. `story:non-independent-review-becomes-decision-request` (body, "Files") calls it "the `ModelDecider` module of `crates/loom-executor`".
- **Files, edited:** `crates/loom-executor/src/lib.rs`. Inferred: gains `pub mod model_decider;` next to `pub mod model_selector;` (`lib.rs:34-40`). The module list in the crate doc (`lib.rs:24-29`) probably gains a line too.
- **Files, edited:** `crates/loom-executor/Cargo.toml`. Inferred: gains `b10x-loom-decision = { path = "../loom-decision" }`. Today it has `llm-core` at `:11` and no decision crate. `crates/loom-decision` does not exist in the tree yet (`story:decision-request-entity` creates it).
- **Files, possibly edited:** `crates/loom-executor/src/model_selector.rs`. Inferred. `answer` (`model_selector.rs:362-396`) returns only `Value`: it drops the `TurnOutcome` and its `observation`, which is where usage lives (`metering.rs:313`). The story's "usage returned beside the reply" needs either a sibling of `answer` there that also hands back the outcome, or a copy of its logic in the new module. `request` (`:328`), `ask`, `Discard` and `Briefing::send` are reusable as they are.
- **Symbols:** `ModelDecider` (new), `Decider`, `DeciderReply` and `DeciderFailure::{Unavailable, InvalidReply}` (from `b10x-loom-decision`, not yet existing, declared by `story:decider-port-and-grounding-check`). Cited.
- **Symbols, reused:** `model_selector::{request, answer, Briefing, AskError, Discard}` and `llm_core::{Model, ToolChoice::Named, ToolName, TurnOutcome}`. Cited: `model_selector.rs:42-48,305-396`. The reuse is inferred.
- **Symbols, reused:** the `decide` tool, `ToolChoice::Named`, `parallel_tool_calls: Some(false)`, and equal-calls-count-once. `answer` already does the last (`:383-391`). Cited.
- **Also likely:** none. The story creates no ESS noun: the schema projects `loom.decision.DeciderReply`, declared by the port story. Cited.
- **Documents:** none beyond the house files. Cited: no acceptance bullet is a document.
- **Confidence:** medium. The crate, the test path and the pattern to copy are cited. The module file name, the `Decider` trait signature (sync or async, return type, where usage goes) and the `DeciderReply` ground shape come from stories 3 and 4, which are still drafts.
- **Would collide with:**
  - `crates/loom-executor/src/lib.rs`, `crates/loom-executor/Cargo.toml`, `crates/loom-executor/src/model_decider.rs` and `crates/loom-executor/tests/model_decider_identity.rs`: `story:non-independent-review-becomes-decision-request` extends `ModelDecider` with its identity report (body "Files"; "Waits on"). Ordered: a `depends_on` path exists from it to this story (`non-independent-review…` → `decision-routing-by-holder` → `decider-port-and-grounding-check` → `autonomy-grant-table`). The chain I walked from it reached `story:llm-decider` (the reachability check came back non-null), but I did not print the intermediate edge.
  - `crates/loom-executor/src/model_decider.rs`: `story:escalate-when-conditions` has `ModelDecider` gain the cost on its answer (body, "Files"; "Waits on"). Ordered: direct `depends_on story:llm-decider`.
  - `story:approval-decisions-resume-held-runs` builds a `ModelDecider` for `--decider ROUTE` (body line 121). It lands in `loom-cli`, not in the executor. Ordered: direct `depends_on story:llm-decider`.
  - `crates/loom-decision/**`: this story only reads it. `story:decider-port-and-grounding-check`, `story:autonomy-grant-table` and `story:decision-request-entity` write it. Ordered: this story depends on the port story, which depends on the other two.
  - `model_selector.rs`: no other undelivered story names it. `grep` over `.engineering/planning/story` found only this story among non-implemented ones.
  - `crates/loom-executor/Cargo.toml`: no other undelivered story scopes it. The three that do (`llm-routes-pin`, `cli-catalog-model-route`, `llm-credentials-bearer`) are implemented.
- **Safety fact:** adding `b10x-loom-decision` to the executor's dependencies breaks no layering test. `commission_import.rs` and `crate_names.rs` check Commission's crates and the workspace package set, not the executor's dependency list. `loom-decision`'s own manifest must name no executor, which is the port story's acceptance. Pointed at those test files, unproven. One test still needs a new row: `crates/loom-executor/tests/crate_names.rs:18` (`EXPECTED: [..; 29]`) must include `loom-decision`, and `story:decision-request-entity` owns that edit.
