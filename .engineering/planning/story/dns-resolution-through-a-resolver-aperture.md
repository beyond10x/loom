---
format: aep.planning-md/3
id: story:dns-resolution-through-a-resolver-aperture
kind: story
status: draft
title: net.host.resolves resolves at a declared resolver through Substrate, once Substrate serves one
summary: Blocked on a Substrate resolver surface; NXDOMAIN false with TTL 0; timeout unavailable; tested on a loopback responder
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:tcp-reachability-through-an-aperture
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
  path: crates/loom-bindings/tests/gate_run.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_catalog.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_dns.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/observation.yaml
revision: 7
---
## Outcome

The `observe` executor answers `net.host.resolves` (`host`, `resolver`) through Substrate: whether
`host` resolves at a resolver the engine declares, with the value engineering-protocols gives,
`{resolves, ttl_seconds, resolver, addresses}` (`crates/assertion-providers/src/network.rs:106-108`,
tag `0.16.0`): the addresses (A and AAAA), the resolver, and the minimum TTL of the answer. NXDOMAIN
is `resolves: false` with TTL 0. A timeout, a server error, a referral, a truncated reply that needs
TCP, an unresolved CNAME chain, or an NXDOMAIN from a server that is neither recursive nor
authoritative is `Unavailable`. A positive answer for one type contradicted by NXDOMAIN for the
other is refused `AnswerContradicted`, as upstream errors (`network.rs:100-105`); a malformed reply
(a wrong transaction id, a bad header, a question that does not match, trailing bytes, a truncated
record) is refused `AnswerMalformed`, as upstream errors. Substrate refusing the resolver is a
refusal, never `false`. A host is checked as upstream checks it (`network.rs:39-48`): one trailing
`.` is trimmed and the name lowercased first, then an empty name, one over 253 bytes, an empty
label, a label over 63 bytes or with a byte outside letters, digits and `-`, or starting or ending
with `-`, is refused `HostNameInvalid`, `ObserveRun.reason` naming the host.

How the arguments meet a resolver, the same for a bound action and for a gate request: `resolver`
must be the name of a resolver the engine declares for the action, or exactly its `IP:port`; the
address queried is the engine's, never one only the document names. No story declares a resolver
today: the engine's resolver declaration (a name and an `IP:port`, beside the apertures of
`story:named-egress-per-action`) is part of the surface
`upstream-blocker:substrate-dns-resolver-aperture` asks for, and is written with this story's
rewrite when it clears, with its ESS line and its load-time `BindingsRefusalReason`. Until then no
literal `resolver` is refused at load; the step refuses an undeclared one `ResolverRefused`.

In a gate run (`story:gate-run-collects-a-gates-document`) this story:

- lifts story 8's refusal of `net.host.resolves` by name, and preflights every such request before
  any collection: a `resolver` the action does not declare refuses the run `ResolverRefused`, a
  `host` that is not a DNS name `HostNameInvalid`;
- sets the observation's `valid_until` to `observed_at` plus the lesser of `freshness_seconds` and
  the answer's TTL, as engineering-protocols does
  (`crates/canon-engineering-assertions/src/lib.rs:648-658`);
- makes `observe_operations_cover_the_catalog` resolve `net.host.resolves` (behind the observation
  function `net.dns.lookup` and the recipe `net.host.resolves`) to `ObserveOperation`
  `HostResolves`, in both of its lists.

A refusal is recorded by the epic's rule (§ Refusals): at a step, `ObserveRun.refusal`; this story
adds no load-time reason (above).

This story is written as a requirement on the surface
`upstream-blocker:substrate-dns-resolver-aperture` asks for, and is rewritten against it when it
clears; it assumes no API of it.

## Why

engineering-protocols sends its own DNS queries over UDP to an explicit `IP:port` resolver from the
caller's network (`crates/assertion-providers/src/network.rs:36-119`, tag `0.16.0`;
`crates/assertion-providers/README.md:51-54`), and `canon-engineering-assertions` caps the observation's lifetime by the TTL
(`crates/canon-engineering-assertions/src/lib.rs:651-658`). Substrate `0.21.0` gives a confined run
no resolver and no UDP: "The sandbox never resolves anything: it gets no resolver at all"
(`crates/substrate-host/src/aperture.rs:9`), and egress serves "no UDP, no ICMP, no raw sockets"
(`docs/design/10-destination-bound-egress.md:58-59`). The resolver address would be engine
configuration, as an aperture is (`AGENTS.md` § Bindings document).

## Waits on

- `upstream-blocker:substrate-dns-resolver-aperture`.
- `story:tcp-reachability-through-an-aperture`: shared `ObserveOperation`, `observe.rs`, the network
  module, `crates/loom-bindings/src/observe/gates.rs`, `crates/loom-bindings/tests/observe_catalog.rs`,
  `crates/loom-bindings/tests/gate_run.rs`, the ESS files, `generated/rust/loom/` and
  `CHANGELOG.md`; through it, `story:gate-run-collects-a-gates-document`.

## ESS first

`ess/domains/bindings.yaml`: `ObserveOperation` gains `HostResolves` (spelled `net.host.resolves`).
`ess/domains/observation.yaml`: `ObserveRefusal` gains `ResolverRefused`, `HostNameInvalid`,
`AnswerContradicted` and `AnswerMalformed` (`decode.rs` maps the spelling `net.host.resolves`;
`load.rs` checks the arguments `host` and `resolver`). On that commit `task drift` fails and
`crates/loom-bindings/tests/observe_dns.rs` does not compile.

## Evidence record

As `story:observe-executor-reads-workspace-files` § Evidence record. DNS answers change, so two runs
may differ in `result`; the report's value names the resolver, the addresses and the TTL.

## Acceptance

Tests in `crates/loom-bindings/tests/observe_dns.rs` and, for the gate run,
`crates/loom-bindings/tests/gate_run.rs`, against a loopback DNS responder the test runs (as the
egress cases use loopback listeners, `AGENTS.md` § Intake and the command line); no resolver outside loopback.
"Refused X": `ObserveRun.outcome` `Refused`, `ObserveRun.refusal` X, the step refused naming it, no
evidence record.

- An A and an AAAA answer: the report's `value` has `resolves: true`, both addresses, and the
  smaller TTL as `ttl_seconds`.
- NXDOMAIN: `value` has `resolves: false` and `ttl_seconds` 0.
- A positive A answer followed by NXDOMAIN for AAAA: refused `AnswerContradicted`.
- A timeout, SERVFAIL, a referral and a truncated reply: each `ObserveRun.outcome` `Unavailable`
  and the table's `unavailable` row (engineering-protocols'
  `dns_retains_ttl_and_definitive_negative_but_timeout_is_unknown`).
- A CNAME answer whose target has no address in the reply: `Unavailable`.
- An NXDOMAIN with neither the recursion-available nor the authoritative bit: `Unavailable`.
- A reply with another transaction id, and a reply with trailing bytes: each refused
  `AnswerMalformed`.
- A host that is not an ASCII DNS name (empty, over 253 bytes, `a..b`, a label over 63 or with a
  byte outside letters, digits and `-`, or starting or ending with `-`): refused `HostNameInvalid`,
  `reason` naming the host, and the responder counts no query.
- `Example.ORG.`: the responder receives a query for `example.org`, and the value is that name's.
- A resolver named by its exact `IP:port` as the engine declares it: the same value as by its name.
- A resolver the engine does not declare, or one Substrate refuses: refused `ResolverRefused`; no
  record with a `false` row.
- Gate run, TTL 30 and `freshness_seconds` 60: the bundle observation's `valid_until` is its
  `observed_at` plus 30 s.
- Gate run, TTL 120 and `freshness_seconds` 60: `valid_until` is `observed_at` plus 60 s.
- Gate run, NXDOMAIN (TTL 0): `valid_until` equals `observed_at`.
- Gate run, a `net.host.resolves` request naming an undeclared resolver as its second request:
  refused `ResolverRefused` before the first request's command starts (its marker file does not
  exist).
- `observe_operations_cover_the_catalog` resolves `net.host.resolves` to `HostResolves` and refuses
  no operation by name any more.
- `observe_reads_only_through_the_reader` (story 1) still finds no `UdpSocket` under
  `crates/loom-bindings/src/observe*`.
- `CHANGELOG.md` `### Added`: `net.host.resolves`; `### Changed`: `**Breaking:**` for the new enum
  variants.
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `generated/rust/loom/`,
`crates/loom-bindings/src/observe.rs`, `crates/loom-bindings/src/observe/network.rs`,
`crates/loom-bindings/src/decode.rs` (the spelling), `crates/loom-bindings/src/load.rs` (the
arguments),
`crates/loom-bindings/src/observe/gates.rs` (lift the refusal, the preflight, the TTL lifetime),
`crates/loom-bindings/tests/observe_dns.rs` (new), `crates/loom-bindings/tests/observe_catalog.rs`,
`crates/loom-bindings/tests/gate_run.rs`, `CHANGELOG.md`. The Substrate-facing file is unknown until
the blocker clears.

## Out of scope

DNS over TCP, DoT or DoH. Record types other than A and AAAA. The DNS cases of
engineering-protocols' `fixtures/assertions/network.yaml` and `network-project.yaml`: carried when
this story is rewritten against the cleared surface (§ Open 10 of the ledger).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (the `observe` module, the network operation) with its two ESS domain files; placeable now, but none of it exists in the tree yet (story 1 creates `observe.rs` and `observation.yaml`, story 9 creates `observe/network.rs`) — cited (story `## Files`, `## ESS first`)
- **Files:** `ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `crates/loom-bindings/src/observe.rs`, `crates/loom-bindings/src/observe/network.rs`, `crates/loom-bindings/src/observe/gates.rs` — cited (story `## Files`; `ls crates/loom-bindings/src` shows only decode.rs, lib.rs, load.rs, tree.rs)
- **Tests:** `crates/loom-bindings/tests/observe_dns.rs` (new), `crates/loom-bindings/tests/observe_catalog.rs`, `crates/loom-bindings/tests/gate_run.rs` — cited (story `## Files`, `## Acceptance`)
- **Symbols:** `ObserveOperation::HostResolves` (spelled `net.host.resolves`), `ObserveRefusal::{ResolverRefused, HostNameInvalid, AnswerContradicted}` — cited (story `## ESS first`; neither enum exists in the tree, `git grep` finds no `ObserveOperation`)
- **Symbols (reused, owned by earlier stories):** `ObserveRun.outcome` / `.refusal`, `ObserveOutcome::{Known, Unavailable, Refused}` — cited (`story:observe-executor-reads-workspace-files` § ESS, `:121-125`)
- **Symbols (upstream, tag 0.16.0):** `network::dns` at `crates/assertion-providers/src/network.rs:36-119` in engineering-protocols; TTL cap at `crates/canon-engineering-assertions/src/lib.rs:648-658` — cited (verified: `git show 0.16.0:<path>` in `~/beyond10x/engineering-protocols`)
- **Also likely:** `crates/loom-bindings/src/decode.rs` — inferred (story 1 says `decode.rs` maps the document spelling of each operation, `observe-executor-reads-workspace-files.md:118`; this story's `## Files` does not list it)
- **Documents:** none beyond the house-rule files (`CHANGELOG.md`, `generated/rust/loom/`) — cited (story `## Files` names no guide page)
- **Substrate-facing file:** unknown until `upstream-blocker:substrate-dns-resolver-aperture` clears; `crates/loom-intake-slice/src/confinement.rs` is where story 9 provisionally puts its Substrate surface, so this one may land there, but I leave it out of `paths` — inferred
- **Placeable now:** the two ESS enum additions, `ObserveOperation::HostResolves`, the catalog test's two lists, the `HostNameInvalid` preflight (a pure host-name check, ported from upstream `network.rs:39-48`), and the `valid_until` = `observed_at` + min(`freshness_seconds`, TTL) rule in `gates.rs` — inferred (none needs the Substrate surface; the story itself assumes no API of it)
- **Not placeable until the blocker clears:** the resolver query, `ResolverRefused` from Substrate, the Unavailable cases (timeout, SERVFAIL, referral, truncated) and the loopback-responder cases — cited (story lines 46-48, `## Files` last sentence; blocker `## Waits on`: Substrate gives a confined run no resolver and no UDP at 0.21.0)
- **Not placeable at all yet:** "a resolver the engine declares for the action" — no story or engine configuration declares a resolver (`named-egress-per-action` declares apertures only), so `ResolverRefused` on an undeclared resolver cannot be fixtured — inferred (grep of the store for `resolver` finds only this story, the blocker, TCP and the epic)
- **Confidence:** medium — the file list is the story's own and matches the chain's other stories; the tree has none of the files yet, and the Substrate-facing file and the resolver declaration are unknown
- **Would collide with, ordered by `depends_on` (from `aep plan artifact graph --format json`, closure of this story):** `story:tcp-reachability-through-an-aperture` (direct edge; creates `observe/network.rs`, edits `gates.rs`, `observe_catalog.rs`, `gate_run.rs`, `bindings.yaml`, `observation.yaml`, `observe.rs`), `story:gate-run-collects-a-gates-document` (creates `gates.rs`, `gate_run.rs`, `observe_catalog.rs`; ordered through TCP), `story:observe-executor-reads-workspace-files`, `story:observe-reads-documents`, `story:observe-compares-generated-trees`, `story:report-formats-checked-before-the-table`, `story:observe-git-changed`, `story:gate-program-identity-bound-to-spawn` (all edit `bindings.yaml`, `observe.rs`, `observation.yaml`; all in the closure) — cited (each story's `## Files`; epic `observation-collection-as-executors.md:157-166` says the ten stories form one chain)
- **Would collide with, not ordered by any `depends_on` path:** `story:capacity-given-to-an-admission-program`, `story:child-case-own-workspace`, `story:run-caps-per-arrival`, `story:tree-usage-and-model-budget`, `story:concurrent-cases-in-the-tree-daemon` — each lists `ess/domains/bindings.yaml` under `## Files` (the first three also `load.rs`, which this story does not touch); different enums, so a textual merge only — cited. `story:agent-session-through-substrate` (body extends `AgentRun` in `bindings.yaml`, `:69-75`) and `story:bindings-child-path-refusal-names-the-bindings-directory` (acceptance bullet on variants in `bindings.yaml`, `:42`) are the same, with no edge — inferred
- **Story vs code:** see "Story defects" below; the cited upstream and Substrate lines otherwise say what the story says — cited
- **Safety fact:** adding `HostResolves` changes no existing caller, because story 1 declares the enum and its only match sites are the dispatch in `observe.rs` and the catalog test; story 1's `observe_reads_only_through_the_reader` already forbids `UdpSocket` under `src/observe*` (`observe-executor-reads-workspace-files.md:217`) — step 2, unproven
