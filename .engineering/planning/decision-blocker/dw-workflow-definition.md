---
format: aep.planning-md/3
id: decision-blocker:dw-workflow-definition
kind: decision-blocker
status: cleared
title: Nobody has decided whether a workflow definition is a protocol or a Loom host construct
relations:
- blocks: epic:deterministic-workflows
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-14T02:50:22Z", actor: "human:timo", revision: 3}
---
## Question

Is a deterministic workflow defined in the protocol (Canon, engineering-protocols) or by Loom over a protocol?

## Options

- A. A protocol construct: the protocol declares the steps; changes outside Loom first.
- B. A Loom host construct: a Loom definition sequences actions the frontier admits; Canon still decides.
- C. B now, and a story to move it into a protocol after measurement.

Recommendation: C.

## Decision (2026-10-14)

C. A Loom host construct now: a Loom-side definition sequences the actions the protocol frontier admits, and Canon still decides admission and completion; changes stay inside Loom. A draft story moves the definition into a protocol after measurement; where it finally lives is decided when that story is ready.
