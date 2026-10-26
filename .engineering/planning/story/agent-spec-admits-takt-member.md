---
format: aep.planning-md/3
id: story:agent-spec-admits-takt-member
kind: story
status: draft
title: A loom.agent/1 document carries a delivery program's own run configuration and passes the spec check
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: crates/loom-sdk/src/agent.rs
- confidence: inferred
  path: ess/domains/agent.yaml
revision: 4
---
## Outcome

A `loom.agent/1` document can carry a delivery program's own run configuration (takt's `takt`
member) and still pass the spec check, so a `Command` `LoomAgent` whose image reads
`/config/agent.yaml` is admitted with that document as written. The decoder keeps refusing every
member it does not know except the one declared place for a program's own configuration.

## Why

The orchestrator, 2026-10-26, from the takt 0.13.1 release: "`loom.agent/1` must admit a `takt`
member (today CheckAgentSpec refuses unknown members, so the takt document cannot pass the spec
check)". takt 0.13.1's image runs `tini -- takt run --repo /workspace --config
/config/agent.yaml` and reads its run configuration from the document's `takt` member.

## Acceptance

- ESS first: `loom.agent.AgentSpec` declares the member (a named `takt` member, or one opaque
  member for any program's configuration; the story's design step decides and records why), typed
  `Json` or a declared type; scenarios red first.
- `a_spec_with_a_takt_member_checks`: `b10x-loom agent check` exits 0 on a document holding the
  member; a document with any other undeclared member is still refused as `Malformed`.
- The member is walked for a credential like every other member (`SecretLiteral`).
- `deploy/crd/` regenerated; `task crd-drift` exits 0.

## Scope

Derived 2026-10-26 by the coordinator from the release note quoted above; not read in the tree.

- `ess/domains/agent.yaml` (`AgentSpec`) — inferred
- `crates/loom-sdk/src/agent.rs` (decoder), `crates/loom-agent` (check) — inferred
- `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/crds/` — inferred
- Related: `story:command-opaque-config-files` gives a `Command` resource files beside or instead
  of `spec.agent`; this story keeps the single-document contract takt 0.13.1 uses.

## Also reported (S7)

The orchestrator, 2026-10-26, from the takt one-hour pod trial (S7): "loom 0.42.0's LoomAgent
schema rejects a `takt` member in `spec.agent`, so an application's own settings cannot ride in
the loom.agent/1 document and every setting went in as command arguments". Trial record: "strict
decoding refuses `spec.agent.takt`; `--validate=warn` drops it". Wave 2026-10-26-cmdv's
`spec.configFiles` (story:command-opaque-config-files) gives a `Command` resource a way around it
(the program's configuration as an opaque file); this story stays for the loom.agent/1 document
itself.
