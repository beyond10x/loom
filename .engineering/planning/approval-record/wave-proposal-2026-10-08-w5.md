---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-08-w5
kind: approval-record
status: draft
title: Wave 2026-10-08-w5 proposal accepted with no operator turn
tags:
- non-interactive
relations:
- decides: story:connectors-invoker
revision: 1
---
# Wave 2026-10-08-w5 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. The dispatch names this wave.

- Unit: `story:connectors-invoker` (serves `vision:O3`), gap 3 of issue 9: a new crate
  `crates/loom-connectors` implements `ConnectorInvoker` over `connectors-client` `v0.35.0`
  (`Client::invoke_v1alpha2`), the ESS endpoint record in `ess/commission/domains/responsibility.yaml`
  first.
- Cleared before the wave: `upstream-blocker:connectors-attempt-id` (Connectors `v0.35.0`).
- Selection: the one story the dispatch names; `aep plan artifact waves` places it in wave 3 with
  inferred scope; its collisions are with stories not in this wave.
- Adversary: one pass (new effect path; `Refused` versus `Err` mapping).
- Pre-flight: `/` 42G free, load 4.1, one worktree before the wave; builds limited to the touched
  crates with CARGO_INCREMENTAL=0.
- Commits authorised: 1 opening store commit, the unit commits on `impl/connectors-invoker`, 1 merge
  into `wave/2026-10-08-w5`, 1 closing store commit, the merge into `main` through the pull request,
  then the release commit, tag and GitHub Release through the repository's release process.
