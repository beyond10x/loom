---
format: aep.planning-md/3
id: story:stability-version-rule
kind: story
status: draft
title: The SDK stability page states a deprecation window and a tag rule, and release-check holds them
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

`website/docs/reference/sdk-stability.md` states a deprecation window (a listed path is deprecated
for at least one release before it is removed) and a tag rule (a tag whose entry carries a
`**Breaking:**` bullet moves the minor version before 1.0), and `release-check` holds the rule.

## Why

The page promises stability with no window or version rule (`sdk-stability.md:37`). Decided by the
orchestrator, Q6 A: git tags, no registry.

## Acceptance

- `crates/loom-xtask/tests/release_check.rs`
  (`a_removal_without_a_deprecated_release_is_refused`): an entry removing a listed path that no
  earlier entry deprecated fails `release-check`.
- (`a_breaking_entry_on_a_patch_tag_is_refused`).
- The page states both rules and links `story:breaks-between-releases`'s generated page when it
  exists.

## ESS first

No behaviour change in Loom's runtime and no noun; `ess/` is untouched. The red step is
`a_removal_without_a_deprecated_release_is_refused`.

## Not in scope

Publishing to a registry (Q6 A); generating the break page (`story:breaks-between-releases`).

## Scope

Crates: `loom-xtask`. `crates/loom-xtask/src/`, its test, `sdk-stability.md`, AGENTS.md § Releases,
CHANGELOG.md.

## Ready

Ready now.
