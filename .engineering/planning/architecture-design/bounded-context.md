---
format: aep.planning-md/3
id: architecture-design:bounded-context
kind: architecture-design
status: draft
title: Current working state, recent events, retrievable history
relations:
- designs: story:bounded-context
revision: 1
---
## Decision
Extend Briefing rather than the separate governed-loop compactor. Preserve exact intent and instructions in a stable prefix. Put typed current state, recent events and current candidates after it. Revision-bound test state never validates a later edit. Artifact text is quoted data and never updates typed state.

Archive compact events from event one, independently bounded at 16 MiB / 4096 entries. Bulk source remains in ResultStore. Reuse ResultStore reference validation and byte/line/JSON selectors; history listing omits whole metadata records when a page is full. Capacity errors latch and terminate subsequent model access, even after a last-step effect.

At >48 KiB serialized agent request or tail overflow past 64, retire a batch aiming for 32 KiB. Enforce 64 KiB on every bounded request, including schema and JSON escaping. Mandatory oversize fails; lookup answers may be omitted whole with an explicit error. Selection and arguments each allow eight lookups, no model summarization. Reports measure both policies including classifier and retain missing usage as unknown.

## Rollout
Legacy remains default. run_with_options preserves run and SliceRequest source compatibility. Recorded matched workflows establish behavior and byte savings; live quality/cost evaluation remains future work and no proportional cost saving is claimed.
