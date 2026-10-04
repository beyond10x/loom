---
format: aep.planning-md/3
id: epic:runtime-consolidation
kind: epic
status: draft
title: 'Loom is the one runtime repository: commission, governor and intake become crates'
refs:
- provider: atlas
  reference: adr:0090
relations:
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

`beyond10x/loom` is the one runtime repository: a person runs `b10x-loom run "<intent>"`, and a
developer embeds the same runtime through one SDK crate. Commission, governor and intake are crates
here, and their repositories are archived.

## Basis

Operator decision 2026-10-05, option A ("YES Option A"), recorded as Atlas ADR 0090
(`architecture/adr/0090-loom-is-the-one-runtime-repository.md`). Loom's Canon restriction was
removed the same day (`architecture-decision-record:canon-dependency-allowed`).

## What comes in (origin/main, 2026-10-05)

| Repository | Commit | Crates | ESS files | Stories |
|---|---|---|---|---|
| commission | `e61e4f0` | commission, commission-conformance, commission-testkit, commission-docs, commission-xtask | 4 | 13 implemented, 5 draft |
| governor | `81fcc1e` | governor | 0 | 1 implemented |
| intake | `8d25b09` | intake-cli, intake-model, intake-references, intake-router, intake-slice | 3 | 6 implemented |

## Target crates

| Crate | From | Canon |
|---|---|---|
| contracts | commission core: ports, case, frontier, authority | refused by test |
| executor | today's `b10x-loom` | allowed |
| runtime | commission runtime + intake-slice loop and local executor | allowed |
| governor | governor | used |
| intake | intake-router + intake-references | used (ELS registry) |
| sdk | facade over contracts, runtime, executor | allowed |
| cli | intake-cli | used |

`intake-model` goes to llm.

## Stories, in order

1. `story:import-commission`
2. `story:import-governor`
3. `story:import-intake`
4. `story:runtime-merge`
5. `story:loom-cli` and `story:loom-sdk`
6. `story:crate-names`
7. `story:retire-repositories`

## Not in scope

Porting the rest of Harness (its own stories here); the ELS rename (els
`epic:engineering-protocols-rename`, whose `story:consumer-repin` then targets Loom).
