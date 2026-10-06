---
format: aep.planning-md/3
id: story:selection-revalidation
kind: story
status: implemented
title: Revalidate a selected action at the execution boundary
refs:
- provider: commission
  reference: taskboard:M-002
- provider: taskboard
  reference: L-004
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:argument-generator
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:run-pipeline-skeleton
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-executor/src/arguments.rs
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/src/revalidation.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_run_revalidation.rs
- confidence: cited
  path: crates/loom-executor/tests/selection_revalidation.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: website/data/status.json
revision: 28
transitions:
- {from: "draft", to: "proposed", at: "2026-10-06T10:10:28Z", actor: "human:timo", revision: 21, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-06T10:10:28Z", actor: "human:timo", revision: 22, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-10-06T11:13:03Z", actor: "human:timo", revision: 28, decided_on: {"recorded":{"test_result":1,"review_outcome":8,"verification":1}}}
---
## Outcome

Before Loom returns a `ProposedAction`, it revalidates the selection against the current frontier
and case revision, obtained through Commission's `Governor` port and never from the model. Loom's
own pre-check refuses two things, each with the reason named, and returns no `ProposedAction` for
either:

- an action id that is not in the catalogue projected from the frontier current at revalidation;
- a selection made on a catalogue at an older case revision than that frontier's.

A selection that passes both is returned as a `ProposedAction`. **The story ends there.** Loom
invokes no effect and calls no execution adapter. Who invokes the effect of a selected
consequential action, and so who rechecks authority immediately before it, is the open
`decision-blocker:effect-invocation-owner`; that blocker gets no new `blocks` edge, because this
story ends at returning the proposal and is not stopped by the answer. An approval-gated action
does not reach this check: it returns `Suspended` (`story:agent-executor`). Commission revalidates
again on its side (commission `story:stale-revision-action-request`, M-008).

## Shared surface

Depends on `story:argument-generator`: revalidation is the last step before the executor pipeline
returns a `ProposedAction`, which carries the `ProposedActionArguments` that story produces, and
both edit that pipeline in `crates/loom/src/lib.rs`. Depends on `story:run-pipeline-skeleton` for
the revalidation command and the `revalidation` module. `story:harness-loop-port` and
`story:interruption-recovery` depend on it for behaviour. The whole order is in
`story:agent-executor` § Shared surface.

## ESS first

- **Declarations relied on** (landed by `story:run-pipeline-skeleton`): the `loom.run.Selection`
  lifecycle `Selected` → `Admitted` or `Refused`, and `loom.run.RevalidateSelection` with outcomes
  `admitted`, `not-in-frontier` and `stale-revision`. This story does not change `ess/`.
- **Red test:** the first commit adds `revalidation_refuses_before_proposing` in
  `crates/loom/tests/selection_revalidation.rs`; it fails on that commit because Loom returns a
  `ProposedAction` without asking the governor for the current frontier (items 1, 2 and 4). The
  implementation commit makes it pass.

## Domain relations

- `loom.run.Selection -> loom.run.ActionCatalogue` (relation `catalogue`) together with
  `ActionCatalogue.case_revision` give the revision a selection was made at — inferable from
  `ess/domains/run.yaml`, entities `loom.run.Selection` and `loom.run.ActionCatalogue`.
- The current case revision is that of `commission.responsibility.Frontier` (commission
  `ess/domains/responsibility.yaml:182-203` at `013e392`), whose actions commission
  `story:ess-hard-gate` declares as `Frontier.actions: List<FrontierAction>` (one frontier, many
  actions).

## Depends on, outside this store

`commission:story:ess-hard-gate` for the frontier's actions; `commission:story:governor-port`
(M-003) for the port that returns it; `commission:story:agent-executor-port` (M-004) for
`ProposedAction`.

## Scope

Derived 2026-10-06 by `story-scoper` at loom `origin/main` 04a1a73. Every line is **cited** (read
from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-executor` — cited
- **Path map:** `crates/loom/src/lib.rs` → `crates/loom-executor/src/lib.rs`;
  `crates/loom/src/revalidation.rs` → `crates/loom-executor/src/revalidation.rs`;
  `crates/loom/tests/selection_revalidation.rs` → `crates/loom-executor/tests/selection_revalidation.rs` — cited (tree)
- **Files:** `crates/loom-executor/src/revalidation.rs` (a 4-line stub: "story:selection-revalidation
  builds it"), `crates/loom-executor/src/lib.rs:182-266` (`impl AgentExecutor for Loom`),
  `crates/loom-executor/tests/selection_revalidation.rs` (absent; new) — cited
- **Files:** `crates/loom-executor/tests/adversary_run_revalidation.rs:259`,
  `not_in_frontier_follows_the_frontier_actions`, ignored with reason
  `story:selection-revalidation; ESS-SYNTH-003` — cited
- **Placement:** revalidation goes after `request_arguments` (`lib.rs:248`) and before the
  `ProposedAction` return (`:260`). `RequestArguments` refuses a selection that is not `Selected`
  (`ess/domains/run.yaml:545`), and revalidation moves the selection out of `Selected` — inferred
- **Symbols:** `Governor::frontier`, asked once per run (acceptance 4);
  `RevalidateSelectionBehavior for Generated<P>` with `P: TryContext + SelectionStorage`;
  `ExternalCommand::LoomRunRevalidateSelection` (`generated/rust/loom/src/behaviour.rs:109`), which
  now carries the command input; `RequestRecord` — cited
- **Also likely:** `crates/loom-executor/src/arguments.rs`. `RequestRecord` (:56) is the only
  `SelectionStorage` the pipeline holds (:75), so revalidation either wraps it from
  `revalidation.rs` or extends it here — inferred
- **Also likely:** `ess/domains/run.yaml:564-596`. The comment at :566 still blames ess 0.52.0.
  The content changes only if `not-in-frontier` becomes a synthesized membership guard instead of an
  `external:` branch answered from the input's `frontier_actions`. That route regenerates
  `generated/rust/loom/src/run.rs` and `behaviour.rs` — inferred
- **Reads, not changed (old commission references, now in this repo):** `Governor` port →
  `crates/loom-commission/src/ports/governor.rs:14`; `AgentExecutor`/`ProposedAction` (M-004) →
  `crates/loom-commission/src/ports/executor.rs:15`; fake governor →
  `crates/loom-commission-testkit/src/fake_governor.rs:128` (already a dev-dependency of
  `loom-executor`); commission `ess/domains/responsibility.yaml:182-203`@`013e392` →
  `ess/commission/domains/responsibility.yaml:450-476` (`commission.responsibility.Frontier`);
  M-008 stale-revision revalidation → `crates/loom-commission/src/action_request.rs:73`;
  `story:ess-hard-gate` is in this store (implemented) — cited
- **Documents:** none required by the acceptance — cited
- **Confidence:** medium. The story and the tree fix the three files, but the story does not decide
  how `Loom` gets a `Governor` (`AgentExecutor::run` takes none), or whether `not-in-frontier`
  stays `external:` or becomes an ESS guard. Those choices decide whether `Loom::new` callers,
  `ess/` and `generated/` change.
- **Would collide with:** any unit on the run pipeline in `crates/loom-executor/src/lib.rs`
  (:182-266) or `Loom::new` (:74), and with `crates/loom-executor/tests/adversary_run_revalidation.rs`.
  Unmerged `feat/hosted-governor-contract` (1b25fe8) still differs from main there, at the import
  (:13-14) and the `Context::external` impl (:92-97) — cited
- **Would collide with (textual):** on the `external:` route this story must rewrite that same
  `Context::external` impl to answer from `frontier_actions`, which is a merge conflict with that
  branch. On the ESS-guard route only :259 changes. The branch's `generated/rust/loom/src/run.rs` and
  `behaviour.rs` are byte-identical to main 04a1a73, so those two files collide only on the
  ESS-guard route — inferred
- **Safety fact:** Commission revalidates every `ProposedAction` against the governor
  (`crates/loom-commission/src/runtime.rs:492` → `action_request.rs:73`) before `EffectPort::invoke`.
  Loom's pre-check can therefore only narrow what is proposed, and a gap in it reaches no effect —
  step 2, unproven

Stale statements in the sections above, found while scoping (2026-10-06):

- Approval-gated actions no longer return `Suspended`: the tree proposes them (`lib.rs:14-15`, test
  at `lib.rs:344-362`, Atlas ADR 0082), so they reach this check too.
- `decision-blocker:effect-invocation-owner` is `cleared`: Commission invokes the effect (ADR 0082).
  The story still ends at returning the `ProposedAction`.
- The wave-9 note "the generated `Context::external` receives no command input" no longer holds since
  ESS 0.53.0 (`generated/rust/loom/src/behaviour.rs:109`, `:133`).
- Commission's `governor-port`, `agent-executor-port` and `stale-revision-action-request` are not
  artifacts in this store; they map to the code paths under *Reads, not changed*.
- Not established: whether ESS 0.53.0 synthesizes `not-in-frontier` as a membership guard over
  `input.frontier_actions`; how `Loom` gets a `Governor` (`Loom::new` has 44 call sites in 14 files);
  where the wave-9/w10 turn and session binding gap lives.

Confirmed by the implementor in wave 2026-10-06-w1 (the lines above are kept as scoped):

| scoped line | result |
|---|---|
| Placement after `request_arguments`; revalidation moves the selection out of `Selected` | confirmed (`run.yaml:545-548`, `arguments.rs:119`); measured: revalidating before `request_arguments` fails `revalidation_refuses_before_proposing` |
| `arguments.rs`: `RequestRecord` (:56) is the only `SelectionStorage` (:75) | confirmed |
| `ess/domains/run.yaml:564-596` | **range wrong**: `RevalidateSelection` spans `:568-606`; only the comment at :566 changed, nothing was regenerated |
| Collision with `feat/hosted-governor-contract` at the adversary test's `Context::external` | **wrong**: that branch merged before the wave (`743e7c0`), and the executor's answer lives in `src/revalidation.rs`, so the test's `SpecPorts` is untouched; the ignored case sat at `:255-256`, not `:259` |

Settled in the wave: ESS 0.53.0 does not synthesize the guard (3 refusals, so `not-in-frontier` stays
`external:` and the executor answers it from `frontier_actions`); the governor arrives through
`Loom::with_governor`, so `Loom::new` callers are unchanged. Landed in `crates/loom-executor/src/`
`lib.rs`, `revalidation.rs`, `arguments.rs`, five test files, `CHANGELOG.md`, `AGENTS.md` and
`website/data/status.json`.

## Acceptance

The test `revalidation_refuses_before_proposing` in `crates/loom/tests/selection_revalidation.rs`
passes. With the Commission fake governor, it checks:

1. A selection whose action id is absent from the frontier the fake governor returns at
   revalidation is refused, the refusal names the id and the reason "not in frontier", and Loom
   returns no `ProposedAction`.
2. A selection made on a catalogue at case revision `n` while the fake governor returns revision
   `n + 1` is refused, the refusal names both revisions, and Loom returns no `ProposedAction`.
3. A selection that passes both checks is returned as a `ProposedAction` for that action id.
4. The fake governor is asked for the frontier once between the selection and Loom's return, in
   each of the three cases.

## Source

TASKBOARD L-004; Atlas ADR 0072 (revalidate before every effect) and ADR 0073 step 3; the
execution-boundary clause of `epic:loom-native-harness`; `docs/design/loom-design.md:48-70`.

## From wave 2026-10-04-w9 (run-pipeline-skeleton, adversary pass 1)

- The not-in-frontier refusal is declared `external:` in `ess/domains/run.yaml`: ESS 0.52.0 synthesis
  refuses a membership predicate over `input.frontier_actions` (ESS-SYNTH-003/004), and the generated
  `Context::external` receives no command input. This story needs an ESS change (a synthesizable
  membership guard, or input passed to external outcomes) before it can enforce the refusal; the
  adversary case `not_in_frontier_follows_the_frontier_actions` is ignored until then.
- No command binds a selection or revalidation to a turn, session or run, and `ProjectCatalogue` checks
  neither that the turn exists nor that it has one catalogue: a selection can be made against another
  session's catalogue.
- From wave 2026-10-04-w10 (frontier-projection, adversary pass 1): `ActionCatalogue` carries no case
  id and its `frontier` is an opaque string, so a catalogue projected from another case's frontier
  cannot be detected; settle it with the turn binding above.
