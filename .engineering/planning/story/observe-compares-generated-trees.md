---
format: aep.planning-md/3
id: story:observe-compares-generated-trees
kind: story
status: draft
title: An observe action compares two workspace directories by inventory and content digest
summary: generated.matches over Substrate's guarded directory pages and digest-bound reads; links and special files refused; 10,000 entries and 4 MiB per tree; .git invisible
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:observe-reads-documents
scope:
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/observe.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/reader.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/tree.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_files.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_generated.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement/read.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/observation.yaml
- confidence: inferred
  path: website/docs/guides/bindings.md
revision: 8
---
## Outcome

The `observe` executor compares two directories of the workspace: `generated.matches` (`expected`,
`actual`) is `true` exactly when both hold the same relative inventory, every directory (empty ones
included) and every regular file, with the same bytes. Loom walks each directory through
Substrate's guarded directory pages (`read_workspace_path` in directory mode,
`crates/substrate-host/src/lib.rs:656`, pages of `DirectoryPage`,
`crates/substrate-wire/src/lib.rs:1344`) and takes each file's whole-file sha256 from a digest-bound
read (`read_workspace_file_v2`, `DigestedFileSlice.sha256`, `lib.rs:1302-1314`), never reading the
bytes into the report. The report's `value` is the boolean; the inventory of each side, as
`(relative path, "directory" | sha256)` sorted by path, is the observation's
(`ObserveRun.inventories`), so a mismatch says which path differs. It starts no process (story 1's
rule of the file and document operations).

What Substrate lets the walk see and say:

- Each file is at most 1 MiB, Substrate's per-file bound as Loom opens its driver (story 1); a file
  over it is refused `FileOverBound` (`workspace.file-limit`, `substrate-guarded/src/fs.rs:516-530`).
- Below a root, a page lists each entry as `file`, `directory` or `symlink` (wire
  `crates/substrate-wire/src/lib.rs:1542-1546`), so a link is seen by its kind, and an entry
  Substrate omits (a FIFO, socket, device or a name that is not UTF-8) is counted in the page's
  omitted header.
- At a root, a page of a file, of a link or of a FIFO all answer `workspace.path-escape`
  (`fs.rs:1552`, the directory opened with `O_DIRECTORY | O_NOFOLLOW`). The walk reads the root's
  parent page, as story 1's reader does: `symlink` is `LinkInTree`; `file` or a name not listed is
  `NotADirectory`; an absent root is `NotADirectory`; a parent page refused (a link or file in an
  earlier component, which no call tells apart) is story 1's `PathRefused`.
- `.git` is invisible: pages leave out every entry named `.git` without counting it (`fs.rs:861-870`).
  Upstream counts it (`crates/assertion-providers/src/files.rs:193-248`), so two trees that differ
  only in a `.git` entry are `true` here and `false` upstream.
- A page entry carries `name`, `kind` and `size` and no digest (wire `lib.rs:1550-1554`), so a file
  whose size changes between its page entry and its digest read is `ChangedDuringRead`; a change
  that keeps the size is not seen between the two calls, and the inventory holds the digest read's
  sha256.

The directory pages are a method of the `WorkspaceDriver` handle in `b10x-loom-intake-slice` and of
the `WorkspaceReader` trait in `b10x-loom-bindings`, the direction
`story:observe-executor-reads-workspace-files` sets.

## Why

`generated.matches` is engineering-protocols' check that generated output equals what a generator
writes (`crates/assertion-providers/src/files.rs:162-166`, `:193-248`, tag `0.16.0`): it refuses
links and special files, counts extra files and empty directories, and stops at 10,000 entries and
the output byte bound, both counted per tree (a fresh entry map and budget for each side,
`files.rs:205`); this story keeps them per tree. Substrate's recursive tree (`list_workspace_tree_v2`, `lib.rs:693`) lists
only the whole workspace (`WorkspaceTreeQuery` has no path, wire `lib.rs:1356-1362`) and at most
1,000 items (`MAX_LIST_ITEMS`, `:23`), so a generated directory inside a larger workspace would be
cut; directory pages walk one directory at a time under the same guarded open.

## Waits on

`story:observe-reads-documents`: both add `ObserveOperation` variants and arms to
`crates/loom-bindings/src/observe.rs`, and both edit `ess/domains/bindings.yaml`,
`ess/domains/observation.yaml`, `generated/rust/loom/` and `CHANGELOG.md` (and, through it, story
1's executor and reader, and `story:gate-program-identity-bound-to-spawn`, which shares
`ess/domains/bindings.yaml`, `generated/rust/loom/` and `CHANGELOG.md`).

Waiting on this one: `story:report-formats-checked-before-the-table` (direct), and, through the
chain, `story:gates-document-evaluated-in-the-governor` (`generated/rust/loom/`, `CHANGELOG.md`).

## ESS first

`ess/domains/bindings.yaml`: `ObserveOperation` gains `GeneratedMatches` (spelled
`generated.matches`; `decode.rs` maps the spelling, `load.rs` checks the arguments `expected` and
`actual`). `ess/domains/observation.yaml`: `ObserveRefusal` gains `NotADirectory`,
`LinkInTree`, `EntryOmitted` and `TreeLimit`; `ObserveRun` gains `inventories` (optional list of
`TreeInventory`: `root`, `entries` of `TreeEntry` `path`, `sha256` optional). On that commit
`task drift` fails and `crates/loom-bindings/tests/observe_generated.rs` does not compile.

## Evidence record

As `story:observe-executor-reads-workspace-files` § Evidence record. The inventories are sorted, so
two runs over the same trees give the same report and observation bytes except the observation's
instant; records are equal but for `id` and `observed_at`.

## Acceptance

Tests in `crates/loom-bindings/tests/observe_generated.rs`; the delegated cases run under a
delegated scope and skip naming the missing prerequisite, as story 1's do. "Refused X" means:
`ObserveRun.outcome` `Refused`, `ObserveRun.refusal` X, the step refused naming it, and no evidence
record.

- Equal trees: the record's `result` is the row for `true`.
- One changed byte, one extra file, one missing file, and one extra empty directory on either side:
  each the row for `false`, and `ObserveRun.inventories` holds the differing path on one side only
  or with two sha256 values.
- A hidden file (`.x`) is an entry of `ObserveRun.inventories`.
- Two trees equal but for a `.git` directory in one: the row for `true`, and no inventory names
  `.git` (the narrowing against upstream above).
- A link below either root (listed `symlink`): refused `LinkInTree`, `ObserveRun.reason` naming it.
- A link as `expected` or `actual` itself (the parent page lists it `symlink`): refused
  `LinkInTree`.
- A FIFO, socket or device, or a name that is not UTF-8, below a root, which Substrate omits from a
  page (`OMITTED_ENTRIES_HEADER`, wire `lib.rs:1340`): refused `EntryOmitted`.
- `expected` or `actual` that is absent, a file, or a FIFO: refused `NotADirectory`.
- A file of 1 MiB + 1 byte in either tree: refused `FileOverBound`, naming its path.
- More than 10,000 entries in one tree (a test with 10,001 empty files on one side): refused
  `TreeLimit`, `reason` naming the bound and the side; 6,000 entries on each side read.
- More than 4 MiB of file sizes in one tree (engineering-protocols' default output bound,
  `crates/assertion-providers/src/lib.rs:113`; a test with five files of exactly 1 MiB on one side):
  refused `TreeLimit`, `reason` naming the bound; four such files on each side read.
- A fake reader whose page lists a file at 10 bytes and whose digest read then answers 11:
  refused `ChangedDuringRead`.
- Past the action's deadline (a fake reader that blocks, a 1 s timeout): `ObserveRun.outcome`
  `Unavailable`, `reason` `deadline`, and the table's `unavailable` row.
- `no_file_or_document_operation_starts_a_process` (story 1) covers `generated.matches`.
- `CHANGELOG.md` `### Added`: `generated.matches`; `### Changed`: `**Breaking:**` for the new enum
  variants.
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `generated/rust/loom/`,
`crates/loom-bindings/src/observe.rs`, `crates/loom-bindings/src/observe/tree.rs` (new),
`crates/loom-bindings/src/observe/reader.rs` (directory pages on the trait and its
implementation), `crates/loom-bindings/src/decode.rs` (the spelling),
`crates/loom-bindings/src/load.rs` (the arguments), `crates/loom-intake-slice/src/confinement/read.rs`
(directory pages and the omitted count on `WorkspaceDriver`),
`crates/loom-bindings/tests/observe_generated.rs` (new), `crates/loom-bindings/tests/observe_files.rs`
(the no-process test's operation list), `CHANGELOG.md`. No manifest line: the crates it uses are
story 1's.

## Out of scope

Running the generator (a bound program before this action). Comparing a tree with a git revision.
A per-file bound above 1 MiB (story 1's Out of scope). A directory that changes between two of its
pages: Substrate re-reads and re-sorts the directory per page (`fs.rs:277-289`), so an entry added
or removed meanwhile may be skipped or repeated, and the walk does not detect it.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (the observe executor's `generated.matches` operation) — cited, story `## Files`
- **Files:** `ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `generated/rust/loom/`,
  `crates/loom-bindings/src/observe.rs`, `crates/loom-bindings/src/observe/tree.rs` (new),
  `crates/loom-bindings/src/observe/reader.rs`, `crates/loom-intake-slice/src/confinement/read.rs`,
  `crates/loom-bindings/tests/observe_generated.rs` (new), `crates/loom-bindings/tests/observe_files.rs`
  — cited, story `## Files`. At `61fd500b` none of the observe files, `observation.yaml` or `confinement/read.rs` exists; story 1 creates them.
- **Symbols:** `ObserveOperation::GeneratedMatches`, `ObserveRefusal::{NotADirectory, LinkInTree, EntryOmitted, TreeLimit}`,
  `ObserveRun.inventories`, `TreeInventory`, `TreeEntry`, `WorkspaceDriver`, `WorkspaceReader` — cited, story body. All are new or in a not-yet-delivered story.
- **Substrate symbols the code uses:** `read_workspace_path` (`substrate-host` `lib.rs:656`), `read_workspace_file_v2` (`:679`),
  `DirectoryPage` (wire `lib.rs:1344`), `DirectoryEntryKind::Symlink` (`:1534-1538`), `DigestedFileSlice` (`:1302`),
  `Listed.omitted_entries` (`substrate-guarded` `lib.rs:104-107`) — cited, read at `0.21.0`
- **Also likely:** `crates/loom-bindings/src/decode.rs` — inferred, story 1's `## Files` has it map the `fs.file.exists` spellings, so `generated.matches` needs an arm
- **Also likely:** `website/docs/guides/bindings.md` — inferred, story 1 documents the operations there and this story's `## Files` omits it
- **Documents:** `CHANGELOG.md` `### Added` and `### Changed` (**Breaking:** variants) — cited. No other prose document is named.
- **Confidence:** high for the file set (the story names every file and the chain gives the new ones); medium for the
  design of the directory walk, because Substrate `0.21.0` limits what the walk can do (see Story defects)
- **Would collide with:**
  - `story:observe-reads-documents` on `observe.rs`, `observation.yaml`, `bindings.yaml`, `generated/`, `observe_files.rs` — ordered: this story `depends_on` it.
  - `story:observe-executor-reads-workspace-files` on `observe.rs`, `observe/reader.rs`, `confinement/read.rs`, `observation.yaml`, `bindings.yaml`, `observe_files.rs` — ordered: this story reaches it through `observe-reads-documents`.
  - `story:gate-program-identity-bound-to-spawn` on `bindings.yaml`, `generated/` — ordered: this story reaches it through `observe-reads-documents`.
  - `story:bound-program-result-table` on `bindings.yaml`, `generated/` — ordered: this story reaches it through story 1.
  - `story:report-formats-checked-before-the-table` on `observe.rs`, `observation.yaml`, `observe_files.rs` — ordered: it `depends_on` this story (direct).
  - `story:observe-git-changed`, `story:gates-document-evaluated-in-the-governor`, `story:gate-run-collects-a-gates-document`, `story:dns-resolution-through-a-resolver-aperture`, `story:tcp-reachability-through-an-aperture` on `observe.rs`, `observation.yaml`, `generated/`, `observe_files.rs` (git, dns and tcp name `observe.rs` and `observation.yaml`) — ordered: each is downstream of this story through `report-formats-checked-before-the-table`.
  - UNORDERED, on `ess/domains/bindings.yaml` and `generated/rust/loom/` only: `story:bound-agent-node`, `story:bound-artifact-revisions`, `story:agent-node-run-records`,
    `story:action-limits-enforced`, `story:confinement-level-required`, `story:named-egress-per-action`, `story:limit-hits-in-run-log`,
    `story:record-copy-between-cases`, `story:engine-configuration-document`, `story:agent-session-through-substrate`,
    `story:concurrent-cases-in-the-tree-daemon`, `story:run-caps-per-arrival`, `story:capacity-given-to-an-admission-program`,
    `story:child-case-own-workspace`, `story:tree-usage-and-model-budget`. No `depends_on` path joins any of these to this story, either way.
    They need different YAML types than this story (`AgentRun`, `ProgramRun`, `BoundAction` against `ObserveOperation`, `ObserveRefusal`, `ObserveRun`),
    so the collision is regeneration of `generated/rust/loom/` and `CHANGELOG.md`, not a text conflict in `bindings.yaml`.
    I read the file lists from each body, not from a stored `## Scope`.
  - No unordered story names `observe.rs`, `observe/reader.rs`, `confinement/read.rs`, `observation.yaml` or `observe_files.rs`.
  — cited (store graph at `61fd500b`, `depends_on` closure computed both ways) for the ordering; inferred for "collision is regeneration only" on the unordered family
- **Safety fact:** `observe.rs`, `observe/reader.rs`, `confinement/read.rs`, `observation.yaml` and `tests/observe_files.rs` are touched only by
  stories downstream of story 1 (`git grep` of the story bodies, plus the graph); `crates/loom-bindings/src/tree.rs` is a different module (a YAML text tree)
  and `observe/tree.rs` does not clash with it — step 2, unproven
