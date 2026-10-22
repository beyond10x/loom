---
format: aep.planning-md/3
id: story:several-egress-apertures
kind: story
status: draft
title: A confined child reaches every egress aperture the host names, and nothing else
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:substrate-0-8-pin
revision: 1
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
- More apertures than the pinned Substrate allows is refused `ScopeInvalid` before launch, naming
  the count.

## ESS first

`intake.confinement` declares the applied network as a list of apertures; `task intake-drift` is
red on the declaration before the code moves.

## Not in scope

Several certificate bundles; a per-aperture bundle.

## Scope

Crates: `loom-intake-slice`, `ess/intake/`. `crates/loom-intake-slice/src/confinement.rs`
(`with_egress`, `DriverEgress`, `check_record`), `confinement/child.rs`, AGENTS.md, CHANGELOG.md.

## Ready

Waits on `story:substrate-0-8-pin` and a Substrate release with several apertures per run.
