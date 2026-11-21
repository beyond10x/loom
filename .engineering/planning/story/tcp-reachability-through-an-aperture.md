---
format: aep.planning-md/3
id: story:tcp-reachability-through-an-aperture
kind: story
status: draft
title: net.tcp.reachable observes through a named egress aperture whether the destination accepted, once Substrate reports it
summary: Blocked on Substrate's dial outcome; a refused aperture is a refusal, never false; tested on loopback listeners
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:gate-run-collects-a-gates-document
- depends_on: story:named-egress-per-action
scope:
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/observe.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/gates.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/network.rs
- confidence: cited
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-bindings/tests/gate_run.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_catalog.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_tcp.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/observation.yaml
revision: 7
---
## Outcome

The `observe` executor answers `net.tcp.reachable` (`host`, `port`, `network`) through Substrate:
the observation says whether the destination behind one of the action's named egress apertures
accepted a TCP connection. The destination took it: `true`. It refused it: `false`. Anything else
(a timeout, an unreachable network): `Unavailable`. Substrate refusing the dial through a declared
aperture is a refusal, never `false`. An aperture the engine does not declare is refused at load by
`story:named-egress-per-action` (`ApertureUndeclared`), and one the runner cannot serve is refused
before the step starts by that story's runner (`ApertureUnserved` under `Daemon`, a
`ConfinementError` in process); neither reaches this operation. No aperture has a byte ceiling
(`story:named-egress-per-action` pins `max_bytes: None` and leaves a ceiling out of scope).

Loopback tests use single-aperture actions: `story:named-egress-per-action` admits an IPv4 literal
or `localhost` only as an action's first aperture.

How the arguments meet an aperture, the same for a bound action and for a gate request:
`network` is the name of one aperture the action selects (`story:named-egress-per-action`: a name,
a host, a port, declared once in the engine configuration); `host` and `port` must equal that
aperture's host and port. Nothing in the bindings document or the gates document names a
destination the engine did not declare, and the address the connection reaches is the one the
engine pinned for the aperture.

In a gate run (`story:gate-run-collects-a-gates-document`) this story lifts story 8's refusal of
`net.tcp.reachable` by name and of a project's `networks` member:

- a project's `networks` is accepted when every network it names is an aperture the action selects
  and every host it lists under one is that aperture's host; the addresses it binds are not used;
- every `net.tcp.reachable` request is preflighted before any collection: its `network` names an
  aperture the action selects, its `host` is that aperture's host, and its `port` is that
  aperture's port, within 1 to 65,535. This carries engineering-protocols' TCP host admission
  (`crates/canon-engineering-assertions/src/lib.rs:543-564`,
  `tests/adversary.rs:130` `tcp_host_admission_is_preflighted_before_any_collection`, tag `0.16.0`);
- `observe_operations_cover_the_catalog` resolves `net.tcp.reachable` to `ObserveOperation`
  `TcpReachable`, in both of its lists.

A refusal is recorded by the epic's rule (§ Refusals): at load, `BindingsRefusalReason`; at a step,
`ObserveRun.refusal`.

This story is written as a requirement on the surface
`upstream-blocker:substrate-aperture-dial-outcome` asks for, and is rewritten against that surface
when it clears; it assumes no API of it.

## Why

engineering-protocols connects from the caller's own network to an address its project binds
(`crates/assertion-providers/src/network.rs:11-35`, tag `0.16.0`); a refused connection is `false`
and any other failure unavailable (`:29-33`). Inside Substrate `0.21.0` a child reaches a destination
only through an aperture's loopback listener, which Substrate owns, while a relay outside dials the
destination per connection (`crates/substrate-host/src/egress.rs:15-26`). A child's connect therefore
succeeds whether or not the destination accepts (inferred from that design), and the run record
reports bytes per aperture, not the dial's outcome (wire `lib.rs:1864-1913`, read by a helper).
Without the outcome, `false` and `true` cannot be told apart, and a Loom process connecting from the
host would bypass Substrate.

## Waits on

- `upstream-blocker:substrate-aperture-dial-outcome`.
- `story:named-egress-per-action` (another epic): the apertures an action may name.
- `story:gate-run-collects-a-gates-document` (the edge in the store): this story edits its
  `crates/loom-bindings/src/observe/gates.rs`, `crates/loom-bindings/tests/observe_catalog.rs` and
  `crates/loom-bindings/tests/gate_run.rs`, and shares `ObserveOperation`, `observe.rs`,
  `ess/domains/observation.yaml`, `generated/rust/loom/` and `CHANGELOG.md` with it and every
  earlier story.

Waiting on this one: `story:dns-resolution-through-a-resolver-aperture` (the network module, the
same files).

## ESS first

`ess/domains/bindings.yaml`: `ObserveOperation` gains `TcpReachable` (spelled `net.tcp.reachable`);
`BindingsRefusalReason` gains `ApertureNotNamed`, a load-time reason, so
`crates/loom-bindings/tests/bindings_document.rs` does not compile on the ESS commit either. `ess/domains/observation.yaml`: `ObserveRefusal`
gains `ApertureRefused` and `ApertureNotNamed`; `ObserveRun` gains `aperture` (optional: the
aperture's name and the outcome Substrate reported). On that commit `task drift` fails and
`crates/loom-bindings/tests/observe_tcp.rs` does not compile.

## Evidence record

As `story:observe-executor-reads-workspace-files` § Evidence record. The value depends on the
destination's state at the time, so two runs may differ in `result` as well as `id` and
`observed_at`; `ObserveRun.aperture` names the aperture and the outcome Substrate reported.

## Acceptance

Tests in `crates/loom-bindings/tests/observe_tcp.rs` and, for the gate run,
`crates/loom-bindings/tests/gate_run.rs`, against loopback listeners as the egress cases of
`crates/loom-intake-slice/tests/confined_child.rs` use (`AGENTS.md` § Intake and the command line),
or against a test double of the dial outcome where named; no host outside loopback. "Refused X": `ObserveRun.outcome` `Refused`,
`ObserveRun.refusal` X, the step refused naming it, no evidence record.

- An aperture pinned to a listening loopback port: the record's row for `true`.
- An aperture pinned to a closed loopback port: the row for `false`.
- A dial outcome of `timeout` from the test double of the surface the blocker asks for (no
  loopback construction answers "never" reliably through Substrate's relay): `ObserveRun.outcome`
  `Unavailable`, and the table's `unavailable` row.
- A literal `network`, `host` or `port` that is not one of the action's apertures: refused at
  load, `BindingsRefusalReason` `ApertureNotNamed`.
- The same arriving from a request's arguments: refused `ApertureNotNamed`, and the listener counts
  no connection.
- A dial refused by Substrate through a declared aperture (the test double returns a Substrate
  refusal code): refused `ApertureRefused`, `ObserveRun.code` that code; no record with a `false`
  row.
- `tcp_host_admission_is_preflighted_before_any_collection` (gate run): a gates document whose
  second request is `net.tcp.reachable` with a host that is not its aperture's is refused
  `ApertureNotNamed` before the first request's command starts (its marker file does not exist).
- A gate run with a reachable aperture: the report's gate result is `pass`, and the bundle's
  observation for the request has `value` `true`.
- A project `networks` naming only apertures the action selects, with their hosts: the gate run's
  `ObserveRun.outcome` is `Known` (story 8's `ExtensionNotServed` for `networks` is lifted).
- A project `networks` naming an aperture the action does not select: refused `ApertureNotNamed`,
  naming the network.
- `observe_operations_cover_the_catalog` resolves `net.tcp.reachable` to `TcpReachable` and still
  refuses `net.host.resolves` by name.
- `observe_reads_only_through_the_reader` (story 1) still finds no `TcpStream` under
  `crates/loom-bindings/src/observe*`.
- Every loopback case runs an action with one aperture; the test asserts its action's selection
  has length 1.
- `CHANGELOG.md` `### Added`: `net.tcp.reachable`; `### Changed`: `**Breaking:**` for the new enum
  variants.
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `generated/rust/loom/`,
`crates/loom-bindings/src/observe.rs`, `crates/loom-bindings/src/observe/network.rs` (new),
`crates/loom-bindings/src/decode.rs` (the spelling), `crates/loom-bindings/src/load.rs` (the
arguments and the literal `ApertureNotNamed` check, a load-time reason in the comment of
`ess/domains/bindings.yaml:203-209`), `crates/loom-bindings/tests/bindings_document.rs` (the reason
match, the count, a document reaching `ApertureNotNamed`),
`crates/loom-bindings/src/observe/gates.rs` (lift the refusal, the preflight, `networks`),
`crates/loom-intake-slice/src/confinement.rs` (the Substrate surface, once it exists; provisional),
`crates/loom-bindings/tests/observe_tcp.rs` (new), `crates/loom-bindings/tests/observe_catalog.rs`,
`crates/loom-bindings/tests/gate_run.rs`, `CHANGELOG.md`.

## Out of scope

TLS, HTTP or any protocol above TCP. Address families other than Substrate's (IPv4 pinned
addresses at `0.21.0`). The TCP cases of engineering-protocols' `fixtures/assertions/network.yaml`
and `network-project.yaml`: carried when this story is rewritten against the cleared surface (§ Open
10 of the ledger).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong). The story is held by
`upstream-blocker:substrate-aperture-dial-outcome`; the Substrate-facing surface cannot be placed
until it clears.

- **Primary surface:** `crates/loom-bindings` (observe module, `src/observe/network.rs` new, `src/observe/gates.rs`, `src/observe.rs`) — cited (story `## Files`)
- **Files, placeable now:** `ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `crates/loom-bindings/src/observe.rs`, `crates/loom-bindings/src/observe/network.rs` (new), `crates/loom-bindings/src/observe/gates.rs` — cited (story `## Files`)
- **Files, tests:** `crates/loom-bindings/tests/observe_tcp.rs` (new), `crates/loom-bindings/tests/observe_catalog.rs`, `crates/loom-bindings/tests/gate_run.rs` — cited (story `## Files`)
- **Files, cannot be placed:** the Substrate-facing code. The story names `crates/loom-intake-slice/src/confinement.rs` "once it exists; provisional" — inferred. `confinement.rs` holds the launch path today (`with_egress`, `DriverEgress`, `AppliedAperture` use, `:30-38`, `:220-232`, `:418-440`) but no dial-outcome surface. Only the blocker can say where the outcome arrives (a wire field, an SDK call, or a record member).
- **Symbols:** `ObserveOperation::TcpReachable` (spelled `net.tcp.reachable`), `BindingsRefusalReason::ApertureNotNamed`, `ObserveRefusal::{ApertureRefused, ApertureNotNamed}`, `ObserveRun.aperture` (new, optional) — cited (story `## ESS first`)
- **Symbols reused:** `ObserveRun.{outcome,refusal,code}`, `ObserveRefusal`, `ExtensionNotServed`, `RequestNotCollectable` (these come from `story:observe-executor-reads-workspace-files` and `story:gate-run-collects-a-gates-document`); `MAX_EGRESS_APERTURES` (`crates/loom-intake-slice/src/confinement.rs:38`), `EgressAperture` (`:33`) — cited
- **Symbols that do not exist in the tree yet:** `ObserveOperation`, `ObserveRun`, `ObserveRefusal`, `observe_operations_cover_the_catalog`, `observe_reads_only_through_the_reader`, `BoundAction.egress` (`git grep` finds none). `crates/loom-bindings/src` holds only `decode.rs lib.rs load.rs tree.rs`, and `ess/domains/observation.yaml` is absent. The chain stories create all of them — cited (`git grep`, `ls`)
- **Also likely:** none — inferred. The gate-run story owns the gates edit, and the aperture-name check at load reads `BoundAction.egress` from `story:named-egress-per-action`, which this story consumes and does not own.
- **Documents:** none beyond the house-rule files — cited (story `## Files` names no other document)
- **Confidence:** medium. High for the six `loom-bindings` and ESS files and the three tests, which the story names and the epic's chain confirms. Low for the Substrate-facing file, which does not exist and has no surface to point at.
- **Would collide with:** `story:gate-run-collects-a-gates-document` on `observe/gates.rs`, `observe_catalog.rs`, `tests/gate_run.rs`, `observe.rs`, `ess/domains/{bindings,observation}.yaml` — ordered, direct `depends_on` — cited
- **Would collide with (cont.):** `story:observe-executor-reads-workspace-files`, `story:observe-reads-documents`, `story:observe-compares-generated-trees`, `story:report-formats-checked-before-the-table`, `story:observe-git-changed` on `observe.rs` and the two ESS files — ordered, transitively through `story:gate-run-collects-a-gates-document` — cited (graph `depends_on` closure; epic § Stories)
- **Would collide with (cont.):** `story:dns-resolution-through-a-resolver-aperture` on `observe/network.rs`, `observe/gates.rs`, `observe_catalog.rs`, `tests/gate_run.rs`, the ESS files and `observe.rs`. It `depends_on` this story, so it is ordered — cited
- **Would collide with (cont.):** `story:named-egress-per-action` on `ess/domains/bindings.yaml` and, provisionally, `confinement.rs`. It is in this story's `depends_on` — ordered — cited
- **Would collide with (cont.), not ordered:** the `confinement.rs` entry only, because that file is provisional. The unordered draft stories that name it are `story:ingress-routes-per-action`, `story:substrate-daemon-on-another-host`, `story:agent-session-through-substrate`, `story:writable-host-root`, `story:executable-descendants-held`, `story:tree-daemon`, `story:confinement-level-on-oci-and-pod`, `story:python-test-profile`. None has a `depends_on` path to or from this story — inferred (they name the file in prose, and I did not read their scope entries)
- **Would collide with (cont.), not ordered:** `ess/domains/bindings.yaml` is also named by `story:autonomy-grant-table`, `story:capacity-given-to-an-admission-program`, `story:concurrent-cases-in-the-tree-daemon`, `story:child-case-own-workspace`, `story:run-caps-per-arrival`, `story:tree-usage-and-model-budget`, `story:bindings-child-path-refusal-names-the-bindings-directory`, `story:agent-session-through-substrate`. None has a `depends_on` path to or from this story. `generated/rust/loom/` and `CHANGELOG.md` collide with everything — cited (graph; `grep` of the store)
- **Contradicted by the code:** nothing the story cites about the tree. The story's claims about the tree at `39f4da88` (no `TcpStream` under `crates/loom-bindings`, `confinement.rs:38` re-exporting `MAX_EGRESS_APERTURES`, loopback `TcpListener` in `confined_child.rs:22,443-449`) hold. Defects 1 and 2 below are about what Substrate and the dependencies can supply — cited
- **Safety fact:** the host never dials. The story's own acceptance, that `observe_reads_only_through_the_reader` still finds no `TcpStream` under `crates/loom-bindings/src/observe*`, is what keeps Loom from bypassing Substrate. Level 1 (stated), unproven — the test does not exist yet (`git grep` finds no `observe_reads_only_through_the_reader`)
