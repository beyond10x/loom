---
format: aep.planning-md/3
id: story:boundary-loopback-relays
kind: story
status: active
title: The boundary serves model, Connectors-wire and git relays on loopback for declared operations and repositories
relations:
- decomposes: epic:credential-free-agent-pod
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: crates/loom-boundary
- confidence: cited
  path: crates/loom-cli/src
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T00:48:53Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-30T00:48:53Z", actor: "human:timo", revision: 3}
---
## Outcome

`b10x-loom agent boundary` (library `b10x-loom-boundary`) serves, on 127.0.0.1 only, one listener
per declared upstream:

- 8401, a model pass-through: every request under `/v1/` relayed to the model upstream, the
  response streamed back;
- 8402, the Connectors wire: `GET /v1/describe` and `POST /v1alpha2/invoke`, the invoke admitted
  only for a declared operation;
- 8403, a git smart-HTTP relay (`info/refs`, `git-upload-pack`, `git-receive-pack`) for declared
  `<owner>/<repo>` repositories only.

Each relayed request carries `Authorization: Bearer <projected token>`, read from the upstream's
token file on every request so a kubelet rotation is picked up, and every credential header the
agent sent is dropped. A request outside what is declared is refused locally and never reaches the
upstream. The token is never logged and never returned.

## Why

Design D243 § 2 and § 4, decisions A1 and C1 (operator direction 2026-10-30, epic
`epic:credential-free-agent-pod`).

## Acceptance

- `the_model_relay_adds_the_projected_token_and_drops_the_agents_credentials`.
- `the_model_relay_streams_the_response`.
- `an_undeclared_operation_is_refused_and_never_reaches_the_upstream`.
- `a_declared_operation_reaches_the_upstream_byte_for_byte`.
- `an_undeclared_repository_is_refused_and_never_reaches_the_upstream`.
- `a_rotated_token_file_is_read_on_the_next_request`.
- `the_boundary_listens_on_loopback_only`.
- `a_missing_token_file_fails_the_request_and_sends_nothing`.

## ESS first

No ESS change: the boundary is a relay with no noun of its own; its declared operations and
repositories are the `loom.resource.Boundary` members `story:boundary-container-projected-token`
declares.

## Scope

- `crates/loom-boundary/` (new), `crates/loom-cli/src/{lib,main}.rs`
- `website/docs/reference/cli.md`, `website/docs/reference/crates.md` (generated)
- AGENTS.md (new § Boundary relay, § Never narrowed for the projected token), README.md,
  CHANGELOG.md
