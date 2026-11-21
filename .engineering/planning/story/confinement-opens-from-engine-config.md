---
format: aep.planning-md/3
id: story:confinement-opens-from-engine-config
kind: story
status: proposed
title: A host names one Substrate adapter in engine configuration and every child Loom starts runs under it
summary: linux, or local with a reason, opened through confinement::open; unconfined becomes a setting, not a second code path
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:child-launches-through-confinement-adapters
scope:
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-intake-conformance/src/codec.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/src/executor.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/intent.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/resume.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/engine_confinement.rs
- confidence: inferred
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: ess/intake/domains/confinement.yaml
- confidence: inferred
  path: website/docs/concepts/governor-and-intake.md
- confidence: inferred
  path: website/docs/guides/run-an-intent.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:44Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":3}}}
---
## Outcome

A host configures Loom's confinement once, as engine configuration: the name of one Substrate
adapter (`linux`, or `local` with a reason) and what that adapter needs. One constructor builds
the runner from that value and opens the adapter through `substrate_host::confinement::open`; the
test runner, `run_child` and `spawn_child` then run under that adapter. With `local` the child
runs unconfined, declared insecure with the operator's reason, and each launch records the adapter
and the reason. A request never names an adapter. Bound commands reach this runner in
`story:bound-commands-run-confined`, not here.

## Why

`~/beyond10x/sdc/docs/layers.md` § 3: the substrate is injected today only as a `TestRunner`
implementation, and `SubstrateRunner::open` builds a `HostDriver` itself
(`crates/loom-intake-slice/src/confinement.rs:402`). The unconfined route is a second type,
`UnconfinedRunner` (`crates/loom-intake-slice/src/executor.rs:1163-1197`), which has no
`run_child`. So "unconfined" is a different code path, not a setting, and a tree of cases whose
tools need host paths and a Unix socket (the SDC's `slot`, `ess`, `aep` calls) cannot run through
the same funnel as a confined one. Substrate's `local` adapter exists for this: it requires
`declared_insecure` and a non-blank `local_reason`, and host policy can refuse it
(`substrate-host` `confinement/local.rs`, read at 0.21.0).

## ESS first

`ess/intake/domains/confinement.yaml` first: an `EngineConfinement` type with the adapter name
(`Linux`, `Local`) and an optional `local_reason`, the refusal `LocalReasonMissing`, and the
adapter and reason as members of the applied confinement record. On that commit
`task intake-drift` fails; the story's new test
`crates/loom-intake-slice/tests/engine_confinement.rs` does not compile.

## Acceptance

- `engine_confinement.rs` greps `crates/` and finds `confinement::open(` called in exactly one
  function, the constructor that takes the engine configuration value.
- With `Local` and a reason, each of `run_child`, `spawn_child` and the test runner's `run`
  starts a program that writes a file outside its workspace; the write succeeds, and each applied
  record names adapter `local` and the reason.
- With `Local` and a blank reason the constructor is refused before any child starts, naming
  `LocalReasonMissing`; with host policy set to refuse (`SUBSTRATE_CONFINEMENT_LOCAL`), it is
  refused naming the policy.
- With `Linux` the delegated cases of `tests/confinement.rs` and `tests/confined_child.rs` hold
  as before, and each applied record names adapter `linux`.
- `b10x-loom run --confinement` takes `substrate` (the `Linux` value, the default), the new
  `local` with `--local-reason <text>`, and `none`. `--confinement local` without
  `--local-reason` exits 2 naming the flag. A clock query starts no child and has no applied
  record, so the flag's effect is read there from standard output: `--confinement local
  --local-reason x` on a clock query exits 0 and its `confinement:` line names `local` and `x`.
  `none` still selects
  `UnconfinedRunner` and prints one deprecation line on standard error naming `local`.
- `UnconfinedRunner` stays exported for one release and is listed under `### Deprecated` in
  `CHANGELOG.md` (SDK stability page rule).
- `task check` exits 0.

## Out of scope

Bound commands (`story:bound-commands-run-confined`). Removing `HostDriver::open` from the launch
path (`story:child-launches-through-confinement-adapters`). The `macos`, `systemd`, `oci` and
`pod` adapters and remote placement (`story:confines-on-macos-through-substrate`,
`story:mantle-placement`). The session executor's harness spawn and the Connectors invoker.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-intake-slice/src/confinement.rs` — cited. The story names `SubstrateRunner::open`, which calls `HostDriver::open` at `:402`. `run_child` is at `:475`, `spawn_child` at `:525` and the hard-coded `adapter = Some("linux")` at `:256`.
- **Files:**
  - `crates/loom-intake-slice/src/confinement.rs:402` — cited
  - `crates/loom-intake-slice/src/executor.rs:1163-1197` — cited, `UnconfinedRunner` (the story cites the same lines; its `AppliedConfinement` also sets `adapter: None` at `:1190`)
  - `ess/intake/domains/confinement.yaml` — cited, where `EngineConfinement`, `LocalReasonMissing` and the adapter and reason members go. It holds `Backend` as [Substrate, None] and `ConfinementRefusal` today.
  - `crates/loom-intake-slice/tests/engine_confinement.rs` — cited, new, named by the story
  - `crates/loom-intake-slice/tests/confinement.rs` — cited, delegated cases that must hold
  - `crates/loom-intake-slice/tests/confined_child.rs` — cited, delegated cases that must hold
  - `crates/loom-cli/src/lib.rs:777,985` — cited, the `--confinement` flag and `enum Confinement { Substrate, None }`; `--local-reason` is added here
  - `crates/loom-cli/src/main.rs:670,1017,1152` — cited, three sites that map `Confinement` to a runner or `Backend`
- **Symbols:** `SubstrateRunner`, `SubstrateRunner::run_child`, `SubstrateRunner::spawn_child`, `TestRunner::run`, `UnconfinedRunner`, `Confinement`, `Backend`, `AppliedConfinement`, `substrate_host::confinement::open` (0.21.0: `AdapterRequest::{Local(LocalRequest), Linux(Box<LinuxRequest>)}`) — cited
- **Also likely:**
  - `crates/loom-intake-slice/src/intent.rs` (`ResumeHost.confinement: Backend`, `:356`) — inferred. `run --state` keeps the confinement choice, so a `local` run's adapter and reason would have to be kept there.
  - `crates/loom-intake-slice/src/resume.rs` (`Backend` stored as the text "Substrate" or "None", `:510,587`) — inferred, same reason.
  - `crates/loom-intake-conformance/src/codec.rs` (`Backend` by variant name, `:66,140`) — inferred. It is touched only if the `Backend` enum or the applied record's shape changes.
  - `crates/loom-sdk/src/lib.rs` (re-exports at `:117,123`) — inferred. The new constructor and `EngineConfinement` would be exported next to `SubstrateRunner`.
- **Documents:**
  - `website/docs/guides/run-an-intent.md` — inferred. Line 131, "Only `--confinement none` runs with your rights", becomes false once `local` exists.
  - `website/docs/concepts/governor-and-intake.md` — inferred. Line 134 says the same.
- **Confidence:** medium
  - The code surface (`confinement.rs`, `executor.rs`, the ESS domain, `loom-cli`) is high: the story and the tree agree.
  - The ripple into resume state and the SDK is a guess.
  - One acceptance bullet cannot be placed (the clock query, see the unestablished list).
  - This is code and ESS work, not documents only.
- **Would collide with:**
  - any unit editing `SubstrateRunner`'s launch path in `confinement.rs` (`driver`/`open`/`run_child`/`spawn_child`; the predecessor story rewrites exactly this, and the next story reads the same runner from the CLI);
  - any unit editing `UnconfinedRunner` or `backend_name` in `executor.rs`;
  - any unit adding to `ess/intake/domains/confinement.yaml`;
  - any unit editing the `run` flags in `loom-cli` (`RunArgs` in `lib.rs`, or the runner wiring in `main.rs` around `:1143-1180`);
  - any unit changing `ResumeHost` or the resume context.
- **Safety fact:** `local` is reachable only through an explicit engine-configuration value with a non-blank reason, and nothing falls back to it (Substrate `local.rs` module doc, "Nothing falls back to it"; `--confinement substrate` stays the default at `loom-cli/src/lib.rs:777`) — step 2, unproven.
  - The reason is not an open-time input. `LocalRequest` holds only `policy`, and the reason is `ConfinementProfile.local_reason`, checked at admission (refusal 3, then host policy at refusal 5, in `mod.rs`).
  - So "refused before any child starts" means the constructor must admit a probe profile or read the `local_permitted` fact at construction. This is a reading of 0.21.0 and unproven.
