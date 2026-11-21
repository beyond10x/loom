---
format: aep.planning-md/3
id: upstream-blocker:substrate-executable-allow-list
kind: upstream-blocker
status: open
title: Substrate has no executable allow-list on linux, oci and pod, and no deny-list on any adapter
tags:
- sdc
relations:
- blocks: story:executable-allow-list
revision: 2
---
## What is missing

A control in Substrate over which executables a confined run may execute. Read at tag `0.21.0`
on 2026-11-21: `ConfinementProfile.exec_subpaths` ("Subpaths exec is admitted beneath, beyond
the literal program", `crates/substrate-host/src/confinement/profile.rs:71-72`) is proved by the
`macos` adapter only (`macos.rs:3-35`); `linux` refuses a profile that uses it
(`linux.rs:29-31`); the `linux` seccomp filter covers `socket(2)` only
(`crates/substrate-host/src/seccomp.rs:35-40`); `oci` and `pod` have a read-only image pinned by
digest and nothing for the workspace; no adapter has a deny-list. No story in Substrate's
planning store plans it.

## What it blocks

Only the part of `story:executable-allow-list` that holds a started program's descendants to the
lists. The check of the program Loom itself starts does not wait on this.

## What clears it

A Substrate release in which the `linux` adapter (and `oci`, `pod`) proves an executable
allow-list and a deny-list as facts a profile can require.

## Who

Session `sdc` sent the request to session `substrate` on 2026-11-21. That session answered the
same day with its stories, all drafts under Substrate's
`epic:runs-are-governed-and-limit-hits-are-events`, uncommitted on its branch
`plan/sdc-run-governance`, none accepted or implemented: `story:linux-executable-allow-list`,
`story:executable-deny-list`, `story:executable-lists-on-oci-and-pod` and
`story:exec-start-carries-executable-lists` (the wire).
