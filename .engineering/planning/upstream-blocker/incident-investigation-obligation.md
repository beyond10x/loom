---
format: aep.planning-md/3
id: upstream-blocker:incident-investigation-obligation
kind: upstream-blocker
status: open
title: incident.response/1 declares no investigation obligation
relations:
- blocks: story:incident-investigation-open
revision: 1
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
