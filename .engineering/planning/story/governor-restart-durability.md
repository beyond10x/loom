---
format: aep.planning-md/3
id: story:governor-restart-durability
kind: story
status: draft
title: A governor case survives a process restart
relations:
- decomposes: epic:governor
- serves: vision:O1
revision: 1
---
## Outcome

A case the governor holds survives a process restart. Today `epic:governor`'s acceptance ("keeps a
case across a process restart") is undelivered: the governor in `crates/loom-governor` evaluates an
in-process case through `CaseStore` (`story:import-governor`, `story:hosted-governor`,
`story:governor-evaluate`), and no story persists a case. `story:control-plane-storage` makes
storage fallible and excludes a persistence provider, so it does not cover this.

## Acceptance

A `software.change/1` case opened through the governor, with evidence submitted, is read back after
the process that held it ends and a new one starts over the same store: `current_revision`,
`frontier` and `completion` answer as before the restart.

## Open

Which store persists the case (a file store in Loom, or a port the host fills) is not decided; the
story is scoped when it is.
