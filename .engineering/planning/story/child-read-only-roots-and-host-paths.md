---
format: aep.planning-md/3
id: story:child-read-only-roots-and-host-paths
kind: story
status: active
title: A confined child gets read-only roots and files where the host mounts them, and host paths map inside
summary: read-only roots and files, sandbox_path (takt G4-G5)
relations:
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T00:08:44Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-22T00:08:44Z", actor: "human:timo", revision: 4}
---
## Outcome

A host gives a confined child read-only host directories and files, each mounted where the host
names (its own host path included), and translates a host path to the path the child sees, so
programs installed under the user's home, a policy file and a linked worktree's git directory
resolve inside. Closes G4 and, within what Substrate 0.7.18 offers, G5 of takt's
`dependency-blocker:loom-child-confinement`.

## Why

On 0.32.0 (`a238395b`, `crates/loom-intake-slice/src/confinement.rs`):

- G4: a child gets no read-only roots or files (`read_only_roots: Vec::new()` at `:898`,
  `read_only_files` at `:899`, no setter among `:719-823`). The sandbox sees `/usr`, `/bin`,
  `/lib`, `/lib64`, `/proc`, `/dev`, a tmpfs `/tmp` and the workspace.
- G5: the workspace is mounted at `/workspace`; a host path does not resolve inside.

Substrate 0.7.18 (`substrate-wire` `ReadOnlyRoot { host_path, mount }`, `ReadOnlyFile
{ host_path, mount }`) lets the caller choose each mount: at most 4 roots and 16 files; a mount may
not equal, sit beneath or contain `/`, `/usr`, `/bin`, `/lib`, `/lib64`, `/proc`, `/dev`, `/tmp`,
`/workspace`, `/runtime`, `/scratch`, `/etc/hosts` or `/etc/ssl/certs/ca-certificates.crt`; roots,
files and mounts may not overlap; a symlink is refused, never followed; files need the
`exec.read-only-files` capability. The workspace is always at `/workspace` and the child always
starts there (bubblewrap `--chdir /workspace`); Substrate has no working-directory field and no
writable mount outside the workspace.

## Design

- ESS first, `ess/intake/domains/confinement.yaml`: `intake.confinement.ReadOnlyFile
  { host_path, mount }` beside `ToolchainRoot`; `AppliedConfinement` gains
  `read_only_files: List<intake.confinement.ReadOnlyFile>`.
- `ChildCommand::with_read_only_root(host_path, mount)` and `with_read_only_file(host_path, mount)`;
  `ChildCommand::at_host_path(host_path)` is shorthand for a mount equal to the canonical host path.
  Before anything starts, Loom refuses `ScopeInvalid`, naming the path: a host path that is not
  absolute, not UTF-8, not canonical (a `.`, `..` or symlink component), absent, a symlink, or not
  a directory (root) / regular file (file); a mount on or under or containing a reserved Substrate
  mount; overlapping or duplicate mounts; more than 4 roots or 16 files; a root or file that
  overlaps the workspace directory or a certificate bundle the child names. Files on a driver that
  does not publish `exec.read-only-files` are refused `CapabilityUnserved`.
- After the run, the applied confinement must name exactly the roots and files requested; a record
  naming another set is refused, as for writable scopes (`run_child` and the live child alike, once
  `story:live-confined-child` landed).
- `ChildCommand::sandbox_path(&Path) -> Option<PathBuf>`: a host path inside the workspace
  directory maps to `/workspace/<relative>`; one inside a read-only root or equal to a read-only
  file maps to its mount; anything else is `None`. The host uses it to write a child's arguments
  (`--git-dir=…`, a transcript path) and documents that the child starts in `/workspace`.
- Known limit, written in AGENTS.md and the CHANGELOG line: a writable path outside the workspace
  (a linked worktree's common git directory that git must write) is not offered by Substrate
  0.7.18; such a directory can be mounted read-only only.

## ESS first

The declarations above; `task intake-generate` regenerates. Red tests in
`crates/loom-intake-slice/tests/confined_child.rs` (delegated cases, skipping only on a named
missing prerequisite) and unit tests in `confinement.rs` for the refusals.

## Acceptance

- `a_read_only_root_at_its_host_path_is_readable_and_not_writable`: a temp directory under the
  test's `target/` mounted at its host path; the child reads a file there and its write fails.
- `a_read_only_file_is_mounted_where_named`.
- `a_mount_under_a_reserved_path_is_refused_before_launch` (`/usr/x`, `/workspace/x`, `/tmp/x`).
- `overlapping_mounts_are_refused_before_launch`, `a_fifth_root_is_refused_before_launch`.
- `a_symlinked_or_absent_host_path_is_refused_naming_it`.
- `sandbox_path_maps_the_workspace_and_each_root`: workspace file → `/workspace/…`, root file →
  its mount, other → `None`.
- `a_record_naming_other_read_only_roots_is_refused` (unit test on the shared check).
- Every existing `confined_child.rs` case stays green.

## Not in scope

- A writable mount outside the workspace, a working directory other than `/workspace`: not in
  Substrate 0.7.18.
- Live children and output (`story:live-confined-child`).

## Scope

- `ess/intake/domains/confinement.yaml`, `generated/rust/intake/`
- `crates/loom-intake-slice/src/confinement.rs`
- `crates/loom-intake-slice/tests/confined_child.rs`
- `crates/loom-sdk/src/lib.rs` if new types are re-exported
- `AGENTS.md` (the `run_child` paragraph), `CHANGELOG.md`
