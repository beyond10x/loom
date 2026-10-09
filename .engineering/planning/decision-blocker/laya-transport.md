---
format: aep.planning-md/3
id: decision-blocker:laya-transport
kind: decision-blocker
status: cleared
title: How the Laya selector reaches its endpoint
relations:
- blocks: story:laya-selector
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T23:16:39Z", actor: "human:timo", revision: 2}
---
## Question

How does the Laya selector reach its endpoint? `AGENTS.md` § Intake and the command line: model
calls go through llm's crates at tag `0.4.0`, never a hand-written HTTP client; § Boundary: provider
wires belong to llm. llm 0.4.0 has no Laya provider.

## Options

- **A**: the Laya request and response encoding lives in its own Loom crate,
  `crates/loom-selector-laya`, sending through llm's `b10x-llm-http` `HttpClient::post_json`
  (already in `Cargo.lock`); no new third-party crate, and no product crate links it.
- **B**: llm adds a Laya provider first; the story waits on another repository.

## Decided

Option A, 2026-10-09. The encoding lives in `crates/loom-selector-laya` over `b10x-llm-http`. No
product crate (`b10x-loom-cli`, `b10x-loom-sdk`) links it, and no new third-party crate is added.
Moving the wire into llm is a later story if Laya becomes a default selector.
