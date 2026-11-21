---
format: aep.planning-md/3
id: review-result:engineering-protocols-vendor-command-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on story:engineering-protocols-vendor-command (wave 2026-11-21-w4, unit U4, head 8fc87502)
relations:
- reviews: story:engineering-protocols-vendor-command
revision: 1
---
unit: U4, story:engineering-protocols-vendor-command, head 8fc87502 plus one new untracked test file
verdict: red
cases: executed 80→89, red 1
origin: introduced 9, pre-existing 0, undecided 0
wrote-outside-worktree: 1 directory (`~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w4/u4/adv1/`), plus the test fixtures under temp_dir
needs-coordinator: yes. I mutated `protocols_vendor.rs` in place, which the brief allows and the adversary skill forbids. Details below.

`git --no-pager diff --stat` is empty. My only change is the untracked `crates/loom-xtask/tests/adversary_w20261121w4_u4.rs`. I changed no other file.

**Charter deviation:** to run the 20 mutants I edited `crates/loom-xtask/src/protocols_vendor.rs` in the tree, as the brief says, and restored it after each one. The skill text says to mutate only a copy. After the last restore the file is blob `2f4f32d0`, the same as HEAD, and its sha256 matches my backup. I refreshed the file's timestamp so cargo rebuilds it. During that time the pool showed another loom job running `cargo test -p loom-xtask` (#16301), and I could not tell which tree it was building.

**Findings**

1. The command vendors a branch when the tag is missing. Severity warning, verdict INFEASIBLE, origin introduced.
   - Test: `adv_a_branch_named_refs_tags_x_is_not_vendored_as_tag_x`. Its red line, run alone before the suite: `--tag t9 must be refused (after t1: false), but: exit exit status: 0; stdout: protocols/engineering: 1 files from t9 (bde3c170…)`.
   - Cause: `git rev-parse --verify refs/tags/t9^{commit}` at `protocols_vendor.rs:125-126` also finds `refs/heads/refs/tags/t9`. A checkout with no tag t9 but a branch of that name gets that branch locked as tag `t9`. The acceptance says a missing tag is refused.
   - What reaches it: nothing I found. It needs a ref literally named `refs/{heads,remotes,tags}/refs/tags/<tag>`.
   - Fix: resolve the exact ref with `git show-ref --verify --hash refs/tags/<tag>`, then `rev-parse --verify <oid>^{commit}`.
2. Seven guards had no case: each mutant passed all 17 original cases. Each is now caught by a new case that is green on the restored tree. Severity warning, verdict CONFIRMED, origin introduced.
   - `:252-256`, removing the `GIT_*` scrub. The output came from the repository `GIT_DIR` names. Caught by `adv_git_dir_in_the_environment_does_not_redirect_the_read`: `read …/software-change/1.yaml: No such file or directory`.
   - `:261`, removing `--no-replace-objects`. The replacement's bytes were written. Caught by `adv_a_replace_ref_in_the_checkout_does_not_change_the_bytes` (`left: [110, 97, 109, 101, …]` is `name: replaced`).
   - `:274`, removing `GIT_CONFIG_GLOBAL`. The caller's `~/.gitconfig` is read. Caught by `adv_the_callers_global_git_configuration_is_not_read`: `stderr: --tag "t1": not a tag name`.
   - `:276`, removing `GIT_NO_LAZY_FETCH`. A program from the checkout runs. Caught by `adv_a_promisor_remote_the_checkout_allows_runs_no_program`. A partial clone's own config sets `protocol.ext.allow=always` and `remote.origin.url=ext::sh -c touch% <marker>`, so `cat-file` starts a lazy fetch that runs `sh`. My probe showed the marker file appears (`probe1.sh`). This setting is the only guard: the original suite's partial-clone case passes without it.
   - `:185`, removing the empty-tag refusal. A tag holding only `vocabulary.yaml` would wipe the catalog. Caught by `adv_a_tag_with_no_protocol_file_is_refused`.
   - `:107`, removing the check that the root's `protocols/` is not a link. Files are written through the link. Caught by `adv_a_root_protocols_directory_that_is_a_link_is_refused`.
   - `:109`, removing the not-a-directory check. A `protocols/engineering` that is a file gets moved aside. Caught by `adv_a_vendored_path_that_is_a_file_is_refused`.
3. The doc comment at `:247-249` says "no transport is allowed", which is wrong. A repository's own `protocol.<name>.allow` outranks `-c protocol.allow=never` (`probe1.sh`). Only `GIT_NO_LAZY_FETCH` keeps the transport shut. Severity note, verdict CONFIRMED, origin introduced.

**Suite run**, after my cases existed: `ESS_TOOLCHAIN=0.86.0 CARGO_INCREMENTAL=0 cargo test -p loom-xtask --locked --no-fail-fast` exited 101.
- `adversary_w20261121w4_u4`: `test result: FAILED. 8 passed; 1 failed`.
- `protocols_vendor`: `test result: ok. 17 passed; 0 failed`. Every other binary passed.
- Total 89 executed; the implementor reported 80. Log: `adv1/suite.log`.

**Mutants that still survive**, none of them reachable:
- `--literal-pathspecs`: the pathspec `protocols/` has no glob characters.
- `core.hooksPath` and `core.fsmonitor`: no hook, monitor, pager, filter, textconv, alias, sshCommand, alternateRefsCommand or include fires on the four commands even without these settings (`probe2.sh`). `host_git_hardening` already fails the build if either is removed.
- `protocol.allow=never`: covered by `GIT_NO_LAZY_FETCH`. Removing both is caught by the original suite.
- `credential.helper=`: no transport is ever reached.
- `GIT_CONFIG_NOSYSTEM`: a test cannot control `/etc/gitconfig`.
- Each of the two lock decodes (`:59`, `:73`) is redundant with the other. Removing both is caught by 3 original cases.
- Killed by the original 17: the tag-form check, the mode check, decoding bytes as text, and removing both decodes or both fetch guards.

**Checked and sound**
- A tag of a tag locks the peeled commit and the name as given.
- The repository setting stays the constant whatever the remote is.
- Lock path rule: newline, space, uppercase, Unicode, `01`, depth 3, `lock.json` and slash-in-name tree entries are all refused or come out as duplicates.
- A stale staging or old directory with the same pid is removed first.
- `build.rs` reads only `protocols/engineering`.
- Boundaries: `63668a97..8fc87502` does not touch `protocols/`, `.engineering/`, `crates/loom-protocols` or `release.rs`. No `/home/` literal is added. The only change to an existing test is the added helper entry. `Cargo.lock` changes by one line.

**Could not establish**
- Whether a real upstream tag holds other files under `protocols/<name>/`. The whole tag would then be refused; I did not touch `~/beyond10x/engineering-protocols`.
- Git older than 2.44 ignores `GIT_NO_LAZY_FETCH`, which would make finding 2's fourth bullet live there. This machine runs 2.56.
- The by-hand acceptance run at `0.16.0`.

**Written outside the worktree:** `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w4/u4/adv1/` with `probe1.sh`, `probe1/`, `probe2.sh`, `probe2/`, `run-mine.sh`, `mine.log`, `mine-exit.txt`, `mine-df.txt`, `mutate.py`, `mutants.sh`, `protocols_vendor.rs.orig`, `mutants/` (20 × `.log`, `.df`, `.apply`, plus `summary.txt`), `mutants-exit.txt`, `run-suite.sh`, `suite.log`, `suite-exit.txt`, `suite-df.txt`. Test fixtures go under temp_dir (`~/.cache/claude-tmp/loom-adv-w4u4-*`) and are removed when each test ends.

`git status --short`: `?? crates/loom-xtask/tests/adversary_w20261121w4_u4.rs`. Disk: `target/` 5.1G, 94G free.

```findings
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 126, category: acceptance, severity: warning, verdict: INFEASIBLE, origin: introduced, message: "rev-parse matches refs/heads/refs/tags/<tag>, so a missing tag is vendored from a branch of that name and locked as the tag"}
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 252, category: mutant, severity: warning, verdict: CONFIRMED, origin: introduced, message: "removing the GIT_* scrub passed all 17 cases; GIT_DIR would redirect the read"}
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 261, category: mutant, severity: warning, verdict: CONFIRMED, origin: introduced, message: "removing --no-replace-objects passed all 17 cases; a replace ref changes the written bytes"}
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 274, category: mutant, severity: warning, verdict: CONFIRMED, origin: introduced, message: "removing GIT_CONFIG_GLOBAL passed all 17 cases; the caller's global config would be read"}
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 276, category: mutant, severity: warning, verdict: CONFIRMED, origin: introduced, message: "removing GIT_NO_LAZY_FETCH passed all 17 cases; it is the only guard against a checkout's ext:: promisor remote running a program"}
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 185, category: mutant, severity: warning, verdict: CONFIRMED, origin: introduced, message: "removing the empty-tag refusal passed all 17 cases; such a tag would empty the vendored catalog"}
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 107, category: mutant, severity: warning, verdict: CONFIRMED, origin: introduced, message: "removing the protocols/ link check passed all 17 cases; files would be written through the link"}
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 109, category: mutant, severity: warning, verdict: CONFIRMED, origin: introduced, message: "removing the not-a-directory check passed all 17 cases; a file at protocols/engineering would be moved aside"}
- {file: crates/loom-xtask/src/protocols_vendor.rs, line: 248, category: judgement, severity: note, verdict: CONFIRMED, origin: introduced, message: "the doc says no transport is allowed, but a repository's protocol.<name>.allow outranks -c protocol.allow=never"}
```
