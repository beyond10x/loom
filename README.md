# Loom

Loom is the runtime for governed agents. It lets a model act as an agent for one bounded run, and
only through the actions a governed frontier admits at that moment.

**Documentation: <https://beyond10x.github.io/loom/>**, starting at
[Getting started](https://beyond10x.github.io/loom/docs/getting-started).

Since `0.4.0`, Loom includes workspace-free system queries and an extensible protocol
catalog. Loom owns `system-query@1`; engineering definitions and installed custom definitions
remain separate sources. [System queries and custom protocols](https://beyond10x.github.io/loom/docs/guides/system-queries)
describes the clock tool, pinned installation and offline loading. Transient model failures get
at most three attempts on the same binding; reports count every attempt. Persistent overload
is reported as unavailable, and tool effects are never retried by this policy.

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

Version `0.6.0`, released from source at the tag `0.6.0`
([release](https://github.com/beyond10x/loom/releases/tag/0.6.0)). Nothing is on a registry: you
install from the tag or depend on it with `tag = "0.6.0"`. `b10x-loom run` completed a live run
against a hosted model on 2026-10-05
([record](docs/qualification/2026-10-05-b10x-loom-live-run.md)). The
[status page](https://beyond10x.github.io/loom/docs/status) marks every capability shipped,
decided or planned; [CHANGELOG.md](CHANGELOG.md) lists the changes.

Since `0.3.0` the slice keeps inspected file contents as immutable, run-local results. The model
sees bounded previews and can select ranges or reuse content in edits through verified references,
which are expanded before Commission admission;
[result references](docs/design/result-references.md) describes the contract and limits.

Since `0.5.0` an executor can report that the case moved while it worked
(`ExecutorOutcome::CaseMoved`), and Commission judges the run on the case's current frontier
instead of the one it left. Callers that match `ExecutorOutcome` exhaustively handle the new
variant.

Since `0.6.0` a run whose case moved to a revision whose frontier still admits an action ends
`RunOutcome::CaseMovedOn`, naming the Run's revision and the current one; start a new Run at the
current revision. Callers that match `RunOutcome` exhaustively handle the new variant. Loom
requires ESS 0.56.0.

## Run the command line

Since `0.4.0`, Loom supports opt-in `--context-policy bounded` and `--context-report PATH`.
It keeps typed working state and a recent event tail, with retrievable run-local history and a
64 KiB serialized request ceiling. The default remains `legacy`; recorded byte reductions do not
establish live quality or cost savings. See [working context](website/docs/concepts/working-context.md).

You need a Rust toolchain that builds edition 2024, and `git`.

```console
git clone --branch 0.6.0 https://github.com/beyond10x/loom.git
cd loom
cargo install --locked --path crates/loom-cli
b10x-loom --version
```

```text
b10x-loom 0.6.0
```

On a development checkout, `task install` rebuilds the checked-out source and replaces
`~/.local/bin/b10x-loom`. Keep `~/.local/bin` on your `PATH`. It does not fetch or switch branches.

`b10x-loom run` routes an intent to a protocol, opens a governed case and runs until the run is
blocked:

```console
b10x-loom run --workspace <git work tree> --test-cmd "cargo test" "make the failing test pass"
```

For a software change, it prints each step with its effect and evidence and ends on a line such as
`stopped: ApprovalRequired (repository.merge)`. Exit status 0 means a query completed or a software change stopped at that human gate,
3 another stop, 1 a failure, 2 an invalid command line. With `--output jsonl` standard output is
one JSON record per line instead, the last one the terminal record with the stop reason and that
exit status ([run events](https://beyond10x.github.io/loom/docs/reference/run-events)).

Since `0.2.0` tests run confined. A software-change run needs a Codex
login and Linux with bubblewrap and delegated cgroup v2 controllers. Tests default to Substrate:
no network, source and toolchain read-only, workspace writes only under `target/`, a 300-second
timeout, 8 GiB memory and 2,048 processes. Fetch Rust dependencies explicitly before starting;
Loom uses a private offline Cargo home and never fetches them. Without delegation it attempts
one user systemd scope, then stops `ConfinementUnavailable` (exit 3) if confinement is unavailable.
`--confinement none` explicitly opts out and is named in every test observation.
Loom's own git calls run none of the work tree's hooks, and
a run does not start on a work tree whose own git configuration names a program (a filter driver,
a credential helper, an SSH command and the like).
[Run an intent](https://beyond10x.github.io/loom/docs/guides/run-an-intent) explains every line of
the output; the [CLI reference](https://beyond10x.github.io/loom/docs/reference/cli) lists every
flag.

## Embed the runtime

An application depends on one crate, `b10x-loom-sdk` (library `loom_sdk`). It re-exports
Commission's contracts and runtime, the executor, the governor and intake. The application supplies
the selector, the argument generator, the authority provider and the effect port.

Since `0.2.0`, `CanonGovernor::with_protocol` admits host-reviewed Canon protocols;
`with_evaluation_time` supplies trusted freshness time. Durable hosts implement
`governor::FallibleCaseStore`, while existing `CaseStore` users remain compatible. Protocol
admission, durable storage and authenticated evidence remain the embedding application's duties.

```toml
[dependencies]
b10x-loom-sdk = { git = "https://github.com/beyond10x/loom", tag = "0.6.0" }
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
    | confinement: none
  evidence: test_result pass
stopped: ApprovalRequired (repository.merge)
```

[Embed the runtime](https://beyond10x.github.io/loom/docs/guides/embed-the-runtime) walks through
its code.

## Crates

| Package | What it is |
|---|---|
| `b10x-loom-protocols` | The shared protocol catalog and verified offline installations |
| `b10x-loom-cli` | The `b10x-loom` command line |
| `b10x-loom-sdk` | The one crate an embedding depends on |
| `b10x-loom-executor` | The executor: catalogue, action selection, argument generation, and the model wires, turn loop and sessions ported from Harness |
| `b10x-loom-commission` | Commission's contracts and runtime loop, generated from its ESS specification |
| `b10x-loom-governor` | Evaluates a case's protocol with Canon and issues the frontier; executes nothing |
| `b10x-loom-intake-router`, `-references`, `-slice` | Route an intent to a protocol, extract its references, and run the local slice over a git work tree |

The [crate reference](https://beyond10x.github.io/loom/docs/reference/crates) lists all
workspace packages, test kits and repository tools included. If you depended on the archived
`commission`, `governor` or `intake` repositories,
[Move to Loom's crates](https://beyond10x.github.io/loom/docs/guides/move-to-loom) maps the old
names to the new ones.

## Contributing

Changes go through `task check`, which needs Rust, [Task](https://taskfile.dev/), bubblewrap
(`/usr/bin/bwrap`) and the `ess` command line of [ESS](https://beyond10x.github.io/ess/)
([GitHub](https://github.com/beyond10x/ess)). Run `cargo fetch --locked` first to prepare the
full dependency graph for offline checks.
[Run the checks](https://beyond10x.github.io/loom/docs/guides/run-the-checks) explains each step.
Agents read [AGENTS.md](AGENTS.md).

## Licence

Apache-2.0 ([LICENSE](LICENSE)).
