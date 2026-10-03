# Example — Fast Action Selection with Laya

Suppose the current frontier contains exactly:

```text
metrics.inspect
logs.search
release.inspect
```

Loom constructs a compact selection request:

```json
{
  "goal": "Identify the likely source of the latency spike after the last deploy.",
  "candidate_actions": [
    "metrics.inspect",
    "logs.search",
    "release.inspect"
  ]
}
```

A Laya-style selector returns:

```json
{
  "action": "release.inspect",
  "probability": 0.96
}
```

Loom accepts the selection because it is in the candidate set and above threshold.

Only then does the reasoning model receive the selected action schema:

```json
{
  "action": "release.inspect",
  "schema": {
    "service": "string",
    "environment": "string",
    "limit": "integer"
  }
}
```

The model generates:

```json
{
  "service": "checkout-api",
  "environment": "production",
  "limit": 10
}
```

The runtime validates the arguments, reloads the current case/frontier, rechecks authority, and invokes the trusted adapter.

If Laya had returned:

```json
{
  "action": "release.rollback",
  "probability": 0.99
}
```

the runtime would reject it because that action was not in the candidate set.

Fast selection can be wrong about the best admissible action.

It cannot grant a forbidden action.
