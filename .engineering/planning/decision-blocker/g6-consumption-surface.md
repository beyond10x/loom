---
format: aep.planning-md/3
id: decision-blocker:g6-consumption-surface
kind: decision-blocker
status: cleared
title: Nobody has decided whether consumers read Loom as crates, the command, or both
relations:
- blocks: epic:loom-foundation-for-consumers
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-14T02:50:22Z", actor: "human:timo", revision: 3}
---
## Question

Do consumers read Loom as crates only, or also through the b10x-loom command?

## Options

- A. Crates only (loom_sdk pinned by tag); the command is no promised surface.
- B. Both: the crates, and the command with run --output jsonl as a stable contract.
- C. The command only.

Recommendation: B.

## Decision (2026-10-14)

B. Two stable surfaces: the crates (loom_sdk, pinned by tag) and the b10x-loom command, whose run --output jsonl intake.events records are a stable contract. Both stay stable per release; a break is named in the CHANGELOG.
