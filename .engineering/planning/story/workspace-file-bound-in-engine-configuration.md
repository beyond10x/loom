---
format: aep.planning-md/3
id: story:workspace-file-bound-in-engine-configuration
kind: story
status: draft
title: The workspace driver's per-file bound is an engine-configuration field, 1 MiB by default
summary: workspace.max_file_bytes in loom.engine/1, 1 MiB to 16 MiB, refused at load outside it; set on HostConfig.max_file_bytes and read_limit_bytes after minimum; the observe reader's FileOverBound reports the configured bound; low priority, not proposed until sdc says so
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:observe-executor-reads-workspace-files
- depends_on: story:observe-compares-generated-trees
- depends_on: story:engine-configuration-document
- depends_on: story:substrate-daemon-runner
revision: 1
---
## Outcome

The per-file bound of the workspace driver is an engine-configuration field: `workspace.max_file_bytes`
in a `loom.engine/1` file (`story:engine-configuration-document`), an integer number of bytes. With
no `workspace` section, or no `max_file_bytes` member, the bound is 1,048,576 (1 MiB), the value
`HostConfig::minimum` gives today (`crates/loom-intake-slice/src/confinement.rs:229`;
`crates/substrate-host/src/lib.rs:525`, Substrate `0.21.0` at Loom's pinned revision `adfde402`), so
behaviour is unchanged without the field.

The loader (`crates/loom-tree/src/engine.rs`) accepts 1,048,576 to 16,777,216 (16 MiB), both
included, and refuses any other value at load as `EngineRefusal` `FileBoundOutOfRange`, naming
`workspace.max_file_bytes`, the value and the range.

- Lower bound 1 MiB: every story of the epic is tested at Substrate's default, and the field exists
  to raise it.
- Upper bound 16 MiB: memory per read. Substrate checks no upper bound on the field (it is a plain
  public `u64`, `crates/substrate-host/src/lib.rs:319`, passed unchecked to the guarded file system
  at `:1177-1182`; the guarded file system only compares sizes against it, `fs.rs:523`, `:580`,
  `:778`). But a digest-bound read holds the whole file: `read_bounded_complete` allocates the
  file's size and reads it to the end (`crates/substrate-guarded/src/fs.rs:769-785`), the read
  digests all of it and slices from it (`fs.rs:338-353`), and the in-process handle receives the
  slice base64-encoded (`DigestedFileSlice.content`, `crates/substrate-wire/src/lib.rs:1302-1312`).
  So one read costs about 2.3 times the file in memory, once per concurrent read. 16 MiB keeps one
  read under 40 MiB and covers lock files such as `package-lock.json`.

The field reaches the driver through the engine-configuration value the runner's one constructor
takes (`story:confinement-opens-from-engine-config`): `intake.confinement.EngineConfinement` gains
the optional `workspace_max_file_bytes`. `driver_config` (`confinement.rs:224-238`) sets both
`config.max_file_bytes` and `config.read_limit_bytes` (`substrate-host` `lib.rs:319-320`) to it after
`HostConfig::minimum`. Setting the read limit as well means one read still returns a whole file:
`workspace.read-limit` refuses a read larger than the read limit (`fs.rs:329-335`). This applies
only to the in-process adapters (`linux`, `local`). Under `adapter: daemon`
(`story:substrate-daemon-runner`) the daemon opens its own driver, so the loader refuses a
`workspace` section with `FileBoundNotServed`.

Every place the bound is read:

1. The driver open: `driver_config`, as above. Substrate's probe publishes the value as the facts
   `workspace_max_file_bytes` and `workspace_read_limit_bytes` (`crates/substrate-host/src/probe.rs:121-122`),
   and the runner keeps the driver's facts (`machine.facts`, `confinement.rs:411`).
2. The observe reader (`story:observe-executor-reads-workspace-files`): `WorkspaceDriver` returns
   the bound from the driver's published fact, never from a constant. The reader asks for one read
   of at most that size, and `FileOverBound` carries `ObserveRun.over_bound` with `path`, `size` and
   `bound` set to the configured value.
3. The tree comparison (`story:observe-compares-generated-trees`): its byte-sum bound stays fixed at
   4 MiB per tree. It is engineering-protocols' output bound (`crates/assertion-providers/src/lib.rs:113`,
   tag `0.16.0`), a bound on the comparison, not on a read. With a bound above 4 MiB, a tree holding
   one file over 4 MiB is refused `TreeLimit`, while the same file read by `text.contains` is read.
   The 10,000-entry bound stays fixed.
4. Not read: the gate run's inputs read through `git cat-file`
   (`story:gate-run-collects-a-gates-document`) are bounded by Substrate's output limit, a separate
   field (`output_limit_bytes`, `substrate-host` `lib.rs:528`), and stay at 1 MiB.

Substrate's write bound for the guarded workspace write (`workspace.write-limit`, `fs.rs:580`) is the
same field, so it rises with it.

## Why

Session `sdc` kept the 1 MiB bound for `epic:observation-collection-as-executors` and asked for
this story at low priority (`.engineering/drafts/e3-sdc-answers-11.md`, last paragraph). Lock
files can exceed 1 MiB, and until this story lands they are refused `FileOverBound` with their path
and size. Session `substrate` confirmed that raising the bound is Loom's, through the public field
after `HostConfig::minimum` (`.engineering/drafts/e7-substrate-answer-1.md`).

Not to be moved to `proposed` until session `sdc` says so. `sdc` decides when a real SDC file hits
the bound.

## Waits on

- `story:observe-executor-reads-workspace-files`: `WorkspaceDriver`, the reader and `OverBound`.
- `story:observe-compares-generated-trees`: its test file gains the bullet that the byte-sum bound
  stays fixed.
- `story:engine-configuration-document`: the document, its loader, `EngineRefusal` and
  `ess/domains/engine.yaml`.
- `story:substrate-daemon-runner`: it adds `adapter: daemon` to the same section's loader and the
  daemon arm of the constructor, which this story's refusal names. Through it,
  `story:confinement-opens-from-engine-config` (`EngineConfinement`, the constructor).

Also edit `ess/domains/engine.yaml`, `crates/loom-tree/src/engine.rs` or `confinement.rs`, with no
edge to this story: `story:confinement-level-required`, `story:action-limits-enforced`,
`story:executable-allow-list`, `story:named-egress-per-action`, `story:tree-daemon` and the stories
after it. § Open of the ledger.

No story waits on this one.

## ESS first

- `ess/domains/engine.yaml`: `EngineConfiguration` gains the optional `workspace`
  (`loom.engine.WorkspaceSection`); new type `WorkspaceSection` (`max_file_bytes`, optional
  integer, bytes); `EngineRefusal` gains `FileBoundOutOfRange` (member path, value) and
  `FileBoundNotServed` (the adapter).
- `ess/intake/domains/confinement.yaml`: `EngineConfinement` gains the optional
  `workspace_max_file_bytes` (integer, bytes).
- `ess/domains/observation.yaml`: no change (`OverBound.bound` is story 1's).

On that commit `task drift` and `task intake-drift` fail, and
`crates/loom-tree/tests/engine_configuration.rs` does not compile.

## Evidence record

Unchanged: the bound changes which files are read, not what a record holds.

## Acceptance

No test makes a network or model call. The delegated cases run under `systemd-run --user --scope
-p Delegate=yes` and skip naming the missing prerequisite, as
`crates/loom-intake-slice/tests/confined_child.rs` does.

The field (`crates/loom-tree/tests/engine_configuration.rs`):

- `workspace: {max_file_bytes: 1048575}` and `{max_file_bytes: 16777217}`: each refused
  `FileBoundOutOfRange`, naming `workspace.max_file_bytes`, the value and the range.
- `{max_file_bytes: 1048576}` and `{max_file_bytes: 16777216}`: each loads, and the built
  `EngineConfinement.workspace_max_file_bytes` is that value.
- A file with no `workspace` section: `EngineConfinement.workspace_max_file_bytes` is absent.
- `workspace: {max_file_bytes: 4194304}` with `confinement: {adapter: daemon, …}`: refused
  `FileBoundNotServed`, naming `workspace` and `daemon`.

The driver (`crates/loom-intake-slice/tests/engine_confinement.rs`, delegated):

- `the_published_bound_is_the_configured_one`: a runner built with `workspace_max_file_bytes`
  4,194,304 opens a driver whose facts `workspace_max_file_bytes` and `workspace_read_limit_bytes`
  are both 4,194,304.
- `no_field_keeps_one_mebibyte`: a runner built without the field publishes 1,048,576 for both.

The reader at a non-default bound (`crates/loom-bindings/tests/observe_files.rs`, delegated, bound
4,194,304):

- `a_file_of_exactly_the_configured_bound_is_read`: `text.contains` on a file of exactly 4,194,304
  bytes whose needle is its last 8 bytes gives the record's row for `true`.
- `a_file_one_byte_over_the_configured_bound_is_refused`: a file of 4,194,305 bytes gives
  `ObserveRun.refusal` `FileOverBound`, `code` `workspace.file-limit`, and `ObserveRun.over_bound`
  with `size` 4,194,305 and `bound` 4,194,304; no record with the `false` row exists.
- Story 1's 1 MiB bullets pass unchanged on a runner built without the field.

The tree comparison (`crates/loom-bindings/tests/observe_generated.rs`, delegated, bound 8,388,608):

- `the_tree_byte_bound_stays_four_mebibytes`: a tree holding one file of 5 MiB is refused
  `TreeLimit`, `reason` naming 4 MiB.

The rest:

- `CHANGELOG.md` `### Added`: `workspace.max_file_bytes` in `loom.engine/1`.
- The guide page of `story:engine-configuration-document` shows the `workspace` section, and its
  decode test reads it.
- `task check` exits 0.

## Files

`ess/domains/engine.yaml`, `ess/intake/domains/confinement.yaml`, `generated/rust/loom/` and the
intake generated tree (regenerated), `crates/loom-tree/src/engine.rs` (the section, the range, both
refusals), `crates/loom-tree/tests/engine_configuration.rs`,
`crates/loom-intake-slice/src/confinement.rs` (`driver_config`, the constructor's new member),
`crates/loom-intake-slice/src/confinement/read.rs` (`WorkspaceDriver` returns the published bound),
`crates/loom-intake-slice/tests/engine_confinement.rs`, `crates/loom-bindings/src/observe/reader.rs`
(the bound from the handle), `crates/loom-bindings/tests/observe_files.rs`,
`crates/loom-bindings/tests/observe_generated.rs`, the engine-configuration guide page under
`website/docs/`, `CHANGELOG.md`.

## Out of scope

A bound above 16 MiB, or reading a file in slices across calls. A per-workspace or per-action bound.
The daemon's bound under `adapter: daemon`. Raising Substrate's output limit for gate inputs. Making
the tree comparison's 4 MiB byte-sum bound configurable.
