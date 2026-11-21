---
format: aep.planning-md/3
id: story:bound-commands-run-confined
kind: story
status: proposed
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
  path: crates/loom-cli/tests/custom_protocol_commands.rs
- confidence: cited
  path: crates/loom-intake-slice/src/bindings.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/executor.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/intent.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/adversary_w20261021w6_u3.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/bound_command.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/bound_command_confined.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/security_w20261022w2_u1.rs
- confidence: cited
  path: ess/intake/domains/bindings.yaml
- confidence: cited
  path: ess/intake/domains/confinement.yaml
- confidence: inferred
  path: website/docs/guides/embed-the-runtime.md
- confidence: inferred
  path: website/docs/guides/system-queries.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:44Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
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

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-intake-slice/src/bindings.rs` (`CommandEffects`, its `run` at lines 413-439) — cited
- **Files:** `crates/loom-intake-slice/src/bindings.rs:414` (`Command::new` to remove) — cited
- **Files:** `crates/loom-cli/src/main.rs:983-1003` (the opt-in check, moves into the library's adapter selection) — cited
- **Files:** `crates/loom-intake-slice/src/confinement.rs:475` (`SubstrateRunner::run_child`) and `:1037` (`ChildCommand`) — cited, read not changed unless `NoRunner` is declared there
- **Files:** `ess/intake/domains/bindings.yaml`, `ess/intake/domains/confinement.yaml` (add `NoRunner`; the `ConfinementRefusal` variants are on line 12) — cited
- **Files (tests):** `crates/loom-intake-slice/tests/bound_command_confined.rs` (new) — cited
- **Files (tests, must pass unchanged):** `crates/loom-intake-slice/tests/bound_command.rs`, `crates/loom-cli/tests/custom_protocol_commands.rs`, `crates/loom-intake-slice/tests/adversary_w20261021w6_u3.rs`, `crates/loom-intake-slice/tests/security_w20261022w2_u1.rs` — cited
- **Symbols:** `CommandEffects`, `CommandEffects::new`, `CommandEffects::run`, `SubstrateRunner::run_child`, `ChildCommand`, `ConfinementRefusal::CommandUnconfined`, `ActionBindings::unconfined_bindings`, `Execution::Commands` — cited
- **Also likely:** `crates/loom-intake-slice/src/run.rs` (`Execution::Commands` at line 574 and its `CommandEffects::new` call at 668 must carry the runner) — inferred
- **Also likely:** `crates/loom-intake-slice/src/intent.rs` (the `Execution::Commands` build at line 915; refuse with `NoRunner` before `governor.open` at ~line 896) — inferred
- **Also likely:** `crates/loom-cli/src/main.rs` (the runner for bound commands is built from `--confinement` at ~1151-1180, so it has to reach `IntentRequest` for a bindings run) — inferred
- **Also likely:** `crates/loom-cli/src/lib.rs:869-887` (the `--unconfined-bindings` help text) — inferred
- **Also likely:** `crates/loom-intake-slice/src/executor.rs` (`TestRunner` at `confinement.rs:73` has no `run_child`, and `UnconfinedRunner` at `executor.rs:1163` is a separate type; the type the host injects may have to change) — inferred
- **Documents:** `website/docs/guides/system-queries.md` (lines 94 and 170-178 say bound programs are unconfined, so they go stale) — inferred
- **Documents:** `website/docs/guides/embed-the-runtime.md` (SDK host and runner; an SDK host that passes bindings now needs a runner) — inferred
- **Confidence:** medium — the story names the defect sites and tests with line numbers that match the tree. The runner's type and how it reaches `CommandEffects` are not settled, and that decides which of the "also likely" files change.
- **Would collide with:** any unit touching `loom-intake-slice`'s `confinement.rs` or `executor.rs`, the `IntentRequest.runner` field or `Execution` in `intent.rs`/`run.rs`, `ess/intake/domains/confinement.yaml` or `bindings.yaml`, or `loom-cli`'s `run` option handling in `main.rs`
- **Safety fact:** the four existing bound-command tests drive `run_intent_with_options` with a `TestCommand` and recorded models and assert exit, report, evidence and the process-group kill. The change keeps the spawn behind `run_child`, so none of them has to change its assertions. The new `ChildCommand` must keep a cleared environment with only `PATH`, `process_group(0)` semantics and the timeout kill (`bindings.rs:413-439`), and `run_child` never sets a process group — step 2, unproven
