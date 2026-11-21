---
format: aep.planning-md/3
id: story:child-case-own-workspace
kind: story
status: proposed
title: An open executor can give each child case its own workspace directory, and no two open cases of a tree share one by accident
summary: ExecutorSpec.workspace, a JSON pointer to an absolute existing directory; refused on overlap; kept in CaseLink; Loom never creates or removes it
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:child-case-runtime
- depends_on: story:tree-run-command
- depends_on: story:standing-tree-wakes-at-a-host-instant
scope:
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_open_workspace.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area
- confidence: cited
  path: crates/loom-cli/tests/tree_workspaces.rs
- confidence: cited
  path: crates/loom-governor/src/file_store.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: crates/loom-tree/tests/workspace_agent.rs
- confidence: cited
  path: crates/loom-tree/tests/workspace_overlap.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/governor.yaml
- confidence: cited
  path: ess/domains/tree.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:29:07Z", actor: "agent:sdc", revision: 9, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

An `open` executor in a `loom.bindings/1` document may name `workspace`: a JSON pointer whose value
is the child's workspace directory, an absolute path. With `name`, the pointer reads the
arguments. With `each`, every entry of the argument `each` names is an object: its member `name`
(a string) names the child, whose id is `<parent id>.<name>` as `story:child-case-runtime` says,
and the pointer reads the entry. (Without `workspace`, the entries stay the strings
`story:child-case-runtime` takes.) The child then runs in that directory: its placed artifacts
resolve under it, its programs and agent sessions get it as their workspace, and its own `open`
children inherit it unless they name one. A child opened without `workspace` shares its parent's,
as `story:child-case-runtime` says today.

The opening refuses, before any child is created and with the parent's case file byte-identical,
`WorkspaceInvalid` naming the child and the reason: the value is absent or not a string (`Absent`;
also an `each` entry that is not an object or has no string `name`), not absolute
(`NotAbsolute`), not an existing directory (`NotADirectory`), or, after symbolic links are
resolved, equal to, inside or containing the workspace of another case of the tree that is not
terminated, or of another entry of the same opening (`Overlaps`, naming that case or entry; the
root's included). A child that inherits its parent's workspace is the same case as its parent for
this rule, so the opening of a grandchild without `workspace` is never refused `Overlaps`. A
refused opening is a refused step: `tree run` stops `Stuck` naming the case and the refusal, exit 4
(`story:tree-run-command` § Outcome: 4 when stuck), and takes no further step of that case at the
same case revision. The rule is per tree; overlap with a case of another served tree is out of
scope (§ Out of scope). The directory is kept in the child's `CaseLink`, written in the same store
write as the opening, so a reopened store and a restarted daemon continue the case in it. `tree
status` names each case's workspace. A step of a case whose workspace no longer exists is refused
`WorkspaceMissing` naming the case and the path, and the case is stuck. Loom never creates,
empties or removes the directory: after the child terminates it stays as it was.

With its own workspace, a file the child writes moves no artifact of its parent; the parent learns
of the child through its `case_outcome` record only.

## Why

"each story in its own work tree" (`.engineering/drafts/e4-request.md`, item 2); "Each admitted
child gets its own managed worktree from `worktree create` and its own build directory ...,
replacing the shared `work/`" (`~/beyond10x/sdc/docs/orchestration.md` § 2.4). Today
`story:child-case-runtime` § Outcome says "The cases of one tree share the root's workspace", and
its § Out of scope lists "A workspace per child (`story:child-case-own-workspace`)";
`story:bound-artifact-revisions` § Out of scope lists "One workspace per child case". Two
concurrent writers in one workspace would race on the placed paths, so this story is what makes
`story:concurrent-cases-in-the-tree-daemon` useful. Loom does not make the work tree: in the tree
the arguments come from the binding (a literal or an `arguments_from` program), never from a model
(`story:tree-run-command`'s model-free executor), so the SDC's `arguments_from` program can run
`worktree create` and print each story's directory; confined, it needs the common `.git` writable,
which is `story:writable-host-root` (accepted by session `sdc` on 2026-11-21; no edge here). The
`open` executor today takes `name`, `each` and `bindings` (`loom.bindings.ExecutorSpec`,
`ess/domains/bindings.yaml:30-63`: "`name` or `each` (one of them) is the argument that names the
child case"), and every path the loader checks is relative to the workspace
(`crates/loom-bindings/src/load.rs:286-292`). A new executor member enters two allow-lists: the
decoder's `EXECUTOR` (`crates/loom-bindings/src/decode.rs:27`) and the loader's `members` table in
`fit` (`load.rs:228-244`), which refuses a member on another executor `MemberNotForExecutor`. An
`arguments_from` program must print one JSON object (`story:bound-program-result-table` §
Acceptance: one "that exits non-zero or prints anything but one JSON object is refused
`ArgumentsUnreadable`").

## Waits on

- `story:child-case-runtime`: the `open` effect port in `crates/loom-bindings`, `CaseLink` in
  `ess/domains/governor.yaml` and the tree listing this story extends.
- `story:tree-run-command`: `crates/loom-tree`, `TreeStatus` in `ess/domains/tree.yaml`, and the
  root's workspace (its flag not read; the story names none).
- `story:bound-program-result-table` and `story:bound-agent-node`: the program port and the agent
  port that this story hands the case's workspace; both are ancestors through the chain
  (`story:run-caps-per-arrival` depends on both), and both were last changed before this story by
  `story:run-caps-per-arrival` and `story:tree-usage-and-model-budget`.
- `story:standing-tree-wakes-at-a-host-instant`: the story before this one on
  `ess/domains/tree.yaml` (`TreeStatus`) and `crates/loom-tree`; through the chain also
  `story:run-caps-per-arrival`, the earlier story on `ess/domains/bindings.yaml` and
  `crates/loom-bindings/src/load.rs`.

Comes before, on shared files: `story:concurrent-cases-in-the-tree-daemon`
(`ess/domains/bindings.yaml` `WorkspaceInvalid`, `ess/domains/tree.yaml` `TreeStatus`,
`crates/loom-tree`), and through it `story:capacity-given-to-an-admission-program`
(`ess/domains/bindings.yaml`, `crates/loom-bindings/src/load.rs`, the program port).

Unordered with this story on shared files, under the store's rule that the second to land rebases
(the epic's § Order lists them): `story:bindings-child-path-refusal-names-the-bindings-directory`
and `story:agent-session-through-substrate` (`ess/domains/bindings.yaml`, `load.rs`);
`story:decision-request-file-store`, `story:authority-records-bound-and-grounded-in-canon` and
`story:engineering-registry-calls-replaced` (`ess/domains/governor.yaml`, `file_store.rs`);
`story:tree-waits-open-decision-requests` and the other stories on `ess/domains/tree.yaml` and
`crates/loom-tree`. No edge is added for them.

## ESS first

`ess/domains/bindings.yaml`: `ExecutorSpec` gains `workspace` (optional JSON pointer, `Open`
only), the refusals `WorkspaceInvalid` (child, path, reason `Absent`, `NotAbsolute`,
`NotADirectory`, `Overlaps` with the other case or entry) and `WorkspaceMissing` (case, path).
`ess/domains/governor.yaml`: `CaseLink` gains optional `workspace`. `ess/domains/tree.yaml`: the
per-case entry of `TreeStatus` gains `workspace`. On that commit `task drift` fails and
`crates/loom-cli/tests/tree_workspaces.rs` does not compile.

## Acceptance

Files added to `crates/loom-cli/tests/fixtures/area/`:

- `wave.protocol.yaml`, `wave/1`: one `case_open` action `wave.open` opening `item/1` children
  from an `each` argument, and one outcome, `done`, legitimate when every child is terminated.
- `item.protocol.yaml`, `item/1`: a placed artifact `out`; a writer `out.write` (may produce
  `out_written` about `out`); a `case_open` action `step.open` opening one `step/1` child once
  `out_written` is held; outcome `done` when the step child is terminated.
- `step.protocol.yaml`, `step/1`: a placed artifact `step-out`; a writer `step-out.write`; outcome
  `done` when `step-out_written` is held.
- `bin/list-dirs`: an `arguments_from` stub that prints `{"dirs": X}`, where `X` is the JSON array
  in the file `dirs.json` the test writes; each entry is an object `{"name": ..., "dir": ...}`.
- `wave.bindings.yaml`: `wave.open` with `arguments_from: bin/list-dirs`, `each: dirs` and
  `workspace: /dir`, its children bound by `item.bindings.yaml`. `wave-shared.bindings.yaml`: the
  same without `workspace`, the entries then the names alone (`bin/list-dirs` given a `dirs.json`
  of strings).
- `item.bindings.yaml`: `out.write` bound to `bin/write-out` (writes the line `item` to `out` in
  its working directory, exit 0), `step.open` with `name: step` and no `workspace`, its child bound
  by `step.bindings.yaml` (`step-out.write` to `bin/write-out` with the path `step-out`).
  `item-agent.bindings.yaml`: the same with `out.write` bound to the agent adapter `stub` and an
  evidence entry `from: effect`.

`crates/loom-cli/tests/tree_workspaces.rs` drives `tree run`; each bullet ends as it says, no
model. The test's temporary directory holds the root's workspace `root/` and the directories
`a/`, `b/`, `c/` beside it:

- With `dirs.json` naming `a`, `b`, `c` (names `a`, `b`, `c`, absolute paths): the children are
  `<root>.a`, `<root>.b`, `<root>.c`; each item's `out.write` creates `out` in that item's
  directory only, each item's `out` revision moves and the root's `out` (a placed path of the
  root's workspace) does not; each grandchild, opened without `workspace`, is not refused and
  writes `step-out` into its item's directory, and `tree status` names the item's directory for
  the item and for its grandchild; the tree terminates `done`, exit 0.
- With `wave-shared.bindings.yaml` and the names `a`, `b`, `c`: the items write into `root/`, and
  the root's `out` moves at the next evaluation, as `story:child-case-runtime`'s acceptance has
  it; exit 0.
- Refusals, each a `dirs.json` of two entries, the first `{"name": "a", "dir": <a/>}` and the second
  as below (the test also creates `a/sub/`, `p/q/` and a symbolic link `l` to `a/`): the opening
  is refused `WorkspaceInvalid` with the reason named, no child case file is written, the root's
  case file is byte-identical to before the opening, and `tree run` exits 4 `Stuck` naming the
  root:
  - an entry with no `dir` member, one whose `dir` is `7`, one with no `name`, and the string
    `"b"`: `Absent`;
  - the relative path `b`: `NotAbsolute`;
  - a path that does not exist, and a regular file: `NotADirectory`;
  - `root/`: `Overlaps` naming the root;
  - `a/sub/` (inside the first entry's): `Overlaps` naming the first entry;
  - `l`: `Overlaps` naming the first entry;
  - with the first entry `p/q/` instead of `a/`, the second `p/` (containing it): `Overlaps`
    naming the first entry.
- After the store is reopened between two steps, an item's next step runs in its directory and
  `tree status` names it.
- With an item's directory removed between two steps, the next step is refused `WorkspaceMissing`
  and `tree run` exits 4 naming the case.
- After an item terminates, its directory and every file in it are unchanged.

`crates/loom-tree/tests/workspace_overlap.rs`, a library test of the overlap rule on a tree listing
the test builds (a root and two items, one terminated, one open, each with its directory; the
command line has no opening that follows a terminated sibling, so this rule is checked here): a
new entry naming the terminated item's directory is accepted; one naming the open item's
directory, a directory inside it, or one containing it is refused `Overlaps` naming the open item.

`crates/loom-tree/tests/workspace_agent.rs` drives the tree library with `wave.bindings.yaml`
whose children are bound by `item-agent.bindings.yaml`, and a stub agent adapter registered by the
test (no model) that records the workspace it was handed and writes `out` there: for each item the
recorded workspace is the item's directory and `out` lands in it; the tree terminates `done`.

`crates/loom-bindings/tests/bound_open_workspace.rs`: a bindings file with `workspace` on a
`program` executor is refused at load `MemberNotForExecutor` naming the action and
`executor.workspace`; a `workspace` value that is not a JSON pointer is refused naming the action
and the member.

- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/governor.yaml`, `ess/domains/tree.yaml`, generated
`generated/rust/loom/src/{bindings,governor,tree}.rs`; `crates/loom-bindings/src/decode.rs`
(`EXECUTOR` at `:27` gains `workspace`), `crates/loom-bindings/src/load.rs` (`fit`'s `members` at
`:228-244` gains `workspace` for `Open`; the overlap and directory checks), the `open` port's
module (named by `story:child-case-runtime`, not read), and the program port's and the agent
port's modules (in `crates/loom-bindings`, which exists; neither `story:bound-program-result-table`
nor `story:bound-agent-node` names the module), each handed the case's workspace as its working
directory; `crates/loom-governor/src/file_store.rs` (the `CaseLink` codec) and
`crates/loom-governor/src/lib.rs` (the workspace in `CaseState`, which `stored_case` and
`held_case` map); `crates/loom-tree` (a case's workspace, `tree status`);
`crates/loom-cli/tests/tree_workspaces.rs`, `crates/loom-tree/tests/workspace_agent.rs`,
`crates/loom-tree/tests/workspace_overlap.rs`, `crates/loom-bindings/tests/bound_open_workspace.rs`
(new); `crates/loom-cli/tests/fixtures/area/` (adds `wave.protocol.yaml`, `item.protocol.yaml`,
`step.protocol.yaml`, `wave.bindings.yaml`, `wave-shared.bindings.yaml`, `item.bindings.yaml`,
`item-agent.bindings.yaml`, `step.bindings.yaml`, `bin/list-dirs`, `bin/write-out`);
`CHANGELOG.md`.

## Out of scope

Running cases at once (`story:concurrent-cases-in-the-tree-daemon`). Making, merging or removing a
Git work tree. A writable `.git` for a confined program (`story:writable-host-root`). The daemon's
rule that a workspace sits directly under its `workspace_root`
(`story:concurrent-cases-in-the-tree-daemon` checks it at the opening). Overlap with a workspace
of a case of another tree: the rule here is per tree; across served trees the one-writer rule of
`story:concurrent-cases-in-the-tree-daemon` keeps two steps out of one directory.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `ess/domains/bindings.yaml` plus `crates/loom-bindings/src/{decode,load}.rs` (the `workspace` member, its refusals, the load-time check) — cited (story § ESS first, § Files)
- **Files (exist now):** `ess/domains/bindings.yaml:37` (`ExecutorSpec`), `ess/domains/governor.yaml:107` (`StoredCase`, where `CaseLink` is meant to go), `crates/loom-bindings/src/decode.rs:353` (the one `ExecutorSpec { .. }` constructor), `crates/loom-bindings/src/load.rs:286` (`in_workspace`), `crates/loom-governor/src/file_store.rs:310,369,503` (`stored_case`, `held_case`, `encode_case`) — cited (story § Files; lines read from the tree)
- **Files (do not exist at `61fd500b`, created by the stories this one waits on):** `ess/domains/tree.yaml`, `crates/loom-tree`, the `open` port module, the program-port and agent-port modules, and `crates/loom-cli/tests/fixtures/area/` — cited (story § Files and § Waits on; `ls crates` shows no `loom-tree`, `ls ess/domains` no `tree.yaml`)
- **Files (new tests):** `crates/loom-cli/tests/tree_workspaces.rs`, `crates/loom-tree/tests/workspace_agent.rs`, `crates/loom-tree/tests/workspace_overlap.rs`, `crates/loom-bindings/tests/bound_open_workspace.rs` — cited
- **Files (new fixtures in `crates/loom-cli/tests/fixtures/area/`):** `wave.protocol.yaml`, `item.protocol.yaml`, `step.protocol.yaml`, `wave.bindings.yaml`, `wave-shared.bindings.yaml`, `item.bindings.yaml`, `item-agent.bindings.yaml`, `step.bindings.yaml`, `bin/list-dirs`, `bin/write-out` — cited
- **Symbols:** `ExecutorSpec` (`workspace`), `WorkspaceInvalid`, `WorkspaceMissing`, `CaseLink`, `TreeStatus`, `StoredCase` — cited
- **Symbols (the allow-lists a new member must also enter):** `EXECUTOR: [&str; 12]` (`decode.rs:25`) and `members: [(&str, bool, Executor); 11]` in `fit` (`load.rs:228-244`) — cited from the tree. The story does not name them, but `bound_open_workspace.rs` ("a program executor with `workspace` is refused naming the member") can only pass through them.
- **Also likely:** `crates/loom-governor/src/lib.rs` — inferred. `stored_case` and `held_case` map `CaseState` to `StoredCase` and back, so the workspace must live in `CaseState`. The story's § Files does not list `lib.rs`.
- **Also likely:** `crates/loom-conformance` and `ess/scenarios/` — inferred, only if `tree.yaml` gains a scenario for the refusals (`tree-run-command.md:205` has the same caveat).
- **Documents:** none beyond the house-rule files. This is a code story.
- **Confidence:** high for `bindings.yaml`, `load.rs`, `decode.rs`, `governor.yaml` and `file_store.rs`, which I read. Medium for `loom-tree`, `tree.yaml` and the three port modules, because their files do not exist yet and only the stories that create them describe them.
- **Would collide with** (reachability read from `aep plan artifact graph --format json`; "ordered" means a `depends_on` path already runs between the two):
  - `ess/domains/bindings.yaml`, `load.rs`, `decode.rs`:
    - Ordered before this story (ancestors): `story:child-case-runtime`, `story:run-caps-per-arrival`, `story:bound-program-result-table`, `story:bound-agent-node`, `story:bound-artifact-revisions`, `story:record-copy-between-cases`, `story:action-limits-enforced`, `story:named-egress-per-action`, `story:confinement-level-required`, `story:agent-node-run-records`, `story:engine-configuration-document`, `story:limit-hits-in-run-log`, `story:bindings-document` (implemented).
    - Ordered after (descendants): `story:capacity-given-to-an-admission-program`, `story:concurrent-cases-in-the-tree-daemon` (adds the `OutsideWorkspaceRoot` reason to the `WorkspaceInvalid` this story creates), `story:flow-limits-recorded-as-events`.
    - **Unordered:** `story:bindings-child-path-refusal-names-the-bindings-directory` (its body cites `load.rs:203,:286-294`, the same `in_workspace` this story reads, and may add a variant in `bindings.yaml`). `story:agent-session-through-substrate` (extends `AgentRun` in `bindings.yaml`; it depends on `bound-agent-node`, but nothing orders it against this story).
  - `ess/domains/governor.yaml` and `file_store.rs`:
    - Ordered before: `story:child-case-runtime` (creates `CaseLink` and the codec change), `story:bound-artifact-revisions`.
    - **Unordered:** `story:decision-request-file-store` (adds types to `governor.yaml`, cites `file_store.rs:133-146`, builds a new store beside it). `story:authority-records-bound-and-grounded-in-canon` (`governor.yaml` types, different from `StoredCase`). `story:engineering-registry-calls-replaced` (edits the `file_store.rs:15` doc comment). `story:decision-inbox-commands`, which only reads `file_store.rs`.
  - `ess/domains/tree.yaml` and `crates/loom-tree`:
    - Ordered before: `story:tree-run-command`, `story:standing-tree-wakes-at-a-host-instant`, `story:standing-tree-rests-quiescent`, `story:tree-usage-and-model-budget`, `story:run-caps-per-arrival`, `story:action-limits-enforced`, `story:tree-daemon`, `story:limit-hits-in-run-log`.
    - Ordered after: `story:concurrent-cases-in-the-tree-daemon`, `story:capacity-given-to-an-admission-program`, `story:flow-limits-recorded-as-events`.
    - **Unordered:** `story:tree-waits-open-decision-requests` (`tree-waits-open-decision-requests.md:141`: `TreeStatus` gains per-case requests, the same per-case entry this story extends). `story:budget-and-limit-stops-raise-decision-requests`, `story:non-independent-review-becomes-decision-request`, `story:requests-from-canon-pending-decisions` and `story:run-log-references-exec-file-changes` (all add modules and fields to `tree.yaml` and `loom-tree`). `story:ingress-routes-per-action` and `story:confinement-level-on-oci-and-pod` (each adds only a test file to `loom-tree`, so a low-risk collision). `story:authority-records-bound-and-grounded-in-canon` (`CaseApproval` in `tree.yaml`).
  - `crates/loom-cli/tests/fixtures/area/`: only ordered stories use it: `capacity-given-to-an-admission-program`, `concurrent-cases-in-the-tree-daemon`, `tree-usage-and-model-budget`, `flow-limits-recorded-as-events`, `run-caps-per-arrival`, `standing-tree-wakes-at-a-host-instant`, `standing-tree-rests-quiescent`. No other story creates a file with this story's fixture names, so there is no file collision. `concurrent-cases-in-the-tree-daemon` reuses `wave.bindings.yaml` and `bin/list-dirs` (`concurrent-cases-in-the-tree-daemon.md:143,147,197`), which is consistent with this story creating them.
  - This is a wide surface: seven or more unordered stories sit on `tree.yaml` and `loom-tree`. Order this story against `tree-waits-open-decision-requests` before running either concurrently.
- **Contradicted by the code:** see Story defects.
- **Safety fact:** adding `workspace` to `ExecutorSpec` breaks no other constructor, because `decode.rs:353` is the only `ExecutorSpec {` literal in `crates/` (`git grep -n 'ExecutorSpec {' -- crates`). The generated struct is at `generated/rust/loom/src/bindings.rs:168`. Step 2, unproven. Whether any non-Rust reader of the stored `StoredCase` file exists was not checked.
