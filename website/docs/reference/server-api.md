---
title: Server dashboard API
sidebar_label: Server dashboard API
description: Read the agents, current work, terminal runs and measured usage on one Loom server.
---

# Server dashboard API

Use the dashboard URL printed by `b10x-loom agent serve`. Agents using the same dashboard directory appear at that URL. Each agent keeps its own execution state. The listener binds to loopback. Use a tunnel or an authenticated ingress to reach it remotely.

[Download the dashboard OpenAPI 3.1 document](pathname:///loom/api/openapi/dashboard.json). On a running server, **GET `/api/openapi.json`** returns the embedded contract without credentials. **GET `/`** serves the dashboard.

## Start a two-agent dashboard

On the server, start two serving hosts under the same OS user. Give them the same `LOOM_DASHBOARD_DIR` and distinct `--state` directories. Each input spec must already pass `agent check` and have usable model routes. Give A2A hosts distinct listener ports and set their agent bearers in their respective launch environments.

```sh
export LOOM_DASHBOARD_DIR="$HOME/.local/state/scout-reviewer-dashboard"
export LOOM_DASHBOARD_ADDR=127.0.0.1:8765
b10x-loom agent serve --spec scout.yaml --state "$HOME/.local/state/scout" &
b10x-loom agent serve --spec reviewer.yaml --state "$HOME/.local/state/reviewer" &
```

These commands run in the launching shell. A service manager can keep the processes running across logouts. With no dashboard settings, the first host chooses a loopback port and saves it in `address` beneath `$XDG_STATE_HOME/b10x-loom/dashboard` (default `~/.local/state/b10x-loom/dashboard`). Subsequent hosts in that directory use the saved address. Set an explicit port before the first launch if clients need a fixed URL.

On the client, forward a local port to that server over SSH. For the example above, replace `operator@server` with the SSH destination:

```sh
ssh -N -L 127.0.0.1:8877:127.0.0.1:8765 operator@server
```

Open `http://127.0.0.1:8877/` on the client. Enter the dashboard viewer token. The file named `viewer-token` in the server's dashboard directory contains the operator credential for every agent in that scope. Keep the directory private (0700) and the credential files private (0600).

For a restricted viewer, provision a separate random bearer token. Put the lowercase SHA-256 hex digest of its exact token bytes, without a newline, in `viewers.json` in the dashboard directory. Each value is an array of allowed `agents[].id` strings. The operator can obtain those IDs from an unfiltered snapshot. For example, the shape of a one-agent grant is:

```json
{"<sha256-hex-of-viewer-token>": ["<agent-id-from-snapshot>"]}
```

Distribute the actual token privately to that viewer. This file is read when a scoped viewer authenticates. An empty grant list refuses access.

## Read a snapshot

**GET `/api/snapshot`** accepts the dashboard viewer token in the `Authorization: Bearer` header. The printed token-file path belongs to the operator. A scoped viewer sees only the agents assigned to that viewer, including in aggregate counts. An agent’s A2A token belongs to its individual listener.

| Query parameter | Meaning |
| --- | --- |
| `agent` | Optional. Exact `agents[].id` from a snapshot. |
| `from` | Optional. Include terminal runs ending at or after this RFC 3339 instant. |
| `to` | Optional. Include terminal runs ending before this RFC 3339 instant. |

A request without filters includes the visible agents and retained runs. A time filter applies to terminal history. The inventory and active counts still describe current work. An agent outside the viewer's scope and an unknown agent both return 404. Unknown or repeated parameters, empty values, malformed timestamps and an inverted or empty interval return 400.

A successful response is one JSON object. The generated schema lists every member and its type.

| Member | How to read it |
| --- | --- |
| `server`, `time`, `cursor` | Server scope, sample timestamp, and opaque identifier for this snapshot. |
| `agents` | Visible registrations, their stable `id`, current process `instance`, last observation, status, and current work. |
| `runs` | Retained terminal turns with their agent and item IDs, outcome, steps, events, timestamps and observed model usage. The same logical turn has the same ID after replay. |
| `stats` | Aggregates over this selection. Latency percentiles use runs with both timestamps. Coverage counts name the number of runs contributing measured values. Cost in `cost_micro_usd` is in millionths of a US dollar. |
| `partial` | The snapshot has incomplete evidence. Inspect coverage and individual records before interpreting totals. |

An unknown measurement is `null`. A recorded zero is a measurement. Throughput uses completed turns per hour and needs both interval bounds. The dashboard groups `proposed` and `proposed_case` as completed, `failed` and `stopped` as failed, and `declined` and `unclassified` as rejected. A completed proposal records an answer. Application of that proposal belongs to the downstream system.

The dashboard retains up to **512 terminal runs per agent**. Pruning history sets `partial`. Legacy records can lack timestamps, latency, steps and usage. The agent row reports its spec revision, protocol, model and channel polling interval when the host supplies them. External schedules are outside the host's observations.

To price measured model calls, set `LOOM_DASHBOARD_RATE_CARD` on each agent host to the path of a rate-card JSON file before starting it. The document declares `source`, `as_of` (`YYYY-MM-DD`), and `models` keyed by the model identity returned by the provider. For each model, supply `input_usd_per_mtok`, `cached_input_usd_per_mtok` and `output_usd_per_mtok` in US dollars per million tokens. An unpriced model or incomplete usage leaves the turn's cost unknown. `stats.coverage` reports the measured run counts behind the totals. [Rate-card format and arithmetic](https://github.com/beyond10x/loom/blob/main/crates/loom-executor/src/metering.rs)

The 200 response includes a quoted `ETag`. Poll with that value in `If-None-Match` to receive **304 with an empty body** while the selection is unchanged. Keep the prior snapshot on 304. A fresh GET without that header retrieves the entire current selection. The cursor identifies a snapshot and is not a paging token.

| HTTP status | Meaning |
| --- | --- |
| 200 | Snapshot returned. |
| 304 | The conditional request already has this snapshot. |
| 400 | Invalid request or filters. |
| 401 | Dashboard token missing or invalid. |
| 403 | The viewer has no agent grants. |
| 404 | The requested agent is unknown or outside the viewer’s scope. |
| 503 | The viewer configuration or snapshot could not be read. |

## Read the agent's registered spec

**GET `/api/agents/{agent}/spec`** returns the source that Loom validated when this agent registered. Substitute the opaque `agents[].id` from the snapshot. Send the **operator** token from the private `viewer-token` file in the `Authorization: Bearer` header. The endpoint accepts no query parameters.

The response is `application/yaml; charset=utf-8`. Whitespace, comments and the original serialization are preserved. If the registered file used JSON syntax, the body retains that syntax. The quoted `ETag` is the lowercase SHA-256 digest of those exact bytes. Compare it to the agent row’s `spec_revision` to confirm the body describes the revision you selected. If the agent registers a different revision between requests, refresh the agent row. Reading the source performs no agent action.

A spec can include internal addresses, filesystem paths, binding configuration and an agent’s operating instructions. Source access requires the operator credential even for a viewer allowed to see this agent’s runs. The dashboard snapshot includes observation metadata and the spec revision. It excludes the source text.

| HTTP status | Meaning |
| --- | --- |
| 200 | The operator receives the complete registered document. |
| 400 | Invalid request or unexpected query parameters. |
| 401 | Dashboard token missing or invalid. |
| 403 | An authenticated scoped viewer requested source. The same refusal covers every agent ID. |
| 404 | The agent ID is unknown or invalid, or no source was saved for it, including legacy registrations. |
| 503 | The viewer configuration or source could not be read, or source bytes failed the revision check. |

## Dispatch work to an agent

On each A2A listener, **GET `/ready`** checks readiness and **GET `/live`** checks liveness. Those probes accept unauthenticated requests. The agent card and JSON-RPC endpoint require the agent bearer.

The dashboard exposes observation reads. Submit work to the **agent’s own A2A address**, using its agent bearer. Its agent card is at `/.well-known/agent-card.json`. Its configured JSON-RPC path accepts `SendMessage`, `SendStreamingMessage` and `GetTask`. A2A task IDs belong to that listener’s process. Dashboard run IDs belong to the agent state directory and logical item. [Agent serve command](./cli.md#b10x-loom-agent-serve) · [A2A wire and replay examples](https://github.com/beyond10x/loom/blob/main/docs/qualification/2026-10-25-agent-a2a.md)

## Specification and generation

The generated ESS reference covers the [dashboard data and views](./ess/loom-dashboard.md) and the [observation request](./ess/loom-observation.md).

The ESS model declares the viewer’s snapshot request and the operator’s source request, their input fields, outcomes and response types. Its HTTP binding assigns the GET paths, status codes and response bodies. The public projection adds the YAML media type for the text response, HTTP cache handling, bearer schemes, the self-description route and explicit JSON nulls. Run `task dashboard-api` in the repository to regenerate the public contract. `loom-xtask dashboard-api --check` compares the committed document with a fresh projection. [ESS model](https://github.com/beyond10x/loom/tree/main/ess/dashboard) · [HTTP binding](https://github.com/beyond10x/loom/blob/main/ess/dashboard/http.yaml)
