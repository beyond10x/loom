---
format: aep.planning-md/3
id: review-result:bound-program-result-table-w7-acceptance-2
kind: review-result
status: active
title: Plan critic (acceptance), wave 7 round 2, on story:bound-program-result-table
relations:
- reviews: story:bound-program-result-table
revision: 1
---
needs-revision

story:bound-program-result-table — the Outcome's check "`git grep -n unconfined` over `crates/loom-bindings` … prints nothing" stops holding once the story is built, because its own Acceptance test `a_local_runner_with_another_reason_is_refused_command_unconfined` goes into `crates/loom-bindings/tests/bound_program.rs` and contains the lowercase word `unconfined` — .engineering/planning/story/bound-program-result-table.md:53-54
story:bound-program-result-table — the wave 6 line says "the tests of `crates/loom-intake-slice` that cover `select_runner` and `child` pass", but its command runs only `--test bound_command_confined`; `tests/bound_command.rs`, `security_w20261121w6_u1.rs`, `security_w20261022w2_u1.rs` and `adversary_w20261021w6_u3.rs` also go through that path (`unconfined_bindings: Some(true)`, `PATH` alone) and are not run — .engineering/planning/story/bound-program-result-table.md:250-254
story:bound-program-result-table — the command `git grep -n 'effect: write' -- …/bound_program.rs` shows only that the file contains some writer action somewhere, not that the run-count tests drive a writer, and the line says "three run-count tests" where the section holds four (:192-201) — .engineering/planning/story/bound-program-result-table.md:202-204
story:bound-program-result-table — the preamble defines the recording runner as "a `TestRunner` that is not `local`", and the mount line then uses "a recording runner that answers `local_reason` with `UNCONFINED_FLAG_REASON`", so the two contradict each other unless the preamble allows that variant — .engineering/planning/story/bound-program-result-table.md:118 and :247
story:bound-program-result-table — two Outcome clauses have no Acceptance line: the order "the request's [arguments], else the binding's literal `arguments`" (only `arguments_from` is tested), and that the record carries "the `upstream` artifacts' revisions" — .engineering/planning/story/bound-program-result-table.md:40-41 and :45

Round-1 findings: all five are closed.
- No runner: closed by the no-runner lines and the `UnconfinedRunner` line (:225-235).
- `task check`: closed. The `task drift` line and the `bound_program` count line replace it.
- Blob line: closed. It is split (:180-190), the temporary-file part is now the check that only SHA-256-named files remain, and the `crates/loom-governor` part is a `git diff` command.
- Lines with several outcomes: closed. They are split into one outcome per line.
- No named test: closed. There are 51 `Test:` names plus the commands.

Checked and holding:
- `/usr/bin/cat` and `/usr/bin/env` under `local`: the local route's environment is the variables the command sets, plus any it allows (`engine.rs:359-371`). Wave 6 asserts exactly `PATH=<host>\n` under `local` (`bound_command_confined.rs:134-162`). `env` reads no standard input, and a write that fails because of that is ignored, not refused (`engine.rs:522-533`).
- `UnconfinedRunner` only defines `backend` and `run` (`executor.rs:1165-1200`). So `preflight_child` keeps the trait default and refuses `NoRunner` (`confinement.rs:112-115`), and `local_reason` keeps the default `None`.
- A test in `crates/loom-bindings` can build a recording runner like `Answering` from public items only (`bound_command_confined.rs:25-30`, `:340-362`). `directory()` (`confinement.rs:1461`) and `host_paths()` (`:1517`) are `pub`.
- The two `git diff --stat $(git merge-base main HEAD)..HEAD` commands print nothing on a unit branch unless the unit touches those paths.
- `git grep -nE 'struct CommandRunner|fn select_runner'` prints `:255 pub(crate) struct` and `:366 fn select_runner(` at HEAD.
- `git grep -ln … NoRunner` prints `bound_command_confined.rs` alone at HEAD.

What I read: 1 artifact at store revision 16 (`aep plan artifact show`), and at `aa943153` `executor.rs`, `confinement.rs`, `confinement/engine.rs`, `bindings.rs`, the five bound-command test files, and the stdin code of the Substrate checkout `adfde40`.

What I could not establish:
- Whether `FLAG_REASON` in `bound_command_confined.rs` equals `UNCONFINED_FLAG_REASON` (not read).
- Whether `/usr/bin/cat` exists on hosts without a merged `/usr` (it does on this host's distribution family).
- Out of my lane: the design of the two new public functions.

```findings
- file: .engineering/planning/story/bound-program-result-table.md
  line: 53
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome's check 'git grep -n unconfined over crates/loom-bindings prints nothing' stops holding once the story is built, because its own Acceptance test a_local_runner_with_another_reason_is_refused_command_unconfined goes into crates/loom-bindings/tests/bound_program.rs and contains the lowercase word unconfined"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 250
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the wave 6 line says the tests that cover select_runner and child pass, but its command runs only --test bound_command_confined; tests/bound_command.rs, security_w20261121w6_u1.rs, security_w20261022w2_u1.rs and adversary_w20261021w6_u3.rs also go through that path and are not run"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 202
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the command git grep -n 'effect: write' on bound_program.rs shows only that the file contains some writer action somewhere, not that the run-count tests drive a writer, and the line says three run-count tests where the section holds four"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 247
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the preamble defines the recording runner as a TestRunner that is not local, and the mount line then uses a recording runner that answers local_reason with UNCONFINED_FLAG_REASON, so the two contradict each other unless the preamble allows that variant"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 40
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: pre-existing
  message: "two Outcome clauses have no Acceptance line: the order of the request's arguments before the binding's literal arguments (only arguments_from is tested), and that the record carries the upstream artifacts' revisions"
```
