---
format: aep.planning-md/3
id: story:http-binding-cluster-ca
kind: story
status: implemented
title: An http binding reads an https endpoint a cluster CA signs, trusting the PEM file LOOM_HTTP_CA_FILE names
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-http-binding
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-agent
- confidence: cited
  path: crates/loom-cli
- confidence: cited
  path: crates/loom-conformance
- confidence: cited
  path: docs/qualification/2026-10-25-agent-http-binding.md
- confidence: cited
  path: ess/domains/agent.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/scenarios/http-binding-cluster-ca
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference/cli.md
- confidence: cited
  path: website/docs/reference/ess
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:20:19Z", actor: "human:timo", revision: 13}
- {from: "proposed", to: "active", at: "2026-10-26T13:20:19Z", actor: "human:timo", revision: 14}
- {from: "active", to: "implemented", at: "2026-10-26T14:47:20Z", actor: "human:timo", revision: 16, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

A served agent's `http` binding reads an `https` endpoint whose certificate a cluster CA signs. The
host setting `LOOM_HTTP_CA_FILE` names a PEM file; `b10x-loom agent serve` reads it once, before
anything runs, and every `http` binding read trusts its certificates in addition to the webpki
roots. Unset, nothing changes. A file that cannot be read, holds no certificate or is over 1 MiB
stops `agent serve` with exit 1 naming the variable, before a model call, a read or a record. The
setting reaches no model call, no Connectors child and no A2A listener.

## Why

`docs/qualification/2026-10-25-agent-http-binding.md` § Not verified here: "the reader has no
cluster-CA setting yet, so an endpoint whose certificate a cluster CA signs fails the read as
unavailable", and the TLS backend is "enabled by another dependency's features in the workspace
build, not by `crates/loom-agent/Cargo.toml`". Filed from the agents wave by the orchestrator
(dispatch-queue #13, 2026-10-26): "a cluster-CA setting for in-cluster TLS endpoints (the http
binding) ... implement the cluster-CA setting if it is one unit".

Decided by the coordinator: a host setting, not a spec member. Which CA signs an endpoint is a
property of the cluster the agent runs in, not of the agent, as `LOOM_A2A_BEARER` is a host setting
beside `expose.a2a`; a spec member would change the `LoomAgent` CRD, which the same item forbids
during the trial. On Kubernetes the file comes from `spec.secretFiles` (or the service account's
`ca.crt`) and the variable from the `spec.secretRef` Secret, with no controller change.

## Acceptance

- ESS first: a stateless command `loom.agent.AdmitHttpTrust` (inputs: whether the variable is set,
  whether the file was read, its bytes, its certificate count) with refusals `Unreadable`,
  `TooLarge` and `NoCertificate` and accepting outcomes for the webpki roots alone and for the
  extended roots; `task conform` runs it against the Rust decision function.
- `crates/loom-agent/tests/`: an `https` loopback listener whose certificate a test CA (generated at
  test time, no key committed) signs is refused without the setting and read with it; no network.
- `agent serve` with the variable naming a missing file, an empty file, a file with no certificate,
  or a file over 1 MiB exits 1 naming the variable and never prints the file.
- `crates/loom-agent/Cargo.toml` declares reqwest's rustls TLS feature itself.
- `task check` exits 0 with `deploy/crd/` and `charts/` unchanged.

## ESS first

The first commit adds `AdmitHttpTrust`, its error and reason enum to `ess/domains/agent.yaml` only;
`task drift` is red on it.

## Not in scope

Client certificates (mTLS); a per-binding CA in the spec; trusting the CA for model or Connectors
traffic; the operator rendering the variable.

## Scope

Derived 2026-10-26 by the coordinator. **cited** = read in the tree, **inferred** = could be wrong.

- **Primary surface:** `crates/loom-agent/src/http.rs` (`HttpGet`, client built at :262) — cited
- **Files:** `crates/loom-agent/src/serve.rs` (start-up refusals) or `crates/loom-cli/src/` (`agent serve`) — inferred
- **Files:** `crates/loom-agent/Cargo.toml`, `Cargo.lock` (a dev-dependency to mint a test CA) — inferred
- **Files:** `ess/domains/agent.yaml` (new command after `AdmitServe` :786) — cited
- **Files:** `crates/loom-conformance/src/` (the agent target) — inferred
- **Generated:** `generated/rust/loom/`, `website/docs/reference/ess/`, `website/data/ess/` — cited
- **Documents:** AGENTS.md § Agent spec (http paragraph), README.md, website guide for `agent serve`, `docs/qualification/2026-10-25-agent-http-binding.md`, CHANGELOG.md — cited
- **Confidence:** medium
