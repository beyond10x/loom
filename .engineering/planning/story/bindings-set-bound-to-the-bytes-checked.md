---
format: aep.planning-md/3
id: story:bindings-set-bound-to-the-bytes-checked
kind: story
status: draft
title: A tree step reads the bindings the embedder checked
summary: 'Runbook''s TOCTOU gap: read_bindings_set digests the bytes it decodes (root plus open-executor files, sha256sum lines); run_tree_checked and approve_checked take the checked loom.tree.BindingsSet and compare at every read before anything runs; run_tree and approve pin at their first read; a change refuses BindingsChanged naming the file; ESS BindingsSet, BindingsFile, TreeRefusalReason BindingsChanged'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-tree/src/approve.rs
- confidence: inferred
  path: crates/loom-tree/src/bindings_set.rs
- confidence: inferred
  path: crates/loom-tree/src/lib.rs
- confidence: cited
  path: crates/loom-tree/src/open.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: inferred
  path: crates/loom-tree/tests/bindings_set
- confidence: cited
  path: crates/loom-tree/tests/embedder/src/main.rs
- confidence: cited
  path: crates/loom-tree/tests/main.rs
- confidence: cited
  path: crates/loom-tree/tests/suite_format/mod.rs
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: inferred
  path: website/docs/guides/embed-the-runtime.md
revision: 16
---
## Outcome

A tree step runs under the bindings the embedder checked, or not at all. Loom reads the tree's
bindings set (the root file and every file its `open` executors name, as `tree open` walks them)
once per read, digests the very bytes it decodes, and compares the digest with the one the
caller checked. On any difference it refuses `BindingsChanged`, naming the file, before any
step, decision or line is written. An embedder (`runbook run`) gets the set from Loom
(`read_bindings_set`), checks the documents in it (for example, that no binding names a model
program in a run opened without a model node), and passes the set's digest to
`run_tree_checked` or `approve_checked`. Within one call, a program that rewrites a bindings
file, or the manifest's `bindings` member, between two steps gets the next step refused, not
run under the new file. Runbook asked for this (design authority). Cites are at `d8944d49`.

**Before.** `run_tree` reads the manifest under the store lock
(`crates/loom-tree/src/run.rs:199-201`) and then loads every case's bindings file by path
(`tree_cases`, `:547-592`, through `crate::open::load`, `crates/loom-tree/src/open.rs:273-299`),
once before the loop (`run.rs:230`) and again before every step (`:237`). `tree approve` reads
the manifest (`crates/loom-tree/src/approve.rs:186-188`) and loads the case's bindings by path
for a person action (`case_bindings`, `run.rs:1866-1888`, called at `approve.rs:220` through
`person_action`, `:428-435`). Nothing ties those bytes to what the embedder checked: a bound
program of an earlier step, or a process it left behind, can rewrite the root file, a child file
(`child_file`, `run.rs:594-628`) or the manifest between the embedder's check and Loom's read,
and inside one `run_tree` call between two steps (time of check to time of use). No digest of
bindings exists in Loom today: `contract_digest` in `generated/rust/*/plan.json` is ESS's, and
`content_revision` (`crates/loom-bindings/src/revision.rs:80`) digests workspace artifacts.

**After.**

- `read_bindings_set(store, catalog) -> Result<BindingsSet, Refused>` reads the manifest, walks
  the set as `tree open` does (`open.rs:330-396`: the root, then each `open` executor's
  `bindings` file, relative to the directory of the file that names it, its real path below that
  directory, at most `MAX_TREE_DEPTH` deep), reads each file once with `read_regular`
  (`crates/loom-tree/src/store.rs:24`), and decodes and checks those same bytes (`load`'s
  steps, `open.rs:282-294`). The value holds, per file, its path and SHA-256, the decoded
  document, and the set's digest (D3).
- `run_tree_checked(store, catalog, settings, checked, observer, cancel)` and
  `approve_checked(request, checked)` take the `loom.tree.BindingsSet` the embedder checked.
  `run_tree` and `approve` keep their signatures and pin the set at the call's first read (D5).
- Every read of the set inside a call compares, before anything of that round runs: in
  `run_tree` right after `refuse_held` (`run.rs:204`), before the case store opens, and before
  each loop round's case list (`:237`), before reconcile, take-over and selection; in `approve`
  right after the manifest is read under the lock (`approve.rs:188`), before `reconcile`
  (`:215`) and before the person action is prepared (`:220-222`). `tree_cases` and
  `case_bindings` take their documents from the set read at that point instead of loading files
  again.
- A difference refuses `BindingsChanged` (D4), naming the file in `Refused::file` and in the
  detail: `"the bindings set changed after it was checked: {path} was {old}, is {new}"`, `"{path}
  was added"`, `"{path} is gone"`, or, for the manifest, `"the manifest names {root}; the checked
  set's root is {checked}"`.

## Decisions

- **D1. What the call takes.** Default **a**.
  - a. A digest of the bindings set (`loom.tree.BindingsSet`: root, per-file path and SHA-256,
    set digest), which Loom compares with what it read and refuses on a difference by name. It
    keeps the CLI path (the CLI passes nothing, D5), `tree status` (reads by path, D8) and
    replay (reads no bindings: `crates/loom-tree/src/replay.rs` names none) unchanged, and it is
    the only form that makes Runbook's acceptance hold: the next step after a rewrite is
    refused, not run.
  - b. The loaded bindings value itself in place of a path. Loom would never read a file again,
    so a rewrite between steps would be ignored, and the next step would run under the old
    value, not be refused. Every reader that takes a path (CLI, status) needs a second path.
- **D2. What the set is.** Default **a**.
  - a. The root file and every file reached through `open` executors, statically, as `tree
    open` walks them (`open.rs:330-396`), whether or not that child case is open yet: a child
    opened by a later step runs under a file the embedder checked.
  - b. Only the files of the cases the tree holds now (`tree_cases`): a child opened later is
    read unchecked.
- **D3. Canonical bytes.** Default **a**.
  - a. Each file's SHA-256 is over its raw bytes, as read; the set digest is the SHA-256 of the
    lines `<sha256>  <path>\n` (two spaces, as `sha256sum` prints them), sorted by path, `path`
    the file's real path relative to the root file's directory with `/` separators, the root
    first by being its file name. Any byte change counts, a comment included.
  - b. A digest of the decoded document (a comment or a reorder passes). It needs a canonical
    encoding of `BindingsDocument` that Loom does not have, and what Runbook checked is bytes.
- **D4. The refusal.** Default **a**.
  - a. A new `loom.tree.TreeRefusalReason` `BindingsChanged`, the file named, nothing written.
  - b. `BindingsRefused` with a detail: an embedder cannot tell "the file is invalid" from "the
    file changed after the check".
- **D5. A call given no digest (`tree run`, `tree approve` on the command line, `run_tree`,
  `approve`).** Default **a**.
  - a. The call pins the set at its first read and refuses a change at every later read of the
    same call: between two steps of one `tree run`. The CLI keeps its flags.
  - b. No check without a digest.
- **D6. A step's own program changes a bindings file.** Default **a**.
  - a. Refused like any change: the next read refuses `BindingsChanged`. A tree that edits its
    own bindings is run again by the embedder after a fresh check.
  - b. Allowed when the bindings file is a placed artifact the step's binding writes: the set is
    then pinned again after that step.
- **D7. The form of a refusal in the middle of a run.** Default **a**.
  - a. `run_tree_checked` returns `Err(Refused)` with `BindingsChanged`; the lines of the steps
    already taken stand; nothing is written for the refused round, as for `BindingsRefused`
    today (`run.rs:230` and `:237` return through `?`).
  - b. A `Stopped` line and a new `TreeStop`, which needs `ess/domains/runlog.yaml` too.
- **D8. `tree status`.** Default **a**.
  - a. Unchanged: it reads the set by path (`crates/loom-tree/src/status.rs:81`), runs nothing,
    and prints no digest.
  - b. It prints the set's digest, so a person can compare.
- **D9. Where the embedder gets the set.** Default **a**.
  - a. `read_bindings_set` alone; `open_tree` and `OpenedTree` are unchanged.
  - b. `OpenedTree` also carries the set `tree open` read.
- **D10. Program files beside the bindings** (`program_root: bindings`, resolved in `load`,
  `open.rs:291-294`). Default **a**.
  - a. Not in the digest: a program's own bytes are `story:gate-program-identity-bound-to-spawn`
    (the admission unit verifies the file it launched).
  - b. Digested with the set.
- **D11. How the checked value reaches the call.** Default **a**.
  - a. New entry points `run_tree_checked` and `approve_checked`, so `RunSettings`
    (`run.rs:97-112`) and `ApproveRequest` (`approve.rs:33-60`) keep their members: 25
    `RunSettings {` and 12 `ApproveRequest {` literals in 14 files stay as they are.
  - b. A member `bindings: Option<..>` on both structs: every literal changes.

## ESS first

`ess/domains/tree.yaml`, before the code:

- `loom.tree.BindingsFile` {`path`: String, `sha256`: String} and `loom.tree.BindingsSet`
  {`root`: String (the manifest's `bindings` member, absolute), `files`:
  `List<loom.tree.BindingsFile>` (sorted by path), `digest`: String}, with a comment stating D3;
- `loom.tree.TreeRefusalReason` (`:1198-1229`) gains `BindingsChanged`.

The decoded documents travel beside the generated value in a Rust type of `b10x-loom-tree`
(inferred: `ReadBindingsSet { set: loom::tree::BindingsSet, documents: .. }`), not in ESS. Then
`task generate` and `task suite-generate` (`SLOT_TIMEOUT: 30m`); never hand-edit generated
files. The ess pin stays 0.97.4.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are
POSIX sh, never Python. No test starts `b10x-loom` or lives in the `tree_run` test binary. No
test checks its own wall time. A slot job ends within 10 minutes, or names its time with `slot
--timeout` (at most 30 minutes). No test binary is added.

**`crates/loom-tree/tests/main.rs`, a new module `bindings_set`** (inferred name, a directory as
`runs_spent/` is), in process through `read_bindings_set`, `run_tree_checked`, `run_tree` and
`approve_checked`, under Substrate's `local` adapter with `UNCONFINED_FLAG_REASON`, a fixed
`TreeTime::At`, `max_steps: Some(10)`. Fixture (inferred shape; the unit writes it): one root
case with two writers, `work.first` (`first.sh`) and `work.second` (`second.sh`), each writing
its own artifact; `first.sh` may append `# changed` to the root bindings file when the file
`target/rewrite` exists; a second fixture whose root has an `open` executor naming
`child.bindings.yaml`.

1. `a_rewrite_between_two_steps_refuses_the_next` (red first; Runbook's acceptance).
   `target/rewrite` exists. `run_tree_checked` with the set read before the call: one `action
   finished` line for `work.first`, then `Err` with `BindingsChanged` naming the root file, its
   old and new SHA-256; no line for `work.second`. The same through `run_tree` (D5 a).
   Failing first: `run_tree_checked` does not exist; through `run_tree` today `work.second`
   runs under the rewritten file.
2. `a_change_before_the_call_runs_nothing`. The set is read, then the root file is changed, then
   `run_tree_checked`: `BindingsChanged`, and the case's run log is byte for byte what it was
   (no `evaluated` line). Failing first: as test 1.
3. `a_child_file_change_is_refused_by_name` (D2 a). The second fixture, the child not opened
   yet: the child file changed after the read; `run_tree_checked` refuses naming
   `child.bindings.yaml`. Failing first: as test 1.
4. `a_manifest_that_names_another_file_is_refused`. After the read, the manifest's `bindings`
   member is rewritten to name a valid copy at another path: refused `BindingsChanged` with the
   manifest text of the After section. Failing first: as test 1.
5. `approve_checks_the_set_before_writing`. A tree waiting on a person action; the set read;
   the root file changed; `approve_checked` with that action: `BindingsChanged`, and the store
   (approvals file, run log, evidence) is unchanged. Failing first: as test 1.
6. `an_unchanged_set_runs_as_before`. No rewrite: `run_tree_checked` and `run_tree` give the
   same lines and stop as `run_tree` today; `read_bindings_set` twice gives equal values, and
   its digest equals the D3 formula computed in the test over the fixture files. Failing first:
   none for `run_tree` (the guard).

**`crates/loom-tree/tests/suite_format/mod.rs`**, in process through `run_suite`:

7. `a_suite_cycle_refuses_a_step_after_a_bindings_rewrite` (Runbook's `tree test`). A cycle:
   `open`, a `write` step that creates `target/rewrite`, then `run` with `refusal:
   BindingsChanged` (the suite's expected refusal, `suite/decode.rs:704-711`,
   `suite/run.rs:950-953`). The cycle passes. Failing first: the reason does not exist; the run
   takes `work.second`.

**`crates/loom-tree/tests/embedder`** (the embedder crate, `tests/embedder/src/main.rs`):

8. The exports `read_bindings_set`, `run_tree_checked`, `approve_checked` and the set type are
   used once from outside the crate, as Runbook will call them. Failing first: not exported.

**Gate.** `task check` exits 0; `task slow` passes after the ESS change.

## Out of scope

- A program's own bytes (D10; `story:gate-program-identity-bound-to-spawn`).
- The protocol files of the catalog: `StoredProtocolChanged` already guards them.
- Runbook's own check and its calls; this story gives the calls.
- `tree status` printing a digest (D8 b) and `OpenedTree` carrying the set (D9 b).

## Scope

Derived 2026-11-23 from the plan tree at `d8944d49`. **Cited** means read at the line named;
**inferred** means a reading that could be wrong.

- `ess/domains/tree.yaml` (`:167-179`, `:1198-1229`) — cited; the two entities are new.
- `generated/rust/loom/` and `generated/ess/loom/` — regenerated.
- `crates/loom-tree/src/run.rs` (`:199-204`, `:230-237`, `:547-592`, `:594-628`, `:1866-1888`)
  — cited.
- `crates/loom-tree/src/approve.rs` (`:150`, `:186-188`, `:215-222`, `:428-435`) — cited.
- `crates/loom-tree/src/open.rs` (`:273-299`, `:330-396`) — cited; the shared walk is inferred.
- `crates/loom-tree/src/lib.rs` (exports) — inferred; `crates/loom-tree/src/bindings_set.rs`
  (new module) — inferred.
- `crates/loom-tree/tests/main.rs`, `tests/suite_format/mod.rs`, `tests/embedder/src/main.rs`
  — cited files; the module `bindings_set` is inferred.
- `website/docs/guides/embed-the-runtime.md`, `AGENTS.md`, `CHANGELOG.md` — inferred.
- Read and not changed — cited: `crates/loom-tree/src/store.rs` (`:24`), `src/status.rs`
  (`:81`), `src/replay.rs`, `src/suite/decode.rs` (`:704-711`), `src/suite/run.rs`
  (`:950-953`); `crates/loom-bindings/src/revision.rs` (`:80`).

## Edges

- No `depends_on`: what it builds on (`tree open`'s walk, `tree_cases`, the suite's `refusal`
  member) is on this branch.
- **ESS regeneration.** Every story that changes `ess/` conflicts on the `inputs:` line of
  `generated/ess/loom/synthesis.txt` and on `generated/rust/loom/`: merged one at a time, the
  second regenerating after the first lands.
- **Would collide with**, no edge:
  - `story:run-caps-per-arrival` (proposed): `run.rs` (its `TreeStop::RunsSpent`), `tree.yaml`.
    Another region of `run.rs` (the stop, not the case list). Either order; regenerate.
  - `story:concurrent-cases-in-the-tree-daemon`, `story:standing-tree-rests-quiescent`,
    `story:per-run-program-path` (proposed): `run.rs`, `approve.rs`, `open.rs`. Each orders by
    its own edges; this story first as Runbook's request, the others rebase on the set reader.
  - `story:gate-program-identity-bound-to-spawn` (proposed): the program bytes beside the
    bindings (D10). No shared file named; no order.
  - `story:tree-library-exports-what-an-engine-needs` (active): `lib.rs` exports and
    `tests/embedder`. Textual merge, either order.
  - `story:rework-allowance-after-a-rejection` and `story:writer-not-called-leaves-the-due-set`:
    `crates/loom-tree/tests/main.rs` (one `mod` line each). Either order.
