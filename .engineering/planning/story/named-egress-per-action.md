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
- depends_on: story:executable-allow-list
- depends_on: story:mixed-egress-launches
revision: 3
---
## Outcome

A process a tree run starts has no network unless its action selects egress, and then only to the
destinations selected. The engine configuration's `apertures` section declares each allowed
destination once, as a named aperture (a name of Substrate's grammar `[a-z][a-z0-9_]{0,63}`, a
host, a port, TCP); a binding selects apertures by name for its action and never states a host or a
port.

At load (`tree open` and `tree run`), in `crates/loom-tree/src/engine.rs`, each action's selection
is refused, naming the action:

- a selected value outside the name grammar, such as a `host:port`, `ApertureNameInvalid` naming the
  value, before any lookup;
- a name the engine does not declare, `ApertureUndeclared`;
- a selection `ApertureSetInvalid` naming the rule and the names, for each of the rules Loom's
  launch applies today (`crates/loom-intake-slice/src/confinement.rs:1236-1239`, `:1820-1850`), which
  this story moves to load time: two apertures naming one host, compared without case and without a
  trailing `.`; a second or later aperture whose host is an IPv4 literal or `localhost`; more than 4
  apertures (`MAX_EGRESS_APERTURES`); a name twice.

In process, Loom resolves each declared aperture's host once, when the engine configuration is
loaded for `tree run` or `tree serve` (`resolve_apertures` in `crates/loom-tree/src/engine.rs`), to
the pinned socket address of Substrate's `EgressAperture` (name, host, port, `pinned`,
`max_bytes: None`), the first address the host resolves to; a host that resolves to none is refused
`ApertureUnresolved`, naming the aperture and the host, before the first step. The runner hands the
selected apertures to Substrate with `ChildCommand::with_egress` per aperture, and Substrate gives
the process a loopback listener per aperture and no other route. Under `Daemon`, nothing is
resolved: the runner sends the names only, and an aperture the daemon does not publish with that
destination refuses the step before it starts `ApertureUnserved`, naming the aperture
(`story:substrate-daemon-runner`).

Whether a run's network is held is read through the mapping of
`story:confinement-level-required` as the fact `network_held`: the snapshot's `exec.no-egress` in
process, `Machine.exec_no_egress` under `Daemon`, never on `local`. A confinement that does not
prove it refuses to start a run `NetworkUnheld`, unless the engine configuration declares the
network advisory, and then each step's confinement record says `network_held: false`. The family
`network_egress`, which says whether apertures can be forwarded, is not asked. A binding may name
`ingress`, and it is refused at load, naming the action, until `story:ingress-routes-per-action`.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "egress and ingress allow-lists
(host, port, direction)". Substrate 0.21.0: apertures are operator-declared and referenced by
name (`Network::Egress { apertures }`, `crates/substrate-host/src/confinement/profile.rs:100-105`),
a destination in a request is refused (`exec.aperture-destination-in-request`), at most 4 per run
(`MAX_EGRESS_APERTURES`, `crates/substrate-wire/src/lib.rs:381`) and 6 per daemon deployment
(`MAX_DEPLOYMENT_EGRESS_APERTURES`, `crates/substrate-daemon/src/runtime.rs:1090`), enforced on
`linux` only (`crates/substrate-host/src/egress.rs:1-13`); a declared aperture is "resolved once at
declaration" and pins what it resolved to (`EgressAperture`, `crates/substrate-host/src/aperture.rs`);
`ingress_routes` is a reserved field and any value is refused (`profile.rs:93-96`). A daemon's
apertures are its own configuration, published as `exec_egress_apertures` (wire `lib.rs:4164`), and
a start names them with `CommandBuilder::apertures` (`crates/b10x-substrate-sdk/src/lib.rs:1813`)
or `PipeSessionBuilder::apertures` (`:1311`). Loom's `ChildCommand::with_egress`
(`crates/loom-intake-slice/src/confinement.rs:1244`) takes a whole `EgressAperture` from its
caller and maps nothing; the engine declares name, host and port only, hence the resolution step.

The family `network_egress` is served only when an aperture is declared and proved
(`egress_apertures_fact` returns none for an empty declaration, `egress.rs:117`; `linux.rs:192-195`),
so a managed daemon that declares none, and an in-process driver opened for an action with no egress
(`DriverEgress::NONE`, `confinement.rs:164`), serve no `network_egress` while their network is held.
The held network is the capability fact `exec.no-egress` (`CapabilityFacts.exec_no_egress`, wire
`lib.rs:3933`; set by the host probe, `crates/substrate-host/src/probe.rs:135`; `Machine.exec_no_egress`,
SDK `model.rs:30`), and a run's applied record names its network mode (`AppliedNetwork::None`,
`Aperture` or `Apertures`, wire `lib.rs:1815-1819`). The SDC's tools need this list before they can
leave the `local` adapter: `ess` and `aep` need none, `slot` needs its daemon socket (session `sdc`,
2026-11-21).

## Waits on

- `story:executable-allow-list`: the story before this one in the chain; both edit
  `ess/domains/engine.yaml` (`EngineRefusal`, `StepConfinement`), `crates/loom-tree` and the launch
  path in `crates/loom-intake-slice/src/confinement.rs`. Through the chain,
  `story:confinement-level-required` gives the mapping this story reads for `network_held`, and
  `story:substrate-daemon-runner` the daemon path and `ApertureUnserved`'s check.
- `story:tree-run-command`: `tree open` and `tree run`, where the load-time refusals happen.

## ESS first

`ess/domains/engine.yaml`: `AperturesSection` (name, host, port; `advisory`), `EngineConfiguration`
gains optional `apertures`, `EngineRefusal` gains `ApertureNameInvalid`, `ApertureUndeclared`,
`ApertureSetInvalid` (action, rule, names), `ApertureUnresolved`, `ApertureUnserved`,
`NetworkUnheld` and `IngressUnserved`, and `StepConfinement` gains `egress` (the names) and
`network_held`. `ess/domains/bindings.yaml`: `BoundAction` gains optional `egress` (aperture names)
and `ingress` (names). On that commit `task drift` fails and `crates/loom-tree/tests/action_egress.rs`
does not compile.

## Acceptance

Under the in-process `linux` adapter, with a local listener as the destination and no network call
beyond the host:

- A program whose action selects no egress fails to connect to the listener, and its step's record
  says `network_held: true`.
- A program whose action selects the listener's aperture connects through it, and fails to
  connect to a second local listener that is declared but not selected.
- A binding that selects an undeclared name is refused at `tree open` `ApertureUndeclared`; one
  that states `127.0.0.1:8080` instead of a name is refused `ApertureNameInvalid` naming the value.
- A binding that selects five apertures, one that selects two apertures of one host (`a.example`
  and `A.example.`), one whose second aperture's host is `127.0.0.1`, and one that names an
  aperture twice are each refused at `tree open` `ApertureSetInvalid` naming the rule.
- An aperture whose host does not resolve is refused `ApertureUnresolved` before the first step
  of `tree run`.
- A binding with `ingress` is refused at load `IngressUnserved`.
- Under `local`, which does not prove `network_held`, a run is refused before its first step
  `NetworkUnheld` unless the engine configuration declares the network advisory, and then each
  step's confinement record says `network_held: false`.
- Each step's confinement record names the apertures its action selected.

Under `Daemon` with a managed daemon, which declares no aperture and serves no `network_egress`:

- A program whose action selects no egress fails to connect to a listener on the host, and its
  step's record says `network_held: true`.
- A step whose action selects an aperture is refused before its program starts `ApertureUnserved`,
  naming the aperture.

- `task check` exits 0.

## Out of scope

Serving ingress (`story:ingress-routes-per-action`, held by `upstream-blocker:substrate-ingress`).
A byte ceiling per aperture. UDP. Egress on `oci` and `pod`, which serve no network family. A Unix
socket as an aperture (the SDC's `slot`). Resolving a host again while a run goes on.
