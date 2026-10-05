---
format: aep.planning-md/3
id: story:retire-repositories
kind: story
status: implemented
title: commission, governor and intake are archived and every registry points at Loom
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:crate-names
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T10:06:50Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T10:06:50Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-05T13:27:20Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"verification":1}}}
---
## Outcome

`beyond10x/commission`, `beyond10x/governor` and `beyond10x/intake` are archived, and every
registry points at Loom.

## Acceptance

- Each repository's README names Loom as its home (bot PR), then the repository is archived by the
  bot (`PATCH /repos/beyond10x/<r>` `{"archived":true}`).
- gates-policy no longer enrolls them; the org secret is refreshed by its owner (agents stop and
  report at that step).
- Atlas catalog entries and the workspace guidance rosters (`src/workspace.rs`, `GUIDANCE`) drop
  them.
- No beyond10x repository's `Cargo.toml` names a git dependency on any of the three
  (`grep` over all primaries).

## Depends on

`story:crate-names`.

## Scope (inferred)

GitHub settings of the three repositories; `gates-policy/policy.json`; Atlas `catalog/`,
`src/workspace.rs`.
