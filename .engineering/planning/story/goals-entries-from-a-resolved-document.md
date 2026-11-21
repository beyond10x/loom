---
format: aep.planning-md/3
id: story:goals-entries-from-a-resolved-document
kind: story
status: draft
title: A file-backed GoalsEntries provider reads the resolved goals document the engine configuration names, for tree run and the daemon
summary: loom.engine/1 gains a goals section naming the sdc.goals/1 document sdc-goals resolve writes. The loader refuses a missing, unreadable, unresolved, other-format or over-1-MiB file. FileGoalsEntries in the governor reads the file on every check, with no network and no program, and answers digest, window and override per entry for rule D93. tree run and tree serve pass it to the tree's HeldCaseGrounds; without it a goals ground stays invalid_goals. Low priority
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:goals-grounds-and-free-answers
- depends_on: story:engine-configuration-document
- depends_on: story:tree-daemon
revision: 2
---
## Outcome

Through the binary a goals ground can be checked. A file-backed implementation of the host port
`GoalsEntries` (`story:goals-grounds-and-free-answers`), `FileGoalsEntries`, reads the resolved
goals document that the engine configuration names. `tree run` and the daemon pass it to the
tree's grounding check. Without it, a goals ground stays refused `invalid_goals` with the reason
`no goals port`, as story 23 says.

**The document Loom reads.** It is the output of `sdc-goals resolve --at <instant> <layer files>`
(`~/beyond10x/sdc/goals/src/main.rs:42`, `:240-256` at `00e3557`; `docs/requester.md:82-86`):

- one JSON document `{"format": "sdc.goals/1", "at": <instant>, "entries": [...]}` (`main.rs:254`,
  `goals/src/check.rs:9`);
- each entry with its fields as written, plus `layer` and `digest` (`main.rs:181-191`);
- only the entries in their window at `at` (`valid_from` <= `at` < `valid_until`), without a target
  that an entry in the window overrides, ordered company, product, request (`check.rs:428-443`).

SDC makes the document's sha256 the revision of the root case's artifact `goals`
(`docs/requester.md:20`).

Loom reads, per entry:

- `id`, `digest`, `valid_from`, the optional `valid_until` and the optional `overrides`;
- no other member, and it keeps the others as they are.

Loom reads the `digest` as written and computes none. A layer file (`format: sdc.goals/1` with
`layer` and no `at`) is not a resolved document.

**The engine configuration.** `loom.engine/1` (`story:engine-configuration-document`) gains an
optional section `goals` with one member, `document`, the path of the resolved goals document. A
relative path is resolved against the directory of the engine configuration file. The loader in
`crates/loom-tree/src/engine.rs` checks the section when it loads the file, before any case
opens. Each refusal is an `EngineRefusal` that names the path:

- `GoalsDocumentMissing`: no file at the path;
- `GoalsDocumentUnreadable`: the file cannot be read, or is not JSON (the detail names why);
- `GoalsFormatUnknown`: `format` is missing or is not `sdc.goals/1` (it names the value found);
- `GoalsDocumentNotResolved`: an `sdc.goals/1` document that is not `resolve`'s output. It names
  the member: `at` or `entries` missing, `layer` present, or an entry without `id` or `digest`;
- `GoalsDocumentTooLarge`: the file is over 1,048,576 bytes (1 MiB). The size is read before the
  file is, and the refusal names the size and the bound.

An undeclared member of the section is refused `MemberUndeclared`, as every section's is.

**When the file is read.** It is read on every call of the port, that is, once per goals
citation checked. Nothing is cached between two checks.

- A file that changed between two checks gives the second check the new content. A changed digest
  is then `digest_changed`, and a removed entry is `no_entry`.
- A file that, at a check, is missing, unreadable, not `sdc.goals/1`, not a resolved document, or
  over the bound answers that the document does not validate, so the ground is refused
  `invalid_goals`. The reason names the path and the cause.
- The daemon reads the file the same way, so a new `sdc-goals resolve` reaches a served tree
  without a restart. The engine configuration itself is not reloaded (`story:engine-configuration-document`,
  § Out of scope).

**What the port answers from the file**, for an entry id at the check's instant (the request's
`opened_at`, story 23):

- The entry is in the document: the state holds its `digest`, `valid_from` and `valid_until`.
  Story 23's `goals_verdict` then gives `digest_changed`, `not_yet_valid` or `expired` by comparing
  them with the citation and the instant.
- It is not, and an entry of the document that is in its window at the instant names it in
  `overrides`: the state names that entry as `overridden_by`.
- It is not, and nothing names it: the state has no entry, so the ground is refused `no_entry`.
- The document validates when it passes the load checks above at the time of the call.

Rule D93 holds as story 23 states it. A moved goals revision alone (the held case's `goals`
revision against the cited one) is the note `revision_moved`, not a refusal. This story adds no
revision of its own, and it does not hash the file.

**No network and no program.** Loom reads the file with the standard library's file calls only.
It opens no socket, and it runs no program: not `sdc-goals`, and not `$SDC_GOALS`.

**Who passes the provider:**

- `tree run --engine FILE` and `tree serve --engine FILE` (`story:tree-daemon` adds the flag to
  `tree serve`) build `FileGoalsEntries` from the `goals` section. They pass it to
  `with_goals_entries` in the `HeldCaseGrounds` build that `tree run` and the daemon share
  (story 23).
- `tree open --engine FILE` loads and refuses the section like the others, and checks no ground.
- `b10x-loom run` and `resume` do not read the engine configuration
  (`story:engine-configuration-document`, § Out of scope), so the slice through the binary passes
  no provider. A host that embeds the slice passes `FileGoalsEntries` through `RunOptions.goals`
  (story 23).
- The `decisions` commands run no grounding check, so they take no provider.

**Files:**

- `ess/domains/engine.yaml`, `ess/domains/decision.yaml`, and the regenerated `generated/rust/loom`.
- `crates/loom-governor/src/file_goals.rs` (new, beside the governor's file stores) and
  `crates/loom-governor/src/lib.rs` (its `mod` line).
- `crates/loom-tree/src/engine.rs` (the section and its refusals), and the `tree run` and daemon
  code that builds `HeldCaseGrounds`.
- The `tree` command arms of `crates/loom-cli` that load `--engine`.
- `website/docs/` (the guide page of `story:engine-configuration-document` gains the section), and
  `website/docs/reference/cli.md` (regenerated).
- Tests: `crates/loom-governor/tests/file_goals.rs` and `crates/loom-tree/tests/engine_goals.rs`.

## Why

Session `sdc` will drive Loom through the binary, so a goals ground must be checkable there. It
asked for one story, at low priority, for a file-backed `GoalsEntries` provider named in the
engine configuration, with rule D93. It accepts `invalid_goals` through the binary until this
story lands (`.engineering/drafts/e3-sdc-answers-11.md`, (c)). The resolved document is what SDC
already hands Loom (`~/beyond10x/sdc/docs/requester.md:185` at `00e3557`, "the host passes the
resolved goals document").

## Waits on

- `story:goals-grounds-and-free-answers`: `GoalsEntries`, `GoalsEntryState`, `goals_verdict`,
  `with_goals_entries` and the tree's port, which this story implements and fills.
- `story:engine-configuration-document`: `loom.engine/1`, its loader and `--engine`. It also says
  that each story adding a section depends on it.
- `story:tree-daemon`: `tree serve --engine` and the daemon.
- No edge orders this story against the other stories that add a section to
  `ess/domains/engine.yaml` (`story:confinement-level-on-oci-and-pod`,
  `story:executable-descendants-held`, `story:ingress-routes-per-action`,
  `story:substrate-daemon-on-another-host`, `story:capacity-given-to-an-admission-program`,
  `story:agent-session-through-substrate`, `story:workspace-file-bound-in-engine-configuration`):
  each adds a section of its own and reads none of the others', so the order between them does
  not matter. They share the file, the typed scope names it, and `aep plan artifact waves` keeps
  two stories with one path out of one wave. (The coordinator removed seven such edges on
  2026-11-21: five of those stories are drafts of an epic nobody has accepted, and an edge to
  them would hold this story for no reason of order.)

  The other editors of `ess/domains/engine.yaml` come before it through story 23: story 22, then
  `story:concurrent-cases-in-the-tree-daemon`, `story:tree-daemon` and the chain under it down to
  `story:engine-configuration-document`.

## ESS first

`ess/domains/engine.yaml`:

- `EngineConfiguration` gains the optional `goals`, a `GoalsSection` with `document` (a path).
- `EngineRefusal` gains:
  - `GoalsDocumentMissing` (path);
  - `GoalsDocumentUnreadable` (path, detail);
  - `GoalsFormatUnknown` (path, found);
  - `GoalsDocumentNotResolved` (path, member);
  - `GoalsDocumentTooLarge` (path, bytes, bound).

`ess/domains/decision.yaml` gains the shape Loom reads:

- `ResolvedGoalsDocument`: `format`, `at`, `entries`;
- `ResolvedGoalsEntry`: `id`, `layer`, `digest`, `valid_from`, optional `valid_until`, optional
  `overrides`. The entry's other members are kept and not read.

On that commit `task drift` fails, and `crates/loom-governor/tests/file_goals.rs` and
`crates/loom-tree/tests/engine_goals.rs` do not compile.

## Acceptance

In `crates/loom-tree/tests/engine_goals.rs`, the loader, over `loom.engine/1` files whose `goals`
section names:

- a path with no file: refused `GoalsDocumentMissing` naming the path;
- a file holding `{`: refused `GoalsDocumentUnreadable` naming the path;
- a file with mode `000` (the test skips when it runs as root): refused `GoalsDocumentUnreadable`
  naming the path;
- a resolved document with `format: sdc.goals/2`: refused `GoalsFormatUnknown` naming
  `sdc.goals/2`;
- a layer file (`format: sdc.goals/1`, `layer: product`, no `at`): refused
  `GoalsDocumentNotResolved` naming `at`;
- a resolved document of 1,048,577 bytes: refused `GoalsDocumentTooLarge` naming the path,
  `1048577` and `1048576`;
- a resolved document of exactly 1,048,576 bytes: loaded.

Further loader and command bullets in the same file:

- A section with `documnet` is refused `MemberUndeclared` naming `goals.documnet`.
- `tree open --engine FILE` with a section the loader refuses exits non-zero naming the refusal,
  and writes no case file.

In `crates/loom-governor/tests/file_goals.rs`, `FileGoalsEntries` over a fixture resolved document
`F` (`at: 2026-11-21T00:00:00Z`), and story 23's `goals_verdict` with the request opened at
`2026-11-21T12:00:00Z`. `F`'s entries:

- `company/c1`: digest `sha256:aa`, valid from `2026-11-01` to `2026-12-01`;
- `product/p2`: valid from `2026-11-01`, with `overrides: company/c3`.

The bullets:

- `goals@g3#company/c1` with digest `sha256:aa` is verified with no note.
- `goals@g3#company/c9` is refused `[no_entry]`.
- `goals@g3#company/c1` with digest `sha256:bb` is refused `[digest_changed]`.
- With the request opened at `2026-10-31T23:59:59Z`, `company/c1` is refused `[not_yet_valid]`.
- With the request opened at `2026-12-01T00:00:00Z`, `company/c1` is refused `[expired]`.
- `goals@g3#company/c3` is refused `[overridden]`, and the state names `product/p2`.
- With `F` replaced after the provider was built by a file holding `{`, the next check refuses
  `company/c1` `[invalid_goals]`, and the reason names `F`'s path.
- With `F` replaced by a file of 1,048,577 bytes, the next check refuses it `[invalid_goals]`
  naming the bound.
- Two checks of `company/c1`, with `F` rewritten between them so that its digest is `sha256:cc`:
  the first state answers `sha256:aa` and the second `sha256:cc`.
- With `PATH` set to a directory holding a recording `sdc-goals`, and `SDC_GOALS` naming that
  program, a check answers as above, and the program records no call.

In `crates/loom-tree/tests/engine_goals.rs`, on the three-level fixture tree of
`story:tree-run-command`, with a recorded decider reply citing `goals@<goals revision>#company/c1`
with digest `sha256:aa`, and no model call:

- `tree run --engine FILE`, with `FILE`'s `goals.document` naming `F`, records the recommendation
  verified, and `decisions show` prints the `grounding_report` `1 grounds, all resolve`.
- With the root case's `goals` artifact at a later revision than the cited one, the same run
  records it verified, and the `grounding_report` holds `revision_moved`.
- With `F`'s `company/c1` digest changed to `sha256:bb`, the run's request carries no
  recommendation, and `no_recommendation_reason` names `digest_changed`.
- `tree run` without `--engine`, and with an engine file that has no `goals` section, both refuse
  the ground `invalid_goals` naming `no goals port`.
- With `b10x-loom tree serve --engine FILE` serving the store, the served tree's request records
  the recommendation verified.

Last:

- `task docs-check` exits 0, with `website/docs/reference/cli.md` regenerated.
- `task check` exits 0.

## Out of scope

- Reading the goals layer files, running `sdc-goals`, or computing an entry's digest.
- The engine configuration in `b10x-loom run` and `resume`.
- Reloading a changed engine configuration in a running daemon.
- A cache of the goals document between checks.
- A per-file bound for workspace files (`story:workspace-file-bound-in-engine-configuration`).
