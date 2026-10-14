---
format: aep.planning-md/3
id: story:release-names-embedder-breaks
kind: story
status: implemented
title: release-check refuses a release entry that does not name what breaks for a consumer
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
- depends_on: story:pinned-consumer-check
- depends_on: story:sdk-host-port-stability
- depends_on: story:command-event-contract
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T10:13:48Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-14T10:14:20Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-14T21:39:17Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

`loom-xtask release-check` refuses a release whose CHANGELOG entry does not say what breaks for a
consumer: every entry carries a `Breaking:` sentence, and `Breaking: none.` is refused when either
stable surface changed since the previous tag (the pinned consumer, the host-port witness, or the
run-events contract snapshot).

## Why

Both stable surfaces, the crates (`loom_sdk` pinned by tag) and the `b10x-loom` command's
`run --output jsonl` records, stay stable per release, and a break is named in the CHANGELOG. The
epic's third acceptance asks that a release name every break a pinned consumer sees. Entries do so
today by habit (0.20.0's entry has a `Breaking:` sentence); nothing refuses one that forgets.

## Acceptance

A release whose entry lacks a `Breaking:` sentence, or says `Breaking: none.` while a surface file
changed since the previous tag, fails `release-check` naming the entry and the changed files:

- `crates/loom-xtask/tests/release_check.rs::release_check_names_an_entry_without_a_breaking_sentence`
- `crates/loom-xtask/tests/release_check.rs::release_check_refuses_breaking_none_when_a_stable_surface_changed_since_the_previous_tag`
  over a fixture repository with two tags, where `consumers/pinned/`,
  `crates/loom-sdk/tests/host_ports.rs` or `docs/contracts/run-events-v1.json` differs between them.
- `crates/loom-xtask/tests/release_check.rs::release_check_accepts_breaking_none_when_no_stable_surface_changed`
- `crates/loom-xtask/tests/release_check.rs::release_check_passes_on_the_first_tag` (no previous
  tag: only the sentence is required).
- `the_repository_passes_release_check_at_its_workspace_version` still passes.
- `release.yml` fetches the tags `release-check` diffs against.

## ESS first

No behaviour change for Loom's runtime or command: the rule is in the release tool. No ESS noun
is involved. The red step is `release_check_names_an_entry_without_a_breaking_sentence`.

## Domain relations

None.

## Depends on

`story:pinned-consumer-check`, `story:sdk-host-port-stability` and `story:command-event-contract`,
which create the three surface files the rule diffs.

## Not in scope

- Deciding whether a given change is a break: the release author writes the sentence; the check
  refuses only its absence, or `none` beside a changed surface.
- Rewriting earlier CHANGELOG entries.

## Scope

- `crates/loom-xtask/src/release.rs`, `crates/loom-xtask/tests/release_check.rs`
- `.github/workflows/release.yml`: tags fetched for `release-check`
- `AGENTS.md`: the release row in § Checks that hold the rules
- `CHANGELOG.md`: the header paragraph naming the rule

Touches none of `crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`,
`ess/intake/`, `generated/rust/intake`, `crates/loom-mandate`, `ess/domains/authority.yaml`,
`generated/rust/loom`, `Cargo.toml`, `Cargo.lock`.

## Ready

Ready once its three dependencies are implemented; it needs no answer from anybody.
