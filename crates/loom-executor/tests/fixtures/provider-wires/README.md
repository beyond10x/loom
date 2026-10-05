# Provider-wire contracts

The two wires the ported provider adapters (`crates/loom-executor/src/harness/messages/` and
`crates/loom-executor/src/harness/responses/`) are held to, copied byte for byte from `beyond10x/harness`
at revision `798325f03cf5a18df8fadb346d31b314826136ec` (release 0.13.3):

| wire | version | Harness path |
|---|---|---|
| `anthropic-messages` | `2026-08-31` | `contracts/provider-wires/anthropic-messages/2026-08-31/` |
| `openai-responses` | `2026-08-31.1` | `contracts/provider-wires/openai-responses/2026-08-31.1/` |

Each is the version Harness's own contract tests pin at that revision. A version directory is
immutable: a change to what Loom sends or accepts cuts a new dated version beside it.
`crates/loom-executor/tests/harness_port.rs` checks that the request body each ported adapter builds equals
the wire's pinned `fixtures/turn-request.json`.
