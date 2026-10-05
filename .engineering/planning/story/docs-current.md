---
format: aep.planning-md/3
id: story:docs-current
kind: story
status: active
title: Loom's site, README and AGENTS.md describe the one runtime repository
relations:
- decomposes: epic:runtime-consolidation
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: README.md
- confidence: inferred
  path: crates/loom-commission-docs
- confidence: inferred
  path: crates/loom-docs
- confidence: inferred
  path: docs
- confidence: inferred
  path: website
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T10:57:32Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T10:57:32Z", actor: "human:timo", revision: 3}
---
## Outcome

Loom's site (https://beyond10x.github.io/loom/), README.md and AGENTS.md describe Loom as it is on
`main`: the one runtime repository (Atlas ADR 0090) with the absorbed commission, governor and
intake crates, `b10x-loom run`, `b10x-loom-sdk`, llm 0.1.7 and canon-engineering 0.1.0, written
with the `docs` skill (`~/beyond10x/.agents/skills/docs/SKILL.md`) and its checklist.

## Acceptance

- Every checklist item of the docs skill is done or named as not applicable with the reason.
- Every CHANGELOG entry since the site's last content change is either reflected on a page or
  named as not consumer-relevant.
- Generated pages are current (`loom-docs generate --check`, `loom-commission-docs generate --check`),
  the site builds with broken links set to throw, and every command on a page was run in this tree.
- Ecosystem tools are linked to their public docs and GitHub repositories.

## ESS first

None: documentation; no behaviour change.
