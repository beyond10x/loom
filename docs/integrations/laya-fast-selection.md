# Laya Integration — Fast Action Selection

## Why it fits

Laya's current agent-routing guidance describes a fast typed-decision model selecting:

- tool;
- model tier;
- whether tools are needed;
- whether review is needed;

while the stronger reasoning model performs the deeper work and fills tool arguments.

That maps directly onto Loom's proposed split.

## Intended Loom integration

Laya is one implementation of:

```text
ActionSelector
```

not:

```text
Governor
AuthorityProvider
ArgumentGenerator
Executor
```

## Flow

```text
AEP/Canon/Commission
    derives admissible frontier
            ↓
Loom
    extracts candidate action IDs
            ↓
Laya
    chooses one candidate with probability
            ↓
confidence gate
            ↓
reasoning model
    generates arguments for selected action
            ↓
JSON/schema validation
            ↓
Commission
    rechecks case revision/frontier/authority
            ↓
Connector/Substrate
    performs effect
```

## Confidence policy

Example policy:

```text
p >= 0.90
    accept fast selection

0.60 <= p < 0.90
    use reasoning-model selector

p < 0.60
    use full planner / ask for clarification / no-op
```

Exact thresholds must be calibrated per protocol and measured by Metaharness; the embedding host supplies the value for the run's protocol, and Loom ships no default.

## Large catalogues

Laya currently recommends splitting large action sets into hierarchical choices.

Loom can support:

```text
tool family
    ↓
specific action
```

The hierarchy must only contain frontier-admissible actions.

## Locality

Prefer an abstraction that can support:

- local Laya inference;
- hosted Laya;
- other typed decision models;
- deterministic selectors.

Do not make Loom semantically dependent on one vendor/model.

## References

- https://laya.studio/use-cases/agent-tool-routing
- https://laya.tools/laya-for-agents
