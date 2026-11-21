---
format: aep.planning-md/3
id: review-result:observe-git-in-a-linked-worktree-design-r2
kind: review-result
status: active
title: Plan critic design, confirming pass, on story:observe-git-in-a-linked-worktree
relations:
- reviews: story:observe-git-in-a-linked-worktree
revision: 1
---
needs-revision

story:observe-git-in-a-linked-worktree — § Waits on still says "No story waits on this one" and that tcp and dns "have no edge to it", and § Scope still says "Nothing depends on this story", "No ESS change" and lists `AGENTS.md`; the graph now has `story:tcp-reachability-through-an-aperture depends_on story:observe-git-in-a-linked-worktree`, so both sections should name tcp (and dns through it) as waiting on this story — .engineering/planning/story/observe-git-in-a-linked-worktree.md:182-185, :300, :304, :312; `aep plan artifact graph`

**Earlier findings, checked against revision 5:**
1. Foreign repository: fixed. Step 5 compares the canonical `W/gitdir` with the canonical `<root>/.git` (:65-68). `another_repositorys_worktree_is_refused` (:232-234) and `a_workspace_link_to_an_outside_git_directory_is_refused` (:238-239) are bullets a test can fail.
   - The bound holds. Git writes the back-pointer in the repository that owns the worktree. A stale or another worktree's back-pointer either fails to resolve or names a different `.git`.
   - What can still be mounted is this worktree's own repository only (:85-86, :153-154). That is a bound, so no blocker.
2. Two outcomes for one kind of tampering: fixed. There is one declared member, `LinkedWorktreeRefused` (:71-83). § ESS first names it in `ess/domains/observation.yaml` (:189-191). `Unavailable` now means only "no `.git`", and has its own bullet (:227-228).
3. Gate-run claim: fixed. The mount is added in the one launch function (:47-48, :124-127), so story 8's body does not change. `a_gate_run_in_a_linked_worktree_reads_the_base` (:212-215) reads `gate_run.rs`, which story 8 creates, and an edge records that. The `cat-file` content is now listed in what leaves the child (:159-160).
4. Configuration from the mount: fixed. Four bullets (:256-266), with the precedence of the `-c` flags stated (:113-120).
5. `hooks/` and `modules/`: fixed (:140-142).

**The shape after the revision:**
- **Ordering on the shared files:** `ess/domains/observation.yaml`, `generated/rust/loom/` and `CHANGELOG.md` are shared with tcp and dns. The new edge orders tcp after this story, and dns is ordered through tcp (`dns -> tcp depends_on`).
- **Other stories:** no other story changes `observation.yaml`. `workspace-file-bound-in-engine-configuration` names the file but changes nothing in it (:112). `generated/rust/loom/` is touched by every ESS change across the store, which is the parallel-safety critic's concern.
- **Cycle:** none. This story points to stories 6 and 8. Story 8 points to stories 6 and 7, and story 7 to story 6. Only tcp points into this story, and dns points into tcp. No walk comes back.

**What I read:** the revised body in full, the e7r5 ledger, the bodies and edges of tcp and dns, and `aep plan artifact show` and `graph`.
- Edges: I walked 12 `depends_on` edges, outside the set as well.
- Acceptance reads: 5 traced to a producer (the story-6 fixtures and recording launcher, `LaunchRefused`/`ObserveRun`, the hardening helper, story 8's `gate_run.rs`), and an edge records all 5. `applied.read_only_roots` already exists.

**What I could not establish:**
- I did not read the e7r5 work order (`e7r5-brief.md`). I judged the stored body and the ledger, not the coordinator's six decisions.
- Out of the set: tcp's body names the new edge only in its frontmatter (line 15) and has no Waits-on line for it. The e7r5 ledger's § Open 2 said that line would be needed.
- The ledger's § Open 2 still says the tcp edge "is not added here". That is now stale too.

```findings
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 182
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "§ Waits on still says 'No story waits on this one' and that tcp and dns 'have no edge to it', and § Scope still says 'Nothing depends on this story', 'No ESS change' and lists AGENTS.md; the graph now has story:tcp-reachability-through-an-aperture depends_on story:observe-git-in-a-linked-worktree, so both sections should name tcp (and dns through it) as waiting on this story"
```
