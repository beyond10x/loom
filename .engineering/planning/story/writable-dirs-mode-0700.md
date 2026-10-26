---
format: aep.planning-md/3
id: story:writable-dirs-mode-0700
kind: story
status: implemented
title: The writable emptyDirs of a LoomAgent pod are mode 0700 and owned by the pod user
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-cli/src/stage.rs
- confidence: cited
  path: crates/loom-operator/src/render.rs
- confidence: cited
  path: crates/loom-operator/tests
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T19:11:31Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-26T19:11:31Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-26T20:55:52Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
## Outcome

The writable emptyDirs the controller mounts in a `LoomAgent` pod (`/tmp` and `HOME`
`/var/lib/agent` for the `Command` variant) are owned by the pod user with mode 0700, so a program
that refuses a credential file under a group- or world-writable directory accepts files it writes
there.

## Why

Trial S5/S6, 2026-10-26, run 5: the harness refused a token written under the `/tmp` emptyDir
("secret source does not satisfy the required access protections"; the directory was mode 2777).
Gap: "Writable emptyDirs (/tmp, HOME) mode 0700 so credential readers accept them".

## Acceptance

- `the_writable_dirs_are_0700_and_owned_by_the_pod_user`: the rendered pod makes `/tmp` and `HOME`
  mode 0700, uid 10001 (through an init step, `fsGroup` handling or a subdirectory, as the story
  decides; Kubernetes gives emptyDirs mode 0777 and `fsGroup` adds setgid).
- A reader with the gates' `protected_read` rules accepts a file written there (test as
  `adv_staged_files_under_a_setgid_world_writable_root_pass_the_gates_reader` does).
- No CRD change.

## Scope

Derived 2026-10-26 by the coordinator from `b6e35835`.

- `crates/loom-operator/src/render.rs` (the `Command` pod's emptyDirs, security context) — cited
- `crates/loom-cli/src/stage.rs` (if an init step prepares the directories) — inferred
- `crates/loom-operator/tests/` — cited
- AGENTS.md § Resource, design § 9a, CHANGELOG.md — cited

## Also reported

The orchestrator, 2026-10-26, from the takt 0.13.1 release: "emptyDir modes
(story:writable-dirs-mode-0700 may already cover it, link it)". This story is that gap; takt
0.13.1 reports it again from its own image.

The orchestrator, 2026-10-26, from the takt one-hour pod trial (S7): "still: /tmp and HOME are
mode 2777 without the sticky bit, which fails 3 takt tests". Trial record: `cargo test -p
b10x-takt --lib` "127 passed, 3 failed with the pod's /tmp (2777); 130 passed with `TMPDIR` a 0700
dir".
