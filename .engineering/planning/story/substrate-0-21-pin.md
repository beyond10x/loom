---
format: aep.planning-md/3
id: story:substrate-0-21-pin
kind: story
status: active
title: Loom builds on Substrate 0.21.0 with no behaviour change
summary: 'From 0.14.0: adapter stdin and live output become available to later stories'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
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
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:44Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-11-21T00:12:43Z", actor: "human:timo", revision: 7, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w1"}
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

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-intake-slice` — cited, the story names its manifest and `src/confinement.rs`; it is the only crate that depends on the Substrate crates
- **Files:** `crates/loom-intake-slice/Cargo.toml:31-32` — cited, the two `rev = "0b9d50c6…"`, `version = "=0.14.0"` dependency lines (tag `0.21.0` is `adfde4020b14741b167532f46c7a0e9cebf2b3d3`, read with `git rev-parse`)
- **Files:** `crates/loom-intake-slice/src/confinement.rs` — cited, the story names it; Substrate's 0.18.0 changelog also names it as the file that must adapt. Sites: `settle` (`cgroup: record.cgroup.clone()`, :851), `check_record` (destructures the wire `AppliedConfinement` without `..`, :2794; `cgroup.is_empty()`, :2822), `admitted_facts` (:3320), `ConfinementRequest` literals (:1519, :3244, test :3952, :4860), wire `AppliedConfinement` literals in its test module (:3854–3914, :4202, :4658)
- **Files:** `crates/loom-intake-slice/tests/confinement.rs` — cited, the story names it; `assert_eq!(record.cgroup, result.applied.cgroup)` (:100) compares `Option<String>` with `String` on 0.21.0, and the facts list (:89–95) is asserted exactly
- **Files:** `crates/loom-intake-slice/tests/confined_child.rs` — cited, the story names it; the same `record.cgroup` comparison at :90, :743, :1323
- **Files:** `crates/loom-intake-slice/tests/host_git_hardening.rs:548-552` — cited, read in the tree: it asserts the host package is version `"0.14.0"` from source `…?rev=0b9d50c6…#0b9d50c6…`, and fails on any new pin. The story does not name it.
- **Files:** `crates/loom-intake-slice/src/confinement/workdir.rs`, `crates/loom-intake-slice/src/confinement/several_apertures.rs`, `crates/loom-intake-slice/src/confinement/security_w20261023w1_u3.rs` — inferred: each builds `substrate_wire::AppliedConfinement { cgroup: "…".to_owned(), … }` in test helpers (`workdir.rs:307`, `several_apertures.rs:235`, `security_w20261023w1_u3.rs:34` and `:82`). That should stop compiling when `cgroup` becomes `Option<String>` and `requires` appears, but I did not compile.
- **Symbols:** `FACT_ORDER`, `Fact`, `ConfinementProfile` — cited, in the story. Loom uses only `FACT_ORDER`, via `.iter()` in `admitted_facts`; Loom's own `ConfinementProfile` is the generated `intake_model` type, not Substrate's.
- **Symbols:** `substrate_wire::AppliedConfinement` (`cgroup: Option<String>`, new `requires`) — cited, in the story; `substrate_wire::ConfinementRequest` (new `requires: Option<ConfinementRequires>`) — cited, Substrate's wire diff 0.14.0..0.21.0 (`crates/substrate-wire/src/lib.rs`, `ConfinementRequest`). The story does not list the `ConfinementRequest` break.
- **Also likely:** `crates/loom-operator/src/main.rs:53` and `crates/loom-operator/tests/startup.rs` — inferred, listed only to be checked, not edited. Both state that the operator's graph carries rustls `aws-lc-rs` through `b10x-substrate-host`; the 0.21.0 host moves its rustls and hyper dependencies behind the default `kubernetes` feature, and I did not resolve the feature graph.
- **Documents:** none to edit. A grep of `website/` and `docs/` finds no page that names the Substrate version; `docs/qualification/2026-10-05-confined-tests.md` mentions 0.7.10 as history. House-rule files: `AGENTS.md`, `CHANGELOG.md` (`### Changed`) and `Cargo.lock`. `Cargo.lock` gains `b10x-substrate-guarded` and `b10x-substrate-kubernetes-client` from the 0.21.0 host's new dependencies.
- **Confidence:** high — the story, Substrate's own changelog and the tree agree on the files; the only open question is how much of the test helper code needs edits, which only a compile shows
- **Would collide with:** any unit editing `crates/loom-intake-slice/src/confinement.rs`. That means the child-launch and engine-config stories, and `bound-commands-run-confined` if it routes through `SubstrateRunner`. It also collides with any unit editing `crates/loom-intake-slice/Cargo.toml` or `Cargo.lock`: `story:canon-0-9-0-pin` names both and moves `b10x-canon-engineering` (line 24) in the same manifest. Also any unit editing `tests/confinement.rs`, `tests/confined_child.rs` or `tests/host_git_hardening.rs`.
- **Safety fact:** this is not a no-behaviour-change pin on its face. On 0.21.0 the `linux` adapter proof also proves `StdinPipe` and `OutputLive` whenever the floor holds (Substrate `crates/substrate-host/src/confinement/linux.rs:200-203`). `admitted_facts` (`confinement.rs:3320`) returns every proved fact in `FACT_ORDER`, so a confined launch would record five facts (`whole_tree_kill`, `ipc_socket_allowlist`, `ipc_signals_contained`, `stdin_pipe`, `output_live`) instead of three. Two tests assert exactly three: the unit test at `confinement.rs:3543` (its served fixture has the floor, so it gains two) and `tests/confinement.rs:89-95` (real, ignored). Step 2 (pointed at `file:line`), unproven.
