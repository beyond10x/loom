---
format: aep.planning-md/3
id: review-result:adversary-w47-loom-host-git-hardening-pass-1
kind: review-result
status: active
title: Wave 2026-10-05-w47 adversary, loom story:host-git-hardening, pass 1
relations:
- reviews: story:host-git-hardening
revision: 1
---
```
unit: story:host-git-hardening, worktree loom-w47-host-git-hardening at a5b8612 (impl/host-git-hardening) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 31→34, red 3
origin: introduced 4 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 path (build dir, now deleted)
needs-coordinator: none
```

Cases added: `crates/loom-intake-slice/tests/adversary_host_git.rs` (3, all red): a second run on a
workspace whose test planted a filter in `.git/config`; a planted `.git/commondir` moving the
config past the digest; an included work-tree config the model edits.

Held: hooks and fsmonitor overridden on every call; gpg.program blocked by commit.gpgsign=false; no
temp hooks directory left behind; a refused edit leaves HEAD and the work tree unchanged; `.git`
paths refused by `inside()`; network-only and interactive keys not reachable (no network git,
commits always pass `--message`); config.worktree recorded; work-tree `.gitattributes` only selects
drivers defined in config; alternates run no program.

```findings
[
  {"file": "crates/loom-intake-slice/src/git.rs", "line": 144, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "case::open re-records whatever .git/config holds, so a filter (or diff.external) a previous run's test planted runs on the next run's git add, and only hooks, fsmonitor and signing are neutralised for config already present at open."},
  {"file": "crates/loom-intake-slice/src/git.rs", "line": 41, "category": "contract-drift", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "A planted .git/commondir redirects the config git reads while the recorded .git/config stays byte-identical, so the digest passes and the planted filter runs."},
  {"file": "crates/loom-intake-slice/src/git.rs", "line": 162, "category": "contract-drift", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "Files reached through include.path are not digested, so with an include of a work-tree file the model's own repository.edit can define and trigger a clean filter in one edit."},
  {"file": "crates/loom-intake-slice/src/git.rs", "line": 203, "category": "concurrency", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "The per-call hooks directory is writable by any process running as the same user while git runs; core.hooksPath=/dev/null closes it, and today only an escaped test process that already holds operator rights reaches it."}
]
```
