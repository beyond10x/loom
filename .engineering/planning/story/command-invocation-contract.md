---
format: aep.planning-md/3
id: story:command-invocation-contract
kind: story
status: draft
title: The b10x-loom subcommands, flags and resume and approve exit statuses are a checked V1 contract
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
- depends_on: story:release-names-embedder-breaks
revision: 1
---
## Outcome

The way a program invokes `b10x-loom` is a stated, checked contract beside the run-events one: every
subcommand, each of its flags and positional arguments (name, whether it takes a value, its listed
values, whether it is required or repeatable, its default), and the exit statuses `approve` and
`resume` return. A committed snapshot, `docs/contracts/cli-v1.json`, is compared with the clap
definition by `task docs-check`, so a change that breaks a script or CI job calling the command
fails Loom's gate unless the snapshot moves with it, and a release that moves it cannot say
`Breaking: none.`

## Why

The `b10x-loom` command is one of Loom's two stable surfaces: it stays stable per release, and a
break is named in the CHANGELOG (decision-blocker `g6-consumption-surface`, cleared, option B).
`story:command-event-contract` made the `run --output jsonl` records and the run's exit statuses a
checked contract and left the invocation out. A program reading that stream first invokes
`b10x-loom run --workspace … --output jsonl …`, and the engineering factory (consumer C, served
first) drives `approve` and `resume` by their exit statuses. Today `website/docs/reference/cli.md`
is regenerated from the clap definition on every change, so a renamed flag or a re-numbered resume
status moves the page with it and nothing notices.

## Acceptance

`task docs-check` fails when the clap definition of `b10x-loom` (`b10x_loom_cli::Cli`) or the
`approve` and `resume` exit statuses differ from the committed contract in a way that breaks an
invoking program, and passes on additions such a program does not see:

- `crates/loom-docs/src/cli_contract.rs::the_v1_cli_contract_matches_the_clap_definition` (unit
  test) walks `Cli` through `clap::CommandFactory` and the statuses `approve_exit_status` and
  `resume_exit_status` decide, and asserts the snapshot equals them.
- `crates/loom-docs/src/cli_contract.rs::a_removed_or_renamed_subcommand_or_flag_is_a_break`,
  `an_optional_flag_made_required_is_a_break`, `a_narrowed_value_list_or_changed_default_is_a_break`,
  `a_changed_exit_status_is_a_break` and `a_new_optional_flag_or_subcommand_is_not_a_break` drive the
  comparison over hand-built definitions.
- `loom-docs generate` rewrites the snapshot; `loom-docs generate --check` (in `task docs-check`)
  reports each difference by subcommand and flag and says whether it is a break.
- `website/docs/reference/cli.md`, as `loom-docs` generates it, carries a "Stability" section
  stating the rule above and linking the snapshot and the run-events contract; `task website` exits 0.
- `release-check`'s set of stable-surface files (`story:release-names-embedder-breaks`) includes
  `docs/contracts/cli-v1.json`, and
  `crates/loom-xtask/tests/release_check.rs::release_check_refuses_breaking_none_when_the_cli_contract_changed_since_the_previous_tag`
  passes over a fixture repository whose two tags differ only in that file.
- `task check` exits 0 on the tree with the snapshot committed.

## ESS first

No behaviour change: the clap definition in `crates/loom-cli/src/lib.rs` and the exit-status
functions are read, not changed; the story adds a snapshot, a check and documentation. The command
line is not declared in `ess/` (its stream is, in `ess/intake/domains/events.yaml`, and stays as it
is), and the story introduces no noun, so `ess/` is untouched. The red step is
`the_v1_cli_contract_matches_the_clap_definition`, which fails before the snapshot exists.

## Domain relations

None. Subcommands and flags are clap definitions, not domain nouns, and no `ess/1` document is read
or changed.

## Depends on

`story:release-names-embedder-breaks`, whose surface-file set the fifth acceptance line extends.

## Not in scope

- Changing any subcommand, flag or exit status.
- The command's human output, its help prose and the `run --output jsonl` records and run exit
  statuses (`story:command-event-contract`).
- Exit statuses other than those `approve_exit_status` and `resume_exit_status` decide (`evaluate`,
  `protocols`, `plugin`), which stay as their help text states them.
- How a consumer installs the command at a tag.
- Any change in a consumer's repository; the agent platform (consumer A), which waits.

## Scope

Crates: `loom-docs`, `loom-xtask`.

- `docs/contracts/cli-v1.json` (new, written by `loom-docs generate`)
- `crates/loom-docs/src/cli_contract.rs` (new), `crates/loom-docs/src/main.rs`,
  `crates/loom-docs/src/cli.rs` (the "Stability" section of the generated page)
- `website/docs/reference/cli.md` (regenerated, never hand-edited)
- `crates/loom-xtask/src/release.rs`, `crates/loom-xtask/tests/release_check.rs`: the surface-file
  set and one test
- `AGENTS.md`: the snapshot in § Generated files and a row in § Checks that hold the rules;
  `CHANGELOG.md` (**Unreleased**)

Touches none of `crates/loom-intake-slice`, `crates/loom-governor`, `ess/`, `generated/`,
`Cargo.toml`, `Cargo.lock`. It reads `crates/loom-cli/src/lib.rs`: a running change to a flag turns
this check red until the snapshot is regenerated, which is the check working.
`crates/loom-xtask/src/release.rs` is also in `story:co-pinned-consumer-check`'s scope (one entry
each in one set); `AGENTS.md` and `CHANGELOG.md` are shared prose; merge them.

## Ready

Ready once `story:release-names-embedder-breaks` is implemented; it needs no answer from anybody.
