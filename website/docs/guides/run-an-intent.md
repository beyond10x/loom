---
title: Run an intent
sidebar_position: 1
description: Point b10x-loom run at a git work tree, read its output line by line, and know why it stopped.
lede: b10x-loom run takes an intent and a git work tree, lets a hosted model choose each action inside the governed frontier, and stops with a stated reason, normally at the merge, which it never performs.
source: crates/loom-cli/src/lib.rs, crates/loom-intake-slice/src/run.rs, ess/intake/domains/routing.yaml, docs/qualification/2026-10-05-b10x-loom-live-run.md
---

# Run an intent

For local date/time, see [System queries and custom protocols](system-queries.md): a system query
uses the same command without a workspace or confinement. The requirements below apply to software
changes. Routing occurs before workspace validation or test confinement.

## Before you run it

- **A Codex login.** Run `codex login` once. The credential in `~/.codex/auth.json` is read, and
  renewed when it is close to expiry, by [llm](https://beyond10x.github.io/llm/)
  ([GitHub](https://github.com/beyond10x/llm)), never by Loom's own code.
- **Linux, bubblewrap and delegated cgroup v2 controllers.** Since 0.2.0, Loom
  confines tests with Substrate. When delegation is absent, Loom attempts one user systemd scope.
  A remaining failure stops with a named refusal and exit 3.
- **Dependencies fetched before the run.** Rust and system tools are supported. Tests have no
  network and use an offline private Cargo home with read-only dependency caches. Source and
  toolchain are read-only; workspace writes are restricted to `target/`. A missing cached
  dependency fails the test with a prefetch instruction; Loom never fetches automatically.
  A symlinked `target/`, a hardlink from it to an outside file, or a socket/device inside it
  is refused as `ScopeInvalid`. Remove that entry before retrying. Hardlinks entirely inside
  `target/` remain supported for Cargo's incremental cache.
  Loom's own git calls run none of the work tree's
  hooks. A run does not start on a work tree whose own git configuration names a program (a
  filter driver, a credential helper, an SSH command and the like), and stops when the test
  command changed that configuration or the git directory.
- **A test command without shell syntax.** `--test-cmd` is split at white space and run without a
  shell: `cargo test --quiet` works, `cd x && make` does not.

## Run it

```console
b10x-loom run --workspace <git work tree> --test-cmd "cargo test" "make the failing test pass"
```

Every flag and its default is on the [CLI reference](../reference/cli.md). `--max-steps` (default
20) bounds the actions performed, refused ones included. `--model` and `--classifier-model` name the
models that choose actions and classify the intent: Codex models, or, with `--catalog <PATH>`, route
aliases of that [llm](https://beyond10x.github.io/llm/) catalog file. A catalog route serves the
port its first target declares, never falls back to another target, and needs an account without
a credential, because the command line supplies none. An alias the catalog does not declare, or a
catalog that cannot be read, stops the run with exit status 1 before any model call.

`--state <DIR>` keeps the run in a directory, created when missing: the governed case, the
commission's Suspension when the run stops for an approval, and the commission's resume context
(the intent, the accepted route, the workspace, the test command, the step budget, the
confinement, and the agent model with its catalog), written before the commission runs and
removed when the run ends without a Suspension. A directory that cannot be written, or a
workspace, cgroup root or catalog path that is not UTF-8, stops the run with exit status 1 before
any model call. A run whose process dies early can leave a resume context with no Suspension: only a
context whose commission holds a Suspension can be resumed. Otherwise output and exit status are
the same as without it, except at the human gate: only a last line
`stopped: ApprovalRequired (<action>)` leaves a Suspension to approve, and a run that stops there
holding none prints `stopped: ApprovalRequired (<action>), nothing suspended` and exits 4.
Without `--state` nothing outlives the run.

A run kept with `--state` that stopped for an approval is continued in two further commands,
each in its own process. `b10x-loom approve <COMMISSION> <CAPABILITY> --state <DIR>` writes a
grant for the request the commission's Suspension holds, and performs nothing; the capability is
the one the run named when it stopped, and the commission is the id its resume context and
Suspension carry. `b10x-loom resume <COMMISSION> --state <DIR>` rebuilds the run from the resume
context, reports the workspace's `HEAD` to the case, performs the granted request once, releases
the Suspension and runs on, printing the same lines as `run`; a commit made after the run stopped
moves the case, and the resume is refused `CaseMoved` (14). The model starts the resumed run
without the earlier steps. A granted `repository.merge` is a merge hand-off: `resume` records the
commission, the request, the workspace and its `HEAD` in the state directory, and moves no Git
ref; merging happens outside Loom. A commission hands one `HEAD` off once: a resumed run that
stops at the merge's gate again at that `HEAD` prints
`stopped: MergeHandedOff (<request>, HEAD <sha>)`, and `approve` refuses a merge at it
`AlreadyHandedOff` (23); new work, a new `HEAD`, is approved and handed off again. A resume that
performs nothing exits with the status of its refusal, `NotGranted` (16) without a grant and
`NothingSuspended` (10) once the Suspension is released among them; the
[CLI reference](../reference/cli.md) lists every status.

`--confinement substrate` is the default. `--cgroup-root` selects an explicit delegated
root. Tests retain their 300-second timeout, with 8 GiB memory and 2,048-process limits.
Only `--confinement none` runs with your rights and environment; the run output and every test
observation name this choice. Confined observations include the actual Substrate applied record.

## Read the output

The lines below are from the [live run of 2026-10-05][live], abbreviated.

| Line | Meaning |
|---|---|
| `reference: <kind> <value>` | A tracker key, chat permalink, merge or pull request or URL found in the intent, extracted without a model |
| `picked software-change@1 (confidence 0.98)` and `reason:` lines | The router's proposal of a protocol. A proposal, never authority |
| `refused: <why>` | The router refused its own pick: outside the registry, or below `--threshold`. No case is opened |
| `frontier: repository.edit (admissible), …, repository.merge (blocked), …` | What the governor's evaluation admits before a step |
| `step 3: repository.edit {…}` | The action Loom proposed and the runtime admitted, with its arguments |
| `effect: committed; HEAD is f9fa1bc…` | What the local effect adapter did; further lines (`\|`) are command output or file contents |
| `effect: refused: <reason>` | The adapter refused the action (a path outside the work tree, an argument the action does not take); the step still counts |
| `evidence: test_result pass` | Evidence the verifier submitted from the test command's exit status. A model's words are never evidence |
| `stopped: ApprovalRequired (repository.merge)` | Why the run ended |

For a program rather than a person, `--output jsonl` writes one JSON record per line instead,
ending with one terminal record that carries the stop reason and the exit status: see
[Run events](../reference/run-events.md).

## Why it stopped

| `stopped:` | When | Exit status |
|---|---|---|
| `Completed (answered)` | A system query has verified clock evidence and its protocol completed | 0 |
| `ApprovalRequired (<action>)` | The only useful action needs authority, which the command line never grants: normally the merge, once the tests pass on the current revision | 0 |
| `NothingAdmissible` | Loom proposes nothing and the frontier admits nothing | 3 |
| `StepBudget` | `--max-steps` actions were taken | 3 |
| `NoLocalExecutor` | The picked protocol needs tools or artifact initialization this host does not provide | 3 |
| `Refused` | The router refused its pick | 3 |
| `ConfinementUnavailable` | The requested confinement could not be provided; no passing evidence is submitted | 3 |

A failure (no usable answer from the model, a work tree the case cannot open on, the model out of
reach) prints `b10x-loom: <error>` and exits 1. A command line that is not valid exits 2:

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

The commits the run made are in the work tree, on its current branch; nothing was pushed. Review
them and merge yourself: that is the step the run stopped at.

[live]: https://github.com/beyond10x/loom/blob/main/docs/qualification/2026-10-05-b10x-loom-live-run.md
