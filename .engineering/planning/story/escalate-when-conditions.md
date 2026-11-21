---
format: aep.planning-md/3
id: story:escalate-when-conditions
kind: story
status: proposed
title: A run's escalate_when conditions, checked by a program after the decider replies, send the result to a person or end the run
summary: 'Closed set of four conditions: decider escalate, decider invalid, paths_touched, decision_cost_usd_over; touched paths from the host; no free expressions'
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decision-routing-by-holder
- depends_on: story:llm-decider
scope:
- confidence: inferred
  path: crates/loom-decision/src/autonomy.rs
- confidence: inferred
  path: crates/loom-decision/src/escalate_when.rs
- confidence: cited
  path: crates/loom-decision/src/lib.rs
- confidence: inferred
  path: crates/loom-decision/src/routing.rs
- confidence: cited
  path: crates/loom-decision/tests/escalate_when.rs
- confidence: inferred
  path: crates/loom-executor/src/model_decider.rs
- confidence: inferred
  path: crates/loom-executor/tests/llm_decider.rs
- confidence: cited
  path: ess/domains/autonomy.yaml
- confidence: cited
  path: ess/domains/decision.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:20Z", actor: "agent:sdc", revision: 8, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

A run's autonomy may list `escalate_when` conditions, and the router applies them after the
decider replies and before any `Decide` or `TakeRecommendation` by the decider. The list is part
of the run's autonomy input beside the overrides, kept in the grant table and in
`AutonomyResolved`; an empty list is not written, so the record of a run without one keeps the
bytes `story:autonomy-grant-table` gives it. Each entry is a single-key condition from a closed
set, as session `sdc` answered on 2026-11-21 (from `~/beyond10x/sdc/docs/autonomy-levels.md`
§ 3.1, `escalation.escalate_when`):

- `decider: escalate`: the decider answered `Escalate`;
- `decider: invalid`: the decider failed (`InvalidReply` or `Unavailable`);
- `paths_touched: [<glob>, …]`: one of the touched paths of the subject that the host handed in
  matches a glob;
- `decision_cost_usd_over: <number>`: the decider's cost for this decision is above the number.

A list with any other key, an entry with two keys, a key with a value of the wrong type, an empty
glob, a glob with a character other than literal path characters, `/`, `*` and `**`, or a cost
that is negative, not finite or not a number is refused `BadCondition` at resolution, naming the
entry. There are no free expressions.

Globs: `*` matches within one path segment and never `/`; `**` as a whole segment matches zero or
more segments; every other character matches itself. `?`, `[`, `]`, `{` and `}` are refused. The
module matches them itself, about thirty lines, so `crates/loom-decision/Cargo.toml` gains no glob
crate (`globset` is in `Cargo.lock:2825` only as a transitive dependency, and its `*` crosses `/`
unless configured).

Cost: the decider's reply carries an optional `cost_micro_usd`, a non-negative integer.
`ModelDecider` (`story:llm-decider`) takes it from the llm usage's `cost_micro_usd`, as the
workflow executor reads it (`crates/loom-executor/src/workflow.rs:1687`), clamping a negative
figure to 0 as the executor does (`:1675`). `decision_cost_usd_over: X` holds when the reply's
`cost_micro_usd` is strictly above `X × 1 000 000` rounded toward zero; a cost equal to it does not
hold. A reply that reports no cost counts as above any threshold: an unknown fails toward less
authority (`AGENTS.md` § Rules). A decider failure carries no reply and no cost; the cost
condition is not evaluated on it, and `decider: invalid` governs it.

Touched paths: the host hands in the touched paths of the subject in the `DecisionStop` as an
optional list; SDC takes them from the story diff, and later from the exec file-change record
(answer of session `sdc`, 2026-11-21). An absent list (the host does not know) counts as matching
any `paths_touched` condition, failing toward less authority; an empty list (the host knows nothing
was touched) matches none.

The conditions apply to the decider's reply under a class whose holder involves the decider:
`decider`, `decider_final`, `rule_and_decider` and `rule_and_decider_final`. Under `person` the
router calls no decider and the recommendation is a reused prior decision
(`story:decision-routing-by-holder`); no condition is evaluated on it, nor under `advised`, `rule`
or `rule_final`. When a condition holds, the decider's result becomes an `Escalate`: under a
non-final holder the request is escalated to the escalation person, as the routing story
escalates; under a final holder the run ends instead
(`~/beyond10x/sdc/docs/autonomy-levels.md:318-320` and `:470`): the request stays Open, the route
answers `RunEnds`, and the `EscalationSuppressedRecord` carries the cause `condition` and names the
entry that held in its reasons. A condition that holds under a final holder therefore ends the run
where, with no condition listed, the routing story would have decided `best` or the denying
option. Under `rule_and_decider*` the decider's result is its `Take` or veto: a condition that
holds turns a `Take` into an `Escalate` as well. What the host then does on `RunEnds` is the host
stories' (`story:decision-routing-by-holder`).

Its code goes in its own module of `crates/loom-decision`, `escalate_when`, with one `mod` line in
`lib.rs`; the routing module calls it after the decider replies, and the autonomy module carries
the list in the resolver input, the grant table and `AutonomyResolved`. In `crates/loom-executor`,
`ModelDecider`'s module sets the cost on its reply.

## Why

`autonomy-levels.md` § 2.3, rule 5: "A class decided by the decider is sent to a person when a
deterministic condition holds … At A5 such a condition ends the run instead"
(`autonomy-levels.md:318-320`). Session `sdc` gave the written form on 2026-11-21: the four keys of
`autonomy-levels.md` § 3.1, `escalation.escalate_when`, evaluated by a program after the decider
replies and before `Decide`.

## Waits on

`story:decision-routing-by-holder`: the router this extends, and both change
`ess/domains/decision.yaml` and `crates/loom-decision`. `story:llm-decider`: `ModelDecider` gains
the cost on its reply. `story:autonomy-grant-table` through the first.

## ESS first

`ess/domains/autonomy.yaml` gains `EscalationCondition` (`DeciderEscalate`, `DeciderInvalid`,
`PathsTouched`, `DecisionCostUsdOver`) and the run's `escalate_when` list on the autonomy input,
the grant table and `AutonomyResolved`, with the refusal `BadCondition`.
`ess/domains/decision.yaml`: `DecisionStop` gains optional `touched_paths`, the decider's reply
gains optional `cost_micro_usd`, and `SuppressionCause` gains `condition`. On that commit
`task drift` fails, `crates/loom-decision/tests/escalate_when.rs` does not compile, and the new
cost test in `crates/loom-executor/tests/llm_decider.rs` fails (`ModelDecider`'s reply carries no
cost).

## Acceptance

In `crates/loom-decision/tests/escalate_when.rs`, with the fixtures of
`story:decision-routing-by-holder`:

- At `A3` on `roadmap` with `paths_touched: ["**/auth/**"]`, a verified decider reply on a stop
  whose touched paths include `src/auth/login.rs` leaves the request Escalated to the escalation
  person; with touched paths `src/ui/list.rs` it is Decided by the decider.
- The same at `A5` leaves the request Open, answers `RunEnds`, and keeps one
  `EscalationSuppressedRecord` with cause `condition` naming the `paths_touched` entry.
- With `decision_cost_usd_over: 1.0` at `A3`, a reply reporting 1 500 000 micro-USD is Escalated.
- With `decision_cost_usd_over: 1.0` at `A3`, a reply reporting 1 000 000 micro-USD is Decided by
  the decider.
- With `decision_cost_usd_over: 1.0` at `A3`, a reply reporting 500 000 micro-USD is Decided by the
  decider.
- With `decision_cost_usd_over: 1.0` at `A3`, a reply reporting no cost is Escalated.
- With `decision_cost_usd_over: 1.0` at `A5`, a decider failure is Decided `B` with cause
  `failure`, not `condition`.
- At `A5` with `decider: escalate` listed, an `Escalate` with `best: A` ends the run and decides
  nothing; without it listed, the same reply is Decided `A`.
- At `A5` with `decider: invalid` listed, a decider failure ends the run with the request Open;
  without it listed, it is Decided `B` (the denying option).
- A run listing `paths_touched` and a stop whose `touched_paths` is absent escalates at `A3`.
- A run listing `paths_touched` and a stop whose `touched_paths` is empty is Decided by the decider
  at `A3`.
- `paths_touched: ["src/*.rs"]` matches `src/lib.rs` and does not match `src/auth/login.rs`;
  `paths_touched: ["src/**"]` matches both.
- At `A4` on `merge` (`rule_and_decider`) with `paths_touched: ["**/auth/**"]` and touched path
  `src/auth/login.rs`, a decider `Take` of the rule's verdict leaves the request Escalated and not
  Decided.
- At `A5` on `merge` (`rule_and_decider_final`), the same `Take` with the same touched path leaves
  the request Open, answers `RunEnds`, and appends one record with cause `condition`.
- With `paths_touched: ["**"]` listed and a matching touched path, at `A3` on `rollback` (`rule`) a
  verified rule proposal is Decided by `rule:stub`, at `A1` on `roadmap` (`advised`) the request
  waits on the person with the recommendation, and at `A0` on `roadmap` (`person`) with a reusable
  prior the request waits on the person with the prior's recommendation; none is Escalated.
- `AutonomyResolved` for a run listing `[{decider: escalate}, {paths_touched: ["**/auth/**"]}]`
  carries the list in that order, and the grant table answers it; for a run listing none, the
  record bytes equal those the fixture of `story:autonomy-grant-table` gives.
- A list with `when: "cost > 1"`, an entry `{decider: escalate, paths_touched: []}`,
  `paths_touched: [""]`, `paths_touched: ["src/?.rs"]`, `decision_cost_usd_over: -1` and
  `decision_cost_usd_over: .nan` are each refused `BadCondition` naming the entry.

In `crates/loom-executor/tests/llm_decider.rs`:

- A recorded llm turn whose usage carries `cost_micro_usd` 1 500 000 gives `cost_micro_usd`
  1 500 000 on `ModelDecider`'s reply, and one carrying -5 gives 0; no test calls a model.
- `task check` exits 0.

## Out of scope

Computing touched paths: the host hands them in. Conditions beyond the four keys. Escalation of
a rule's own result. Conditions on a reused prior decision. A cost on a decider failure.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-decision`, a crate no story has created yet. `story:decision-request-entity` creates it (`crates/loom-decision` absent from the tree at `61fd500b`). — cited
- **Files:** `crates/loom-decision/src/lib.rs` (one `mod` line) — cited
- **Files:** `crates/loom-decision/tests/escalate_when.rs` (the red test) — cited
- **Files:** `ess/domains/decision.yaml` (`DecisionStop.touched_paths`, answer `cost_micro_usd`, `SuppressionCause::condition`) — cited
- **Files:** `ess/domains/autonomy.yaml` (`EscalationCondition`, `escalate_when` on input/grant table/`AutonomyResolved`, `BadCondition`) — cited
- **Files:** `crates/loom-decision/src/escalate_when.rs` (the story's "own module"; the name is not given) — inferred
- **Files:** the routing module (`crates/loom-decision/src/routing.rs`, from `tests/routing.rs` in `story:decision-routing-by-holder`) — inferred. The router applies the conditions "after the decider replies", so a call into the new module needs an edit there. The story says only "its own module".
- **Files:** the autonomy module (`crates/loom-decision/src/autonomy.rs`, from `tests/autonomy.rs` in `story:autonomy-grant-table`) — inferred. The list is added to the resolver input, the table and `AutonomyResolved`.
- **Files:** `crates/loom-decision/Cargo.toml` — inferred. Glob matching needs a dependency. `globset 0.4.20` is in `Cargo.lock:2825` (transitive), and no crate manifest names it.
- **Files:** `crates/loom-executor/src/model_decider.rs` (`ModelDecider`'s cost) and `crates/loom-executor/tests/llm_decider.rs` — inferred. `story:llm-decider` names only the test file; the module name is a guess.
- **Symbols:** `EscalationCondition` (`DeciderEscalate`, `DeciderInvalid`, `PathsTouched`, `DecisionCostUsdOver`), `BadCondition`, `DecisionStop`, `SuppressionCause`, `EscalationSuppressedRecord`, `RouteOutcome::RunEnds`, `AutonomyResolved`, `ModelDecider` — cited
- **Symbols:** `crates/loom-executor/src/workflow.rs:1685-1688` (`usage.cost_micro_usd.map(count)`, an `Option<i64>` from llm usage) is the pattern to copy. It is read, not changed. — cited
- **Also likely:** `crates/loom-conformance/src/decision.rs` (named by `story:decision-request-entity`), only if a new `DecisionStop` field breaks it — inferred
- **Documents:** none beyond the house-rule files — cited (acceptance is all code and ESS)
- **Confidence:** medium. Both ESS files and `lib.rs` are named by the story. Every Rust file except `lib.rs` and the red test depends on module names that stories not yet delivered will choose.
- **Would collide with:** `ess/domains/decision.yaml` and `crates/loom-decision` (`lib.rs`, routing, autonomy modules). All of the following are ordered by `depends_on`:
  - `story:decision-request-entity`, `story:autonomy-grant-table`, `story:decider-port-and-grounding-check`, `story:decision-routing-by-holder`: ancestors. The chain is escalate -> routing -> decider-port -> grant-table -> entity.
  - `story:decided-record-written-then-recorded`: direct dependent. `silence-and-expiry-at-a-host-tick`, `approval-decisions-resume-held-runs`, `question-and-outcome-decisions-hold-runs`, `tree-waits-open-decision-requests` and `non-independent-review-becomes-decision-request` follow it. Each edits `decision.yaml` or the routing module and each is a transitive dependent.
  - No unordered collision found among the 20.
- **Would collide with:** `ess/domains/autonomy.yaml` and the autonomy module. Only `story:autonomy-grant-table` creates them, and it is an ancestor. — cited
- **Would collide with:** `crates/loom-executor` (`ModelDecider`, `tests/llm_decider.rs`). `story:llm-decider` is a direct `depends_on`, so ordered. — cited/inferred as above
- **Safety fact:** the conditions run only for the four decider-involving holders, and an unknown cost or touched-path list fails toward escalation, so a missing field never grants more authority. This is stated in the story (`AGENTS.md` § Rules, `AGENTS.md:93`, which says "fail toward less authority"). Level 1, stated, unproven. Holder `rule` and `rule_final` are not evaluated; that is a stub-count bullet, not proven in code.
