---
format: aep.planning-md/3
id: review-result:observe-git-in-a-linked-worktree-design-r1
kind: review-result
status: active
title: Plan critic design, first review, on story:observe-git-in-a-linked-worktree of epic:observation-collection-as-executors
relations:
- reviews: story:observe-git-in-a-linked-worktree
- reviews: epic:observation-collection-as-executors
revision: 1
---
needs-revision

story:observe-git-in-a-linked-worktree — the step-3 layout check (`W` = `C/worktrees/<name>`) does not tie `C` to this workspace, so a `.git` file written by an earlier action can name any other repository's `worktrees/<name>` on the host, such as another managed worktree under `~/.local/state/worktree`, and that repository's whole common directory (objects, refs, `config` with credentials) gets mounted; the body needs a check that `W/gitdir` names `<root>/.git`, canonical, and a bullet that fails on a foreign repository — .engineering/planning/story/observe-git-in-a-linked-worktree.md:42, :139-143

story:observe-git-in-a-linked-worktree — a `.git` or `commondir` that breaks the layout gives `Unavailable` with a free-text `linked worktree:` reason, while a `C` inside the workspace gives Refused `LaunchRefused`; one kind of tampering gets two outcomes, and the layout refusal should be a declared `ObserveRefusal` member, which makes § ESS first non-empty — .engineering/planning/story/observe-git-in-a-linked-worktree.md:44-46, :108-110, :142

story:observe-git-in-a-linked-worktree — the outcome says the gate run's git launches (`rev-parse`, `cat-file`) also get the mount, but the title, the "What leaves the child" sentence (it leaves out the `cat-file` blob content) and every acceptance bullet cover `git.changed` only; either add a bullet for `engineering.gates` in a linked worktree and name the blob in what leaves, or drop the claim — .engineering/planning/story/observe-git-in-a-linked-worktree.md:86-88, :102, :118-150

story:observe-git-in-a-linked-worktree — no bullet shows that a `config` now read from the mounted common directory (fsmonitor, hooks path, promisor remote) still runs nothing; story 6's two bullets build that config inside a clone's workspace only — .engineering/planning/story/observe-git-in-a-linked-worktree.md:118-150, .engineering/planning/story/observe-git-changed.md:205-208

story:observe-git-in-a-linked-worktree — the exposure list names refs, objects, stash, reflogs, other worktrees and `config`, but not `hooks/` (scripts the child can read) or `modules/` (each submodule's whole repository and its own `config`) — .engineering/planning/story/observe-git-in-a-linked-worktree.md:74-77

**What I read:** 1 artifact under review, plus `story:observe-git-changed` and `story:gate-run-collects-a-gates-document` in full and the e7r3 ledger, the sdc answer and both briefs.
- Commands: `aep plan artifact show`, `relations`, `graph`, `validate` (it reports `valid`; its warnings concern older review-results only).
- Code checked: `confinement.rs:78-90`, `:855-866`, `:1020-1040`, `:1100-1140`, `:1335-1350`, `:2235-2290` at the tree's HEAD, and Substrate `0.21.0` `crates/substrate-guarded/src/fs.rs` `validate_api_path`. Every one of those citations matches the code.
- Edges: I walked 10 `depends_on`/`decomposes` edges, outside the set as well, up to `gate-program-identity-bound-to-spawn` and `bound-commands-run-confined`. No `depends_on` edge points into this story, so there is no cycle.
- Acceptance reads: 5 traced to a producer (the `observe_git.rs` fixtures and the recording launcher, `LaunchRefused`, the linked-worktree case it replaces, the git helper the hardening scan counts). All 5 come from `story:observe-git-changed`, and an edge records it. `TestExecution.applied.read_only_roots` already exists.
- The edge to `gate-run-collects-a-gates-document` is there because both edit `observe/git.rs` (no acceptance reads gate-run's output), so it is not a serialising chain.

**What I could not establish:**
- Whether earlier actions can really rewrite `<root>/.git`. Story 6 says children write only under `target/`; I took the brief's premise that the workspace is writable.
- The tree's HEAD is `b50781ed`, not `61fd500b`. The cited lines still match.
- The runner's "reached through a link" refusal (:53-54) can never fire, because Loom hands it a `C` that is already canonical. This is an unease, not a finding.
- Out of my lane (parallel safety): the story shares `CHANGELOG.md` with `story:tcp-reachability-through-an-aperture` and `story:dns-resolution-through-a-resolver-aperture`, and no edge records it. The story says so itself at :104-106.
- I found no cycle and no split abstraction.

```findings
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 42
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the step-3 layout check (W = C/worktrees/<name>) does not tie C to this workspace, so a .git file written by an earlier action can name any other repository's worktrees/<name> on the host, such as another managed worktree under ~/.local/state/worktree, and that repository's whole common directory (objects, refs, config with credentials) gets mounted; the body needs a check that W/gitdir names <root>/.git, canonical, and a bullet that fails on a foreign repository"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 44
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "a .git or commondir that breaks the layout gives Unavailable with a free-text 'linked worktree:' reason, while a C inside the workspace gives Refused LaunchRefused; one kind of tampering gets two outcomes, and the layout refusal should be a declared ObserveRefusal member, which makes ESS first non-empty"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 102
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome says the gate run's git launches (rev-parse, cat-file) also get the mount, but the title, the 'What leaves the child' sentence (it leaves out the cat-file blob content) and every acceptance bullet cover git.changed only; either add a bullet for engineering.gates in a linked worktree and name the blob in what leaves, or drop the claim"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 118
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no bullet shows that a config now read from the mounted common directory (fsmonitor, hooks path, promisor remote) still runs nothing; story 6's two bullets build that config inside a clone's workspace only"
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 74
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the exposure list names refs, objects, stash, reflogs, other worktrees and config, but not hooks/ (scripts the child can read) or modules/ (each submodule's whole repository and its own config)"
```
