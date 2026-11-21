---
format: aep.planning-md/3
id: story:substrate-0-21-pin
kind: story
status: implemented
title: Loom builds on Substrate 0.21.0 with no behaviour change
summary: 'From 0.14.0: adapter stdin and live output become available to later stories'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/confinement/security_w20261023w1_u3.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/confinement/several_apertures.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/confinement/workdir.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/host_git_hardening.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/security_w20261121w1_u1.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/security_w20261121w1_u1_pass2.rs
- confidence: cited
  path: crates/loom-operator/src/main.rs
- confidence: cited
  path: crates/loom-operator/tests/startup.rs
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:44Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-11-21T00:12:43Z", actor: "human:timo", revision: 7, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w1"}
- {from: "active", to: "implemented", at: "2026-11-21T01:32:44Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
## Outcome

Loom builds on Substrate 0.21.0 (`b10x-substrate-host` and `b10x-substrate-wire`, tag `0.21.0`,
`adfde402`). Confined test runs and confined children behave as on 0.14.0, with one visible
difference: the applied confinement record lists every fact the adapter proved, and on 0.21.0 the
`linux` adapter proves two more, `stdin_pipe` and `output_live`, so a confined launch records
five facts where it recorded three.

## Why

Loom pins Substrate 0.14.0 (`crates/loom-intake-slice/Cargo.toml:31-32`, rev `0b9d50c6`).
Substrate 0.15.0 added standard input and live output to the confinement adapters (facts
`stdin_pipe` and `output_live`), which `story:child-launches-through-confinement-adapters` waits
on, and `story:confinement-opens-from-engine-config` needs the adapters as released.

## ESS first

Exempt from an `ess/` change: no declaration of Loom's specification changes (the applied
record's facts are a list of names already). The red step is the build: on the new pin
`crates/loom-intake-slice/src/confinement.rs` does not compile until the breaks below are
followed.

## Acceptance

- `crates/loom-intake-slice/Cargo.toml` pins both Substrate crates at the 0.21.0 tag's commit with
  `version = "=0.21.0"`, `Cargo.lock` holds one copy of each, and the pin assertion in
  `crates/loom-intake-slice/tests/host_git_hardening.rs:548-552` names the new version and
  commit.
- The breaks are followed: `FACT_ORDER` grows from 11 to 13 facts (Substrate 0.15.0);
  `substrate_wire::AppliedConfinement.cgroup` becomes `Option<String>` and the type gains
  `requires` (0.18.0); `substrate_wire::ConfinementRequest` gains `requires`.
- A Substrate record with no cgroup is refused where an empty cgroup is refused today
  (`check_record`, `confinement.rs:2822`), with the same refusal.
- The two tests that assert the recorded facts exactly (`confinement.rs:3543` and
  `crates/loom-intake-slice/tests/confinement.rs:89-95`) expect the five facts
  `whole_tree_kill`, `ipc_socket_allowlist`, `ipc_signals_contained`, `stdin_pipe` and
  `output_live`; every other assertion of `tests/confinement.rs` and `tests/confined_child.rs`
  is unchanged.
- The statement in `crates/loom-operator/src/main.rs:53` and
  `crates/loom-operator/tests/startup.rs` about rustls `aws-lc-rs` in the operator's graph still
  holds on the new lock file, or is corrected in the same commit.
- `AGENTS.md`, `CHANGELOG.md` (`### Changed`, naming the two added facts) and any site page that
  names the Substrate version move with the pin.
- `task check` exits 0.

## Out of scope

Launching through the adapters (`story:child-launches-through-confinement-adapters`) and choosing
the adapter from configuration (`story:confinement-opens-from-engine-config`). Substrate's `git`
and `kubernetes` host features.

## Scope

Rewritten 2026-11-21 at the close of wave 2026-11-21-w1 from the unit's commits (`git diff --name-status ecefe180 01871cfd`). Every line is cited from that diff.

- **Primary surface:** `crates/loom-intake-slice` — cited, 10 of the 16 changed files are under it (manifest, `src/confinement.rs`, three helper modules, three existing test files, two new test files)
- **Files:** `crates/loom-intake-slice/Cargo.toml` — cited, `substrate-host` and `substrate-wire` move from rev `0b9d50c6…` / `=0.14.0` to rev `adfde402…` / `=0.21.0`; `substrate-host` takes `default-features = false, features = ["git"]`
- **Files:** `crates/loom-intake-slice/src/confinement.rs` — cited, 124 lines: `settle`, the `ConfinementRequest` and wire `AppliedConfinement` literals follow the 0.21.0 shapes (`requires`, `cgroup: Option<String>`); `OWNED_MOUNTS` gains `/etc/alternatives`; `check_record` refuses a record naming no cgroup, an empty one, or `requires`; unit test `a_record_naming_no_cgroup_or_unrequested_requirements_is_refused` and the fact-list test change are new
- **Files:** `crates/loom-intake-slice/src/confinement/workdir.rs`, `crates/loom-intake-slice/src/confinement/several_apertures.rs`, `crates/loom-intake-slice/src/confinement/security_w20261023w1_u3.rs` — cited, each test helper's `AppliedConfinement` literal takes `cgroup: Some(…)` and `requires: None` (3 lines each)
- **Files:** `crates/loom-intake-slice/tests/confinement.rs` — cited, the exact fact list gains `stdin_pipe` and `output_live` (five facts, the three of 0.14.0 first); `record.cgroup` is compared through `as_deref()`
- **Files:** `crates/loom-intake-slice/tests/confined_child.rs` — cited, the three `record.cgroup` comparisons go through `as_deref()` against `Some(result.applied.cgroup.as_str())`
- **Files:** `crates/loom-intake-slice/tests/host_git_hardening.rs` — cited, the pinned host package version `0.14.0` becomes `0.21.0` and its source rev becomes `adfde402…`
- **Files:** `crates/loom-intake-slice/tests/security_w20261121w1_u1.rs` and `crates/loom-intake-slice/tests/security_w20261121w1_u1_pass2.rs` — cited, new files, the security review's tests of the 0.21.0 pin (131 and 93 lines)
- **Files:** `crates/loom-operator/src/main.rs` — cited, only the comment above the `ring` provider install changes: `aws-lc-rs` came with `b10x-substrate-host` until it is taken without its `kubernetes` feature
- **Files:** `crates/loom-operator/tests/startup.rs` — cited, the module comment changes the same way; no code change
- **Files:** `Cargo.lock` — cited, 79 lines changed for the new Substrate rev and its dependency set
- **Files:** `AGENTS.md` — cited, the confined-launch fact list names five facts, the reserved-mount list names `/etc/alternatives`, the record check names the cgroup and `requires` refusal, and the operator's rustls paragraph says the graph holds `ring` alone
- **Files:** `CHANGELOG.md` — cited, two `### Changed` lines under `[Unreleased]`: the Substrate move with the two new facts, and a **Breaking** line for `ChildCommand` refusing `ScopeInvalid` a mount on, under or containing `/etc/alternatives`
- **Files:** `website/docs/reference/sdk-stability.md` — cited, one table row (`ChildCommand`, `ReadOnlyMount`, `ReadOnlyFile`) adds `/etc/alternatives` to the list of Substrate-owned mounts
- **Symbols:** none — the diff adds or changes no public symbol; `OWNED_MOUNTS` is private, and the `/etc/alternatives` refusal is a behaviour of the existing `ChildCommand` checks
- **Predicted and not touched:** none
