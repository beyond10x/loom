---
format: aep.planning-md/3
id: review-result:observe-git-in-a-linked-worktree-acceptance-r1
kind: review-result
status: active
title: Plan critic acceptance, first review, on story:observe-git-in-a-linked-worktree of epic:observation-collection-as-executors
relations:
- reviews: story:observe-git-in-a-linked-worktree
- reviews: epic:observation-collection-as-executors
revision: 1
---
needs-revision

story:observe-git-in-a-linked-worktree — the only limit on what gets mounted is the directory layout in steps 1-3, and nothing ties `W` back to this workspace (for example by requiring that `W/gitdir` names `<root>/.git`). A `.git` file written by an earlier action, saying `gitdir: <another repository>/.git/worktrees/<name>`, passes every step and every check the runner makes, so that other repository's whole common directory gets mounted, including its `config`, credentials and objects. No acceptance bullet makes a `.git` point at another repository's worktree directory, at `/` or at a home directory and expect a refusal — .engineering/planning/story/observe-git-in-a-linked-worktree.md:38-42, :139-143

story:observe-git-in-a-linked-worktree — "a `C` reached through a link … is refused `ScopeInvalid`" can never fire, because steps 1-2 already turn `W` and `C` into canonical paths, and a canonical path contains no link. A link in the workspace pointing at an outside `W` is followed with no check, and no bullet tests it — .engineering/planning/story/observe-git-in-a-linked-worktree.md:40, :53; crates/loom-intake-slice/src/confinement.rs:2262-2263

story:observe-git-in-a-linked-worktree — no bullet checks that configuration in the mounted common directory runs nothing. Story 6's `the_repository_configuration_cannot_run_a_program` and its promisor case use a workspace config in a clone. In a linked worktree, git reads `C/config` (and `W/config.worktree`) from the mount, and no bullet puts fsmonitor, hooksPath or a promisor remote there and expects no marker and no connection — .engineering/planning/story/observe-git-changed.md:205-208; .engineering/planning/story/observe-git-in-a-linked-worktree.md:118-148

story:observe-git-in-a-linked-worktree — as written, `a_common_directory_inside_the_workspace_is_refused` produces `Unavailable` from step 3, not `LaunchRefused`. If only `commondir` names a directory inside the workspace, `W` is no longer `C/worktrees/<name>`. The bullet does not say that `.git` and `W` also lie in the workspace, at `<that directory>/worktrees/<name>`, which is the only way the runner's check is ever reached — .engineering/planning/story/observe-git-in-a-linked-worktree.md:142-143, :42

story:observe-git-in-a-linked-worktree — four cases have no test:
- the outcome says a link at `.git` makes the operation `Unavailable`, with no launch;
- it says the same for a file over 4 KiB;
- a `gitdir:` naming a path that does not exist is not covered;
- an unreadable `.git` file is not named at all.

`a_malformed_git_file_is_unavailable` covers only three other cases — .engineering/planning/story/observe-git-in-a-linked-worktree.md:44-46, :139-141

story:observe-git-in-a-linked-worktree — the body says the gate run (`git cat-file` for `engineering.gates`) "gets the mount the same way" in a linked worktree. No acceptance bullet runs `engineering.gates` in a linked worktree, and `story:gate-run-collects-a-gates-document` has no such case either — .engineering/planning/story/observe-git-in-a-linked-worktree.md:101-102; .engineering/planning/story/gate-run-collects-a-gates-document.md:193

story:observe-git-in-a-linked-worktree — § Exposure leaves out parts of the common directory the mount also shows:
- `hooks/`: the hook scripts can be read even though none run;
- `modules/`: the full git directories of submodules, each with its own `config`;
- `info/`, `FETCH_HEAD` and `logs/`.

So the statement handed to the operator is incomplete — .engineering/planning/story/observe-git-in-a-linked-worktree.md:74-77

story:observe-git-in-a-linked-worktree — § Files lists an edit to `AGENTS.md` § Intake and the command line ("the mount beside the helper"), and no acceptance bullet checks for it — .engineering/planning/story/observe-git-in-a-linked-worktree.md:154

What I read: 1 artifact, whole body (`aep plan artifact show story:observe-git-in-a-linked-worktree`, the file with line numbers). Also read: story 6 in full; the gate-run story's linked-worktree references; ledger § Open 17 and 21 in `e7r3-changes.md`; `confinement.rs:78-90, 855-866, 1029-1036, 1111-1136, 1341-1345, 2240-2290`; `generated/rust/intake/src/confinement.rs` (`AppliedConfinement.read_only_roots`, `ToolchainRoot`); Substrate `0.21.0` `crates/substrate-guarded/src/fs.rs:955-958`; `AGENTS.md` grep.

What I could not establish:
- `crates/loom-bindings/src/admission.rs` and `observe/git.rs` do not exist yet (stories 2 and 6 create them), so `linked_worktree` and the launch function could not be checked against code.
- Whether git inside the sandbox accepts `/repository` without a `safe.directory` complaint, given how user IDs map in bubblewrap: not checked, and no run was allowed.
- Out of my lane (parallel-safety): `CHANGELOG.md` is shared with the TCP and DNS stories, and there is no edge between them; the body says so itself (:104-106).
- Confirmed: ESS first "None" holds, since `/repository` and the `linked worktree:` reason prefix are not declarations, and the ledger records the exposure as § Open 21, "Settles: the operator".

```findings
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 42
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the only limit on what gets mounted is the directory layout in steps 1-3, and nothing ties W back to this workspace (for example W/gitdir naming <root>/.git); a .git written by an earlier action pointing at another repository's .git/worktrees/<name> passes every check and mounts that repository's common directory, and no bullet tests another repository, / or a home directory"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 53
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the refusal of a C reached through a link can never fire because steps 1-2 canonicalize W and C first, so a workspace link to an outside W is followed with no check and no bullet tests it"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 118
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet puts fsmonitor, hooksPath or a promisor remote in the mounted C/config (or W/config.worktree) of a linked worktree and expects no marker and no connection; story 6's cases cover only a clone's workspace config"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 142
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "as written, a_common_directory_inside_the_workspace_is_refused gets Unavailable from step 3, because the bullet does not place .git and W inside the workspace at <dir>/worktrees/<name>, the only way the runner's ScopeInvalid check is reached"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 44
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "a link at .git, a file over 4 KiB, a gitdir naming a missing path and an unreadable .git have no test, and the outcome does not name the unreadable case at all; a_malformed_git_file_is_unavailable covers three other cases only"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 102
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body says the gate run's git cat-file gets the mount in a linked worktree, but no acceptance bullet runs engineering.gates in a linked worktree"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 74
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the exposure statement leaves out hooks/, modules/ (submodules' whole git directories with their own config), info/, FETCH_HEAD and logs/, all visible through the mount"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 154
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the AGENTS.md § Intake and the command line edit listed in § Files has no acceptance bullet that checks for it"
```
