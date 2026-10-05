# Loom

The native agent harness: it makes a model behave like an agent for one bounded run, inside a
governed frontier.

Loom implements [Commission](https://github.com/beyond10x/commission)'s executor contract. For each
run it:

1. projects the frontier's admissible actions into the model-visible catalogue;
2. selects one action (a reasoning model, a deterministic rule, or a fast typed selector);
3. asks the model for arguments to that action only, and validates them against its schema;
4. hands the action back for revalidation against the current case revision and authority;
5. returns it as a `ProposedAction`. The Commission runtime rechecks frontier, case revision and
   authority, then invokes it through a trusted adapter (Atlas ADR 0082).

A selector can be wrong about which admissible action is best. It cannot produce an action the
frontier does not contain.

Loom succeeds the Beyond10x Harness. Harness's turns, sessions, providers, tool loop, streaming,
compaction, budgets and records are ported here step by step; Harness stays in service until its
consumers have moved.

## Status

Bootstrap. Design: [`docs/design/loom-design.md`](docs/design/loom-design.md); action selection:
[`docs/contracts/loom-action-selection.md`](docs/contracts/loom-action-selection.md); fast selection:
[`docs/integrations/laya-fast-selection.md`](docs/integrations/laya-fast-selection.md).

## Run it

```console
cargo install --path crates/intake-cli
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

## Build

```console
task check
```

## Licence

Apache-2.0.
