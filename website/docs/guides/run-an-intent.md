---
title: Run an intent
sidebar_position: 1
description: Point b10x-loom run at a git work tree, read its output line by line, and know why it stopped.
lede: b10x-loom run takes an intent and a git work tree, lets a hosted model choose each action inside the governed frontier, and stops with a stated reason, normally at the merge, which it never performs.
source: crates/loom-cli/src/lib.rs, crates/loom-intake-slice/src/run.rs, ess/intake/domains/routing.yaml, docs/qualification/2026-10-05-b10x-loom-live-run.md
---

# Run an intent

## Before you run it

- **A Codex login.** Run `codex login` once. The credential in `~/.codex/auth.json` is read, and
  renewed when it is close to expiry, by [llm](https://beyond10x.github.io/llm/)
  ([GitHub](https://github.com/beyond10x/llm)), never by Loom's own code.
- **A git work tree whose test command you would run yourself.** There is no sandbox. The test
  command and the work tree's git hooks run model-edited code with your rights and your
  environment. The path checks bound what Loom writes, not what that code does.
- **A test command without shell syntax.** `--test-cmd` is split at white space and run without a
  shell: `cargo test --quiet` works, `cd x && make` does not.

## Run it

```console
b10x-loom run --workspace <git work tree> --test-cmd "cargo test" "make the failing test pass"
```

Every flag and its default is on the [CLI reference](../reference/cli.md). `--max-steps` (default
20) bounds the actions performed, refused ones included. `--model` and `--classifier-model` name the
models that choose actions and classify the intent.

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

## Why it stopped

| `stopped:` | When | Exit status |
|---|---|---|
| `ApprovalRequired (<action>)` | The only useful action needs authority, which the command line never grants: normally the merge, once the tests pass on the current revision | 0 |
| `NothingAdmissible` | Loom proposes nothing and the frontier admits nothing | 3 |
| `StepBudget` | `--max-steps` actions were taken | 3 |
| `NoLocalExecutor` | The router picked a protocol other than `software-change@1`, which is the only one the slice can perform | 3 |
| `Refused` | The router refused its pick | 3 |

A failure (no usable answer from the model, a work tree the case cannot open on, the model out of
reach) prints `b10x-loom: <error>` and exits 1. A command line that is not valid exits 2:

```console
b10x-loom run
```

```text
error: the following required arguments were not provided:
  --workspace <DIR>
  <INTENT>

Usage: b10x-loom run --workspace <DIR> <INTENT>

For more information, try '--help'.
```

## After the run

The commits the run made are in the work tree, on its current branch; nothing was pushed. Review
them and merge yourself: that is the step the run stopped at.

[live]: https://github.com/beyond10x/loom/blob/main/docs/qualification/2026-10-05-b10x-loom-live-run.md
