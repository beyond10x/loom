---
format: aep.planning-md/3
id: story:system-query
kind: story
status: active
title: Compose protocol sources and execute verified system time queries
summary: Implement the user-approved protocol catalog, pinned installation and read-only clock query plan.
relations:
- informed_by: story:hosted-governor
- serves: vision:O1
scope:
- confidence: cited
  path: crates
- confidence: cited
  path: ess/intake
- confidence: cited
  path: generated
- confidence: inferred
  path: protocols
- confidence: cited
  path: website
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T10:46:01Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-07T10:46:01Z", actor: "human:timo", revision: 3}
---
## Outcome and authorization

The operator approved the revised plan and explicitly requested implementation. Loom owns
system-query@1; engineering-protocols remains a definition provider, not the owner of this query.
The same run command answers current time without a workspace, Git or Substrate and keeps the
existing software-change path. Custom Canon definitions are composed from bundles, memory,
local snapshots and selected regular Git blobs pinned to full commits. Installs are explicit;
runs verify digests and load offline. Code and authority are supplied only by the trusted host.

## Design

One immutable catalog supplies routing, declared artifacts and governor admission. Duplicate
identities, mismatched major revisions and shadowing built-ins refuse before calls. User XDG
data stores exact YAML, SHA-256 and provenance atomically; replace is explicit. Git acquisition
has no checkout, package execution, filters or submodules. Time is one clock sample converted
to local numeric-offset time and UTC, verified against case/intent and displayed directly.
Only this verified evidence permits answered; model text and other cases cannot complete it.
Custom clock protocols reuse host bindings; unsupported requirements name NoLocalExecutor.

## ESS first

Commit 9346427 declares intake.protocols and intake.query. task intake-drift fails on this
commit with missing protocols.rs/query.rs and digest drift; its output is retained in
.engineering/drafts/spec-red.log. Generate before implementation. Canon protocol YAML and
recorded-model regression expectations precede the implementation that satisfies them.

## Acceptance

- Built-in, local-installed and pinned-Git custom clock cases finish with verified local/UTC time.
- No query workspace access, Git invocation, subprocess clock or confinement initialization.
- Invalid/duplicate/corrupt source, unbound action, forged/cross-case observation and write picks refuse.
- Existing SliceRequest/run/run_with_options callers and software approval/refusal behavior survive.
- Both context modes give the same fixed-clock result, bounded requests fit 64 KiB and metrics
  retain unknown provider counters without source payloads.
- task check, task plan, website build and a live exact-intent smoke qualify the candidate.

## Scope

ESS intake and generated intake models; new protocol catalog/install crate and Loom protocol YAML;
governor, intake router/slice, CLI and SDK extension points; corresponding tests and user docs.
One story because catalog registration, case initialization and clock execution share contracts.
Read-only architecture review and independent adversarial review accompany implementation.
No Jira ingestion, arbitrary tool plugins, named timezone conversion or general assistant.
