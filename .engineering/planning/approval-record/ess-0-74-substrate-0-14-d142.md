---
format: aep.planning-md/3
id: approval-record:ess-0-74-substrate-0-14-d142
kind: approval-record
status: approved
title: ESS 0.74.0 and Substrate 0.14.0 pins approved by session b10x-chef-orchestrator
summary: 'D142, 2026-10-28: newest ESS and Substrate in Loom; substrate-0-12-pin closed'
relations:
- decides: story:substrate-0-12-pin
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-28T15:03:56Z", actor: "human:timo", revision: 2, executor: "agent:loom-pins-ess074-sub014", correlation: "D142"}
---
## The stop

Item D142 (2026-10-28): move Loom's ESS pins (crates and the `ess` CLI) to 0.74.0 and its
Substrate pin from 0.13.0 to the newest released Substrate (0.14.0); verify story:substrate-0-12-pin
against the store and the code and move it to implemented with evidence; implement
story:child-launches-through-confinement-adapters only if it is unblocked and accepted.

## Who decided

Session `b10x-chef-orchestrator`, 2026-10-28, under the operator's standing grant for class-C items
(a dependency upgrade, including the ess pin; commits and pull requests through the bot route). A
relayed approval, not a bypass. The operator's words, as relayed:

> ALWAYS upgrade to most recent ESS

## Decisions taken inside the item

- The Substrate pin names the 0.14.0 tag commit (`0b9d50c6`) with an exact `=0.14.0`, as every
  Substrate pin here did. The move is listed under `### Changed`, as the 0.10.0 to 0.13.0 move was.
- story:substrate-0-12-pin moves to implemented: every acceptance item holds on `main` (Loom 0.46.0)
  and on 0.14.0.
- story:child-launches-through-confinement-adapters is not implemented: it is a draft, and Substrate
  0.14.0 serves the scoped workspace write but not standard input, a live output stream or a
  per-launch aperture through a confinement adapter (its `## Waits on`).
- No release: a pin move does not require one under `AGENTS.md` § Releases.
