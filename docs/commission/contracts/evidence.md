# Contract Sketch — Observation and Evidence

## Observation

Raw report from a runtime/integration/verifier.

```yaml
source: connector:github-actions
subject: implementation:R2
observed_at: 2026-10-04T00:00:00Z
payload:
  conclusion: success
```

## Evidence

Validated, typed, attributable information admitted by the protocol/governor.

```yaml
kind: test_result
subject:
  type: implementation
  revision: R2
producer:
  principal: service:ci
observed_at: 2026-10-04T00:00:00Z
facts:
  tests.pass: true
provenance:
  source: github-actions
  run_id: "1234"
```

## Rule

```text
Observation
   ≠
Evidence
```

An evidence adapter may interpret one or more observations.

The governor validates evidence applicability.

A raw model statement such as "tests passed" is not automatically evidence.
