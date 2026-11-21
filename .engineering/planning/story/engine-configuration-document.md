---
format: aep.planning-md/3
id: story:engine-configuration-document
kind: story
status: draft
title: One engine configuration file, loom.engine/1, is loaded with refusals and given to tree open, tree run and tree serve
summary: A format tag, a loader that refuses undeclared members, --engine, and the step's confinement record in the run log
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:agent-node-run-records
revision: 3
---
## Outcome

An operator configures the engine of a tree in one file, `loom.engine/1`: YAML, with JSON read as
YAML, holding a `format` member and one section per concern. This story creates the document with
its first section, `confinement` (the adapter, `linux` or `local`, and the `local_reason`), and its
loader: a function in `crates/loom-tree/src/engine.rs` that reads and decodes the file, refuses a
file it cannot read or parse naming the path (and, for a parse error, the line), refuses an
undeclared member at any level and a missing or other `format`, each naming the member's path,
refuses a `local` adapter with no or a blank `local_reason`, and builds from the section the
`intake.confinement.EngineConfinement` value that the constructor of
`story:confinement-opens-from-engine-config` takes. `tree open` and `tree run` take the file as
`--engine <path>`; `tree serve` takes the same flag when `story:tree-daemon` adds it. Without
`--engine`, the `--confinement` and `--local-reason` flags of `tree run` build the same value as a
file holding only that section, and giving both `--engine` and `--confinement` exits 2.

Every `action finished` line of a program step carries the step's confinement record,
`loom.engine.StepConfinement`, which this story starts with the adapter and, under `local`, the
reason. The record travels from the runner to the line on the step's `ProgramRun`: the program
port converts the applied record `run_child` returns (`TestExecution.applied`,
`intake.confinement.AppliedConfinement`) into a `StepConfinement` on `ProgramRun.confinement`, and
the tree copies it into the `action finished` line. Steps that start no process (`person`, `agent`
until `story:agent-session-through-substrate`, copy and child steps) write the line without the
member. Each later story of this epic that adds a section to the document or a member to that
record depends on this story.

## Why

Five stories of this epic add a part of the engine's configuration (the level, limits, executable
lists, apertures, run slots), and the reviewed plan has no document for it.
`story:bindings-document` says only that an adapter, an aperture and a remote endpoint are engine
configuration and never binding data (`.engineering/planning/story/bindings-document.md:76`).
`story:confinement-opens-from-engine-config` declares the value (`EngineConfinement` in
`ess/intake/domains/confinement.yaml`) and the one constructor that takes it, and
`story:tree-run-command` gives `tree` the flags `--confinement substrate|local` and
`--local-reason`. Neither defines a file, a loader, or how a long-lived `tree serve` receives the
value.

The step's applied record, `intake.confinement.AppliedConfinement`
(`ess/intake/domains/confinement.yaml:53-71`, with `adapter` and `facts`), is the field `applied` of
the `TestExecution` that `run_child` returns (`crates/loom-intake-slice/src/confinement.rs:79-83`,
`:475`), and a tree keeps it nowhere: `ProgramRun` holds the action, exit, blob digest and whether
it timed out (`story:bound-program-result-table`), and the `action finished` event holds the exit or
refusal, blob digest, evidence ids and revision moves (`story:hash-chained-run-log`). The intake
system and the loom system are separate ESS systems (`ess/intake/system.yaml`, `ess/system.yaml`),
so the loom-side document declares its own section and record types and the program port converts
them. This story edits no file of `crates/loom-intake-slice`.

## Waits on

- `story:agent-node-run-records`: the last story of `epic:protocol-tree-on-bindings` to edit
  `ess/domains/runlog.yaml`, `ess/domains/tree.yaml`, `ess/domains/bindings.yaml`,
  `crates/loom-tree` and `crates/loom-cli/src/lib.rs`, which this story edits next. Every other
  story of that epic comes before it, among them `story:tree-run-command` (the `tree` commands and
  their confinement flags), `story:bound-program-result-table` (`ProgramRun`) and
  `story:confinement-opens-from-engine-config` (`EngineConfinement` and its constructor).

## ESS first

A new domain `loom.engine` in `ess/domains/engine.yaml`, declared in `ess/system.yaml` and listed
in `ess/ess-inputs.yaml`: `EngineConfiguration` (`format`, `confinement`), `ConfinementSection`
(`adapter`: `Linux` or `Local`; `local_reason`), `StepConfinement` (`adapter`, `local_reason`) and
`EngineRefusal` (`DocumentUnreadable` (path, optional line), `FormatUnknown`, `MemberUndeclared`,
`LocalReasonMissing`). `ess/domains/runlog.yaml`: the `action finished` event gains the optional
member `confinement` (`loom.engine.StepConfinement`). `ess/domains/bindings.yaml`: `ProgramRun`
gains the optional member `confinement` (`loom.engine.StepConfinement`). The domain declares types
only, so no conformance scenario is added. On that commit `task drift` fails and
`crates/loom-tree/tests/engine_configuration.rs` does not compile.

## Acceptance

- A file with an undeclared member at the top level, and one with `confinement.adaptor`, are each
  refused `MemberUndeclared` naming the member's path.
- A file with no `format` is refused `FormatUnknown` naming `format`; one with
  `format: loom.engine/2` is refused `FormatUnknown` naming the value.
- A file that does not exist, and one holding `confinement: [`, are each refused
  `DocumentUnreadable` naming the path, the second also the line.
- `confinement: {adapter: local}` with no reason or a blank one is refused by the loader
  `LocalReasonMissing` (the `EngineRefusal` variant), before the constructor is called and before
  any child starts.
- `tree run --engine` with a file holding `confinement: {adapter: linux}` runs the three-level
  fixture tree of `story:tree-run-command` with the same exit and the same order of steps as
  `--confinement substrate`; every `action finished` line of a program step has
  `confinement.adapter` `linux`, and the `person` step's line has no `confinement`.
- With `adapter: local` and the reason `x`, every `action finished` line of a program step names
  `local` and `x`.
- `tree run --engine <file> --confinement local` exits 2 naming both flags. `tree open --engine`
  with a file the loader refuses exits non-zero naming the refusal and writes no case file.
- A guide page under `website/docs/` shows one complete `loom.engine/1` file that
  `engine_configuration.rs` decodes from the page's code block, and `website/docs/reference/cli.md`
  is regenerated.
- `task check` exits 0.

## Out of scope

Every section beyond `confinement`: the daemon endpoint (`story:substrate-daemon-runner`), the
level (`story:confinement-level-required`), limits (`story:action-limits-enforced`), executable
lists (`story:executable-allow-list`), apertures (`story:named-egress-per-action`), the agent
harness (`story:agent-session-through-substrate`), run slots (`story:tree-daemon`). Reading the
file in `b10x-loom run`. Reloading a changed file in a running daemon.
