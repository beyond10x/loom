---
format: aep.planning-md/3
id: story:llm-credentials-bearer
kind: story
status: implemented
title: A wire takes its credential from an llm-credentials reference
relations:
- decomposes: epic:downstream-adoption
- serves: vision:O3
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-executor/Cargo.toml
- confidence: inferred
  path: crates/loom-executor/src/harness/wire/bearer.rs
- confidence: inferred
  path: crates/loom-executor/src/harness/wire/mod.rs
- confidence: inferred
  path: crates/loom-executor/tests/
- confidence: inferred
  path: ess/domains/run.yaml
- confidence: inferred
  path: generated/rust/loom/
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T07:47:45Z", actor: "human:timo", revision: 12}
- {from: "proposed", to: "active", at: "2026-10-08T07:47:45Z", actor: "human:timo", revision: 13}
- {from: "active", to: "implemented", at: "2026-10-08T08:28:18Z", actor: "human:timo", revision: 14, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

The ported Harness wires (`ResponsesClient`, `MessagesClient`) take a `BearerSource` from their
embedder (`crates/loom-executor/src/harness/wire/bearer.rs`); the only implementation is
`StaticBearer`. Add a `BearerSource` over llm's `b10x-llm-credentials` `SecretResolver`, selected
by a `SecretRef` named in the run configuration together with its `CredentialKind`, so a run can
use an OAuth subscription login (for example a Codex login through llm's `CodexAuthFile`) with no
secret value in any configuration. Gap 2 of `epic:downstream-adoption`.

## Domain relations

- Run configuration -> credential reference, one per wire, the configuration owns the reference
  and the resolver owns the secret; the reference may exist before any secret does - inferable
  (inferred from `crates/loom-executor/src/harness/wire/bearer.rs:57-75`, one `BearerSource` per
  client, and llm `b10x-llm-credentials` 0.3.1 `SecretRef`, "a non-secret, operator-defined lookup
  name"; no ess/1 document declares this relation).
- Credential reference -> `CredentialKind`, many-to-one - inferable (inferred from
  `crates/loom-executor/src/harness/wire/bearer.rs:50-55`).

## Acceptance

A wire client built from a configuration that names a credential reference of kind `Oauth` sends,
as its bearer, the secret an injected fake `SecretResolver` returns for that reference (local
recorded wire, no network, no credential file).

## Checks

- A missing reference or a resolver refusal stops the call with an error that names the reference
  and never the secret value.
- The configuration that names the reference serialises with no secret value in it.

## ESS first

No record that can hold a credential reference exists yet: `Endpoint`
(`crates/loom-executor/src/harness/responses/mod.rs:98`) holds only `base_url`, `model` and
`context_window`, and keeps the credential out on purpose, and `ess/domains/run.yaml` declares no
wire configuration. So the first commit declares it in the `loom.run` domain
(`ess/domains/run.yaml`): a wire credential record beside the endpoint, never inside it, holding a
credential-reference newtype and the credential kind enum (`ApiKey`, `Oauth`), and no secret
field. Validate with the newest `ess`, `task generate`, then build the adapter from the generated
record and replace the hand-written `CredentialKind` in `bearer.rs` with the generated type
(`task no-hand-model` holds this). The red test of the first commit is the new adapter test in
`crates/loom-executor/tests/`.

## Upstream

None needed: llm 0.3.1 (already in `Cargo.lock`, pulled in through `b10x-llm-providers`) ships
`b10x-llm-credentials` with `SecretResolver`, `SecretRef`, `CoordinatedResolver` and, behind the
`codex-auth-file` feature, `CodexAuthFile`. The executor gains a direct dependency on it at tag
`0.3.1`.

## Notes

- `BearerSource::bearer` is synchronous and `SecretResolver::resolve` returns a future; the adapter
  must bridge without blocking inside a running Tokio runtime. Settle the bridge in the design.
- Renewal on a rejected token (`CoordinatedResolver::refresh`) has no hook in `BearerSource`; out
  of scope here, recorded as a follow-up if a 401 path needs it.
- The `b10x-loom` command line does not use these wires (it builds its model with llm
  `codex_model`, which reads the Codex login itself), so this story changes no CLI flag.
- Loom code never reads a credential file (AGENTS.md § Never): the resolver is injected.
- Shared surface: `generated/rust/loom/` (regenerated from `ess/domains/run.yaml`) with
  `story:governor-evaluate`, which depends on this story.
