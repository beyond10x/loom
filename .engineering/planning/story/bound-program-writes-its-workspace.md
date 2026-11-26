---
format: aep.planning-md/3
id: story:bound-program-writes-its-workspace
kind: story
status: implemented
title: 'A bound program writes its whole workspace under linux when its bindings document declares workspace: read_write'
summary: 'loom.bindings/1 member workspace: read_write and ChildCommand::with_read_write_workspace (Substrate WorkspaceAccess::ReadWrite)'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bindings-document
- depends_on: story:child-writable-workspace
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src
- confidence: cited
  path: crates/loom-bindings/tests/adversary_w20261121w1_u2.rs
- confidence: cited
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_retry.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement/engine.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/guides/bindings.md
- confidence: cited
  path: website/docs/reference/.generated.json
- confidence: cited
  path: website/docs/reference/ess/loom-bindings.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-26T11:34:12Z", actor: "agent:loom", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-26T11:34:15Z", actor: "agent:loom", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-26T11:39:21Z", actor: "agent:loom", revision: 6, decided_on: {"recorded":{"test_result":1}}, correlation: "feat/bound-program-writable-workspace"}
---
## Outcome

A bound program run under Loom's `linux` confinement writes its whole workspace when its bindings
document declares it. A `loom.bindings/1` document takes a new optional top-level member,
`workspace: read_write`; with it every program the document binds (a `program` or `person`
executor's `argv`, every `arguments_from` and every `handoff_when`) runs with Substrate's
`WorkspaceAccess::ReadWrite`: it writes a file at the workspace root, makes directories and writes
in them, and still writes nothing outside the workspace. Without the member nothing changes: a
program writes only `target/` and a `handoff_when` check nothing. A child bindings file an `open`
executor names declares its own member and inherits nothing.

`ChildCommand::with_read_write_workspace` is the builder: the request carries
`WorkspaceAccess::ReadWrite`, the applied-confinement check accepts a record of that access and
refuses any other, and beside `with_writable` or `with_read_only_workspace` the launch is refused
`ScopeInvalid` before anything starts. `TestRunner::run` keeps `target/` alone.

## Why

A consumer runs its programs as Loom bindings under `--confinement linux`. Its programs write
records and edit the tree in their workspace, at the root and in directories they make. Today
`bound_program_child` (`crates/loom-intake-slice/src/bindings.rs`) builds a `ChildCommand` with
no writable path, so a confined program writes only `target/` (the default
`WorkspaceAccess::Scoped` with `target`), and the consumer cannot run confined at all.
`story:child-writable-workspace` added named writable directories, which a host can give a child;
a bindings document has no way to name them, and a program that writes at the workspace root
cannot be served by a directory list.

## Acceptance

- `crates/loom-intake-slice/src/confinement.rs`
  `a_read_write_workspace_is_requested_and_only_its_record_is_accepted`: the request carries
  `WorkspaceAccess::ReadWrite`, for a child run to its end and a live child; a record of another
  access is refused `CapabilityUnserved`, and a default child's request refuses a read-write
  record.
- `…::a_read_write_workspace_beside_another_access_is_refused_before_launch`: beside
  `with_writable` or `with_read_only_workspace`, in either order, refused `ScopeInvalid` before
  any driver opens.
- `crates/loom-intake-slice/tests/confined_child.rs`
  `a_read_write_child_writes_its_whole_workspace_and_nothing_outside` (delegated, skipped on a
  named missing prerequisite): the child writes the root and a directory it makes, not a host path
  outside, the driver record reports `ReadWrite`; a default child cannot write the root.
- `crates/loom-bindings/tests/bound_program.rs`
  `a_confined_program_of_a_read_write_document_writes_its_whole_workspace` (delegated, `linux`
  adapter through `ProgramEffects`): with the member the program writes the root and a new
  subdirectory and nothing outside; without it the same program cannot write the root.
- `…::a_read_write_document_gives_its_program_and_arguments_from_the_whole_workspace` and
  `crates/loom-bindings/tests/bound_retry.rs`
  `a_handoff_check_of_a_read_write_document_writes_the_workspace_as_its_writer_does`: every child
  the document's programs start is read-write, and none without the member.
- `crates/loom-bindings/tests/bindings_document.rs`
  `a_read_write_workspace_is_accepted_and_its_absence_is_the_default` and
  `a_workspace_other_than_read_write_is_refused_naming_it` (`Malformed`, member `workspace`).
- `task drift` red after the change of `ess/domains/bindings.yaml` and green after
  `task generate`; `task check` exits 0 in CI.

## ESS first

`loom.bindings.WorkspaceWrites` (one variant, `ReadWrite`) and the optional member
`BindingsDocument.workspace` in `ess/domains/bindings.yaml`; `generated/rust/loom/`, the suite
under `generated/ess/loom/` and the reference pages regenerated through their tasks.

## Scope

- `ess/domains/bindings.yaml`, `generated/rust/loom/`, `generated/ess/loom/`,
  `website/docs/reference/ess/`, `website/data/ess/`
- `crates/loom-intake-slice/src/confinement.rs`, `crates/loom-intake-slice/src/confinement/engine.rs`
- `crates/loom-bindings/src/decode.rs`, `child.rs`, `program.rs`, `arguments.rs`, `open.rs`,
  `handoff.rs`
- tests named above
- `website/docs/guides/bindings.md`, `website/docs/reference/sdk-stability.md`,
  `website/data/status.json`, `CHANGELOG.md`, `AGENTS.md`
