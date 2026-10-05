# Loom

The runtime for governed agents: it makes a model behave like an agent for one bounded run, inside
a governed frontier, and it never lets the model act outside it.

For each step of a run Loom:

1. projects the frontier's admissible actions into the model-visible catalogue;
2. selects one action (a model, a deterministic rule, or later a fast typed selector);
3. asks for arguments to that action only;
4. returns it as a `ProposedAction`. Commission's runtime rechecks frontier, case revision and
   authority, then hands it to an effect port (Atlas ADR 0082).

A selector can be wrong about which admissible action is best. It cannot produce an action the
frontier does not contain.

Commission's contracts and runtime, the Canon governor and intake live in this repository too
(Atlas ADR 0090); their former repositories are archived. Loom succeeds the Beyond10x Harness,
whose model wires, turn loop and sessions are ported here step by step.

Documentation: <https://beyond10x.github.io/loom/>. Changes: [CHANGELOG.md](CHANGELOG.md).

## Status

Early. No release yet: depend on a Git revision. `b10x-loom run` has completed a live run against
a hosted model ([record](docs/qualification/2026-10-05-b10x-loom-live-run.md)); the
[status page](https://beyond10x.github.io/loom/docs/status) lists what is shipped and what is
planned.

## Run it

```console
cargo install --locked --path crates/loom-cli
b10x-loom run --workspace <git work tree> --test-cmd "cargo test" "make the failing test pass"
```

`b10x-loom run` routes the intent to a protocol, opens a governed case and runs Loom on it until the
run is blocked. It prints the references it found in the intent, the protocol it picked, the
frontier before each step, each step with its effect and evidence, and a last line
`stopped: <reason>`, for example `stopped: ApprovalRequired (repository.merge)`. It never merges,
pushes or deploys.

It needs a Codex login (`codex login`). There is no sandbox: the test command and the workspace's
git hooks run model-edited code with your rights, so use it only on a workspace whose test command
you would run yourself.

The exit status is 0 when the run stopped at its human gate (`ApprovalRequired`), 3 when it stopped
for another reason, 1 when it failed and 2 for a command line that is not valid.
`b10x-loom run --help` lists the flags.

## Embed it

An application embeds the governed runtime through one crate, `b10x-loom-sdk`
([`crates/loom-sdk`](crates/loom-sdk)). It re-exports Commission's contracts and runtime, the Loom
executor, the governor and intake; the application supplies the selector, the argument generator,
the authority provider and the effect port.

[`crates/loom-sdk/examples/software_change.rs`](crates/loom-sdk/examples/software_change.rs) is a
whole embedding: it opens a case on `software-change@1` over a scratch git repository, runs the loop
over scripted fake models (no network, no login) and stops at `ApprovalRequired (repository.merge)`.

```console
cargo run --locked -p b10x-loom-sdk --example software_change
```

## Build

Needs Rust, [Task](https://taskfile.dev/) and the `ess` command line.

```console
task check
```

## Licence

Apache-2.0.
