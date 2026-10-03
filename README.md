# Loom

The native agent harness: it makes a model behave like an agent for one bounded run, inside a
governed frontier.

Loom implements [Commission](https://github.com/beyond10x/commission)'s executor contract. For each
run it:

1. projects the frontier's admissible actions into the model-visible catalogue;
2. selects one action (a reasoning model, a deterministic rule, or a fast typed selector);
3. asks the model for arguments to that action only, and validates them against its schema;
4. hands the action back for revalidation against the current case revision and authority;
5. executes it through a trusted adapter.

A selector can be wrong about which admissible action is best. It cannot produce an action the
frontier does not contain.

Loom succeeds the Beyond10x Harness. Harness's turns, sessions, providers, tool loop, streaming,
compaction, budgets and records are ported here step by step; Harness stays in service until its
consumers have moved.

## Status

Bootstrap. Design: [`docs/design/loom-design.md`](docs/design/loom-design.md); action selection:
[`docs/contracts/loom-action-selection.md`](docs/contracts/loom-action-selection.md); fast selection:
[`docs/integrations/laya-fast-selection.md`](docs/integrations/laya-fast-selection.md).

## Build

```console
task check
```

## Licence

Apache-2.0.
