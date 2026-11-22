---
title: Run an intent
sidebar_position: 1
description: Point b10x-loom run at a git work tree, read its output line by line, and know why it stopped.
lede: b10x-loom run takes an intent and a git work tree, lets a hosted model choose each action inside the governed frontier, and stops with a stated reason, normally at the merge, which it never performs.
source: crates/loom-cli/src/lib.rs, crates/loom-intake-slice/src/run.rs, ess/intake/domains/routing.yaml, docs/qualification/2026-10-05-b10x-loom-live-run.md
---

# Run an intent

The requirements below apply to software changes. A system query, such as local date and time,
uses the same command without a workspace or confinement: see
[System queries and custom protocols](system-queries.md). Routing occurs before workspace
validation or test confinement.

## Before you run it

- **A Codex login.** Run `codex login` once. [llm](https://beyond10x.github.io/llm/)
  ([GitHub](https://github.com/beyond10x/llm)) reads the credential in `~/.codex/auth.json` and
  renews it when it is close to expiry. Loom's own code does not read it.
- **Linux, bubblewrap and delegated cgroup v2 controllers.** Since 0.2.0, Loom
  confines tests with Substrate. When delegation is absent, Loom attempts one user systemd scope.
  A remaining failure stops with a named refusal and exit 3.

- **Dependencies fetched before the run.** Rust and system tools are supported. Tests have no
  network and use an offline private Cargo home with read-only dependency caches. A missing cached
  dependency fails the test with a prefetch instruction. Loom does not fetch it.
- **A test command without shell syntax.** `--test-cmd` is split at white space and run without a
  shell: `cargo test --quiet` works, `cd x && make` does not.

Source and toolchain are read-only, and workspace writes are restricted to `target/`. A symlinked
`target/`, a hardlink from it to an outside file, or a socket or device inside it is refused as
`ScopeInvalid`. Remove that entry before retrying. Hardlinks entirely inside `target/` remain
supported for Cargo's incremental cache.

Loom's own git calls run none of the work tree's hooks. A run does not start on a work tree whose
own git configuration sets a program, such as a filter driver, a credential helper or an SSH
command. It stops when the test command changed that configuration or the git directory.

## Run it

```console
b10x-loom run --workspace <git work tree> --test-cmd "cargo test" "make the failing test pass"
```

Every flag and its default is on the [CLI reference](../reference/cli.md). `--max-steps` (default
20) bounds the actions performed, refused ones included. `--model` selects the model that chooses
actions and `--classifier-model` the one that classifies the intent. Both take Codex models, or,
with `--catalog <PATH>`, route aliases of that [llm](https://beyond10x.github.io/llm/) catalog
file.

A catalog route serves the port its first target declares and does not fall back to another
target. It needs an account without a credential, because the command line supplies none. An
alias the catalog does not declare, or a catalog that cannot be read, stops the run with exit
status 1 before any model call.

A software change opens with three case inputs, which you give and no model does:

- `--risk <trivial|standard|elevated|critical>`, default `standard`, selects the obligations the
  change owes: tests at every level, a review from `standard`, release provenance from
  `elevated`, a healthy deployment at `critical`.
- `--change-kind <feature|bugfix|refactor>`, default `feature`. A bugfix reproduces its
  regression first: `implementation.edit` is admissible only once a test failed on the current
  revision, and the change owes `red_first.required`. It is discharged only when the run writes
  its regression test with `tests.write`, runs it red, and only then edits the implementation.

- `--affects-security-boundary <true|false>`, default `false`. With `true` the implementation is
  verified only after another principal approved a security review. The slice does not submit
  one, so such a run does not reach the merge.

With the defaults a run behaves as it did before the inputs existed. An obligation the inputs do
not select is not open. A protocol without inputs, such as a system query, ignores the three flags.

`tests.write` writes only test paths, for every change kind. A path that none of the test-path
patterns matches refuses the whole action before anything is written, and the refusal names that
path. `implementation.edit` is not held to the patterns.

`--test-path <GLOB>`, repeatable, replaces the defaults: `tests/**`, `**/tests/**`, `test/**`,
`**/test/**`, `**/*_test.*`, `**/test_*.*`, `**/*.test.*`, `**/*.spec.*`. A pattern is relative to
the workspace root. `*` matches within one path segment and `**` across any number of them.

A bugfix's regression test is the file its `tests.write` wrote. `--regression-test-command <CMD>`
gives the command that runs that file. It is split at white space like `--test-cmd` and has
`{path}` in at least one word, for example `cargo test --test {path}` for a runner that takes a
file, or `sh {path}`. Without the flag no run is a regression result, so `red_first.required`
stays open.

On a bugfix, after every `tests.run` that follows a committing `tests.write`, the slice runs that
command once per test path those writes committed, with `{path}` filled. Each run is its own
observation, and the slice submits a `regression_test_result` from each. The suite's failure is
not one: a suite that does not read the written file does not test it.

`--state <DIR>` keeps the run in a directory, created when missing. It holds the governed case,
the commission's Suspension when the run stops for an approval, and the commission's resume
context. The resume context is the intent, the accepted route, the workspace, the test command,
the step budget, the confinement, and the agent model with its catalog. It is written before the
commission runs and removed when the run ends without a Suspension.

A directory that cannot be written, or a workspace, cgroup root or catalog path that is not UTF-8,
stops the run with exit status 1 before any model call. A run whose process dies early can leave a
resume context with no Suspension: only a context whose commission holds a Suspension can be
resumed.

Otherwise output and exit status are the same as without `--state`, except at the human gate. Only
a last line `stopped: ApprovalRequired (<action>)` leaves a Suspension to approve. A run that stops
there holding none prints `stopped: ApprovalRequired (<action>), nothing suspended` and exits 4.
Without `--state` nothing outlives the run.

A run kept with `--state` that stopped for an approval is continued in two further commands, each
in its own process:

1. `b10x-loom approve <COMMISSION> <CAPABILITY> --state <DIR>` writes a grant for the request the
   commission's Suspension holds, and performs nothing. The capability is the one the run named
   when it stopped. The commission is the id its resume context and Suspension carry.

2. `b10x-loom resume <COMMISSION> --state <DIR>` rebuilds the run from the resume context,
   reports the workspace's `HEAD` to the case, performs the granted request once, releases the
   Suspension and runs on, printing the same lines as `run`. The model starts the resumed run
   without the earlier steps. A commit made after the run stopped moves the case, and the resume
   is refused `CaseMoved` (14).

A resume that performs nothing exits with the status of its refusal: `NotGranted` (16) without a
grant, `NothingSuspended` (10) once the Suspension is released. The
[CLI reference](../reference/cli.md) lists every status.

A granted `repository.merge` is a merge hand-off. `resume` records the commission, the request,
the workspace and its `HEAD` in the state directory, and moves no Git ref: merging happens outside
Loom. A commission hands one `HEAD` off once: a resumed run that stops at the merge's gate again at
that `HEAD` prints `stopped: MergeHandedOff (<request>, HEAD <sha>)`.

A merge the agent proposed again there holds no Suspension, and the state directory keeps no
resume context for it, so `approve` finds nothing to approve (`NothingSuspended`, 10). New work, a
new `HEAD`, is approved and handed off again.

Both commands take `--mandate <FILE>` to clear the approval through Mandate instead of Loom's grant
alone. `FILE` is a JSON file you trust as the host (`loom.authority.MandateHost`). It holds
Mandate's verified context the commission acts in, the approver's principal, and the policy. The
approver is neither the subject nor its actor. The policy sets the approver, the relation, the
capabilities, those that need an approval, and how many seconds an approval lasts.

With `--mandate`, `approve` also records a Mandate approval of the held request by that approver
in the state directory's approval log, `mandate-approvals.jsonl`, and the grant carries it.
`resume` then performs the request only when Mandate's `check_with_approval` clears and consumes
that approval for the subject, the capability and the case. It writes the consumption to the log
before anything is performed.

A second resume with the same approval, an expired approval and a grant that carries none are
refused `NotGranted` (16) and perform nothing. Loom configures Mandate from the file over
Mandate's own in-memory ports until Mandate ships a policy adapter. Without `--mandate`, both
commands behave as before, except that `resume` refuses a grant that carries a Mandate approval
with `NotGranted` (16) and performs nothing: only Mandate clears it.

`--confinement substrate` is the default. `--cgroup-root` selects an explicit delegated
root. Tests retain their 300-second timeout, with 8 GiB memory and 2,048-process limits.

Only `--confinement local` and the deprecated `--confinement none` run with your rights and your
host's file system and network. `local` needs `--local-reason <TEXT>`. The run goes through
Substrate's `local` adapter, declared insecure, and prints `confinement: local (<reason>)`. Every
test observation records adapter `local` and the reason. Tests run in the workspace with your
`PATH`, `HOME`, `RUSTUP_HOME` and `CARGO_HOME` and nothing else of your environment. They are
killed with their whole process group at their timeout.

On a host that leaves `SUBSTRATE_CONFINEMENT_LOCAL` unset, the flag and the reason are the whole
opt-in. A host policy of `SUBSTRATE_CONFINEMENT_LOCAL` other than `permit` refuses `local`
(`ConfinementUnavailable`). A `local` run is not kept for resume: `--state` with it is refused,
exit 2. `none` prints its deprecation on standard error. The run output and every test observation
record the choice. Confined observations include the actual Substrate applied record.

## Read the output

The lines below are from the [live run of 2026-10-05][live], abbreviated, with its
`repository.edit` written as the action that replaced it: since engineering-protocols `0.5.0` the
slice writes tests with `tests.write` and edits the implementation with `implementation.edit`.

| Line | Meaning |
|---|---|
| `reference: <kind> <value>` | A tracker key, chat permalink, merge or pull request or URL found in the intent, extracted without a model |
| `picked software-change@1 (confidence 0.98)` and `reason:` lines | The router's proposal of a protocol. It carries no authority |
| `refused: <why>` | The router refused its own pick: outside the registry, or below `--threshold`. No case is opened |
| `frontier: implementation.edit (admissible), …, repository.merge (blocked), …` | What the governor's evaluation admits before a step |
| `step 3: implementation.edit {…}` | The action Loom proposed and the runtime admitted, with its arguments |
| `effect: committed; HEAD is f9fa1bc…` | What the local effect adapter did. Further lines (`\|`) are command output or file contents |
| `effect: refused: <reason>` | The adapter refused the action, for example a path outside the work tree, a `tests.write` path outside the test paths, or an argument the action does not take. The step still counts |
| `evidence: test_result pass` | Evidence the verifier submitted from the test command's exit status. A model's words are not evidence |
| `evidence: test_result fail, regression_test_result fail` | A test run on a `--change-kind bugfix` run with `--regression-test-command`, and the regression test results from it. One that fails before the first `implementation.edit`, in an earlier second (Canon orders whole seconds), discharges `red_first.required`. Loom records each observation in a later second than the one before, waiting up to 2 seconds for the clock to pass it. A clock that stepped back fails the step |
| `evidence: implementation_change` | An `implementation.edit` committed. The record is about the new `HEAD`. Every record carries the host clock's `observed_at` and the verifier as its producer |
| `stopped: ApprovalRequired (repository.merge)` | Why the run ended |

`--output jsonl` writes one JSON record per line for a program to read. The last is a terminal
record that carries the stop reason and the exit status: see [Run events](../reference/run-events.md).

## Why it stopped

| `stopped:` | When | Exit status |
|---|---|---|
| `Completed (answered)` | A system query has verified clock evidence and its protocol completed | 0 |
| `ApprovalRequired (<action>)` | The only useful action needs authority, which the command line does not grant: normally the merge, once the tests pass on the current revision | 0 |
| `NothingAdmissible` | Loom proposes nothing and the frontier admits nothing | 3 |
| `StepBudget` | `--max-steps` actions were taken | 3 |
| `NoLocalExecutor` | The picked protocol needs tools or artifact initialization this host does not provide | 3 |
| `Refused` | The router refused its pick | 3 |
| `ConfinementUnavailable` | The requested confinement could not be provided. No passing evidence is submitted | 3 |

A failure prints `b10x-loom: <error>` and exits 1: no usable answer from the model, a work tree the
case cannot open on, or the model out of reach. A command line that is not valid exits 2:

```console
b10x-loom run
```

```text
error: the following required arguments were not provided:
  <INTENT>

Usage: b10x-loom run <INTENT>

For more information, try '--help'.
```

## After the run

The commits the run made are in the work tree, on its current branch, and nothing was pushed. Review
them and merge yourself: the run stopped at that step.

[live]: https://github.com/beyond10x/loom/blob/main/docs/qualification/2026-10-05-b10x-loom-live-run.md
