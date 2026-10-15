---
format: aep.planning-md/3
id: story:governor-independence-declarations
kind: story
status: draft
title: A host declares each producer's independence dimensions and the governor's case snapshot carries them
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A host that opens a case on `CanonGovernor` declares, for each evidence producer, the independence
dimensions it satisfies, and the governor puts those declarations into the `canon-case/1` snapshot
it evaluates. A claim whose evidence match demands independence (`independent_on: [...]`) is then
TRUE or FALSE on records from a declared-independent producer, instead of always UNKNOWN, so an
action whose precondition needs such a claim can become admissible through the governor.

## Acceptance

`a_record_from_a_declared_independent_producer_satisfies_an_independent_on_claim`
(`crates/loom-governor/tests/`): a protocol declaring `independence_dimensions: [different_agent_run]`
and an action whose precondition needs a claim with `independent_on: [different_agent_run]`; the
same passing record makes the action admissible when the host declared its producer on that
dimension, and leaves it blocked (claim UNKNOWN) when it did not.

## Why (Loom 0.22.0)

- `CanonGovernor` builds the snapshot with `independence: Declarations::default()`
  (`crates/loom-governor/src/lib.rs:1165-1167`), so every claim with `independent_on` stays UNKNOWN
  and its action is never admissible. A protocol that says "the run that wrote the candidate
  cannot vouch for it" cannot run on the governor.
- The stateless `loom_governor::evaluate` (`crates/loom-governor/src/lib.rs:1406`) takes a host's own
  snapshot, so a host can supply the declarations there, but only by re-implementing `Governor`,
  `ObservationPort` and `EvidencePort` itself.
- Canon already evaluates the declarations at the version Loom links: Canon's
  `story:evidence-independence` (implemented) puts the dimension ids in the protocol
  (`independence_dimensions`) and the producer -> dimensions map in the case, and leaves resolving
  identities to the trusted runtime context. Loom is that context here; no change to Canon.

## Decisions followed

No accepted ADR in this store or in Canon's store names independence. Canon's
`story:evidence-independence` (citing Atlas ADR 0084) settles the shape: the requirement is declared
on the protocol, the declarations come from trusted context, never from a record's own claim. This
story follows it: the declarations come from the host, through the governor, never from evidence
or a model.

## Sketch

`CanonGovernor::with_independence(producer, &[dimension])`, or an `independence: Declarations`
argument where a case is opened; a dimension the protocol does not declare is refused naming it.
The declarations are part of the held case, so they survive `FileCaseStore` like the inputs.

## Workaround it removes

The host implements `Governor` + `ObservationPort` + `EvidencePort` over `loom_governor::evaluate`
with its own `canon-case/1` snapshot carrying the declarations.

## Scope (inferred)

`crates/loom-governor/src/lib.rs` (case snapshot, `FileCaseStore` record),
`crates/loom-governor/tests/`, `crates/loom-sdk` (re-export), `ess/domains/` if the held case is
declared there, `CHANGELOG.md`.

## Not in scope

Changing Canon's independence semantics; a model or an evidence record supplying a declaration.
