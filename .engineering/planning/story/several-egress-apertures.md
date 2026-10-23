---
format: aep.planning-md/3
id: story:several-egress-apertures
kind: story
status: active
title: A confined child reaches every egress aperture the host names, and nothing else
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:substrate-0-8-pin
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T03:54:53Z", actor: "human:timo", revision: 4, correlation: "wave-2026-10-23-w2"}
- {from: "proposed", to: "active", at: "2026-10-23T03:54:53Z", actor: "human:timo", revision: 5, correlation: "wave-2026-10-23-w2"}
---
## Outcome

`ChildCommand::with_egress` called more than once gives the confined child every named aperture and
nothing else; the applied confinement names each one.

## Why

Substrate gives a run at most one aperture (AGENTS.md § Intake and the command line), so a child
that needs `api.github.com` and `github.com` (the bot route) cannot be confined. takt's forge step
is blocked on it (design § 5 gap 2).

## Acceptance

- `crates/loom-intake-slice/tests/confined_child.rs`
  (`a_child_reaches_each_named_aperture_and_nothing_else`): two loopback listeners as two
  apertures; the child reaches both and a third listener is refused.
- `AppliedConfinement.network` lists every aperture; a record naming another set is refused.
- A driver shared by children is keyed by the whole aperture set.
- More apertures than the pinned Substrate allows (`MAX_EGRESS_APERTURES`, 4 in Substrate 0.10.0)
  is refused `ScopeInvalid` before launch, naming the count.
- The second to fourth aperture names its host by name: Substrate 0.10.0 listens the n-th
  aperture on `127.0.0.n` and refuses `localhost` past the first
  (`exec.aperture-host-listed-twice`), so an IPv4 literal host, or `localhost`, on any but the
  first is refused `ScopeInvalid` before launch, naming the host; two apertures naming one host
  (ASCII case-insensitive, without a trailing `.`) are refused the same way. The first keeps
  today's rule (`127.0.0.1` the only IPv4 literal).

## ESS first

`intake.confinement` declares the applied network as a list of apertures; `task intake-drift` is
red on the declaration before the code moves.

## Not in scope

Several certificate bundles; a per-aperture bundle.

## Scope

Crates: `loom-intake-slice`, `ess/intake/`. `crates/loom-intake-slice/src/confinement.rs`
(`with_egress`, `DriverEgress`, `check_record`), `confinement/child.rs`, AGENTS.md, CHANGELOG.md.

## Ready

Ready since Substrate 0.10.0 (`105cf718`, linked by Loom 0.36.0): a sandbox may name `apertures`,
2 to `MAX_EGRESS_APERTURES` (4), the applied network an array carrying each `listen` address
(Substrate CHANGELOG 0.9.0 § Added). Accepted in wave 2026-10-23-w2.
