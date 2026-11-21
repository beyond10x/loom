---
format: aep.planning-md/3
id: story:commission-docs-read-one-ess-version-line
kind: story
status: draft
title: loom-commission-docs reads one version line from ess, so docs-drift passes through the ess dispatcher
summary: ess --version prints two lines through the dispatcher; the generator takes both and docs-drift fails locally
tags:
- from-finding
relations:
- informed_by: approval-record:loom-wave-2026-11-21-w1
revision: 1
---
## Outcome

`task commission:docs-drift` and `task commission:docs` give the same result whether `ess` on
`PATH` is a release binary or the dispatcher that delegates to the pinned release.

## Found

Pre-existing; found by the whole gate of wave 2026-11-21-w1 on `b38eb7d1`, not introduced by it
(`approval-record:loom-wave-2026-11-21-w1`, § Decisions taken during the wave).

- `ess_version` in `crates/loom-commission-docs/src/main.rs:143-153` takes the whole standard
  output of `ess --version`, trimmed, as the version.
- The generator writes it into `website/docs/reference/commission/domain-graph.json` as
  `"source": "commission ess/commission/ compiled with <version>"`.
- With the `ess` 0.96.0 dispatcher on `PATH` and `ESS_TOOLCHAIN=0.86.0`, `ess --version` prints two
  lines on standard output (`ess 0.96.0`, `ess 0.86.0`), so a fresh generation writes
  `compiled with ess 0.96.0\ness 0.86.0` and `generate --check` exits 1 against the committed
  `compiled with ess 0.86.0`.
- With the 0.86.0 binary first on `PATH` (`~/.cache/ess/toolchains/0.86.0`) the same check exits 0.
- CI installs the pinned release directly and is not affected (inferred from the comment at
  `main.rs:142`, "CI pins the release"; the workflow was not read).

## Acceptance

- A unit test of `loom-commission-docs` gives `ess_version` a stub program that prints two version
  lines and asserts the last line is taken, and a stub that prints one line and asserts that line.
- A stub that prints no line is refused with the existing "printed no version" error.
- `task commission:docs-drift` exits 0 on an unchanged tree with the dispatcher on `PATH` and
  `ESS_TOOLCHAIN` set to the repository's pin.

## Out of scope

- The dispatcher's output; it belongs to the `ess` repository.
- `loom-docs` and `loom-xtask`: `task docs-check` and `task drift` passed through the dispatcher
  in the same gate.
