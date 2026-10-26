---
format: aep.planning-md/3
id: review-result:adversary-writable-dirs-mode-0700-pass-1
kind: review-result
status: active
title: Adversary pass 1, wave 2026-10-26-cmdv u5, story:writable-dirs-mode-0700, at 9f039c9f
relations:
- reviews: story:writable-dirs-mode-0700
revision: 1
---
# Adversary pass 1, wave 2026-10-26-cmdv u5, story:writable-dirs-mode-0700, at 9f039c9f

Agent `aep:adversary` (opus). Header and findings block verbatim; prose condensed.

```
unit: u5 (story:writable-dirs-mode-0700) pass 1, tree loom-cmdv-u5 at 9f039c9f plus one untracked test file
verdict: green
cases: executed 354→357 (b10x-loom-cli 190→193, operator 164→164), red 0
origin: introduced 0, pre-existing 0, undecided 0
wrote-outside-worktree: scratch only (diff, suite, clippy and task logs)
needs-coordinator: no
```

Guards in `crates/loom-cli/tests/adv_cmdv_u5_prepare_dirs.rs` (committed by the coordinator in
77320545): `adv_a_fifo_in_place_of_a_dir_is_refused_without_blocking`,
`adv_every_special_bit_is_cleared_and_a_repeated_dir_is_no_error`,
`adv_a_dangling_link_is_refused_and_not_followed`.

Suites: `b10x-loom-cli` 193 passed, `b10x-loom-operator` 164 passed, clippy, `task docs-check`,
`task chart-check` exit 0. Held: setgid and umask under a 3777 or 7777 root, a link or file in
place of the directory, the mode set through the open fd, rerun, the init container's security
context and resources, `RESERVED_MOUNTS`, `HOME` in `CONTROLLER_ENV`, the gates `protected_read`
rule, the golden change limited to `takt-delivery.yaml`, the docs. The kubelet keeping an existing
`subPath` directory and applying `fsGroup` ownership only at an emptyDir's first setup is reasoned
from its code, not observed on a cluster.

```findings
[
{"file": "crates/loom-cli/src/stage.rs", "line": 134, "category": "mutant", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "the 0700 read-back after fchmod cannot fail on a test filesystem, so removing it leaves the suite green; it guards only a filesystem or ACL that silently alters the mode"}
]
```
