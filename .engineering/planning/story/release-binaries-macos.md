---
format: aep.planning-md/3
id: story:release-binaries-macos
kind: story
status: proposed
title: The release carries Apple binaries for both Mac architectures, with the limit that they run unconfined
summary: aarch64 and x86_64 apple-darwin archives from macos-15 runners, check-first gate, local adapter only, no signing; Windows is a named non-goal
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:release-binaries
scope:
- confidence: cited
  path: .github/workflows/binaries.yml
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-cli/tests/
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T05:04:59Z", actor: "agent:loom", revision: 5}
---
## Outcome

Every Loom release that carries the Linux binary of `story:release-binaries` also carries
`b10x-loom` for `aarch64-apple-darwin` and `x86_64-apple-darwin`:

- Each target's binary comes in a `.tar.gz` archive with a `.sha256` file beside it, under the
  same naming: `b10x-loom-<version>-<target>.tar.gz` holding `b10x-loom-<version>-<target>/` with
  `b10x-loom` and `LICENSE`.
- It is built read-only in the same `binaries.yml`, kept as part of the same workflow artifact
  `b10x-loom-<version>`, and attached and verified by the bot in the same steps.

A macOS binary confines nothing:

- Loom admits only Substrate's `linux` adapter and its `local` adapter (`engine.rs:90-120`).
  Off Linux, `admitted_facts` refuses every confined driver as `BackendMissing`, "Substrate's
  linux adapter serves Linux only" (`confinement.rs:3471-3480`).
- On macOS, `--confinement substrate` therefore stops `ConfinementUnavailable`, and only the
  `local` adapter runs, unconfined and declared insecure. Substrate's Seatbelt `macos` adapter
  is not reachable from Loom.
- This limit is written on the install page, in `README.md` and in `--help`, because the help
  already names the adapter (`crates/loom-cli/src/lib.rs:356-358`, `:1101-1102`;
  `tree.rs:122`).

The binaries are not signed or notarized. Signing and notarization are out of scope.

## Why

Timo, typed on 2026-11-22: "if it's cheap to add, then let's also add Apple and Windows binaries."

macOS is cheap:

- `check.yml` already builds and tests `b10x-loom-sdk` and `b10x-loom-intake-slice` on
  `macos-15` with rustc `1.99.1` (`.github/workflows/check.yml:106-130`). That build includes
  `b10x-substrate-host` with its `git` feature, and through it `curl`, `git2` and the C crates.
- A cross check from Linux with rustc 1.99.1 reached 17 crates with no rustc error. It stopped
  at four C crates that need an Apple C toolchain, an OpenSSL and a zlib for the target: `ring`,
  `libz-sys`, `openssl-sys` and `psm`. A macOS runner has the first and the SDK's zlib. In
  `git2` and `curl-sys`, `openssl-sys` is taken only off macOS (inferred from those crates'
  manifests; the first gate proves it).
- `b10x-loom` itself was not compiled: "compiles as is" is not established; the first gate is.

Windows is not cheap. It is a named non-goal of this story, for these reasons:

- A check for `x86_64-pc-windows-msvc` gave 8 rustc errors in 3 places
  (`crates/loom-governor/src/locked_dir.rs:197`, which uses `std::os::unix`; the `loom-boundary`
  tests; `b10x-tool-toolchain` 0.5.0 in the tools repository, `src/lib.rs:34`) and 5 C-crate
  failures.
- `b10x-substrate-host` is Linux and Unix only (`lib.rs:63`, `confinement/local.rs:25`).
- A port needs a Windows driver in Substrate, a change in the tools repository, a Job Object in
  place of process groups, and cfg-gating of about 13 production files and 14 test crates.

## Design

1. **First gate.** Before any workflow change, run `cargo check -p b10x-loom-cli --locked
   --target <t>` on `macos-15` for both targets, as a job of `check.yml` beside `macos`. The
   unit reports both results. A rustc error is fixed here when it is a cfg-gate of a few lines;
   anything larger stops the story and is reported, with the error and the file.
2. **Build.** `binaries.yml` gains one job per target, each `runs-on: macos-15`,
   `permissions: contents: read`, `timeout-minutes: 60`:
   - checkout with `persist-credentials: false`, and `dtolnay/rust-toolchain` at the pin and
     toolchain `check.yml:116-118` uses, with the target added;
   - `cargo build --release --locked -p b10x-loom-cli --bin b10x-loom --target <t>`;
   - the archive as named above;
   - `shasum -a 256 <archive> > <archive>.sha256`, which writes the same `<64 hex>  <file name>`
     line as `sha256sum`.
   `x86_64-apple-darwin` is built from the arm64 runner with `--target` (inferred to work through
   Xcode's clang). If the first gate refuses that, the job moves to an Intel macOS runner instead.
3. **Keep.** Each job uploads its two files to the artifact `b10x-loom-<version>`. The Linux
   job's files are kept in the same artifact, with the `actions/upload-artifact` pin of
   `story:release-binaries`, `merge-multiple` or one artifact per target; the unit picks the
   form, and `AGENTS.md` names it.
4. **Attach and verify.** These are the steps of `story:release-binaries`, run over six files in
   place of two. A release is finished only when `sha256sum -c` prints `OK` for all three
   archives after `gh release download`.
5. **Install on macOS.** `README.md` and `website/docs/getting-started.md` give:

   ```sh
   f=b10x-loom-$V-aarch64-apple-darwin.tar.gz
   gh release download "$V" --repo beyond10x/loom --pattern "$f" --pattern "$f.sha256"
   shasum -a 256 -c "$f.sha256" && tar -xzf "$f" && "./${f%.tar.gz}/b10x-loom" --version
   ```

   For Gatekeeper, an archive fetched with a browser carries the quarantine mark, and the
   unsigned binary is then refused. The one-line workaround is
   `xattr -d com.apple.quarantine ./b10x-loom-<version>-<target>/b10x-loom`. `gh` and `curl` set
   no mark. A signed build is out of scope.

## ESS first

Exempt: no declared domain changes (workflows, help text, documents, tests). The red step is the
new tests below, failing before the workflow and the documents change.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network. The
tests read the tree's files only and run in the gate's test step (`slot task check`). None
builds, downloads or calls GitHub. They go in `crates/loom-xtask/tests/release_check.rs`, unless
noted.

- `binaries.yml` has a job per macOS target with `runs-on: macos-15`, `permissions` exactly
  `contents: read`, a build with `--locked` and `--target <t>`, the archive name
  `b10x-loom-${{ inputs.version }}-<t>.tar.gz` with a `.sha256` beside it, and an upload into the
  artifact `b10x-loom-${{ inputs.version }}`. Test:
  `binaries_workflow_builds_both_macos_targets_read_only`.
- No workflow is granted `contents: write`, the new jobs included. Test:
  `no_workflow_is_granted_contents_write`, of `story:release-binaries`.
- `check.yml` runs `cargo check -p b10x-loom-cli --locked --target` for both macOS targets on
  `macos-15`, with the toolchain its `macos` job pins, as the binaries jobs do. Tests:
  `the_check_workflow_checks_the_cli_for_both_macos_targets`,
  `the_macos_binaries_use_the_toolchain_the_macos_check_pins`.
- `README.md` and `website/docs/getting-started.md` each hold the macOS download of the
  workspace version with `shasum -a 256 -c`, the `xattr` line, and the sentence that a macOS
  binary runs only the `local` adapter and confines nothing. Test:
  `readme_and_site_name_the_macos_download_and_its_limit`.
- `b10x-loom tree run --help` and `b10x-loom run --help` say that `substrate` confinement is
  Linux only. Test: `the_help_says_substrate_confinement_is_linux_only`, in
  `crates/loom-cli/tests/`, running the built binary's `--help`.
- `AGENTS.md` § Releases names the six assets and the `shasum` command. Test:
  `agents_md_releases_names_the_six_assets`.
- `CHANGELOG.md` `[Unreleased]` `### Added` names both targets and the limit.

These can only be proved on the forge, after the pull request, and the unit reports them as open
until then:

the first gate (`cargo check` green on `macos-15` for both targets); a `binaries.yml` run green
for all three targets; the attached archives verifying with `sha256sum -c`; and `--version`
printing `b10x-loom <V>` on an arm64 Mac and on an Intel Mac or under Rosetta 2.

## Out of scope

- Windows (`x86_64-pc-windows-msvc`), a named non-goal; the reasons are in § Why.
- Signing and notarization; reaching Substrate's Seatbelt `macos` adapter from Loom.
- A universal (`lipo`) binary, Homebrew, and a registry publish.

## Scope

Read at `1016c251`. Every line is cited or inferred.

- `.github/workflows/binaries.yml`, new in `story:release-binaries`: inferred (not yet there).
- `.github/workflows/check.yml` `:106-130` and `crates/loom-xtask/tests/release_check.rs`. Cited.
- `crates/loom-cli/src/lib.rs:353-359`, `:1096-1106`, and `crates/loom-cli/src/tree.rs:122`:
  the help text. Cited. The test file in `crates/loom-cli/tests/` is inferred.
- `crates/loom-intake-slice/src/confinement.rs:3471-3480` and `confinement/engine.rs:90-120`.
  Read only, cited.
- `README.md` `:170-185`, `website/docs/getting-started.md` `:15-25`, `AGENTS.md`, `CHANGELOG.md`.
  Cited.

Edges: depends_on `story:release-binaries`, and collides with it on `binaries.yml`,
`release.yml` and `release_check.rs`; collides with `story:readme-logo-and-badges` on
`README.md`.
