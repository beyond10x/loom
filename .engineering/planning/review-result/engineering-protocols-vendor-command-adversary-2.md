---
format: aep.planning-md/3
id: review-result:engineering-protocols-vendor-command-adversary-2
kind: review-result
status: active
title: Adversary pass 2 (last) on story:engineering-protocols-vendor-command (wave 2026-11-21-w4, unit U4, head 9dd940d7)
relations:
- reviews: story:engineering-protocols-vendor-command
revision: 1
---
unit: U4, story:engineering-protocols-vendor-command, head 9dd940d7 plus one new untracked test file
verdict: green
cases: executed 92→99, red 0
origin: introduced 4, pre-existing 0, undecided 0
wrote-outside-worktree: 1 directory (`~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w4/u4/adv2/`), plus the test fixtures under temp_dir
needs-coordinator: no

None of my new cases fails. Both corrections hold: my pass-1 red case now passes, and the version gate works. Four smaller findings remain, all about prose, a weak check and one error message. One of my pass-1 cases no longer guards what it used to (see Mutants).

`git --no-pager diff --stat` is empty. My only change is the untracked `crates/loom-xtask/tests/adversary_w20261121w4_u4_pass2.rs`, with 7 cases. It is green on its own (`test result: ok. 7 passed`) and in the package suite.

**How I mutated:** in the tree, while the coordinator said no other agent was building there. I restored the file after each mutant and checked its blob id each time. At the end `git hash-object` gives `03d1dc80`, the same as `HEAD:crates/loom-xtask/src/protocols_vendor.rs`.

**Findings**

1. The doc comment at `protocols_vendor.rs:293-295` claims too much. It calls `ext` "the transport that starts a program", but other transports start one too. Severity warning, origin introduced.
   - With `GIT_NO_LAZY_FETCH` absent and both `-c protocol.*` settings in place, a checkout that allows `file` and sets `remote.origin.uploadpack` runs that program. One that allows `ssh` and sets `core.sshCommand` runs that one (`adv2/probe3.sh`: `marker-uploadpack` and `marker-ssh` both appear).
   - So for those transports, `GIT_NO_LAZY_FETCH` plus the version gate is the only guard. The comment should say so.
   - Case: `adv2_a_file_promisor_remote_with_an_upload_pack_program_runs_nothing`. It is green now and fails under mutants p3 and p4.
2. `AGENTS.md:56-61` describes the command but does not say it needs Git 2.44.0 or newer. The `--help` text in `main.rs` does not say so either. Severity warning, origin introduced.
3. The `host_git_hardening.rs:523-535` check only looks for the five setting names as quoted literals, not their values. I showed this by reading. Severity note, origin introduced.
   - `.env("GIT_NO_LAZY_FETCH", "0")` still contains `"GIT_NO_LAZY_FETCH"` and passes the check.
   - Mutant p4 shows that this change brings the lazy fetch back. Only the pass-2 `file://` case catches it.
4. A tag that points at a tree is refused, but with the wrong reason. The refusal says `tag tree-tag: not in the checkout …; fetch it there first`, though the tag is there (`protocols_vendor.rs:173-174`). The failure to peel the tag to a commit reuses the missing-tag message. Severity note, origin introduced.

**Mutants re-run**

- Caught by a case:
  - p1, `show-ref` back to `rev-parse`: caught by my pass-1 branch case and `adv2_no_other_ref_spelled_refs_tags_x_is_taken_for_tag_x`.
  - p3, `GIT_NO_LAZY_FETCH` removed: caught only by the pass-2 `file://` case. My pass-1 `ext::` case now passes under p3, because `-c protocol.ext.allow=never` stops `ext` by itself.
  - p4, `GIT_NO_LAZY_FETCH` set to "0": caught by the same `file://` case.
  - p5, version gate removed: caught by `adv2_a_git_older_than_2_44_on_path_is_refused_before_any_other_git_call`.
  - p6, `GIT_*` scrub removed: caught by the pass-1 `GIT_DIR` case.
  - p7, `--no-replace-objects` removed: caught by the pass-1 replace case and `adv2_a_replace_ref_for_the_tag_object_does_not_move_the_tag`.
- Survives: p2, `-c protocol.ext.allow=never` removed. Nothing can observe it while `GIT_NO_LAZY_FETCH` holds, and the hardening scan already fails the build if the literal is removed.

**Checked and sound**

- Exact ref:
  - An annotated tag and a tag of a tag both peel to the commit.
  - A packed ref resolves.
  - A tag ref that is a directory is refused.
  - `refs/refs/tags/x`, `refs/tags/refs/tags/x`, `refs/remotes/refs/tags/x` and `…/x/HEAD` are not taken for tag `x`.
  - A replace ref for the tag object is ignored.
  - A symbolic ref at `refs/tags/x` resolves to its target. That is the checkout's own ref, so I am not raising it.
- Version gate:
  - It runs before every other Git call: the wrapper's log holds only the `--version` call.
  - Git not on `PATH`, or a `git` that reports 2.43.0, is refused with nothing written.
  - Every later call uses the same `git` lookup on `PATH`.
- `supported_git`, by reading: it compares numbers, not strings, so `10.0.0` and `2.100.0` pass. `2.44`, an empty string, text before the version and an overflowing number are all refused as unreadable.
- `-c protocol.ext.allow=never` alone does stop the `ext::` promisor remote (probe3).
- With `GIT_NO_LAZY_FETCH` set, the `file://` program does not run (probe3).
- The hardening test only gained lines. Its checks of the three older helpers are unchanged.
- The test directory has had no change since `796b0fc3`, and my pass-1 file is byte-identical to the copy as delivered.
- Package suite: `ESS_TOOLCHAIN=0.86.0 CARGO_INCREMENTAL=0 cargo test -p loom-xtask --locked --no-fail-fast` exited 0, with 99 passed and 0 failed (`adv2/suite.log`).

**Could not establish:** whether Git `2.44.0-rc0`, which the gate accepts as 2.44.0, already honours `GIT_NO_LAZY_FETCH`.

**Written outside the worktree:** in `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w4/u4/adv2/`: `probe3.sh`, `probe3/`, `run-pass2.sh`, `pass2.log`, `pass2-exit.txt`, `pass2-df.txt`, `mutate.py`, `mutants.sh`, `protocols_vendor.rs.orig`, `mutants/` (7 × `.log`, `.df`, `.apply`, plus `summary.txt`), `mutants-exit.txt`, `run-suite.sh`, `suite.log`, `suite-exit.txt`, `suite-df.txt`.

`git status --short`: `?? crates/loom-xtask/tests/adversary_w20261121w4_u4_pass2.rs`

```findings
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 294, category: contract-drift, severity: warning, verdict: CONFIRMED, origin: introduced, message: "the doc calls ext the transport that starts a program, but file (remote.uploadpack) and ssh (core.sshCommand) also start one, and only GIT_NO_LAZY_FETCH plus the version gate stop them"}
- {file: AGENTS.md, line: 56, category: contract-drift, severity: warning, verdict: CONFIRMED, origin: introduced, message: "the description of protocols-vendor does not say it needs Git 2.44.0 or newer"}
- {file: crates/loom-intake-slice/tests/host_git_hardening.rs, line: 523, category: mutant, severity: note, verdict: CONFIRMED, origin: introduced, message: "the check matches setting names only, so .env(\"GIT_NO_LAZY_FETCH\", \"0\") passes it while lazy fetch is back"}
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 174, category: judgement, severity: note, verdict: CONFIRMED, origin: introduced, message: "a tag pointing at a tree is refused as not in the checkout, fetch it first, though the tag is present"}
```
