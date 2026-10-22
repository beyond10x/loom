---
format: aep.planning-md/3
id: story:release-binaries
kind: story
status: draft
title: Every Loom release carries a b10x-loom binary per target, with its checksum, as GitHub Release assets
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

Every Loom release's GitHub Release carries a `b10x-loom` binary per supported target as an asset,
each with a SHA-256 checksum, so a reader installs Loom by download and an image build copies a
pinned binary instead of compiling.

## Why

Session `b10x-chef-orchestrator`, 2026-10-22: "file a draft story for Loom release binaries (GitHub
Release assets per target, the download the tutorial and M3 image need), next wave candidate."

`release.yml` verifies the tag and runs the check, and creates no artifact (`contents: read`,
AGENTS.md § Releases step 4). `story:confine-an-agent-tutorial` installs by download, and
`story:agent-project-kubernetes` places a pinned `b10x-loom` in the image's `bins` node.

## Acceptance

- On a release tag, `release.yml` builds `b10x-loom` with `--locked` for each target the story
  settles (at least `x86_64-unknown-linux-gnu`; `aarch64-apple-darwin` if it builds), after
  `release / check` is green.
- Each binary is packaged `b10x-loom-<version>-<target>.tar.gz` with a `.sha256` beside it.
- The assets are uploaded to the bot's GitHub Release for the tag through the bot route; the
  workflow holds no write token of a person. How the workflow gets write access without breaking
  the bot-only rule is settled before the story is ready.
- `crates/loom-xtask/tests/release_check.rs` holds the workflow's shape: the build jobs need the
  check, and the asset names follow the pattern.
- AGENTS.md § Releases says a release is finished only once the assets and checksums are verified;
  `website/docs/getting-started.md` installs by download.

## ESS first

No behaviour change in Loom's declared domains: a release workflow and a docs page. The red step is
the extended `release_workflow_verifies_the_tag_and_reuses_the_check` test.

## Open before it can be ready

- Write access: the bot App creates the Release (`b10x-gates api`); whether a workflow may upload
  assets to it, or the bot uploads artifacts a read-only workflow built, I don't know.
- macOS target: whether CI builds Loom on `macos-latest` (bubblewrap and Substrate are Linux-only
  dependencies of `loom-intake-slice`): I don't know.

## Not in scope

A registry publish (`cargo publish`, Homebrew); Windows binaries.

## Scope

`.github/workflows/release.yml`, `crates/loom-xtask/tests/release_check.rs`, AGENTS.md § Releases,
README.md, `website/docs/getting-started.md`, CHANGELOG.md.

## Ready

Not yet: the two open items above.
