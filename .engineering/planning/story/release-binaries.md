---
format: aep.planning-md/3
id: story:release-binaries
kind: story
status: draft
title: Every Loom release carries a b10x-loom binary per target, with its checksum, as GitHub Release assets
tags:
- sdc
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: .github/workflows/binaries.yml
- confidence: cited
  path: .github/workflows/release.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-xtask/tests/release_check.rs
- confidence: cited
  path: website/docs/getting-started.md
revision: 4
---
## Outcome

Every Loom release's GitHub Release carries a `b10x-loom` binary for `x86_64-unknown-linux-gnu`
as an asset, with a SHA-256 checksum beside it, so a reader installs Loom by download and an image
build or a consumer's CI copies a pinned binary instead of compiling. No workflow is granted
`contents: write`: the tag's `release` run builds and keeps the binary as a workflow artifact, and
the bot attaches it to the Release it creates.

## Why

Session `b10x-chef-orchestrator`, 2026-10-22: "file a draft story for Loom release binaries (GitHub
Release assets per target, the download the tutorial and M3 image need), next wave candidate."
Runbook's end state compiles no Rust: its CI "downloads a pinned `b10x-loom` with its checksum"
(Runbook's triage answer, wave 2, item 4).

`release.yml` verifies the tag, runs the check and publishes the images; it is read-only at the
top (`.github/workflows/release.yml:6-7`) and its one write is `packages: write` for `images.yml`
(`release.yml:46-53`). It creates no GitHub Release: the bot does, through `b10x-gates api --method
POST --path /repos/beyond10x/loom/releases` (AGENTS.md § Releases step 5, `AGENTS.md:2526-2531`).

## Design

The write-access item is settled as the planner proposed it: the workflow builds read-only, the
bot attaches. The attach cannot go through `b10x-gates api`, which sends a JSON body to
`https://api.github.com` only (`~/beyond10x/gates/src/delivery.rs:145-158`,
`src/main.rs:243-266`); a release asset is a binary body sent to `uploads.github.com`. The bot's
route for it is `b10x-gates gh`, which runs `gh` with the bot's installation token in the child's
environment and admits the `release` and `run` commands (`src/delivery.rs:56-72`).

1. **Build.** A new reusable workflow `.github/workflows/binaries.yml` (`on: workflow_call`, input
   `version`), called by `release.yml` as job `binaries` with `needs: check` and
   `permissions: contents: read`, like `images` is called (`release.yml:46-53`). One job,
   `binaries / x86_64-unknown-linux-gnu`, `runs-on: ubuntu-24.04`, `permissions: contents: read`,
   in the container image the agent image builds in
   (`rust:1.99-bookworm@sha256:9181b56c…`, `deploy/images/loom-agent/Dockerfile:2`), so the
   binary's glibc floor is bookworm's, as the image's is. Steps: checkout with
   `persist-credentials: false`; `cargo build --release --locked -p b10x-loom-cli --bin b10x-loom`
   (the Dockerfile's command, `Dockerfile:8`); package
   `b10x-loom-<version>-x86_64-unknown-linux-gnu.tar.gz` holding the directory
   `b10x-loom-<version>-x86_64-unknown-linux-gnu/` with `b10x-loom` and `LICENSE`; write
   `b10x-loom-<version>-x86_64-unknown-linux-gnu.tar.gz.sha256` as `sha256sum` prints it
   (`<64 hex>  <file name>`).
2. **Keep.** The same job uploads both files as one workflow artifact named
   `b10x-loom-<version>` with `actions/upload-artifact@043fb46d…` (the pin `pages.yml:57` uses),
   `retention-days: 30`, `if-no-files-found: error`. The artifact is the only place the files
   live between the build and the attach. `upload-artifact` writes through the runner's own
   artifact token, not `GITHUB_TOKEN`, so the job needs no write permission.
3. **Attach.** After the `release` run is green and the Release exists (AGENTS.md § Releases
   step 5), the releasing session runs, in a scratch directory:
   - `gh run list --repo beyond10x/loom --workflow release.yml --branch <version> --json
     databaseId,conclusion --limit 1` (read-only `gh`) for the run id;
   - `gh run download <run-id> --repo beyond10x/loom --name b10x-loom-<version> --dir <scratch>`
     (read-only `gh`);
   - `sha256sum -c b10x-loom-<version>-x86_64-unknown-linux-gnu.tar.gz.sha256`, which prints
     `b10x-loom-<version>-x86_64-unknown-linux-gnu.tar.gz: OK`;
   - `b10x-gates gh -- release upload <version> <scratch>/b10x-loom-<version>-…tar.gz
     <scratch>/b10x-loom-<version>-…tar.gz.sha256 --repo beyond10x/loom` (the bot).
4. **Verify.** `gh release download <version> --repo beyond10x/loom --pattern 'b10x-loom-*'
   --dir <scratch2>`, then `sha256sum -c` there prints `OK`, and the digest equals the one the
   build job printed in its log. A release is finished only then.
5. **Consume.** Runbook's CI keeps the version and the digest in its own repository and runs:

   ```sh
   f=b10x-loom-$V-x86_64-unknown-linux-gnu.tar.gz
   gh release download "$V" --repo beyond10x/loom --pattern "$f"
   echo "$SHA256  $f" | sha256sum -c -
   tar -xzf "$f" && "./${f%.tar.gz}/b10x-loom" --version
   ```

   `sha256sum -c -` prints `<file>: OK`; `--version` prints `b10x-loom <V>` (clap's `version`,
   `crates/loom-cli/src/lib.rs:215-219`). The digest it checks
   is the one it pinned, not the `.sha256` asset, which arrives from the same place as the binary.

The one GitHub write that does not go through `b10x-gates api` is the asset upload, through the
bot's `b10x-gates gh`. AGENTS.md § Commits, pushes and GitHub writes (`AGENTS.md:2467-2469`) names
that exception.

## ESS first

No behaviour change in Loom's declared domains: two workflows, a test and documentation. The red
step is the extended `release_workflow_verifies_the_tag_and_reuses_the_check` test and the new
`binaries_workflow_builds_read_only_and_keeps_the_archive_and_its_checksum`, both failing before
`binaries.yml` exists.

## Acceptance

All in `crates/loom-xtask/tests/release_check.rs`:

- `release_workflow_verifies_the_tag_and_reuses_the_check`: `release.yml` has a job calling
  `./.github/workflows/binaries.yml` with `needs: check`, `permissions` exactly
  `contents: read`, `with: version: ${{ github.ref_name }}` and no `secrets`.
- `binaries_workflow_builds_read_only_and_keeps_the_archive_and_its_checksum`: `binaries.yml`
  runs only on `workflow_call`, and its steps build with `--locked`, name the archive
  `b10x-loom-${{ inputs.version }}-x86_64-unknown-linux-gnu.tar.gz` with a `.sha256` beside it,
  and upload both as the artifact `b10x-loom-${{ inputs.version }}`.
- `no_workflow_is_granted_contents_write`: no job and no workflow under `.github/workflows/`
  holds `contents: write`, and every `permissions` of `binaries.yml` is exactly `contents: read`.
- `the_binaries_build_uses_the_agent_images_builder`: the job's `container` image equals the
  `FROM … AS builder` image of `deploy/images/loom-agent/Dockerfile`, digest included.
- `agents_md_releases_names_the_attach_and_verify_commands`: AGENTS.md § Releases holds
  `b10x-gates gh -- release upload` and `sha256sum -c`, and says a release is finished only once
  the assets and their checksums are verified.
- `readme_and_site_download_the_release_binary_with_its_checksum`: README.md and
  `website/docs/getting-started.md` each hold a download of
  `b10x-loom-<workspace version>-x86_64-unknown-linux-gnu.tar.gz` and a `sha256sum -c` line.

All six read files of the tree and parse YAML; none builds, downloads or calls GitHub.

## Not in scope

A registry publish (`cargo publish`, Homebrew); Windows, macOS and `aarch64` binaries; a static
(`musl`) binary; signing or attestation of the assets; changing `b10x-gates` (an upload verb on
`api` is the gates repository's work).

## Scope

Derived from `8a8fad3b`. Every line is **cited** (read in the tree) or **inferred** (a reading that
could be wrong).

- **Files:** `.github/workflows/release.yml` (jobs 15-53; the new `binaries` job beside `images`)
  — cited
- **Files:** `.github/workflows/binaries.yml` (new) — cited
- **Files:** `crates/loom-xtask/tests/release_check.rs` (`workflow` 806-810,
  `release_workflow_verifies_the_tag_and_reuses_the_check` 813-929,
  `readme_and_site_pin_the_workspace_version_as_a_tag` 762-804) — cited
- **Files:** `AGENTS.md` (§ Commits, pushes and GitHub writes 2465-2475; § Releases 2477-2537)
  — cited
- **Files:** `README.md` (install, 170-185), `website/docs/getting-started.md` (install, 15-25),
  `CHANGELOG.md` (`### Added`) — cited
- **Not touched:** `Taskfile.yml` (holds no release task), `crates/loom-xtask/src/release.rs`
  (`release-check` reads the tag, the manifest and the changelog only) — cited
- **Would collide with:** any unit changing `.github/workflows/release.yml`, `images.yml` or
  `crates/loom-xtask/tests/release_check.rs`; the release commit of the wave that ships it moves
  README and the site's version lines too — inferred
- **Safety fact:** the existing test requires every step `run` of a non-`images` job in
  `release.yml` to be the release check (`release_check.rs:873-888`); calling `binaries.yml` with
  `uses:` keeps `release.yml`'s own jobs free of steps, so that assertion stands — cited
- **Safety fact:** `b10x-gates gh` scans its arguments before it starts `gh`
  (`~/beyond10x/gates/src/main.rs:204-241`), so a scratch path is scanned, the archive's bytes are
  not — cited
