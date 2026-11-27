---
format: aep.planning-md/3
id: epic:server-hosted-agent-dashboard
kind: epic
status: draft
title: Every server running background agents serves their live operator dashboard
summary: Specify the dashboard in ESS-UI, render it by hand in Vue, and serve fleet, current work, history and metered usage from the agent server.
relations:
- serves: vision:O3
- informed_by: epic:tree-daemon-under-enforced-limits
- informed_by: epic:plugin-layer
revision: 1
---
# Server-hosted Loom agent dashboard

## Required outcome

Starting a background Loom agent on a server brings up a browser UI on that server as part of normal startup. The operator opens one reported URL and sees every agent they are allowed to observe on that server, what each is doing now, and what happened on prior runs. A second agent joins the same inventory. Completing a turn leaves its history visible while the host waits for the next item. Closing the browser has no effect on execution.

The UI contract is an authored `ess-ui/1` document checked against Loom's ESS model. Loom ships a hand-written Vue 3 renderer and its compiled assets with the server. A fresh deployment serves the UI and observation API from the same origin, including on a host with no development tools or CDN access. The first delivery is observational. Existing Loom interfaces keep authority over dispatch, approvals, cancellation and execution.

## First acceptance workflow

Run two background agents with distinct state directories on a clean server using the normal start commands. Open the URL emitted by startup from a second machine through the configured access path. Both agents appear. Dispatch a turn to Scout, hold its fixture at a known action, and inspect its current run in the browser while Reviewer remains idle. Release the fixture, forward the proposal to Reviewer and verify both terminal outcomes against the recorded items. Refresh the page and restart a host. The same completed runs remain addressable and counters stay stable. Repeat using only the packaged runtime and a browser.

## Existing owners and missing contracts

- `b10x-loom agent serve` owns served agent lifetime. Its A2A listener accepts tasks and exposes per-task progress. An operator observing work submitted by other clients needs a host-wide inventory and read feed. Discovering work by polling known A2A task IDs cannot supply it.
- `loom.plugin.RecordLine` and `TurnStep` carry terminal item and step evidence. Existing records can lack step details. `loom.metering.RunUsageReport` supplies model usage where an executor captured it. `loom.runlog` and `loom.tree` own case-tree events and status. Keep the contributed records under their existing owners. Add observed start/end timestamps and stable associations at the producer where they are missing. Never manufacture an old duration from a log file's modification time.
- `loom-operator` owns Kubernetes workload status and scheduling. Read lifecycle or schedule data from its published status when available. A schedule managed by an external service can be reported as external with unknown next start.
- Follow the Sessionspec repo's `ui/sessionspec.ui.yaml`, `ui/tests/` and `ui/app/` split. ESS owns the UI language. Loom owns the screens, data bindings, Vue components and shipped static bundle.

## Host and browser contract

Proposed topology: one observation service per configured server scope. The first background host starts or attaches to it, later agents register concurrently, and supervision keeps the URL stable across individual agent exits. Use an explicit local registration channel, scoped read access and liveness leases. Registration is idempotent by instance identity. Retain last-known status and timestamp when a host disappears. Define who owns the service during startup, restart and shutdown for systemd and deployed workloads. Report an actionable startup/readiness error when the required UI cannot be served. A silent headless fallback fails this requirement.

Use separate stable identities for server, agent specification, running instance and logical turn. Scope the durable item key by its state store. A listener task ID is a short-lived lookup handle. Open a consistent initial snapshot and resume live updates after its cursor. On a gap, reconnect or expired cursor, refresh the snapshot. A record replay or overlapping snapshot and event must leave the aggregate unchanged. Read the bounded page of history requested. Keep execution independent of browser backpressure.

## What the operator sees

1. **Server overview.** Agent name, instance, host, readiness, idle/running/waiting/stopped or stale state, last activity, active job summary, and schedule when known. Selecting an agent preserves a stable deep link.
2. **Agent and current work.** Spec identity and revision, protocol, model route or harness where applicable, current run and action, start time, elapsed time, steps and model-call budget remaining, pending approval or stop reason. Use an explicit unknown field when older records or a backend omit it. Show the last observation time beside stale data.
3. **Run detail.** Ordered events from acceptance through completion, rejection or failure. Show the recorded outcome, action results and linked child or handoff only when the producer supplies a correlation. A completed proposal and an applied external effect have distinct labels.
4. **Stats.** For the selected server, agent and UTC interval, show current active runs, unique completed/failed/rejected turns, throughput per hour, elapsed p50/p95 with sample count, and model calls, input/output tokens and cost where captured. Define the completion chart by terminal timestamp, count one logical turn per terminal outcome and show the denominator for outcome rates. Latency uses runs with both timestamps. Price only metered usage with a known rate and currency. Return completeness alongside totals so missing measurements render as unavailable or partial. Include refusal and budget-stop counts with reason, and a drill-down to contributing runs.

## Delivery sequence

1. **Author the contracts.** Declare typed host inventory, run detail, live update and aggregate views in the Loom ESS model. Declare pages, routes, reads, filters, component addresses and loading, empty, stale, partial and denied states in `ui/loom.ui.yaml`. Provide fixtures for two agents, an in-flight action, a recorded result, an unpriced model and a disconnected host. Pass `ess ui check --path ui/loom.ui.yaml --model ess` and `ess ui test --path ui/loom.ui.yaml ui/tests/overview.yaml` against the repo's pinned tools.
2. **Prove the vertical slice.** Add the host discovery and observation adapter at the serving boundary. Build the minimal Vue overview and run detail by hand. Embed the bundle. Pass the first two-agent workflow through the packaged binary before adding charts.
3. **Make history and statistics reliable.** Connect real step, outcome and meter producers. Add snapshot/resume and bounded retention. Compare UI totals to deduplicated records, including repeat delivery, host restart, missing telemetry, budget exhaustion and a requested interval containing zero runs.
4. **Ship the server lifecycle.** Integrate the same UI contract into supported background entry points. Cover the simplest direct agent host first, then supervised service, managed case-tree execution and Kubernetes as their owners expose status. Test concurrent starts, port collisions, one agent exiting while another stays up, normal shutdown, upgrade, direct navigation after refresh and an authenticated remote browser. Advertise the URL through startup output and deployment status.
5. **Prove renderer parity.** Run the same ESS-UI scenarios through the Vue browser tests using stable component addresses. Compare displayed rows, drill-down, time filtering, event order and error states with the contract fixtures. Execute a browser journey against two actual local Loom processes and a recorded model. Keep normal gates offline with shared fixtures, each test bounded to 60 seconds and the test step to 120 seconds.

## Exposure and completion evidence

Use the configured server access boundary and read-only viewer identity. The browser receives a same-origin viewer session with server/agent scoping. Model credentials, agent bearers, raw prompts and unrestricted tool output stay out of overview responses, bundled assets and metric labels. Verify that a viewer scoped to one agent cannot read the other agent through lists, details, streams or statistics. Reuse the deployment's authenticated ingress for remote access and local binding for a workstation.

The completion record includes the ESS and ESS-UI check outputs, the packaged launch commands and advertised URL, a browser capture of a live action, the corresponding two agent records, reconciled counters before and after replay/restart, and the observed behavior when one host is unavailable. The implementation remains draft until these workflows are demonstrated.
