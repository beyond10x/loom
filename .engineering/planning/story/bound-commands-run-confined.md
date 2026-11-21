---
format: aep.planning-md/3
id: story:bound-commands-run-confined
kind: story
status: implemented
title: A bound command starts only through the injected runner, and the library refuses one without it
summary: CommandEffects calls run_child; --unconfined-bindings selects the local adapter with a reason
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:confinement-opens-from-engine-config
- depends_on: story:ess-096-upgrade
scope:
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-intake-slice/src/bindings.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement/engine.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/executor.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/intent.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/bound_command_confined.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/security_w20261121w6_u1.rs
- confidence: cited
  path: ess/intake/domains/bindings.yaml
- confidence: cited
  path: ess/intake/domains/confinement.yaml
- confidence: inferred
  path: website/docs/guides/embed-the-runtime.md
- confidence: inferred
  path: website/docs/guides/system-queries.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 17
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:44Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-11-21T10:53:57Z", actor: "agent:sdc", revision: 7, decided_on: {"recorded":{"review_outcome":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w6"}
- {from: "active", to: "implemented", at: "2026-11-21T12:56:32Z", actor: "agent:sdc", revision: 17, decided_on: {"recorded":{"test_result":1,"review_outcome":3,"verification":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w6"}
---
## Outcome

A command bound to a protocol action starts only through the runner the host injected. The library
has no spawn of its own for bound commands: `CommandEffects` builds a `ChildCommand` (argv, the
arguments as JSON on standard input, the timeout, a cleared environment) and calls `run_child`.
Without a runner a bound action is refused before anything starts. `--unconfined-bindings` and the
file's `unconfined_bindings: true` select the `local` adapter with a reason that says which of the
two opted in.

## Why

`~/beyond10x/sdc/docs/layers.md` § 4, violation 4: `CommandEffects::run`
(`crates/loom-intake-slice/src/bindings.rs:414-439`) calls `Command::new` directly, with no
working directory and no confinement, while `SubstrateRunner::run_child` exists
(`crates/loom-intake-slice/src/confinement.rs:475`). The opt-in is checked only in the command
line (`crates/loom-cli/src/main.rs:983-1003`); an SDK host that sets `IntentRequest.bindings`
runs the commands unconfined with no gate (grep: `unconfined_bindings` appears in the slice only
in `decode`).

## ESS first

`ess/intake/domains/bindings.yaml` first: the summary's rule changes from "run unconfined only
when the operator opts in" to "run through the engine's confinement; the opt-in selects the
`local` adapter", and `ess/intake/domains/confinement.yaml` gains the refusal `NoRunner`. On that
commit `task intake-drift` fails. The story's new test
`crates/loom-intake-slice/tests/bound_command_confined.rs` fails: a bound command that reads a
file outside its workspace succeeds today.

## Acceptance

- `crates/loom-intake-slice/src/bindings.rs` contains no `Command::new`; a test greps for it.
- A bound program that writes its standard input and its environment into its report receives
  exactly the request's arguments as one JSON object and an environment whose only variable is
  `PATH`.
- Under the `linux` adapter a bound command that reads a host file outside its workspace and its
  declared read-only roots fails, and one that writes inside the workspace succeeds.
- The existing tests of bound commands pass with no change to what they assert:
  `crates/loom-intake-slice/tests/bound_command.rs`,
  `crates/loom-cli/tests/custom_protocol_commands.rs`,
  `crates/loom-intake-slice/tests/adversary_w20261021w6_u3.rs` and
  `crates/loom-intake-slice/tests/security_w20261022w2_u1.rs` (exit, report, evidence, and the
  kill of the whole process group at the timeout).
- A host that passes bindings and no runner gets a refusal naming `NoRunner` before any process
  starts.
- With `--unconfined-bindings` the command runs under the `local` adapter and the applied record's
  reason names the flag; with `unconfined_bindings: true` in the file and no flag, the reason
  names the file's member; with neither and no served adapter the run stops
  `ConfinementUnavailable` before any model call, as today.
- `task check` exits 0.

## Out of scope

The `loom.bindings/1` format (`story:bindings-document`). Result tables
(`story:bound-program-result-table`). Egress apertures for bound commands.

## As built, wave 2026-11-21-w6

Where the unit's commits (`9b4170fb..0b0a50ce`) differ from the text above, the commits are what
holds:

- `CommandRunner::child` builds the `ChildCommand` (`crates/loom-intake-slice/src/bindings.rs`),
  and `CommandEffects` calls it. `TestRunner` gains `run_child` and `preflight_child`
  (`crates/loom-intake-slice/src/confinement.rs`), which the text expected to stay unchanged.
- The opt-in is checked in two places: the command line still checks it
  (`crates/loom-cli/src/main.rs`), and the library selects the runner (`bindings.rs`,
  `select_runner`), so an SDK host that sets bindings is held to the same rule.
- `b10x-loom run --action-bindings` with no opt-in runs its commands under Substrate's `linux`
  adapter when that adapter is served. The Acceptance names only the case with no served
  adapter; before, a run with bindings and no opt-in stopped at exit 3 whatever was served. With
  `--confinement none` or `local` and no opt-in it still stops so. `--unconfined-bindings`
  beside `--state` is refused, exit 2. Both are `**Breaking:**` in the CHANGELOG.
- A bound program starts in `--workspace`, or in a private temporary directory removed after the
  run, which the text does not mention. Before, it started in the directory `b10x-loom` ran in:
  a relative `argv` and a relative output path no longer resolve from there (`**Breaking:**`,
  found by security pass 2).
- One `Kill` per run. The adversary case of the base went red because Loom's engine sent `Kill`
  twice on the timeout path, shown by a probe; the planner chose on 2026-11-21 to send one and to
  pin it with a counting fake of the runtime (three tests in
  `crates/loom-intake-slice/src/confinement/engine.rs`: the timeout, a failed wait, a failed
  write). A `Kill` that cannot be sent ends as a refusal, never as "was killed", and is not
  sent again (a fourth test, from security pass 1's note).
- Under `local` only the command's process group is killed at the timeout. A process that left
  the group and still holds the command's output is not killed, and the step reads
  `refused by its runner: CapabilityUnserved`, where the base read `was killed after N s`. A
  program ended by a signal and a program that cannot be started read `refused by its runner`
  too. `ess/intake/domains/bindings.yaml` states the two adapters' reach.
- Security pass 1 (`review-result:bound-commands-run-confined-security-1`) found that under
  `local` a stdin write stalled by a process outside the group lifted the timeout (25.4 s
  measured for 1 s): the launch joined the write and the wait. The launch now drops a write still
  pending once the run has ended. The `linux` path joins too and was left as it is: Substrate
  kills the run's cgroup at the exec timeout, read in the code by the implementor and by
  security pass 2 (`review-result:bound-commands-run-confined-security-2`) and not run, since it
  needs a delegated runner.
- The four test files the Acceptance names are unchanged. Six unit tests of the base's
  `bindings.rs` were deleted with the spawn code; the Scope section below says what holds each
  case now, and that no case holds a standard error stream alone or a leader reaped before the
  timeout.
- No third security pass ran: correction 2 changed prose and committed the reviewer's case, and
  the coordinator read its diff (evidence of kind `verification` on this story).

## Scope

Rewritten 2026-11-21 at the close of wave 2026-11-21-w6 from the unit's commits (`git diff --stat 9b4170fb 0b0a50ce`: 41 files, 2138 insertions, 517 deletions). Every line is cited from that diff.

- **Primary surface:** `crates/loom-intake-slice` — cited, 8 of the 41 changed files are under it (six `src/` files, two new test files); `crates/loom-cli` holds 2 more, `ess/intake/domains` 2, and 29 are generated output, derived pages and documents
- **Files (ESS sources):** `ess/intake/domains/bindings.yaml` (+16 -10: the summary says bound commands run through the engine's confinement, the runner the host injected, that the opt-in selects the `local` adapter with a reason naming which opt-in it was, and what the timeout kill reaches under `linux` and under `local`) and `ess/intake/domains/confinement.yaml` (+6 -4: `ConfinementRefusal` gains the variant `NoRunner`, and the domain summary says what `NoRunner` and `CommandUnconfined` now mean) — cited
- **Files (generated):** `generated/rust/intake/**` (16 files, digest lines and the `NoRunner` variant), `website/data/ess/intake-{bindings,confinement}.domain-graph.json` (2 files) and `website/docs/reference/ess/{index.mdx,intake-bindings.md,intake-confinement.md,intake-routing.md}` (4 files), all regenerated from `ess/intake` by `task intake-generate` (`cargo run -p loom-xtask -- intake-generate`) — cited
- **Files:** `crates/loom-intake-slice/src/bindings.rs` — cited, +237 -413: the spawn code (`Ended`, `run`, `drain`, `finish`, `exited`, `stop`, the `POLL` constant and the `std::process` imports) is gone; a new `CommandRunner` (`select`, `child`, `run`) and `select_runner` choose the runner and build the `ChildCommand`, and `CommandEffects` gains a `commands` field, calls `run_child` once (`:496`), prints each step's `confinement:` line and reads the runner's `TestExecution`; the six unit tests that went with the spawn code are named below
- **Files:** `crates/loom-intake-slice/src/confinement.rs` — cited, +36: the trait `TestRunner` gains `run_child` and `preflight_child` with defaults that refuse `NoRunner` (`no_child_runner`), and `SubstrateRunner` forwards both to its own methods
- **Files:** `crates/loom-intake-slice/src/confinement/engine.rs` — cited, +471 -8: `EngineRunner` forwards `run_child` and `preflight_child`; `LocalRoute::launch` races the stdin write against the wait and drops a write still pending when the run ended, and sends one `Kill` per run (`finish` takes a `killed` flag); a new `#[cfg(test)] mod tests` of 6 cases over a counting fake adapter
- **Files:** `crates/loom-intake-slice/src/intent.rs` — cited, +60 -1: `prepare_intent` selects the runner before the classifier and stops `ConfinementUnavailable` on a refusal (`command_runner`, `commands_refused`), `PreparedIntent` gains the `commands` field, `run_prepared` selects again for a restored route, and the `IntentRequest.runner` and `bindings` docs say bindings start through `runner` in `workspace`
- **Files:** `crates/loom-intake-slice/src/run.rs` — cited, +5: `Execution::Commands` carries `commands`, `drive` passes it to `CommandEffects::new`, and the stop table gains the `ConfinementUnavailable` row for bound commands
- **Files:** `crates/loom-intake-slice/src/executor.rs` — cited, +1: `ended_unreaped` is `#[cfg(test)]`; `UnconfinedRunner` is unchanged and takes the new `NoRunner` default
- **Files:** `crates/loom-cli/src/main.rs` — cited, +54 -12: `execute_on` builds the runner for a bindings run (the flag opens `EngineRunner` on `local` with `UNCONFINED_FLAG_REASON`, the file's opt-in passes none, otherwise `SubstrateRunner`), `--unconfined-bindings` with `--state` is refused, and `bindings_refused` now fires only under `--confinement none` or `local` without an opt-in; `confinement_stopped` is split out of `confinement_refused`
- **Files:** `crates/loom-cli/src/lib.rs` — cited, +13 -9: the help of `--action-bindings` and `--unconfined-bindings`
- **Files (new tests):** `crates/loom-intake-slice/tests/bound_command_confined.rs` (685 lines, 10 cases: the story's test, with the no-`Command::new` grep, the `NoRunner` stop, the stdin and `PATH` case, the reasons and the runner selection, the `local` held-output refusal, the lost-status refusal and the `linux` read and write case) and `crates/loom-intake-slice/tests/security_w20261121w6_u1.rs` (434 lines, 4 cases: the two security passes' cases, committed as the reviewer wrote them)
- **Files (existing tests):** none of the four named tests changes (`bound_command.rs`, `custom_protocol_commands.rs`, `adversary_w20261021w6_u3.rs`, `security_w20261022w2_u1.rs` are not in the diff). Six unit tests of the base's `bindings.rs` `mod tests` were deleted with the spawn code, 0 added there; what the diff holds in their place:
  - `a_group_that_could_not_be_killed_is_never_reported_killed` — `engine.rs` `a_kill_that_could_not_be_sent_is_a_refusal_never_a_kill_and_is_not_sent_again`
  - `a_status_that_cannot_be_read_is_refused_with_its_error_and_never_as_a_timeout` — `engine.rs` `a_run_whose_status_was_lost_is_refused_never_timed_out_and_sent_no_kill`, and `bound_command_confined.rs` `a_command_whose_status_its_runner_could_not_read_is_refused_never_killed`
  - `a_status_lost_after_the_program_ended_is_never_reported_killed` — the same two cases
  - `output_held_past_the_timeout_after_its_leader_was_reaped_is_never_reported_killed` and `standard_error_held_past_the_timeout_after_its_leader_was_reaped_is_never_reported_killed` — `bound_command_confined.rs` `under_local_a_command_whose_output_outlives_its_group_is_refused_by_its_runner_never_killed` (one case, standard output only) and `engine.rs` `a_run_past_its_deadline_is_sent_one_kill_and_waited_for_one_settle`; no case of the diff holds standard error alone
  - `a_leader_reaped_before_the_timeout_is_never_signalled_and_never_reported_killed` — no case of the diff names a reaped leader (the runner owns the process now); the nearest is the `engine.rs` deadline case above
- **Files (documents):** `AGENTS.md` (+27 -9: the bindings paragraph of the intake section), `CHANGELOG.md` (+9: the new items and the Breaking lines, including the working directory and the refusal text), `README.md` (+4 -4: the bound-command sentence), `website/docs/guides/system-queries.md` (+36 -15: where a program starts, absolute paths, confinement per adapter, the new step lines), `website/docs/guides/embed-the-runtime.md` (+10: a host that sets bindings needs `IntentRequest.runner`), `website/docs/reference/sdk-stability.md` (+1 -1: the `TestRunner` pin row), `website/docs/reference/cli.md` (+2 -2: the two options' help, generated from `crates/loom-cli/src/lib.rs`) — cited
- **Symbols:** added `TestRunner::run_child` and `TestRunner::preflight_child` (trait methods with refusing defaults), `bindings::UNCONFINED_FLAG_REASON` and `bindings::UNCONFINED_FILE_REASON` (public constants), `ConfinementRefusal::NoRunner` (generated), and the crate-private `bindings::CommandRunner` with `select`, `child`, `run`, `PreparedIntent.commands`, `Execution::Commands { commands }` and the `commands` argument of `CommandEffects::new`; changed `IntentRequest.runner` (now also starts bound commands), `ConfinementRefusal::CommandUnconfined` (now the host runner on `local` with no opt-in, or `--confinement none`/`local` in the CLI), `EngineRunner`'s launch (one `Kill`, a pending write dropped); removed the private `Ended`, `CommandEffects::run`, `drain`, `finish`, `exited` and `stop` of `bindings.rs`; no public item removed — cited; `ended_unreaped` is now `#[cfg(test)]`
- **Not predicted and touched:** `crates/loom-intake-slice/src/confinement/engine.rs`, `crates/loom-intake-slice/tests/security_w20261121w6_u1.rs`, `website/docs/reference/sdk-stability.md`
- **Predicted and not touched:** `crates/loom-cli/tests/custom_protocol_commands.rs`, `crates/loom-intake-slice/tests/adversary_w20261021w6_u3.rs`, `crates/loom-intake-slice/tests/bound_command.rs`, `crates/loom-intake-slice/tests/security_w20261022w2_u1.rs` (all four named as tests that must pass unchanged, and they do not appear in the diff)
- **Review:** `review-result:bound-commands-run-confined-security-1` (head `200f9e64`, red: 2 warnings) found a stalled stdin write under `local` held a 1 s timeout for 25.4 s (`engine.rs`, fixed by `6c5a3466`) and an incomplete CHANGELOG (no opt-in now runs under `linux`, `--unconfined-bindings` with `--state` refused, the step texts, the `confinement:` line; the CHANGELOG diff holds these); `review-result:bound-commands-run-confined-security-2` (head `6c5a3466`, green: 1 warning) closed the first and found the CHANGELOG omitted the changed working directory and the "refused by its runner" step text, which `0b0a50ce` states in `CHANGELOG.md` and `system-queries.md`
