---
format: aep.planning-md/3
id: story:ess-covers-agent-sessions-and-intake
kind: story
status: active
title: Loom's ESS counts metering at llm's Model, models the agent session executor and runs a conformance suite for ess/intake
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:executor-reports-usage
- depends_on: story:unperformable-names-actions
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: Taskfile.yml
- confidence: inferred
  path: crates/loom-intake-conformance
- confidence: cited
  path: crates/loom-intake-slice/src/resume.rs
- confidence: cited
  path: ess/SKIPPED.md
- confidence: cited
  path: ess/domains/metering.yaml
- confidence: cited
  path: ess/domains/session.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/intake/domains/routing.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: generated/rust/intake
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/reference
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T17:46:06Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-20T17:46:07Z", actor: "human:timo", revision: 6}
---
## Outcome

Loom's ESS specifications cover three things they do not cover today, modelled with the
`ess:specifying` skill before the code that depends on them moves:

1. **Metering counts at llm's `Model`.** `ess/domains/metering.yaml:3` says model calls are counted
   at the harness `ModelPort` (`crates/loom-executor/src/harness/wire/port.rs`), and line 26 says each
   turn is priced in `crates/loom-executor/src/harness/turn_loop/price.rs`. `story:delete-harness-port`
   removes both files. The domain is re-pointed: a call is one `turn` of the llm `Model` the run's
   executor and selectors call, counted by the metered wrapper in
   `crates/loom-executor/src/metering.rs`, and a turn is priced once by the run's rate card, named
   without a harness path. The declared types do not change, so the generated model does not
   either. This part lands in step with `story:metering-on-llm-model`, which depends on this story.
2. **The agent session executor of ADR 0098 point 2 has a typed home.** A new `loom.session`
   domain (name inferred) declares the session an `AgentExecutor` runs through metaharness: which
   kind ran it (harness, Claude Code or Codex), the scratch copy of the workspace it ran in and the
   base that copy was taken from, the change it returns (the files it changed against that base),
   the reference to its trace, and the way that change reaches Commission: as a proposed action
   (`commission.responsibility.ExecutorOutcomeProposedAction`, an action and its arguments) that
   Commission rechecks against the case's revision before an effect applies it to the workspace
   (ADR 0082). A case that moved while the session ran is refused at the recheck and the effect is
   not invoked. The trace is never evidence (ADR 0074). `story:metaharness-session-executor`
   implements against this domain instead of declaring it.
3. **ess/intake is held to a conformance suite.** Today `task check` validates ess/intake
   (`task intake-spec`) and checks its generated model for drift (`task intake-drift`), and runs
   conformance only for `ess/` (`task conform`) and Commission (`task commission:conform`)
   (`Taskfile.yml:16-56`, `Taskfile.commission.yml:38-41`). A new task runs the suite synthesized
   from ess/intake against `b10x-loom-intake-slice` and reads the verdict from the report, with the
   rules `crates/loom-commission-conformance/tests/conform.rs` applies to Commission: at least one
   passed scenario, none failed, every unsupported scenario named in `ess/SKIPPED.md`, and
   `conformance_status` passed. `task check` runs it.

Synthesis over ess/intake refuses two scenarios today (`ess verify conform synthesize --path
ess/intake --suite-format 5`, ess 0.60.0, 2026-10-20: 5 selected scenarios, 2 refusals; `ess specify
validate --path ess/intake --format json` reports `unscenarioed: 2`):

- `ESS-SYNTH-004` on `intake.routing.DropResumeContext/outcome/dropped`: no outcome creates an
  `intake.routing.ResumeContext`, although the slice stores one
  (`crates/loom-intake-slice/src/resume.rs:186`, `keep`).
- `ESS-SYNTH-001` on `intake.routing.RecordMergeHandoff/outcome/already-recorded`: no view of
  `intake.routing.MergeHandoff` projects its identity and state, so the refused subject has no
  witness.

Both are closed in the specification (a declared outcome for what `keep` does, an identity view of
the handoff), not by an authored scenario that hides them.

## Why

ADR 0098 sends agent work to a session executor in Loom and removes the harness port the metering
domain points at. A noun with no typed home is the relation nobody checks later, and the
specification is written before the code that depends on it. Of Loom's three specifications, only
ess/intake runs no conformance suite, so its declared commands are checked by nothing but the
compiler.

## Acceptance

- `git grep -n 'harness/' ess/domains/metering.yaml` prints nothing, and `task drift` exits 0 with
  `generated/rust/loom/` unchanged by the metering edit.
- `ess/domains/session.yaml` is listed in `ess/system.yaml` and `ess/ess-inputs.yaml`;
  `ess specify validate --path ess --strict-requires` exits 0; every relation, cardinality or state
  that ADR 0098, takt's session-executor plan or Loom's code does not state is an `UNMAPPED:` marker,
  and each marker is listed in this story's body as an open question for the owner.
- `ess verify conform synthesize --path ess` lists a scenario for each session outcome: the change
  proposed, the proposal refused at the recheck because the case moved, and the change applied
  through the effect. `task conform` exits 0; each session scenario `b10x-loom-executor` cannot run
  before `story:metaharness-session-executor` lands is named in `ess/SKIPPED.md` with that reason.
- `ess specify validate --path ess/intake --format json` reports `unscenarioed: 0`.
- The intake conformance task exits 0 against `b10x-loom-intake-slice`; a copy of its report with
  one added unsupported scenario that `ess/SKIPPED.md` does not name fails and names it, as
  Commission's check does; `task check` runs the task.
- `task generate`, `task intake-generate` and the website reference pages are regenerated, never
  hand-edited; `task docs-check` and `task check` exit 0.

## ESS first

This story is the specification step for `story:metering-on-llm-model` (the counting point) and
`story:metaharness-session-executor` (the session domain). Each part is its own commit: the
specification, then the regenerated output, then the intake conformance crate and task. The red
step for the intake part is the conformance run before the two refusals are closed, quoting them.

## Depends on

- `story:unperformable-names-actions`, which edits `ess/intake/domains/events.yaml` and
  `generated/rust/intake/` first.
- `story:executor-reports-usage`, which may change `ess/domains/metering.yaml` and
  `generated/rust/loom/` first.

## Not in scope

- The session executor's code and its move from takt (`story:metaharness-session-executor`).
- Metering's code (`story:metering-on-llm-model`).
- Governing a session per tool call (ADR 0098, Consequences: it waits on metaharness).

## Scope

Specifications, generated output, one new conformance crate.

- `ess/domains/metering.yaml` (comments at lines 3 and 26)
- `ess/domains/session.yaml` (new), `ess/system.yaml`, `ess/ess-inputs.yaml`, `ess/SKIPPED.md`
- `generated/rust/loom/` and `website/docs/reference/` (regenerated only)
- `ess/intake/domains/routing.yaml` (the resume-context outcome and the handoff view),
  `generated/rust/intake/` (regenerated)
- `crates/loom-intake-conformance/` (new), `Cargo.toml` (workspace members), `Cargo.lock`
- `crates/loom-intake-slice/src/resume.rs` only where the declared outcome needs a handler
- `Taskfile.yml` (the intake conformance task inside `check`)
- `AGENTS.md` (the task list and the specification rows), `CHANGELOG.md`
