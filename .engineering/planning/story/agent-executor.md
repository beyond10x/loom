---
format: aep.planning-md/3
id: story:agent-executor
kind: story
status: draft
title: Loom implements the Commission AgentExecutor on synthesized model types
refs:
- provider: taskboard
  reference: L-002
relations:
- decomposes: epic:loom-native-harness
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

`b10x-loom` implements the Commission `AgentExecutor` on model types synthesized from `ess/`,
replacing the bootstrap types in `crates/loom/src/lib.rs`, and carries the model-facing approval
stop: an approval-gated selection returns `Suspended` and is never executed by Loom.

## ESS first

- Close the marker on `loom.run.Session` (`ess/domains/run.yaml:44`, "the commission run this
  session executes ... decided in story L-002"): replace `commission_run: String` with the
  commission run reference, typed from the Commission specification where ESS can express it;
  `ess specify validate --path ess` passes.
- Generate the model with `ess generate synthesize --path ess --target rust --layout crate` into a
  committed crate (for example `crates/loom-model`), and make `task check` regenerate it into a
  temporary directory and fail on any difference. No hand-written model type.

## Domain relations

- `loom.run.Session -> commission.responsibility.Run`: each session executes exactly one run; the
  run is owned by its commission, not by Loom, so a session never creates or deletes a run and the
  run exists before the session. Ownership: `commission/ess/domains/responsibility.yaml`, entity
  `commission.responsibility.Commission`, relation `runs` (owns, many), at commission `652537e`. The
  session side is inferable (inferred from `crates/loom/src/lib.rs:71`, one executor invocation per
  commission and frontier; no ess/1 document declares a relation from a Loom session to a run). How
  many sessions one run may have is not declared by this story.

## Depends on, outside this store

Commission `epic:commission-core`: M-004 (`AgentExecutor`) and M-009 (local runtime loop with the
scripted fake governor); ELS `software.change/1` (E-002) for the fixture frontier. The current
`AgentExecutor::run` (`commission/crates/commission/src/lib.rs:54`) passes no run identity; if M-004
does not add one, this story cannot close the marker and its report says so.

## Constraints

Loom decides no authority and no completion. `beyond10x/harness` is not changed.

## Acceptance

With the Commission scripted fake governor serving the ELS `software.change/1` frontier, a
`b10x-loom` test drives Loom as the `AgentExecutor` through successive invocations until it returns
`Suspended` naming the merge approval capability, using only model types from the committed
synthesized crate.

## Source

TASKBOARD L-002; Atlas ADR 0071, 0075; `docs/design/loom-design.md` § Commission integration; the
approval-stop clause of `epic:loom-native-harness`.
