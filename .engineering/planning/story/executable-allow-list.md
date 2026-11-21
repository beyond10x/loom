---
format: aep.planning-md/3
id: story:executable-allow-list
kind: story
status: draft
title: Loom starts no process whose program the engine's allow-list and deny-list do not admit
summary: Checked at bindings load and again before each start on the resolved path; descendants are story:executable-descendants-held
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-run-command
- depends_on: story:limit-hits-in-run-log
revision: 6
---
## Outcome

The engine configuration's `executables` section holds an allow-list and a deny-list, and Loom
starts no process of a tree run whose program is not admitted. A program is named by its canonical
path: absolute, with every symbolic link followed. A list entry is a path or a directory,
canonicalised the same way when the configuration is loaded. A program named with a `/` that is
not absolute resolves against the workspace; a bare name resolves against the directories of the
section's `search_path`, an ordered list of absolute directories, the first that holds it winning,
and a bare name no directory holds is refused `ProgramUnresolved`. `search_path` decides resolution
only and is not the `PATH` variable a child receives.

The check runs when a bindings file is loaded at `tree open` and `tree run` (every `argv[0]` and
`arguments_from[0]` must resolve, be allowed and not be denied, or the file is refused naming the
action and the program) and again in the runner immediately before each start, on the canonical
path resolved then. What Substrate is handed as `argv[0]` is that resolved path, as the child sees
it (`ChildCommand::sandbox_path`), which Substrate's `/usr/bin/env` executes; a resolved path the
child cannot see is refused `ProgramUnresolved`. An `agent` step's harness program is checked only
by the runner before its start, not at load (`story:agent-session-through-substrate`). A binding
never extends the lists.

`allow` is optional and differs from an empty list: no `allow` with no `deny` is no policy, as
today; `allow: []` admits nothing; a `deny` without `allow` is refused when the engine configuration
is loaded. Each step's confinement record names the policy applied as `executable_policy`, the
digest `executable_policy_digest(&ExecutablesSection)` in `crates/loom-tree/src/engine.rs` computes
(SHA-256, prefixed `sha256:`, over the canonical JSON of the sorted canonical allow and deny entries
and the `search_path` in order), or no member when there is no policy, and says
`descendants_held: false`: what a started program itself executes is not held to the lists until
`story:executable-descendants-held`. The policy reaches the runner on the engine confinement
(`EngineConfinement.executables`), so the check before each start holds for every child the runner
starts in a tree run. `b10x-loom run` and its test runner are not held to the lists.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "a process allow-list and
deny-list (which executables may run at all)". Loom runs any argv a bindings file names
(`crates/loom-intake-slice/src/bindings.rs:182-195` refuses only an empty program). Substrate
0.21.0 has an executable allow-list only on `macos` (`exec_subpaths`,
`crates/substrate-host/src/confinement/macos.rs:3-35`); `linux` refuses a profile that uses it
(`linux.rs:29-31`), its seccomp filter covers `socket(2)` only, and no adapter has a deny-list.
Loom can hold the program it starts to the lists today; descendants need the Substrate control
that `upstream-blocker:substrate-executable-allow-list` waits for, so they are a separate story.

Loom's launch starts in `run_child` (`crates/loom-intake-slice/src/confinement.rs:475`),
`spawn_child` (`:525`) and `Prepared::new` (`:736-737`), each of which already checks the program
for what `/usr/bin/env` would misread (`check_program`, `:1652`). Under `linux` a host path is seen
under its mount (`sandbox_path`, `:1446`; it refuses a path through a link, `no_link_on`, `:1452`),
so only `local` shows a started program its host path.

## Waits on

- `story:limit-hits-in-run-log`: the story before this one in the chain; both edit
  `crates/loom-tree` and the launch path in `crates/loom-intake-slice/src/confinement.rs`. Through
  the chain, `story:action-limits-enforced` and `story:confinement-level-required` edit
  `ess/domains/engine.yaml` (`EngineRefusal`, `StepConfinement`) and
  `ess/intake/domains/confinement.yaml` before this story.
- `story:tree-run-command`: `tree open` and `tree run`, where the load-time check runs.

## ESS first

`ess/domains/engine.yaml`: `ExecutablesSection` (optional `allow`, optional `deny`, `search_path`),
`EngineConfiguration` gains optional `executables`, `EngineRefusal` gains `ProgramNotAdmitted`
(action, program, optional entry: the deny or allow entry that decided, absent for a program no
allow entry admits), `ProgramUnresolved` (action, program) and `DenyWithoutAllow`, and
`StepConfinement` gains optional `executable_policy` (the digest) and `descendants_held`.
`ess/intake/domains/confinement.yaml`: `ExecutablePolicy` (optional allow, optional deny, search
path) and `EngineConfinement` gains optional `executables`. On that commit `task drift` and
`task intake-drift` fail and `crates/loom-tree/tests/executable_policy.rs` does not compile. The
load-time check is in `crates/loom-tree/src/engine.rs`; the check before a start is in `run_child`,
`spawn_child` and `Prepared::new`.

## Acceptance

- A bindings file whose `argv[0]` resolves outside the allow-list is refused at `tree open`
  `ProgramNotAdmitted` with no entry; one inside it loads; one inside an allowed directory but named
  on the deny-list is refused, naming the deny entry. The same holds for `arguments_from[0]`.
- Under `local`, with `search_path: [<dir-a>, <dir-b>]` and a compiled stub `tool` (a Rust test
  binary that prints `/proc/self/exe`) in both, a bare `argv[0]` of `tool` resolves to
  `<dir-a>/tool` and the stub prints `<dir-a>/tool`; `./bin/tool` resolves against the workspace
  and the stub prints that path. A bare name in neither directory is refused at `tree open`
  `ProgramUnresolved`.
- Under in-process `linux`, a program inside the workspace is handed to Substrate as
  `/workspace/<relative path>`, the path the start's request names.
- An allowed path that is a symbolic link to a denied program is refused at `tree open` naming the
  deny entry; a program replaced by such a link between load and start is not started, and the
  step is refused `ProgramNotAdmitted`.
- An engine configuration whose `executables` has a `deny` and no `allow` is refused when it is
  loaded, `DenyWithoutAllow` (Loom never hands Substrate a deny-list alone; agreed with session
  `substrate`, 2026-11-21).
- With `allow: []` nothing starts; with no `executables` section the behaviour is today's and each
  step's record has no `executable_policy`.
- A step run under a policy has `executable_policy` equal to `executable_policy_digest` of the
  section; two sections with the same entries in another order give the same digest, and adding
  one entry changes it.
- Each step run under a policy has `descendants_held: false` in its confinement record.
- `task check` exits 0.

## Out of scope

Holding a started program's descendants to the lists (`story:executable-descendants-held`, held by
`upstream-blocker:substrate-executable-allow-list`). Checking a program by content digest. Holding
`b10x-loom run` and its test runner to the lists. Checking an agent harness at load.
