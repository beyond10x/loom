---
format: aep.planning-md/3
id: upstream-blocker:incident-investigation-obligation
kind: upstream-blocker
status: cleared
title: incident.response/1 declares no investigation obligation
relations:
- blocks: story:incident-investigation-open
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T21:08:31Z", actor: "human:timo", revision: 2}
---
## What is missing

`incident.response/1` in beyond10x/engineering-protocols at tag `0.1.0` (`a7d02b4`) declares one
obligation, `restore_service`; it declares no investigation obligation that stays open after
emergency mode is left.

## What it stops

`story:incident-investigation-open`.

## Cleared when

An engineering-protocols release whose `incident.response` protocol declares an investigation
obligation that stays open after `emergency.leave`, and its `inc-492` fixture shows it.

## Resolution

Cleared 2026-10-08: engineering-protocols 0.3.0 (https://github.com/beyond10x/engineering-protocols/releases/tag/0.3.0) declares in `incident.response/1` an obligation `investigate_cause`, discharged by `cause.identified=TRUE`, that stays open after `emergency.leave` (example `inc-492`). Loom still pins 0.1.0; `story:incident-investigation-open` re-pins to `tag = "0.3.0"` first.
