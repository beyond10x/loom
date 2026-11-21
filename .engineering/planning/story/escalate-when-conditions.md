---
format: aep.planning-md/3
id: story:escalate-when-conditions
kind: story
status: draft
title: A run's escalate_when conditions, checked by a program after the decider replies, send the result to a person or end the run
summary: 'Closed set of four conditions: decider escalate, decider invalid, paths_touched, decision_cost_usd_over; touched paths from the host; no free expressions'
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decision-routing-by-holder
- depends_on: story:llm-decider
revision: 2
---
## Outcome

A run's autonomy may list `escalate_when` conditions, and the router applies them after the
decider replies and before any `Decide` or `TakeRecommendation` by the decider. The list is part
of the run's autonomy input beside the overrides, kept in the grant table and in
`AutonomyResolved`. Each entry is a single-key condition from a closed set, as session `sdc`
answered on 2026-11-21 (from `~/beyond10x/sdc/docs/autonomy-levels.md` § 3.1, `escalation.escalate_when`):

- `decider: escalate`: the decider answered `Escalate`;
- `decider: invalid`: the decider failed (`InvalidReply` or `Unavailable`);
- `paths_touched: [<glob>, …]`: one of the touched paths of the subject that the host handed in
  matches a glob (`*` within one path segment, `**` across segments);
- `decision_cost_usd_over: <number>`: the decider's cost for this decision is above the number.

A list with any other key, an entry with two keys, a key with a value of the wrong type, or an
empty glob is refused at resolution, naming the entry. There are no free expressions.

The conditions apply to a class whose holder involves the decider: `decider`, `decider_final`,
`rule_and_decider` and `rule_and_decider_final`. When one holds, the decider's result becomes an
`Escalate`: under a non-final holder the request is escalated to the escalation person, as the
routing story escalates; under a final holder the run ends instead
(`~/beyond10x/sdc/docs/autonomy-levels.md:318-320` and `:467`): the request stays Open, the route
answers `RunEnds`, and the `EscalationSuppressedRecord` carries the cause `condition` and names
the entry that held in its reasons. A condition that holds under a final holder therefore ends the
run where, with no condition listed, the routing story would have decided `best` or the denying
option. Under `rule_and_decider*` the decider's result is its `Take` or veto: a condition that
holds turns a `Take` into an `Escalate` as well. What the host then does on `RunEnds` is the host
stories' (`story:decision-routing-by-holder`).

Its code goes in its own module of `crates/loom-decision`; `lib.rs` gains one `mod` line.

The host hands in the touched paths of the subject in the `DecisionStop`; SDC takes them from the
story diff, and later from the exec file-change record (answer of session `sdc`, 2026-11-21). A
stop with no touched paths under a run that lists `paths_touched` counts as matching, and a
decision whose cost the decider did not report counts as above any `decision_cost_usd_over`: an
unknown fails toward less authority (`AGENTS.md` § Rules). The decider's answer carries its cost
in micro-USD where its model reports one; `ModelDecider` takes it from the llm usage's
`cost_micro_usd`, as the workflow executor does (`crates/loom-executor/src/workflow.rs:1685-1687`).

## Why

`autonomy-levels.md` § 2.3, rule 5: "A class decided by the decider is sent to a person when a
deterministic condition holds … At A5 such a condition ends the run instead". Session `sdc` gave
the written form on 2026-11-21: the four keys of `autonomy-levels.md` § 3.1, `escalation.escalate_when`, evaluated by a
program after the decider replies and before `Decide`.

## Waits on

`story:decision-routing-by-holder`: the router this extends, and both change
`ess/domains/decision.yaml` and `crates/loom-decision`. `story:llm-decider`:
`ModelDecider` gains the cost on its answer. `story:autonomy-grant-table` through the first.

## ESS first

`ess/domains/autonomy.yaml` gains `EscalationCondition` (`DeciderEscalate`, `DeciderInvalid`,
`PathsTouched`, `DecisionCostUsdOver`) and the run's `escalate_when` list on the autonomy input,
the grant table and `AutonomyResolved`, with the refusal `BadCondition`.
`ess/domains/decision.yaml`: `DecisionStop` gains optional `touched_paths`, the decider's answer
gains optional `cost_micro_usd`, and `SuppressionCause` gains `condition`. On that commit
`task drift` fails, and `crates/loom-decision/tests/escalate_when.rs` does not compile.

## Acceptance

With the fixtures of `story:decision-routing-by-holder`:

- At `A3` on `roadmap` with `paths_touched: ["**/auth/**"]`, a verified decider reply on a stop
  whose touched paths include `src/auth/login.rs` leaves the request Escalated to the escalation
  person; with touched paths `src/ui/list.rs` it is Decided by the decider.
- The same at `A5` leaves the request Open, answers `RunEnds`, and keeps one
  `EscalationSuppressedRecord` with cause `condition` naming the `paths_touched` entry.
- With `decision_cost_usd_over: 1.0`, a reply reporting 1 500 000 micro-USD escalates at `A3`; one
  reporting 500 000 is decided; one reporting no cost escalates.
- At `A5` with `decider: escalate` listed, an `Escalate` with `best: A` ends the run and decides
  nothing; without it listed, the same reply is Decided `A`.
- At `A5` with `decider: invalid` listed, a decider failure ends the run with the request Open;
  without it listed, it is Decided `B` (the denying option).
- A run listing `paths_touched` and a stop with no touched paths escalates at `A3`.
- `paths_touched: ["src/*.rs"]` matches `src/lib.rs` and does not match `src/auth/login.rs`.
- At `A4` on `merge` (`rule_and_decider`) with `decider: escalate` listed, a decider `Escalate`
  leaves the request Escalated to the escalation person; with `paths_touched: ["**/auth/**"]` and
  touched path `src/auth/login.rs`, a decider `Take` of the rule's verdict leaves it Escalated and
  not Decided.
- At `A5` on `merge` (`rule_and_decider_final`), the same `Take` with the same touched path leaves
  the request Open, answers `RunEnds`, and appends one record with cause `condition`.
- `AutonomyResolved` for a run listing `[{decider: escalate}, {paths_touched: ["**/auth/**"]}]`
  carries the list in that order, and the grant table answers it.
- Under holder `person`, `advised`, `rule` and `rule_final` no condition is evaluated (a stub
  counts evaluations).
- A list with `when: "cost > 1"`, an entry `{decider: escalate, paths_touched: []}`, and
  `paths_touched: [""]` are each refused `BadCondition` naming the entry.
- A recorded llm turn whose usage carries `cost_micro_usd` gives the same cost on
  `ModelDecider`'s answer; no test calls a model.
- `task check` exits 0.

## Out of scope

Computing touched paths: the host hands them in. Conditions beyond the four keys. Escalation of
a rule's own result.
