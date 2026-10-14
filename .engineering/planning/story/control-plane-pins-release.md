---
format: aep.planning-md/3
id: story:control-plane-pins-release
kind: story
status: archived
title: control-plane runs its agent through loom_sdk pinned to a Loom release tag
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
revision: 2
transitions:
- {from: "draft", to: "archived", at: "2026-10-14T02:54:27Z", actor: "human:timo", revision: 2}
---
## Outcome

control-plane, the beyond10x repository where the engineering factory's integration is proved and
whose results move to the factory, depends on Loom by a release tag (`tag = "<version>"`) instead
of a commit, resolves one copy of each Loom crate, runs its agent through `loom_sdk` on that tag,
and its CI exercises that path.

## Why

The engineering factory is served first, and control-plane is its integration repository. It
already embeds `loom_sdk` (`crates/control-plane-runtime/Cargo.toml`), but pinned to commit
`1b25fe89`, which precedes Loom 0.10.0: it cannot move to a release without crossing every break
from 0.10.0 to the current tag, and nothing tells it which. This story is the epic's first two
acceptance lines for the factory, proved where its integration is proved.

## Acceptance

control-plane's `check.yml` builds and tests its runtime on a Loom release tag, with one copy of
each Loom crate:

- `crates/control-plane-runtime/Cargo.toml` names `b10x-loom-sdk` with `git =
  "https://github.com/beyond10x/loom"` and `tag = "<the current Loom release>"`, no `rev`, and no
  other Loom package.
- control-plane's `crates/control-plane-xtask/tests/foundation_check.rs` gains
  `foundation_check_refuses_a_loom_pin_that_is_not_a_tag` and
  `foundation_check_refuses_two_copies_of_a_loom_crate` (over `cargo tree --duplicates`), and both
  pass on control-plane's tree.
- An existing control-plane test that runs an agent through `loom_sdk::run_until_blocked` with
  `loom_sdk::Loom` passes on the tag (named in the pull request); `task check` in control-plane
  exits 0 and its CI is green.
- Each Loom break crossed on the way (for example `CanonGovernor::open` taking `CaseInputs` since
  0.20.0) is fixed in control-plane without patching Loom; a break the CHANGELOG did not name is
  recorded as a Loom defect story, not patched around.

## ESS first

No behaviour change in Loom: the work is in control-plane. control-plane's own rule for spec-first
work applies there; where the move changes a control-plane noun, its `ess/` changes first.

## Domain relations

None in Loom's specification.

## Not in scope

- Any change to Loom's code. A gap control-plane hits becomes its own Loom story.
- The agent platform: its needs wait until the engineering factory is served.
- Moving results from control-plane into the factory.

## Scope

In control-plane (beyond10x/control-plane): `crates/control-plane-runtime/Cargo.toml`,
`Cargo.lock`, the `crates/control-plane-runtime/src/` files that import `loom_sdk`
(`engine.rs`, `fleet.rs`, `governance.rs`, `loom_model.rs`),
`crates/control-plane-xtask/src/foundation.rs`,
`crates/control-plane-xtask/tests/foundation_check.rs`.

In Loom: nothing. Touches none of `crates/loom-intake-slice`, `crates/loom-governor`,
`crates/loom-cli`, `ess/intake/`, `generated/rust/intake`, `crates/loom-mandate`,
`ess/domains/authority.yaml`, `generated/rust/loom`, `Cargo.toml`, `Cargo.lock` of Loom.

## Ready

Ready now: it can pin the current release (0.20.0). Pinning a release that already carries
`story:sdk-host-port-stability` is better, but not required.
