---
format: aep.planning-md/3
id: story:confine-an-agent-tutorial
kind: story
status: draft
title: A reader confines an agent by following a tutorial that changes with the OS they pick, with no compilation
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:confinement-spec
revision: 1
---
## Outcome

A getting-started tutorial on the site, "Confine and sandbox an agent", that a reader follows
without compiling anything. At the start the reader picks Mac, Linux or Windows, and every tool and
command shown changes with the pick: bubblewrap on Linux, the macOS sandbox on Mac. Windows shows a
small robot crying and apologising, since there is no sandbox to show there. Loom is installed by
download; no page of the tutorial has a `cargo` command.

## Why

Timo, 2026-10-22, relayed by session `b10x-chef-orchestrator` (excerpts kept outside the repository):

> "OS chooser at the start (Mac / Linux / Windows); the toolset changes (bubblewrap on Linux, the
> macOS sandbox on Mac); Windows shows a small robot crying and apologising. For the agent SDK
> tutorial or Conductor's "how to confine it, how to sandbox it"."

> "this tutorial, it kind of must show how to download the thing. Yeah, we don't want to have
> compilation. By the way, that's a thing across all the documentation ... we don't want to have a
> cargo something command."

`website/docs/getting-started.md:19` installs with `cargo install --locked --path crates/loom-cli`
today.

## Acceptance

- `website/docs/guides/confine-an-agent.mdx` opens with one OS chooser (Mac, Linux, Windows); the
  choice switches every OS-specific block on the page, and the page states the default shown before
  a choice.
- Linux names bubblewrap (`/usr/bin/bwrap`) and the delegated user systemd scope the confined runner
  needs; Mac names the macOS sandbox; Windows shows the robot image and an apology, and no command.
- The install step downloads a released Loom binary for the chosen OS; `task website` fails on a
  `cargo ` command in the page (a check in `loom-docs`, `the_tutorial_has_no_cargo_command`).
- Every command on the page was run in the tree and its output pasted from that run (AGENTS.md
  § Documentation); the Mac commands are run on a Mac, or the Mac tab says it is not yet verified.
- `task website` exits 0.

## ESS first

No behaviour change: a documentation page and a docs check. `ess/` is untouched. The red step is
`the_tutorial_has_no_cargo_command` against a page that does not yet exist or still installs with
cargo.

## Open before it can be ready

- Download: Loom ships no binary today. `release.yml` verifies the tag and runs the check, and
  creates no release artifact (AGENTS.md § Releases, step 4). A download needs Loom release
  binaries per OS, which is not filed yet.
- Mac: Substrate has a macOS sandbox backend (`substrate-host/src/seatbelt.rs` in the Substrate
  checkout). Whether Loom's confined runner (`SubstrateRunner`) reaches it on a Mac: I don't know.

## Not in scope

The connectors tutorial; a Windows sandbox.

## Scope

`website/docs/guides/confine-an-agent.mdx` (new), a robot image under `website/static/`,
`website/sidebars`, `crates/loom-docs/src/` (the check), CHANGELOG.md.

## Ready

Waits on Loom release binaries (above), and on `story:confinement-spec` for the SDK calls the page
shows.
