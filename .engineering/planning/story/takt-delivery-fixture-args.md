---
format: aep.planning-md/3
id: story:takt-delivery-fixture-args
kind: story
status: implemented
title: The takt-delivery example passes only the arguments after the takt image's entrypoint
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: crates/loom-operator/tests/golden/takt-delivery.yaml
- confidence: cited
  path: crates/loom-operator/tests/render.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T18:27:39Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-26T18:27:39Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-26T20:55:52Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
## Outcome

The `takt-delivery` example the operator's render tests and golden file use passes only the
arguments that follow the takt image's entrypoint, so a resource copied from it runs `takt run`
once against the takt 0.13.1 image.

## Why

The orchestrator, 2026-10-26, from the takt 0.13.1 release: "loom 0.43.0's takt-delivery golden
fixture passes `args: [run, --repo ...]`, which against the 0.13.1 image entrypoint runs `run`
twice: the fixture should pass only the arguments after the entrypoint, or set
`command.entrypoint` explicitly." The 0.13.1 entrypoint is `tini -- takt run --repo /workspace
--config /config/agent.yaml`; Kubernetes appends `args` to the image's entrypoint.

## Acceptance

- `the_takt_delivery_fixture_runs_takt_once`: the rendered `takt-delivery` container's args hold no
  `run` and no second `--repo`, only what follows the entrypoint (`--iterations 1`).
- `tests/golden/takt-delivery.yaml` re-blessed; no other golden changes.
- No behaviour change in the controller; no CRD change.

## Scope

Derived 2026-10-26 by the coordinator, read in the tree at `e20461ad`.

- `crates/loom-operator/tests/render.rs:467`, `:660` (the fixture and its assertion) — cited
- `crates/loom-operator/tests/golden/takt-delivery.yaml` — cited
- AGENTS.md § Operator or design § 9a, where an example names the args — inferred
