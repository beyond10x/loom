---
format: aep.planning-md/3
id: story:docs-agent-stack-layers
kind: story
status: active
title: Loom's docs place Loom as ADRs 0096 and 0098 do and cite ADR 0108 for the agent spec
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: README.md
- confidence: cited
  path: website/docs/concepts
- confidence: inferred
  path: website/docs/guides
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:20:18Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-26T13:20:18Z", actor: "human:timo", revision: 7}
---
## Outcome

Loom's documentation describes Loom as Atlas ADRs 0096 and 0098 place it: the L3 governed runtime,
which calls llm for its own decisions and metaharness for agent work, and is not a harness that
metaharness compares. AGENTS.md § Serves and § Boundary, README.md and the site stop citing ADR 0071
(superseded by ADR 0096) as current. Where the docs describe the agent spec, one line cites ADR 0108:
Loom owns the `loom.agent/1` schema and serves A2A from the spec; Agency holds the document by value.

## Why

Alignment audit of 2026-10-26 (`b10x-chef-orchestrator`), § C: "Loom AGENTS.md contradicts 0096 |
`AGENTS.md:21` 'Loom is the native executor Metaharness compares'; `:24` cites ADR 0071 (superseded
by 0096)". The orchestrator (dispatch-queue #13): rewrite those lines to the 0096/0098 wording, sweep
README and website docs for the same claim, and add the ADR 0108 reference "where loom's docs
describe the agent spec (one line each), no architecture change".

## Acceptance

- `git grep -n "native executor\|ADR 0071\|0071" -- AGENTS.md README.md website/docs` names no
  sentence presenting ADR 0071 as current or Loom as a harness metaharness compares.
- AGENTS.md § Agent spec, README's agent spec section and the site's agent-spec pages each carry one
  line citing ADR 0108.
- `task docs-check` and `task website` exit 0; the `ess_gate.rs` phrase checks on AGENTS.md hold.

## ESS first

Exempt: documentation only, no behaviour change.

## Scope

Derived 2026-10-26 by the coordinator. All **cited**.

- AGENTS.md :1-38 (§ header, Serves, Boundary), § Agent spec
- README.md (layering paragraph :30-45, agent spec section)
- `website/docs/concepts/commission-and-harness.md` (:7, :42), `website/docs/concepts/one-run.mdx` (:7), `website/docs/concepts/where-loom-ends.md` (:7)
- website pages describing `loom.agent/1` (guides)
- Not touched: CHANGELOG.md (nothing a consumer runs changes)
