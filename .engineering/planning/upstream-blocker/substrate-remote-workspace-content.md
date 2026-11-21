---
format: aep.planning-md/3
id: upstream-blocker:substrate-remote-workspace-content
kind: upstream-blocker
status: open
title: No Substrate route carries a case workspace to a daemon on another host and back
summary: 'At 0.21.0: adoption on the daemon''s host, or per-file writes of at most 1 MiB with no mode; read-only roots are daemon-host paths'
tags:
- e2r4
relations:
- blocks: story:substrate-daemon-on-another-host
revision: 1
---
## What is missing

A way to give a Substrate daemon on another host a case workspace, with the read-only content a
run needs, and to read the changed workspace back. Read at tag `0.21.0` on 2026-11-21:

- A workspace reaches a daemon by adopting a directory already under the daemon's workspace root
  (`Client::adopt_workspace`, `crates/b10x-substrate-sdk/src/lib.rs:218-248`;
  `DaemonConfig.workspaces`, `crates/substrate-daemon/src/runtime.rs:458`), which needs the
  directory on the daemon's host.
- Or a client creates an empty workspace (`POST /v1/workspaces`,
  `crates/substrate-daemon/src/app/routes.rs:60`) and writes files into it one at a time
  (`Workspace::write_file`, SDK `lib.rs:911`; `PUT /v1/workspaces/{id}/files/{*path}`,
  `routes.rs:66-71`). Each file is at most 1 MiB (`MAX_FILE_BYTES`,
  `crates/substrate-wire/src/lib.rs:21`), and a write carries content only (`FileWriteInput { content }`,
  wire `lib.rs:1170-1172`), so no mode (an executable bit) and no symbolic link reach the daemon.
  Reading back is per file or as a tree listing (SDK `read_file`, `tree`, `git_changes`).
- A read-only root is a directory on the daemon's own host (`ReadOnlyRoot.host_path`, wire
  `lib.rs:4732-4737`); nothing carries read-only content from the client.
- A daemon's HTTPS listener needs `DaemonConfig.tls` and a hosted identity authority
  (`runtime.rs:513`, `:515`, paired at `:666`).

## What it blocks

`story:substrate-daemon-on-another-host`. `story:substrate-daemon-runner` serves a daemon on the
same host by adoption and does not wait on this.

## What clears it

A Substrate release in which a client can place a whole directory tree, with files over 1 MiB, file
modes and symbolic links, into a daemon workspace in one operation, name read-only content that is
not on the daemon's host, and read the changed tree back in one operation; or Substrate's statement
that the per-file route is the supported one, with the bounds it keeps, so that Loom's story uses
it as it is.

## Who

Not yet requested. Session `sdc` to send it to session `substrate`, as it sent the other items of
`epic:tree-daemon-under-enforced-limits` on 2026-11-21.
