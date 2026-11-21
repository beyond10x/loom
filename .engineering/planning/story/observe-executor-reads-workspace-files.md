---
format: aep.planning-md/3
id: story:observe-executor-reads-workspace-files
kind: story
status: proposed
title: An observe action reads workspace files through Substrate with no process, and its result table writes the evidence
summary: Observe executor, loom.observation domain, fs.file.exists, fs.dir.exists, text.contains; 1 MiB per file as Substrate sets it; a link, FIFO or oversize file is a recorded refusal, never false
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:engineering-protocols-dependency-removed
- depends_on: story:bound-program-result-table
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-bindings/Cargo.toml
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/observe.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/files.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/reader.rs
- confidence: cited
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_files.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement/read.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/observation.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: website/docs/guides/bindings.md
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:54:58Z", actor: "agent:sdc", revision: 10, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

A `loom.bindings/1` action can be bound to an `observe` executor: Loom performs one operation of a
closed list over the case's workspace, and the binding's result table turns what it observed into
evidence. This story adds the executor, the `loom.observation` domain, and the first three
operations, `fs.file.exists`, `fs.dir.exists` and `text.contains`, under the names and arguments
engineering-protocols gives them (`crates/assertion-providers/README.md:20-23`, tag `0.16.0`).

```yaml
- action: changelog.check
  executor:
    kind: observe
    operation: text.contains
  arguments: {path: CHANGELOG.md, needle: "## [Unreleased]"}
  evidence:
    - kind: changelog_present
      subject: change
      producer: loom-observe
      from: /value
      results: {"true": present, unavailable: unknown, "*": absent}
```

(The document spelling of `website/docs/guides/bindings.md:71-92`: `from` is `exit`, `effect` or a
JSON pointer, `results` a map.)

No process is the rule of the file and document operations: the three here and those
`story:observe-reads-documents`, `story:observe-compares-generated-trees` and
`story:report-formats-checked-before-the-table` add read the workspace through Substrate's guarded
reads and start nothing. `story:observe-git-changed` adds a launch port to the observe port for the
operations that run git or a project command; it never serves a file or document operation.

Every read goes through Substrate's guarded workspace reads on the driver of the case's workspace
(`read_workspace_path`, `read_workspace_file_v2`: `crates/substrate-host/src/lib.rs:656`, `:679`,
served by `HostDriver` at `:1641`, `:1690`, Substrate `0.21.0`), behind a `WorkspaceReader` port.
The trait and its implementation are both in `b10x-loom-bindings`
(`crates/loom-bindings/src/observe/reader.rs`), over `WorkspaceDriver`, a public handle
`b10x-loom-intake-slice` gives on the driver its `SubstrateRunner` opened for a workspace
(`crates/loom-intake-slice/src/confinement/read.rs`). The handle's methods are synchronous: the
slice drives Substrate's `async` reads on its own runtime, as it does for `run_child`, so
`loom-bindings` gains no `tokio`. The direction is forced: `story:bound-program-result-table`
makes `b10x-loom-bindings` depend on `b10x-loom-intake-slice` (its scope's `Files:` line for the
manifest, `bound-program-result-table.md:115`, marked inferred there because that story names no
manifest), so an implementation in the slice of a trait in `loom-bindings` would be a cycle. Moving the trait into the slice would also work, but the trait is
the observe port's contract and Substrate stays pinned in one manifest
(`crates/loom-intake-slice/Cargo.toml:31-32`): the handle returns the slice's own types, and
`loom-bindings` gains no Substrate dependency.

The operation's report is one JSON object, `{"operation": <name>, "outcome": "known", "value":
<value>}` or `{"operation": <name>, "outcome": "unavailable", "reason": <text>}`, kept as a
content-addressed blob as a program's output is (`story:bound-program-result-table`). The evidence
entries read it with a JSON pointer in `from` (`ResultSource::Report`), through the function a
program's entries use; a boolean, number, `null`, object or array is looked up by its canonical JSON
text (`true`, `3`, `null`), a string by itself. An `unavailable` outcome looks up the key `unavailable`, falls to `*`, and with neither is
refused `ResultNotInTable`, as a program's timeout looks up `timeout`.

Each run is one `loom.observation.ObserveRun` observation: the action, the operation, the sha256 of
the arguments as canonical JSON, each workspace path read with the size and whole-file sha256
Substrate named, the outcome (`Known`, `Unavailable` or `Refused`), the refusal when refused, the
reason, Substrate's error code when it gave one, and the report blob's sha256 when a report exists.

The per-file bound is Substrate's: Loom opens its driver with `HostConfig::minimum`
(`crates/loom-intake-slice/src/confinement.rs:229`), and Substrate `0.21.0` refuses a file over
1 MiB with `workspace.file-limit` before reading it (`crates/substrate-guarded/src/fs.rs:516-530`).
So a file of at most 1 MiB is read whole in one digest-bound read, and a larger one is refused
`FileOverBound`, never skipped and never read as a value. Substrate's refusal names no size, so the
reader takes it from the parent's page (`DirectoryEntry.size`, substrate-wire `lib.rs:1550-1554`),
and the run carries both in `ObserveRun.over_bound` (`path`, `size`, `bound`). This epic does not
raise the driver's limit.

Substrate gives one code, `workspace.path-escape`, for a link at any component, a FIFO, socket or
device, a directory read as a file and a file read as a directory (`fs.rs:227-229`, `:519-521`,
`:1552`). The reader tells them apart with one more guarded call, a directory page of the parent,
which lists each entry as `file`, `directory` or `symlink` and omits FIFOs, sockets, devices,
names that are not UTF-8, and `.git` (`fs.rs:843-870`). A page holds at most 1,000 entries and a
`next_cursor` (substrate-wire `lib.rs:23`, `:1349`); the reader follows the cursor to the last page
before it decides that a name is not listed:

- `fs.file.exists` and `fs.dir.exists` read the parent's page and look the name up: `file` is
  `true` for the first and `false` for the second, `directory` the reverse; `symlink` is refused
  `PathRefused`; a name not listed is `false` (absent, or a FIFO, socket or device, which upstream
  also gives `false`, `files.rs:113-122`); a parent that is absent is `false`; a parent page refused
  `workspace.path-escape` (a link or a file in an earlier component, which no further call tells
  apart) is refused `PathRefused`.
- `text.contains` reads the file; on `workspace.path-escape` it reads the parent's page: `symlink`
  is `PathRefused`; `directory` or a name not listed is `NotRegular`; a parent page refused is
  `PathRefused`; `file` (the path changed kind between the two calls) is `ChangedDuringRead`.
- A path with a `.git` component is `resource.not-found` to Substrate (`fs.rs:955-958`), so
  `fs.dir.exists(".git")` is `false` where upstream gives `true`.

An `observe` action's `timeout_seconds` is at most 300, the executor's bound below the 3,600 every
action may name (`MAX_TIMEOUT_SECONDS`, `crates/loom-bindings/src/lib.rs:34`); `load.rs` refuses a
larger one `TimeoutOutOfRange`, naming 300, beside its check of 1 to 3,600 (`load.rs:185-191`). The
default is 10 s.

Refusals are recorded by the rule of the epic (§ Refusals): at load, `check` refuses the document
with a `BindingsRefusalReason` and no run exists; when the observe port is built, it returns an
`ObservePortRefused` and no run exists; at a step, an `ObserveRun` with `outcome: Refused` and its
`refusal`, the step refused naming it, and no evidence record; a table refusal of
`story:bound-program-result-table` (`ResultNotInTable`) is the step's refusal while the
`ObserveRun` keeps its outcome.

## Why

`assertion_providers` reads files with `std::fs` below a root it canonicalizes itself
(`src/files.rs:11-44`, `:45-60`), from the caller's process, with no confinement
(`README.md:17-18`). Loom's bound executors are `program`, `agent`, `person`, `open` and `copy`
(`ess/domains/bindings.yaml:26-28`); none reads a file without a program, and a program started only
to read a workspace file would need Loom's own binary mounted into the sandbox and a launch per
read, for code that is Loom's either way. Substrate already offers the read without a process,
opened beneath the workspace with no link followed and no FIFO waited on
(`crates/substrate-guarded/src/fs.rs:25-36`), so the executor is new and the confinement is
Substrate's.

The semantics follow engineering-protocols (`src/files.rs:107-130`): an absent path is a known
`false` for the two existence checks (`:121`); `text.contains` needs a regular UTF-8 file and a
non-empty read bound, and on an absent path its I/O error becomes `Unavailable` (`files.rs:47`,
mapped by `collect`, `src/lib.rs:157-159`). One deliberate narrowing: engineering-protocols follows
a link that stays below the root (`src/files.rs:22-43`); Substrate's guarded open refuses every link
(`RESOLVE_NO_SYMLINKS`, `fs.rs:28`, in the guarded mask at `:30-31`), so here a link is a refusal
(§ Open 1 of the ledger).

## Waits on

- `story:engineering-protocols-dependency-removed`: the collectors this replaces leave the graph
  there; this story adds no engineering-protocols dependency.
- `story:bound-program-result-table`: the result table, the record builder, the blob store, the
  program effect port this story's port sits beside, and the `b10x-loom-bindings` →
  `b10x-loom-intake-slice` dependency the reader goes through.

Every later story of the epic waits on this one, directly or through the chain 2 → 3 → 4 → 5 → 6
→ 7 → 8: each shares `ess/domains/bindings.yaml` or `ess/domains/observation.yaml`,
`generated/rust/loom/` and `CHANGELOG.md` with it.

## ESS first

One commit changing only `ess/`:

- `ess/domains/bindings.yaml`: `loom.bindings.Executor` gains `Observe`; `ExecutorSpec` gains
  `operation` (`Optional<loom.bindings.ObserveOperation>`); new enum `ObserveOperation` with
  `FileExists`, `DirExists`, `TextContains` (the document spells them `fs.file.exists`,
  `fs.dir.exists`, `text.contains`; `decode.rs` maps the spelling). `BindingsRefusalReason` gains
  `UnknownOperation`, a decode-time reason (an operation name outside the enum has no value to hand
  to `check`), and `InvalidObserveArguments`, a load-time reason; the comment that splits the
  reasons by time (`ess/domains/bindings.yaml:203-209`) lists each in its place. The `ExecutorSpec`
  comment says `operation` belongs to `Observe`.
- `ess/domains/observation.yaml`, new domain `loom.observation`: `ObserveOutcome` (`Known`,
  `Unavailable`, `Refused`); `ObservedRead` (`path`, `size`, `sha256`); `ObserveRun` (`action`,
  `operation`, `arguments_sha256`, `reads`, `outcome`, `refusal` optional
  `loom.observation.ObserveRefusal`, `reason` optional, `code` optional, `report_sha256` optional,
  `over_bound` optional `OverBound`: `path`, `size` optional, `bound`);
  the step enum `ObserveRefusal` (`PathRefused`, `NotRegular`, `NotUtf8`, `FileOverBound`,
  `ChangedDuringRead`, `ReaderUnserved`, `SubstrateRefused`); the port-build enum
  `ObservePortRefusal` (`NoReader`) and `ObservePortRefused` (`refusal`, `detail`).
- `ess/system.yaml` and `ess/ess-inputs.yaml` list the domain.

On that commit `task drift` fails; `crates/loom-bindings/tests/observe_files.rs` does not compile,
and neither does `crates/loom-bindings/tests/bindings_document.rs`, whose exhaustive match over
`BindingsRefusalReason` (`:743-765`) lacks the two new reasons.

## Evidence record

One `canon-evidence/1` record per evidence entry whose table gives a row, against
`website/static/schemas/evidence-1.schema.json:20-79` (Canon `0.9.0`):

- `format`: `canon-evidence/1`.
- `id`: the id the program effect port gives a record (`story:bound-program-result-table`), fresh
  per record. Differs between two runs.
- `kind`, `subject`, `producer`: the entry's members.
- `result`: the table's result for the value read; absent when the matching row has none.
- `subject_revision`: the case's revision of `subject` when the read started (the program rule).
- `observed_at`: one reading of the host clock taken when the operation ended, RFC 3339 UTC.
  Differs between two runs.
- `upstream_revisions`: the case's revision of each `upstream` artifact; absent when the list is
  empty.
- `child_case`: never written.

Two runs over the same binding, case revisions and file bytes write records equal in every member
but `id` and `observed_at`, and reports with the same bytes, so one blob.

## Acceptance

Tests in `crates/loom-bindings/tests/observe_files.rs`, with a fixture protocol and no model,
unless named otherwise. The cases that open a Substrate driver run their own test binary under
`systemd-run --user --scope -p Delegate=yes` and skip naming the missing prerequisite, as
`crates/loom-intake-slice/tests/confined_child.rs` does (`AGENTS.md` § Intake and the command
line); the outcome-to-record cases run on a fake `WorkspaceReader`.

Reads and results (each reads the record written and `ObserveRun.outcome`):

- `fs.file.exists` on a regular file: `ObserveRun.outcome` `Known`, and the record's `result` is
  the table's row for `true`.
- `fs.file.exists` on an absent path and on a path whose parent is absent: the row for `false`.
- `fs.file.exists` on a directory (the parent's page lists it as `directory`): the row for `false`.
- `fs.file.exists` and `fs.dir.exists` on a FIFO (not listed on the parent's page): the row for
  `false`, within the action's deadline and with no writer held on it.
- `fs.dir.exists` on a directory: the row for `true`; on an absent path or a file: the row for
  `false`.
- `fs.dir.exists(".git")` on a workspace holding a `.git` directory: the row for `false` (the
  narrowing against upstream above).
- `a_name_past_the_first_page_is_found`: in a directory of 1,001 files, `fs.file.exists` on the
  name that sorts last (on the second page): the row for `true`, which a reader that stopped after
  one page would not give.
- `text.contains` is `true` exactly when the file's bytes, read whole, hold the needle: a file of
  exactly 1 MiB (1,048,576 bytes) whose needle is its last 8 bytes gives the row for `true`, which
  a reader that stopped after any prefix would not.
- `text.contains` on an absent path: `ObserveRun.outcome` `Unavailable`, `ObserveRun.code`
  `resource.not-found`, and the table's `unavailable` row; never the row for `false`.
- An entry list with two kinds over one operation writes one record per kind, and the run's
  `ObserveRun.reads` holds one read.

Step refusals (each: `ObserveRun.outcome` `Refused`, `ObserveRun.refusal` as named,
`ObserveRun.code` Substrate's code where it gave one, the step refused naming the refusal, and no
evidence record):

- A link as the last component (listed `symlink` on the parent's page), its target inside the
  workspace or not: `refusal` `PathRefused` for each of the three operations, with `code`
  `workspace.path-escape` for `text.contains` (its read gave it) and no `code` for `fs.file.exists`
  and `fs.dir.exists` (they read only the page); the test asserts no record with the `false` row
  exists.
- A link in an earlier component (the parent's page refused): `refusal` `PathRefused`, for each of
  the three operations.
- A FIFO at the path of `text.contains`: `refusal` `NotRegular`, within the action's deadline, with
  no writer held on it.
- A directory at the path of `text.contains`: `refusal` `NotRegular`.
- `a_file_over_the_bound_is_named_with_its_size`: a file of 1 MiB + 1 byte under `text.contains`:
  `refusal` `FileOverBound`, `code` `workspace.file-limit`, `ObserveRun.over_bound.path` the file's
  path and `over_bound.size` 1,048,577; no record with the `false` row exists.
- A file that is not UTF-8 under `text.contains`: `refusal` `NotUtf8`.
- A fake reader whose read answers `workspace.path-escape` and whose parent page then lists the
  name as `file`: `refusal` `ChangedDuringRead`. (A file of at most 1 MiB is read in one slice, so a
  digest change between two slices of one read cannot happen; this is what remains of the case.)
- A driver that serves no digest-bound read: `refusal` `ReaderUnserved`, `code`
  `workspace.file-v2-unserved` (`crates/substrate-host/src/lib.rs:687`).
- A path that is absolute or holds `..` arriving at the step from a request's arguments (a fake
  request, as story 8's gate run makes): `refusal` `PathRefused`, and the fake reader counts no
  read; no record with the `false` row exists.

Load and port build (no `ObserveRun` exists for either):

- A literal path that is absolute or holds `..`: `check` refuses with `BindingsRefusalReason`
  `PathNotInWorkspace`, as for any binding path (`AGENTS.md` § Bindings document).
- An operation name outside the list: `decode` refuses with `UnknownOperation`, naming it.
- An `observe` action with no `operation`: `decode` refuses with `MissingMember`, naming
  `operation`.
- An `observe` action with `argv`, `adapter` or any member of another executor: `check` refuses
  with `MemberNotForExecutor`.
- An `observe` action with `arguments_from` (an argv that starts a process,
  `ess/domains/bindings.yaml:171-172`, `crates/loom-bindings/src/decode.rs:231-233`): `check`
  refuses with `MemberNotForExecutor`, naming `arguments_from`.
- An entry with `from: exit` or `from: effect`: refused as a program's `effect` entry is.
- Arguments that are not exactly the operation's: `check` refuses with `InvalidObserveArguments`,
  naming the member.
- `every_refusal_reason_is_reachable_from_a_document`
  (`crates/loom-bindings/tests/bindings_document.rs:769`) reaches both new reasons, and its count
  of reasons (`:804`) rises by two.
- An observe port built with no reader for a document binding an `observe` action returns
  `ObservePortRefused` with `refusal` `NoReader`, and the port's run count is zero.

No process and no host file access:

- `no_file_or_document_operation_starts_a_process`: a port built with the fake reader counts no
  call to the runner the host gave it for each of the three operations (the runner is the
  `SubstrateRunner` the host hands the port with the reader; this story's port takes it only to
  open the `WorkspaceDriver`), and a delegated case reads the driver's executions and finds none
  for the action. `story:observe-git-changed` keeps this test and adds a counting launcher to it.
- `observe_reads_only_through_the_reader`: the test first asserts its scan of
  `crates/loom-bindings/src/` reaches `observe.rs` and `observe/reader.rs` (failing on today's tree,
  where neither exists, as `crates/loom-intake-slice/tests/host_git_hardening.rs:453-458` asserts
  its scan reaches the executor), then that no file under `crates/loom-bindings/src/observe*` names
  `std::fs`, `File::open`, `std::process::Command`, `TcpStream` or `UdpSocket`.

Limits:

- The wall time: an `observe` action's `timeout_seconds` of 301 is refused at load,
  `TimeoutOutOfRange`, naming 300; 300 is accepted (the executor's bound above).
- A read still running at the deadline (default 10 s; the test uses a fake reader that blocks and a
  1 s timeout): `ObserveRun.outcome` `Unavailable`, `reason` `deadline`, and the table's
  `unavailable` row.
- Substrate's `workspace.io-failed`: `ObserveRun.outcome` `Unavailable`, `code` kept.
- Every Substrate refusal not named above: `outcome` `Refused`, `refusal` `SubstrateRefused`,
  `code` kept (a fake reader returning a code outside those named here shows it). Loom never asks a
  read of more than 1 MiB, so `workspace.read-limit` (`fs.rs:329-335`) reaches only this catch-all.

Records and the rest:

- Each record's members are as § Evidence record says: a test runs one action twice over unchanged
  files and compares the two records with `id` and `observed_at` removed, and the two report blobs.
- The observe effect port reports `performs` only for actions bound to `observe`; the program
  port's tests pass unchanged.
- `crates/loom-governor` and `crates/loom-commission` are not changed, and no manifest gains a
  dependency on Canon or a model provider from `b10x-loom-commission` (`AGENTS.md` § Never;
  `task check`'s dependency guard).
- `CHANGELOG.md` `### Added`: the `observe` executor (this story). `### Changed`, two bullets:
  `**Breaking:** loom.bindings.Executor gains Observe; an exhaustive match on it no longer compiles`
  and `**Breaking:** BindingsRefusalReason gains UnknownOperation and InvalidObserveArguments`
  (`b10x-loom-bindings` is not on the SDK stability page, `website/docs/reference/sdk-stability.md`;
  the bullets are written because a consumer sees the change).
- `website/docs/guides/bindings.md` gains an `observe` action in the page's own
  `format: loom.bindings/1` file: the page's `review@1` protocol (`website/docs/guides/bindings.md:24-58`) gains an action `brief.check` and an evidence
  kind `brief_present`, and the file binds `brief.check` to `text.contains` over `in/brief.md`, the
  brief artifact's path there (`:68`); the guide test
  (`crates/loom-bindings/tests/bindings_document.rs:826-850`) decodes and checks it, and its count of
  executors used (`:842`) rises from five to six.
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/observation.yaml` (new), `ess/system.yaml`,
`ess/ess-inputs.yaml`, `generated/rust/loom/` (regenerated), `crates/loom-bindings/src/observe.rs`
(new, the port and the dispatch), `crates/loom-bindings/src/observe/files.rs` (new, the three
operations and the parent-page classification), `crates/loom-bindings/src/observe/reader.rs` (new,
the `WorkspaceReader` trait and its implementation over `WorkspaceDriver`),
`crates/loom-bindings/src/decode.rs` (`operation` in the closed `EXECUTOR` list at `:27-40`, the
`ExecutorSpec` literal at `:353`, the operation spelling, `UnknownOperation`),
`crates/loom-bindings/src/load.rs` (the `fit` table at `:230-245` gains `operation` for `Observe`
and `arguments_from` refused for it; `InvalidObserveArguments`; `PathNotInWorkspace` for observe
paths; the 300 s timeout bound for `Observe`), `crates/loom-bindings/src/lib.rs`, `crates/loom-bindings/Cargo.toml` (`serde_json = "1"`
and `sha2 = "0.10"`; both are already in `Cargo.lock` through `b10x-loom-intake-slice`, so the lock
changes only in `b10x-loom-bindings`' dependency list), `crates/loom-bindings/tests/observe_files.rs`
(new), `crates/loom-bindings/tests/bindings_document.rs` (the reason match, the count, the guide
test's executor count), `crates/loom-intake-slice/src/confinement/read.rs` (new,
`WorkspaceDriver`), `crates/loom-intake-slice/src/confinement.rs` (module line, the handle's
constructor on `SubstrateRunner`), `website/docs/guides/bindings.md`, `Cargo.lock`, `CHANGELOG.md`,
`AGENTS.md` § Bindings document.

## Out of scope

Documents, generated trees, report files, git and network (stories 3 to 6, 9, 10 of the epic).
Following a link inside the workspace. A per-file bound above 1 MiB: a later Loom story, an
engine-configuration field for the bound that the runner sets on the public
`HostConfig.max_file_bytes` (`crates/substrate-host/src/lib.rs:319`, Substrate `0.21.0`) after
`HostConfig::minimum` (`confinement.rs:229`); lock files such as `package-lock.json` exceed 1 MiB.
Limits Substrate enforces per process (`story:action-limits-enforced`): the file and document
operations start none.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` plus `ess/domains/bindings.yaml` and new `ess/domains/observation.yaml` — cited (Outcome, ESS first, Files)
- **Files (ESS):** `ess/domains/bindings.yaml`, `ess/domains/observation.yaml` (new), `ess/system.yaml`, `ess/ess-inputs.yaml` (`ess/system.yaml:24` and `ess/ess-inputs.yaml:18` list `loom.bindings` and `domains/bindings.yaml` today) — cited
- **Files (loom-bindings):** `src/observe.rs`, `src/observe/files.rs`, `src/observe/reader.rs` (all new), `src/decode.rs`, `src/load.rs`, `src/lib.rs`, `tests/observe_files.rs` (new) — cited (Files)
- **Files (slice):** `crates/loom-intake-slice/src/confinement/read.rs` (new), `crates/loom-intake-slice/src/confinement.rs` (module line at `:55-62`, handle constructor beside `SubstrateRunner::driver` at `:321`) — cited
- **Files (documents):** `website/docs/guides/bindings.md` — cited
- **Symbols:** `Executor` enum (`ess/domains/bindings.yaml:26-28`), `ExecutorSpec`, `BindingsRefusalReason` (`:211-231`), `EXECUTOR` const (`decode.rs:30`, 12 names), `fit` (`load.rs:~215`, array of 11), `source_fits` match (`load.rs:133-137`), `ResultSource::Report` (`decode.rs:513`), `MAX_TIMEOUT_SECONDS` 3600 (`lib.rs:34`), `SubstrateRunner::driver` (`confinement.rs:321`, private), `HostConfig::minimum` (`confinement.rs:229`) — cited
- **Also needed, not in the story's Files:** `crates/loom-bindings/tests/bindings_document.rs` — cited by the tree: its `index` match (`:743-765`) and `0..20` count (`:803`) are exhaustive over `BindingsRefusalReason`, so the ESS commit breaks this file's compilation too; the guide test (`:826-850`) asserts `kinds.len() == 5` and 3 sources — inferred that the story must edit it
- **Also likely:** `crates/loom-bindings/Cargo.toml` (path dependency on the slice, which `story:bound-program-result-table` adds; possibly `serde_json`, `tokio` for the deadline) — inferred, the story names no manifest
- **Documents:** `website/docs/guides/bindings.md` gains the example. Also false after this story: `lib.rs:12` ("executes nothing, starts no process, opens no network connection") and the guide page's lede — inferred
- **Confidence:** medium — the story names every file, but the Substrate behavior it relies on does not match the code (see Story defects), so the slice handle's shape could still change
- **Would collide with:** (dependency chain read from `aep plan artifact graph --format json`)
  - Ordered before it by `depends_on`, no conflict: `story:bound-program-result-table` (direct; same crate and `bindings.yaml`), `story:bound-commands-run-confined` (via the former; `confinement.rs`), `story:canon-0-9-0-pin` (`loom-bindings/Cargo.toml`), `story:engineering-protocols-dependency-removed` (direct)
  - Ordered after it by `depends_on` (it lands first): `story:observe-reads-documents`, `story:observe-compares-generated-trees` (also `confinement/read.rs`), `story:report-formats-checked-before-the-table`, `story:observe-git-changed`, `story:gate-program-identity-bound-to-spawn`, `story:gates-document-evaluated-in-the-governor`, `story:gate-run-collects-a-gates-document`, `story:dns-resolution-through-a-resolver-aperture`, `story:tcp-reachability-through-an-aperture`
  - UNORDERED, on `crates/loom-bindings` (whole dir or `src`) and `bindings.yaml`: `story:bound-agent-node`, `story:bound-artifact-revisions`, `story:agent-node-run-records`, `story:tree-usage-and-model-budget`, `story:child-case-runtime`, `story:record-copy-between-cases`
  - UNORDERED, on `load.rs`, `decode.rs`, `lib.rs`, `bindings.yaml`, `tests/bindings_document.rs`, `guides/bindings.md`: `story:run-caps-per-arrival`, `story:capacity-given-to-an-admission-program`, `story:child-case-own-workspace` (no guide)
  - UNORDERED, on `ess/domains/bindings.yaml` only: `story:concurrent-cases-in-the-tree-daemon`
  - UNORDERED, on `confinement.rs`: `story:python-test-profile`
  - UNORDERED, on `ess/system.yaml` / `ess/ess-inputs.yaml` (list-append conflicts): `story:autonomy-grant-table`, `story:decision-request-entity`, `story:hash-chained-run-log`, `story:tree-run-command`, `story:ess-format-31`, `story:agent-selects-the-connectors-relay`, `story:boundary-admits-declared-http-egress`, `story:send-input-in-the-declared-shape`
  - Body names the same files but records no scope, unordered: `story:bindings-child-path-refusal-names-the-bindings-directory`, `story:action-limits-enforced`, `story:named-egress-per-action`, `story:engine-configuration-document`, `story:confinement-level-required`, `story:limit-hits-in-run-log`
  - All of this is cited from the stored scopes and the graph. The body-only group is inferred.
- **Safety fact:** the three operations start no process because Substrate's guarded read opens beneath the workspace with `RESOLVE_NO_SYMLINKS` and `O_NONBLOCK` (`substrate-guarded/src/fs.rs:26-36`, `0.21.0`). Level 2, pointed at a `file:line`, unproven. Link, FIFO, directory and wrong-kind all return one code, so the fact does not give the story's refusal classes (defect 1).
