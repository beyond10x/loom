---
format: aep.planning-md/3
id: story:named-egress-per-action
kind: story
status: draft
title: A process has no network unless its action selects named egress apertures the engine declares
summary: Host and port declared once in engine configuration, selected by name in the binding; ingress refused until Substrate serves it
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-run-command
revision: 1
---
## Outcome

A process a tree run starts has no network unless its action selects egress, and then only to
the destinations selected. The engine configuration declares each allowed destination once, as a
named aperture (host, port, TCP, an optional byte ceiling); a binding selects apertures by name
for its action and never states a host or a port. A name the engine does not declare refuses the
bindings file at load. Loom hands the selected names to Substrate, which gives the process a
loopback listener per aperture and no other route. Ingress is declared the same way and is
refused at load, naming the action, until Substrate serves it.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "egress and ingress allow-lists
(host, port, direction)". Substrate 0.21.0: apertures are operator-declared and referenced by
name (`Network::Egress { apertures }`, `crates/substrate-host/src/confinement/profile.rs:100-105`),
a destination in a request is refused (`exec.aperture-destination-in-request`), at most 6 per
deployment and 4 per run, enforced on `linux` only (`crates/substrate-host/src/egress.rs:1-13`);
`ingress_routes` is a reserved field and any value is refused (`profile.rs:93-96`). Loom already
maps a `host:port` of `ChildCommand::with_egress` to an aperture name
(`story:child-launches-through-confinement-adapters`, "Waits on"). The SDC's tools need this
list before they can leave the `local` adapter: `ess` and `aep` need none, `slot` needs its
daemon socket (session `sdc`, 2026-11-21).

## ESS first

`ess/domains/tree.yaml`: `EngineApertures` (name, host, port, optional byte ceiling).
`ess/domains/bindings.yaml`: `BoundAction` gains optional `egress` (aperture names) and `ingress`
(names), and `BindingsRefusal` gains `ApertureUndeclared` and `IngressUnserved`. On that commit
`task drift` fails and `crates/loom-bindings/tests/action_egress.rs` does not compile.

## Acceptance

Under the `linux` adapter, with a local listener as the destination and no network call beyond
the host:

- A program whose action selects no egress fails to connect to the listener.
- A program whose action selects the listener's aperture connects through it, and fails to
  connect to a second local listener that is declared but not selected.
- A binding that selects an undeclared name is refused at load `ApertureUndeclared`; one that
  states a `host:port` instead of a name is refused by the decoder.
- A binding that selects more than 4 apertures is refused at load naming Substrate's bound.
- A binding with `ingress` is refused at load `IngressUnserved`.
- Under `local`, a run whose bindings select or omit egress is refused unless the engine
  configuration declares the network advisory, and then each step's record says the network was
  not held.
- A transfer past an aperture's byte ceiling ends the step, and the applied record names the
  aperture and the bytes.
- `task check` exits 0.

## Out of scope

Ingress (`upstream-blocker:substrate-ingress`). UDP. Egress on `oci` and `pod`, which serve no
network family. A Unix socket as an aperture (the SDC's `slot`).
