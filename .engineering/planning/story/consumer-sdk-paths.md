---
format: aep.planning-md/3
id: story:consumer-sdk-paths
kind: story
status: draft
title: loom-xtask consumer-paths reads the loom_sdk paths a consumer names and reports those the stability page does not list
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

The "Used by control-plane" table of the SDK stability page is re-read from the consumer by a
tool, not kept by hand. `loom-xtask consumer-paths --consumer <dir>` parses every `.rs` file under
a consumer's tree, collects each `loom_sdk` path it names (`use` trees with their groups and
renames, paths written inline, and items named through a `loom_sdk` module it imported, as
control-plane names `wire::Item`), and compares them with the page's table: it lists each path the
consumer names that no row covers and each row the consumer no longer names, and exits non-zero
when either list is non-empty.

## Why

The epic asks for the SDK surface each consumer needs to be read from the consumer, not assumed.
`story:sdk-host-port-stability` read control-plane's imports once, by hand, into the page's table,
and `crates/loom-sdk/tests/host_ports.rs::every_control_plane_path_on_the_page_resolves` holds that
every row resolves. Nothing notices when control-plane, the engineering factory's integration point
(served first; cleared `decision-blocker:g6-first-consumer`), starts naming a path the table does
not list: the path is then outside the promise and outside `release-check`'s view, though the
consumer depends on it. Re-reading by hand at each release does not scale to a second consumer.

## Acceptance

`loom-xtask consumer-paths` reports exactly the paths a consumer tree names that the page's table
does not cover, and the rows it no longer names:

- `crates/loom-xtask/tests/consumer_paths.rs::a_path_the_consumer_names_and_the_page_omits_is_reported`
  and `a_row_the_consumer_no_longer_names_is_reported` run the command over fixture trees under
  `crates/loom-xtask/tests/fixtures/` and assert the listed path and a non-zero exit.
- `crates/loom-xtask/tests/consumer_paths.rs::grouped_renamed_inline_and_module_relative_paths_are_read`
  asserts `use loom_sdk::{a::B, c::{D as E}}`, an inline `loom_sdk::f::G`, and `wire::Item` after
  `use loom_sdk::loom::harness::wire;` are each read as their full `loom_sdk` path.
- `crates/loom-xtask/tests/consumer_paths.rs::an_associated_item_is_covered_by_its_type`: a consumer
  naming `loom_sdk::Loom::new` or an enum variant is covered by the row of its type, as the page
  states.
- `crates/loom-xtask/tests/consumer_paths.rs::the_page_table_matches_the_witness`: the command run
  with the page's own rows as a consumer tree (the `use` lines of `host_ports.rs`) exits 0.
- Run over a checkout of control-plane at the revision named in the pull request, the command exits
  0, or the table is updated in the same change until it does; the run is recorded in the pull
  request. `task check` exits 0.
- `website/docs/reference/sdk-stability.md` says the table is read with this command and names the
  control-plane revision it was last read at.

## ESS first

No behaviour change for the runtime or the command: a read-only developer tool and documentation.
No ESS noun is involved, so `ess/` is untouched. The red step is
`a_path_the_consumer_names_and_the_page_omits_is_reported`, which fails before the subcommand
exists.

## Domain relations

None. A consumer path and a page row are not nouns of any `ess/1` document in this repository, and
the story introduces none.

## Open questions

- None that stop this story. The page holds one consumer table today; a table for the agent
  platform (consumer A) waits with that consumer (cleared `decision-blocker:g6-first-consumer`), and
  the command takes the table to compare with as an argument only when a second one exists.

## Not in scope

- Running the command in Loom's CI against a consumer's repository: Loom's gate reads no other
  repository, so the control-plane run is recorded, not gated.
- Paths of packages beside `loom_sdk` (Canon, llm), which `story:co-pinned-consumer-check` covers.
- Adding or removing rows for any reason other than what the read reports; widening the promise.
- Any change in control-plane.

## Scope

Crates: `loom-xtask`.

- `crates/loom-xtask/src/consumer_paths.rs` (new), `crates/loom-xtask/src/main.rs`: the
  `consumer-paths` subcommand (clap derive)
- `crates/loom-xtask/tests/consumer_paths.rs` (new), `crates/loom-xtask/tests/fixtures/consumer-*/`
  (new)
- `website/docs/reference/sdk-stability.md`: the "Used by control-plane" introduction and any row
  the read changes
- `crates/loom-sdk/tests/host_ports.rs`: one `use` per row the read changes, only when it changes one
- `AGENTS.md`: a row in § Checks that hold the rules; `CHANGELOG.md` (**Unreleased**)

Uses `syn`, already a dependency of `loom-xtask`. Touches none of `ess/`, `generated/`,
`crates/loom-xtask/src/release.rs`, `crates/loom-intake-slice`, `crates/loom-governor`,
`crates/loom-cli`, `Cargo.toml`, `Cargo.lock`. A change to `host_ports.rs` is one
`release-check` counts as a stable-surface change; `sdk-stability.md`, `AGENTS.md` and `CHANGELOG.md` are shared
prose; merge them.

## Ready

Ready now; it needs no answer from anybody.
