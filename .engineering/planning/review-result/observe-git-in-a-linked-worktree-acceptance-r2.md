---
format: aep.planning-md/3
id: review-result:observe-git-in-a-linked-worktree-acceptance-r2
kind: review-result
status: active
title: Plan critic acceptance, confirming pass, on story:observe-git-in-a-linked-worktree
relations:
- reviews: story:observe-git-in-a-linked-worktree
revision: 1
---
needs-revision

story:observe-git-in-a-linked-worktree — step 5 checks the back-pointer by its canonical path, which follows links. Some other repository may have a worktree entry whose `gitdir` names a path inside this workspace, either a worktree of that repository nested here or a stale entry for one that was removed. A run can put a link at that path pointing to `<root>/.git`; step 5 then passes and that repository's common directory is mounted. So "nothing else" (:85-86) and "step 5 refuses any other" (:153-154) do not hold. Step 5 should compare the back-pointer's path without following links, and one bullet should cover a foreign back-pointer that reaches `<root>/.git` through a workspace link — .engineering/planning/story/observe-git-in-a-linked-worktree.md:65-68

My 8 round-4 findings against revision 5:
1. **Foreign repository, fixed except for the case above.** Step 5 at :65-68 ties `W` to this workspace. Four bullets back it: another repository's worktree at :232-234, `gitdir: /` at :235, a directory that is no git directory at :236-237, a workspace link at :238-239.
2. **Unreachable link refusal, fixed.** The sentence is gone. :93-94 names the runner's one remaining case, and :60-61 says step 5 decides when step 2 follows a link.
3. **Configuration from the mount, fixed.** Bullets at :259-266. :116-120 names what covers it.
4. **Inside-workspace bullet, fixed.** The fixture at :251-254 passes steps 1-5, so step 6 does the refusing.
5. **Layout cases, fixed.** Bullets at :243-247, and :75 now names the unreadable case.
6. **Gate run, fixed.** `a_gate_run_in_a_linked_worktree_reads_the_base` at :212-215.
7. **Exposure, fixed.** :137-144 adds `hooks/`, `modules/`, `info/`, `logs/`, `FETCH_HEAD`, `packed-refs` and `worktrees/`. It is marked § Open 1 of e7r5, "Settles: the operator" (:131-132; ledger :100).
8. **AGENTS.md, fixed.** It is no longer in § Files (:279-282).

The other questions you asked:
- **`LinkedWorktreeRefused`:** one declared member covers every case where something is present but unacceptable (:71-81). § ESS first names it (:189-191), and "Refused" is defined once in a way a test can check (:203-205). `Unavailable` is left only for "no `.git` at all", with a bullet (:227-228). Nothing is recorded as absent or negative.
- **Race between check and mount:** only the run's first git launch reads `.git`. After that `C` and `GIT_DIR` are fixed, so a later rewrite of `.git` changes nothing. Swapping `W` or `C` needs a write outside the workspace, and Loom names no writable path there (`confinement.rs:1034-1036`). A back-pointer in a repository the run can write means a repository inside the workspace, which step 6 refuses (:251-254). Nothing new here beyond the finding above.
- **`safe.directory`:** the statement holds. Substrate `0.21.0` `crates/substrate-host/src/process.rs:74` is `USER_NAMESPACE_ARGV = ["--unshare-user", "--disable-userns"]`, and `git grep '"--uid"\|"--gid"'` at `0.21.0` finds nothing. So bubblewrap keeps the caller's uid.
- **The four configuration bullets:** each has an observable outcome (a marker file, or a listener that gets a connection). They would also pass without the hardening, because none of the four commands reads the index or runs a hook. That still counts as an acceptable guard, so it is not a finding.

What I read: 1 artifact, the whole body (`aep plan artifact show story:observe-git-in-a-linked-worktree` and the stored file with line numbers, revision 5). Also the e7r5 ledger in full, Substrate `0.21.0` `process.rs:60-95` with a grep for uid/gid, and the gate-run story's refusal and `Unavailable` lines.

What I could not establish:
- Whether git with `worktree.useRelativePaths` off writes the back-pointer as a canonical path or only as an absolute one. If it writes a non-canonical absolute path, a comparison that does not follow links has to normalise it without resolving links.
- Out of my lane (scope): the prose § Scope still lists `AGENTS.md` (:300) and says "No ESS change" (:304), and :303 says the call site is not stated. All three contradict revision 5's frontmatter scope and § ESS first.
- Out of my lane (parallel-safety): `observation.yaml`, `generated/rust/loom/` and `CHANGELOG.md` are shared with stories 9 and 10, with no edge between them. Ledger § Open 2 records this.

```findings
- file: .engineering/planning/story/observe-git-in-a-linked-worktree.md
  line: 65
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "step 5 checks the back-pointer by its canonical path, which follows links, so another repository's worktree entry whose gitdir names a path inside this workspace passes once the run puts a link there to <root>/.git; 'nothing else' (:85-86, :153-154) then does not hold, and no bullet covers a foreign back-pointer reaching <root>/.git through a workspace link"
```
