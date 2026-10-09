---
title: Getting started
sidebar_position: 2
description: Build b10x-loom, run the SDK example without a model, and read what a live run looks like.
lede: Build the command line from source, run a whole governed embedding on scripted fake models in one command, and compare it with a recorded live run.
source: crates/loom-cli, crates/loom-sdk/examples/software_change.rs, docs/qualification/2026-10-05-b10x-loom-live-run.md
---

# Getting started

You need a Rust toolchain (edition 2024) and `git`. Nothing is on a registry, so you build from
the repository at the release tag, `0.13.0`.

## Build the command line

```console
git clone --branch 0.13.0 https://github.com/beyond10x/loom.git
cd loom
cargo install --locked --path crates/loom-cli
b10x-loom --version
```

```text
b10x-loom 0.13.0
```

`b10x-loom run --help` lists its flags and exit statuses; the [CLI reference](./reference/cli.md)
shows the same text.

## Run a governed embedding, without a model

The SDK example opens a case on the `software-change@1` protocol over a scratch git repository with
one failing check, and runs the Commission runtime over it with Loom as the executor. Its selector
and argument generator are scripted fakes: no network, no login. The authority provider grants
nothing, so the run stops when only the merge is left.

```console
cargo run --locked -p b10x-loom-sdk --example software_change
```

```text
workspace: target/loom-sdk-example
step 1: repository.edit {"files":[{"path":"check.txt","contents":"fixed\n"}],"message":"fix the check"}
  effect: committed; HEAD is b03958b42159325c1c16f74ec4b26292a962b734
  evidence: none
step 2: tests.run {}
  effect: the test command exited with 0
  evidence: test_result pass
stopped: ApprovalRequired (repository.merge)
```

The commit id differs on every run. What happened:

1. The governor evaluated the case with Canon and issued a frontier: edit, inspect and run tests
   admissible, merge blocked.
2. Loom selected `repository.edit` and generated its arguments; the runtime rechecked the proposal
   and the local effect adapter committed the file. The governor moved the case to the new
   revision.
3. Loom selected `tests.run`; the adapter ran `grep -qx fixed check.txt`, and the verifier, not the
   model, submitted the passing result as evidence.
4. With the tests passing on the current revision, the merge became *approval required*. The
   authority provider granted nothing, so the run stopped at `ApprovalRequired (repository.merge)`.
   Nothing was merged.

[Embed the runtime](./guides/embed-the-runtime.md) walks through the example's code.

## A live run

`b10x-loom run` does the same with a hosted model choosing each action, over your own git work tree
and test command. It needs a Codex login (`codex login`) and makes model calls, so this page does
not run it; the [live qualification run of 2026-10-05][live] is the recorded result:

```console
b10x-loom run --workspace <scratch> --test-cmd "cargo test --quiet" --max-steps 12 "make the failing test pass"
```

```text
picked software-change@1 (confidence 0.98)
…
step 1: tests.run {}
  effect: the test command exited with 101
  evidence: test_result fail
…
step 3: repository.edit {"files":[{"contents":"…","path":"src/lib.rs"}],"message":"Fix add to return the sum of its operands instead of their difference."}
  effect: committed; HEAD is f9fa1bc0fadc810d5dcd6b33f0b4c3f9967dccca
…
step 4: tests.run {}
  effect: the test command exited with 0
  evidence: test_result pass
frontier: repository.edit (admissible), repository.inspect (admissible), repository.merge (approval required), tests.run (admissible)
…
stopped: ApprovalRequired (repository.merge)
```

The excerpt is cut (`…`); the record has the full transcript, five steps, exit status 0.
[Run an intent](./guides/run-an-intent.md) explains each line and what to check before you point
it at a work tree.

[live]: https://github.com/beyond10x/loom/blob/main/docs/qualification/2026-10-05-b10x-loom-live-run.md

## Next

- [Run the checks](./guides/run-the-checks.md) before you change anything.
- [Crates](./reference/crates.md) lists every package and what it is for.
