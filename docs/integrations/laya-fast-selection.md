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

## As built

`b10x-loom-selector-laya` (`crates/loom-selector-laya`, `LayaSelector`) is the adapter, and an
experiment: it is not Loom's default, and no product crate (`b10x-loom-cli`, `b10x-loom-sdk`)
depends on it.

- **Wire.** `POST <endpoint>/v1/systemone`, as the Laya repository README at commit `1adc59f`
  describes it (§ "Self-Hosting: HTTP Server (Jev-compatible)"). Most Laya servers speak the same
  wire. The request is the goal as `state` and one `choice` question, `action`, whose criteria are
  the candidate action ids. The selector reads `answers.action.choice` and
  `answers.action.answer_confidence` and ignores every other field.
- **Confidence.** `answer_confidence`, the probability of the reported answer, is used, rendered as a
  plain decimal. Laya's `confidence` is 1 minus the normalised entropy of the distribution and is
  not read. A threshold must be calibrated for the option counts in use, so the confidence policy
  above has not been built yet.
- **Candidates.** Every catalogue entry it is handed is offered, a repeated id once. More than 100
  is refused before sending, because `laya-serve` answers 413 above 100 options. None is
  `NothingAdmissible`. For larger catalogues, see the hierarchy described above.
- **Failures.** Each of these is `SelectorError::Unavailable`, sent once and never retried: a
  choice outside the candidates (whatever its probability), a transport failure, a timeout
  (10 s by default, `LayaSelector::with_timeout`), a non-2xx status, an answer that is not JSON or
  nests past 128 levels, a missing `choice` or `answer_confidence`, and a probability outside
  [0, 1]. Loom's catalogue check still stands behind the adapter's own.
- **Locality.** The endpoint is a base URL, with a reverse-proxy prefix such as `/laya` if the
  server needs one, so a local `laya-serve` and a hosted Laya use the same adapter. No credential
  is sent yet. `LAYA_API_KEY` servers need a later change.
- **Transport.** llm's `b10x-llm-http` `HttpClient::post_json`, on a Tokio runtime the selector
  owns. The wire moves to llm if Laya becomes a default selector.
- **Held by** `crates/loom-selector-laya/tests/laya_selector.rs`, against a stub Laya server on a
  loopback port.

## References

- https://laya.studio/use-cases/agent-tool-routing
- https://laya.tools/laya-for-agents
- https://laya.tools/laya-api-servers
- https://github.com/NandhaKishorM/laya/blob/1adc59f7e371deb601fcfa18a14e25db238addcc/README.md
