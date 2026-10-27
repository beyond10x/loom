---
format: aep.planning-md/3
id: approval-record:command-confinement-profiles-approval
kind: approval-record
status: draft
title: story:command-confinement-profiles accepted under the orchestrator grant
relations:
- decides: story:command-confinement-profiles
revision: 2
---
The run is a sub-agent of session `b10x-chef-orchestrator`: no operator turn. The orchestrator
approved the item under the grant in `~/beyond10x/CLAUDE.md` (decision D94, 2026-10-27), quoting
Timo, 2026-10-27: "Speed up your work ... Be faster." Item: a seccomp / AppArmor profile field on
the `Command` variant of `LoomAgent`; additive only; whether `Unconfined` is refused without an
operator setting is decided in the story, recommended "refused by default".

Decisions D1 to D5 are in the story's `## Decisions`. D1 takes the recommendation: `Unconfined` is
refused unless `allowUnconfinedProfiles` is true. One unit, implemented by the coordinator; no
adversary pass (the orchestrator's brief keeps it for hand-written code and the item set a single
PR gate), so the admission walk is held by its own named tests only.
