# Loom

Loom is the runtime for governed agents. It lets a model act as an agent for one bounded run, and
only through the actions a governed frontier admits at that moment.

**Documentation: <https://beyond10x.github.io/loom/>**, starting at
[Getting started](https://beyond10x.github.io/loom/docs/getting-started).

Each step of a run goes the same way. Loom turns the frontier into the catalogue the model sees,
selects one action, asks for that action's arguments and returns a `ProposedAction`. Commission's
runtime, which lives in this repository, rechecks the proposal against the frontier, the case
revision and the authority it needs, and only then hands it to an effect port. A selector can pick
the wrong admissible action. It cannot pick one the frontier does not contain.

## What it is not

Loom does not decide what is legitimate: protocols are evaluated by
[Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon)), and
`software-change@1` comes from
[engineering protocols](https://beyond10x.github.io/engineering-protocols/)
([GitHub](https://github.com/beyond10x/engineering-protocols)). It holds no model credentials and
speaks no provider wire; model calls go through [llm](https://beyond10x.github.io/llm/)
([GitHub](https://github.com/beyond10x/llm)). It grants no authority, and it never merges, pushes
or deploys. [Where Loom ends](https://beyond10x.github.io/loom/docs/concepts/where-loom-ends) draws
the whole boundary.

## Status

Version `0.1.0`, released from source at the tag `0.1.0`
([release](https://github.com/beyond10x/loom/releases/tag/0.1.0)). Nothing is on a registry: you
install from the tag or depend on it with `tag = "0.1.0"`. `b10x-loom run` completed a live run
against a hosted model on 2026-10-05
([record](docs/qualification/2026-10-05-b10x-loom-live-run.md)). The
[status page](https://beyond10x.github.io/loom/docs/status) marks every capability shipped,
decided or planned; [CHANGELOG.md](CHANGELOG.md) lists the changes.

## Run the command line

You need a Rust toolchain that builds edition 2024, and `git`.

```console
git clone --branch 0.1.0 https://github.com/beyond10x/loom.git
cd loom
cargo install --locked --path crates/loom-cli
b10x-loom --version
```

```text
b10x-loom 0.1.0
```

`b10x-loom run` routes an intent to a protocol, opens a governed case and runs until the run is
blocked:

```console
b10x-loom run --workspace <git work tree> --test-cmd "cargo test" "make the failing test pass"
```

It prints each step with its effect and evidence and ends on a line such as
`stopped: ApprovalRequired (repository.merge)`. Exit status 0 means it stopped at that human gate,
3 another stop, 1 a failure, 2 an invalid command line.

Two things before you point it at a work tree. It needs a Codex login (`codex login`). And there is
no sandbox: the test command runs model-edited code with your rights, so use it only where you
would run that test command yourself. Loom's own git calls run none of the work tree's hooks, and
a run does not start on a work tree whose own git configuration names a program (a filter driver,
a credential helper, an SSH command and the like).
[Run an intent](https://beyond10x.github.io/loom/docs/guides/run-an-intent) explains every line of
the output; the [CLI reference](https://beyond10x.github.io/loom/docs/reference/cli) lists every
flag.

## Embed the runtime

An application depends on one crate, `b10x-loom-sdk` (library `loom_sdk`). It re-exports
Commission's contracts and runtime, the executor, the governor and intake. The application supplies
the selector, the argument generator, the authority provider and the effect port.

On the development branch, `CanonGovernor::with_protocol` admits host-reviewed Canon protocols;
`with_evaluation_time` supplies trusted freshness time. Durable hosts implement
`governor::FallibleCaseStore`, while existing `CaseStore` users remain compatible. Protocol
admission, durable storage and authenticated evidence remain the embedding application's duties.

```toml
[dependencies]
b10x-loom-sdk = { git = "https://github.com/beyond10x/loom", tag = "0.1.0" }
```

The example below
is a whole embedding: it opens a case on `software-change@1` over a scratch git repository, drives
it with scripted fake models (no network, no login) and stops before the merge. The commit id in
its output changes on every run.

```console
cargo run --locked -p b10x-loom-sdk --example software_change
```

```text
workspace: target/loom-sdk-example
step 1: repository.edit {"files":[{"path":"check.txt","contents":"fixed\n"}],"message":"fix the check"}
  effect: committed; HEAD is ef2262e9165ef00b2f5cadf487f81dbf3ed120fe
  evidence: none
step 2: tests.run {}
  effect: the test command exited with 0
  evidence: test_result pass
stopped: ApprovalRequired (repository.merge)
```

[Embed the runtime](https://beyond10x.github.io/loom/docs/guides/embed-the-runtime) walks through
its code.

## Crates

| Package | What it is |
|---|---|
| `b10x-loom-cli` | The `b10x-loom` command line |
| `b10x-loom-sdk` | The one crate an embedding depends on |
| `b10x-loom-executor` | The executor: catalogue, action selection, argument generation, and the model wires, turn loop and sessions ported from Harness |
| `b10x-loom-commission` | Commission's contracts and runtime loop, generated from its ESS specification |
| `b10x-loom-governor` | Evaluates a case's protocol with Canon and issues the frontier; executes nothing |
| `b10x-loom-intake-router`, `-references`, `-slice` | Route an intent to a protocol, extract its references, and run the local slice over a git work tree |

The [crate reference](https://beyond10x.github.io/loom/docs/reference/crates) lists all fourteen
workspace packages, test kits and repository tools included. If you depended on the archived
`commission`, `governor` or `intake` repositories,
[Move to Loom's crates](https://beyond10x.github.io/loom/docs/guides/move-to-loom) maps the old
names to the new ones.

## Contributing

Changes go through `task check`, which needs Rust, [Task](https://taskfile.dev/) and the `ess`
command line of [ESS](https://beyond10x.github.io/ess/) ([GitHub](https://github.com/beyond10x/ess)).
[Run the checks](https://beyond10x.github.io/loom/docs/guides/run-the-checks) explains each step.
Agents read [AGENTS.md](AGENTS.md).

## Licence

Apache-2.0 ([LICENSE](LICENSE)).
