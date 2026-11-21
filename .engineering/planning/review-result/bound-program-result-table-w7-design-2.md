---
format: aep.planning-md/3
id: review-result:bound-program-result-table-w7-design-2
kind: review-result
status: active
title: Plan critic (design), wave 7 round 2, on story:bound-program-result-table
relations:
- reviews: story:bound-program-result-table
- reviews: story:gate-program-identity-bound-to-spawn
revision: 1
---
needs-revision

story:gate-program-identity-bound-to-spawn — Revision 16 moves the build of a bound program's `ChildCommand`, including the mount rule, into a public function of `crates/loom-intake-slice`. That function takes `argv[0]` as the program and mounts it at its own host path (`at_host_path`). The gate story's `launch` needs a command whose program is the mount path `/opt/loom/programs/<n>/<name>`, bound with `with_read_only_file`, and refuses any other command `ProgramNameInvalid`. `ChildCommand` has no setter that could change the program afterwards. So the port either stops using the shared function, which gives the mount rule a second definition, or the gate story changes that function, and its typed scope does not list `crates/loom-intake-slice/src/bindings.rs`. Its body has to say which. — .engineering/planning/story/gate-program-identity-bound-to-spawn.md:48,74, .engineering/planning/story/bound-program-result-table.md:60-71, crates/loom-intake-slice/src/confinement.rs:1303

**Round-1 findings**
- Finding 1 (opt-in rule, and which crate runs a binding) is closed. The admission and its refusals are at story:49-58. The one definition of both rules, used by `CommandRunner`, is at :71-74. Which crate runs which binding is at :76-79.
- Finding 2 (start directory, preflight root, mount) is closed at :36-38, :55-58 and :60-66.
- Finding 3 (shared files) is closed. "Would collide with" names both siblings and the shared files (story:343-352). `AGENTS.md` and `CHANGELOG.md` are named in the Documents prose (:333-336).

**The split between the two crates (`bindings.rs:250-400`)**
`CommandRunner::select` and `child` can be rewritten onto the two functions with the wave 6 behaviour unchanged, on these conditions:
- `select_runner` keeps its order: the flag reason first, then the file opt-in, then the public admission. Today the file opt-in also wins over a host runner that is not `local` (`bindings.rs:376-383`). Calling the public admission first would admit that runner instead. No wave 6 test covers the case, because the CLI passes no host runner with the file opt-in (`main.rs`, `execute_on`).
- The public build function takes the argv as a slice, not the intake-model `ActionBinding`, and takes the runner, or its `local_reason`, for the mount rule.
- The `NoRunner` message at `select_runner` names `unconfined_bindings: true`, which means nothing in the port's context. No test pins that text (`git grep` over the tests).
- The workspace, the private temporary directory and `preflight_child` stay in `select`. Nothing in `:250-400` falls outside that.

Cycle check: no manifest depends on `b10x-loom-bindings` (`git grep` over `*.toml`), so the new edge onto `b10x-loom-intake-slice` makes no cycle. It brings `b10x-llm-*`, `b10x-substrate-host` and, until the sibling story removes it, `b10x-canon-engineering` into the dependency tree. No manifest test forbids that: `intake_import.rs` and `governor_import.rs` check only git sources and copies. `observe-executor-reads-workspace-files` plans the same edge and depends on this story.

**Stories that touch `bindings.rs` or depend on this one**
- Among stories not yet implemented, only this story has `crates/loom-intake-slice/src/bindings.rs` in its typed scope, so no other story collides with it unordered.
- `bound-artifact-revisions`, `run-caps-per-arrival` and `report-formats-checked-before-the-table` read nothing about how the port builds its command.

**What I read:** this story at revision 16 (`git diff`, `show`); `gate-program-identity-bound-to-spawn`, `bound-program-declares-its-environment`, `executable-allow-list`, `tree-run-command` and four other dependents; `bindings.rs:1-400`, `confinement.rs`, the manifests, `AGENTS.md:1174-1192`, and the import tests.

**What I could not establish**
- `story:bound-program-declares-its-environment` (draft) is not my artifact, but it is now stale. It quotes "a cleared environment" (:106), which this story no longer says. It says the port "builds its own `ChildCommand`" (:133) and that `bindings.rs` "is unchanged" (:257). Its additions can still be chained onto the returned command (`with_environment`, `with_read_only_root`). It should be fixed when it is planned for wave 8.
- The story names no refusal for the build function's error, the host's `PATH` not being UTF-8 (`bindings.rs:330-335`). That is the acceptance critic's lane.
- `gate-program-identity-bound-to-spawn` § Why still cites the pre-w6 `Command::new` at `bindings.rs:415`. That error already existed and is outside my lane.

```findings
- file: .engineering/planning/story/gate-program-identity-bound-to-spawn.md
  line: 48
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "Revision 16 moves the build of a bound program's ChildCommand, including the mount rule, into a public function of crates/loom-intake-slice that takes argv[0] as the program and mounts it at its own host path. The gate story's launch needs the mount path /opt/loom/programs/<n>/<name> as the program and refuses anything else ProgramNameInvalid. The port must either stop using the shared function or the gate story must change it, and its body does not say which, nor does its typed scope list crates/loom-intake-slice/src/bindings.rs."
```
