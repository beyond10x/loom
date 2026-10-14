---
format: aep.planning-md/3
id: story:pinned-consumer-check
kind: story
status: active
title: A crate outside the workspace pins Loom by path and builds the documented embedding in task check
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: .gitignore
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: consumers/pinned
- confidence: inferred
  path: crates/loom-xtask/tests/pinned_consumer.rs
- confidence: inferred
  path: website/docs/guides/embed-the-runtime.md
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T02:54:27Z", actor: "human:timo", revision: 9}
- {from: "proposed", to: "active", at: "2026-10-14T02:54:28Z", actor: "human:timo", revision: 10}
---
## Outcome

A minimal crate outside Loom's workspace, `consumers/pinned/`, depends on `b10x-loom-sdk` by path
and nothing else, compiles the documented embedding (`crates/loom-sdk/examples/software_change.rs`)
as an external crate would, runs it to `ApprovalRequired (repository.merge)`, and `task check` runs
it. A change that breaks what an embedder compiles against fails Loom's own CI before a release,
not a consumer's build after one.

## Why

`epic:loom-foundation-for-consumers` promises that a consumer pinning a Loom tag moves to the next
one without patching Loom. Today the only embedding Loom compiles is the SDK example, built as part
of the workspace: it sees workspace-internal paths and features, shares the workspace's
`Cargo.toml`, and a re-export that only resolves inside the workspace would not be noticed. A crate
that is its own workspace and names Loom only through `b10x-loom-sdk` is the closest Loom can come
to a consumer without one, and it needs no answer from the consumers.

## Acceptance

`task check` builds and runs `consumers/pinned/` against the tree it runs in, and fails when the
consumer does not compile, does not stop at `ApprovalRequired (repository.merge)`, depends on any
Loom crate other than `b10x-loom-sdk`, or resolves a second copy of a Loom or Canon crate:

- `consumers/pinned/tests/embedding.rs::the_pinned_consumer_stops_at_approval_required` runs the
  consumer's binary (`CARGO_BIN_EXE_…`) and asserts its last line is
  `ApprovalRequired (repository.merge)` and that it merged nothing.
- `crates/loom-xtask/tests/pinned_consumer.rs::the_pinned_consumer_depends_on_the_sdk_alone_by_path`
  reads `consumers/pinned/Cargo.toml` and asserts: it has its own `[workspace]` table, exactly one
  `[dependencies]` entry, `b10x-loom-sdk` with a `path` and no `git`, `tag`, `rev` or `version`,
  and no `[patch]` section.
- `crates/loom-xtask/tests/pinned_consumer.rs::the_pinned_consumer_resolves_one_copy_of_each_loom_crate`
  runs `cargo tree --manifest-path consumers/pinned/Cargo.toml --offline --duplicates` and asserts
  no package named `b10x-loom-*`, `loom-*`, `b10x-canon` or `b10x-canon-engineering` is listed.
- `crates/loom-xtask/tests/pinned_consumer.rs::the_pinned_consumer_compiles_the_sdk_example_source`
  asserts the consumer's `src/main.rs` includes `crates/loom-sdk/examples/software_change.rs` by
  `#[path]` and contains no embedding code of its own, so the documented embedding and the checked
  one are one file.
- `task check` exits 0 on the unchanged tree; with `run_until_blocked` removed from the top of
  `crates/loom-sdk/src/lib.rs` (a local, uncommitted edit) `task consumer-check` exits non-zero
  naming the unresolved import. The run is recorded in the pull request.

## ESS first

No behaviour change: the story adds a check and an out-of-workspace test crate. No runtime type,
command, port or outcome changes, so `ess/` is untouched. The red step is
`the_pinned_consumer_depends_on_the_sdk_alone_by_path`, which fails before `consumers/pinned/`
exists.

## Domain relations

None. A consumer, a release tag and a pin are not nouns of any `ess/1` document in this repository,
and the story introduces none.

## Design notes

- `consumers/pinned/Cargo.toml` declares `[workspace]` so the root workspace (`members =
  ["crates/*"]`) neither includes nor rejects it; the root `Cargo.toml` is not edited.
- Lockfile: `task consumer-check` copies the root `Cargo.lock` to `consumers/pinned/Cargo.lock`
  before it builds, then runs `cargo test --manifest-path consumers/pinned/Cargo.toml --offline`.
  Cargo keeps every locked entry the consumer shares with the workspace and adds the consumer's own
  package, so the consumer resolves exactly the versions and git revisions the workspace locked, and
  `cargo fetch --locked` (already in `check.yml`) is the only fetch. The copied lockfile and the
  consumer's `target/` are git-ignored.
- The consumer builds into its own `consumers/pinned/target/`; no `CARGO_TARGET_DIR` is set.

## Not in scope

- Pinning by tag. A path pin checks the tree before it is tagged; a tag pin can only check a release
  after the fact, which is a consumer's own CI.
- The ports a host implements beyond what the example uses (`story:sdk-host-port-stability`).
- The CHANGELOG rule for breaks (`story:release-names-embedder-breaks`).
- Any change inside a consumer's repository.

## Scope

- `consumers/pinned/Cargo.toml` (new), `consumers/pinned/src/main.rs` (new),
  `consumers/pinned/tests/embedding.rs` (new)
- `crates/loom-xtask/tests/pinned_consumer.rs` (new)
- `Taskfile.yml`: a `consumer-check` task, run by `check`
- `.gitignore`: `consumers/*/target/`, `consumers/*/Cargo.lock`
- `AGENTS.md`: a row in § Checks that hold the rules, and `task consumer-check` in § Gate
- `website/docs/guides/embed-the-runtime.md`: one paragraph saying CI builds the embedding as an
  external crate
- `CHANGELOG.md` (**Unreleased**)

Touches none of `crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`,
`ess/intake/`, `generated/rust/intake`, `crates/loom-mandate`, `ess/domains/authority.yaml`,
`generated/rust/loom`, `Cargo.toml`, `Cargo.lock`. It reads `crates/loom-sdk/examples/software_change.rs`,
which re-exports from `crates/loom-intake-slice` and `crates/loom-governor`, so a running change
there can turn this check red: that is the check working. `AGENTS.md` and `CHANGELOG.md` are shared
prose files; merge them, do not overwrite.

## Ready

Ready now: it needs no answer from either consumer.
