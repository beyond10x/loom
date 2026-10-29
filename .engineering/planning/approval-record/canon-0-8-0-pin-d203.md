---
format: aep.planning-md/3
id: approval-record:canon-0-8-0-pin-d203
kind: approval-record
status: approved
title: Canon 0.8.0, engineering-protocols 0.14.0 and the Loom release (D203)
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-29T12:57:45Z", actor: "human:timo", revision: 2}
---
## Decision

Session `b10x-chef-orchestrator`, 2026-10-29, decision D203, approved under the operator's grant
(class C: a dependency upgrade and a release through Loom's own process), in these words:
"D203: move loom to canon 0.8.0 and release. Approved under the grant ... Blocked consumer: takt
(task:canon-0-8-0 in takt's store needs loom's governor on canon 0.8.0 so takt does not carry two
canon versions)."

1. Loom moves every crate on Canon to Canon 0.8.0 (tag `0.8.0`, `cf17175`). `Predicate` gains
   `Since(EvidenceSince)`; Loom handles it wherever it matches exhaustively and adds no `since`
   behaviour beyond what Canon evaluates.
2. Canon types cross the engineering-protocols boundary (`registry::get` returns Canon's model),
   so engineering-protocols moves with it. The orchestrator, 2026-10-29: "engineering-protocols
   0.14.0 is released and verified, on canon 0.8.0 ... Drop the [patch], pin ep 0.14.0, run your
   gate and ship loom."
3. The ESS pin stays (D198, the operator 2026-10-29: do not move the ESS pin), although
   § Releases says a release moves it.
4. The pin ships alone: the store holds no accepted story. Its pull request names takt as the
   blocked consumer.

## Checked

- Loom `main` is `b94c03f1`, the tag `0.49.1`, with nothing after it.
- No Loom crate matches on `b10x_canon::model::Predicate`; the workspace compiles unchanged.
- The three governor fixtures are byte-identical to engineering-protocols 0.14.0's, and its
  `protocols/` list is the one 0.13.0 had.
