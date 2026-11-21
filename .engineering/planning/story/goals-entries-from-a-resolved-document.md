---
format: aep.planning-md/3
id: story:goals-entries-from-a-resolved-document
kind: story
status: proposed
title: A file-backed GoalsEntries provider reads the resolved goals document the engine configuration names, for tree run and the daemon
summary: loom.engine/1 gains a goals section naming the sdc.goals/1 document that sdc-goals resolve writes. The loader refuses a missing, unreadable, other-format, unresolved or over-1-MiB file. FileGoalsEntries in the governor reads the file on every check, with no network and no program. It refuses invalid_goals unless the file's sha256, written as t plus 16 hex digits, is the root case's goals revision, and it answers digest, window and override per entry for rule D93, within what a document resolved at its own instant can tell. tree run and tree serve pass it to the tree's HeldCaseGrounds; without it a goals ground stays invalid_goals. Low priority
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:goals-grounds-and-free-answers
- depends_on: story:engine-configuration-document
- depends_on: story:tree-daemon
scope:
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/tests/engine_goals.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/goals-tree
- confidence: cited
  path: crates/loom-governor/src/file_goals.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/file_goals.rs
- confidence: cited
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: inferred
  path: crates/loom-tree/src
- confidence: cited
  path: crates/loom-tree/src/engine.rs
- confidence: cited
  path: crates/loom-tree/tests/engine_goals.rs
- confidence: cited
  path: ess/domains/decision.yaml
- confidence: cited
  path: ess/domains/engine.yaml
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T04:44:13Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

Through the binary a goals ground can be checked. A file-backed implementation of the host port
`GoalsEntries` (`story:goals-grounds-and-free-answers`), `FileGoalsEntries`, reads the resolved
goals document that the engine configuration names. `tree run` and the daemon pass it to the
tree's grounding check. Without it, a goals ground stays refused `invalid_goals` with the reason
`no goals port`, as `story:goals-grounds-and-free-answers` says.

**The document Loom reads.** It is the output of `sdc-goals resolve --at <instant> <layer files>`
(`~/beyond10x/sdc/goals/src/main.rs:42`, `:240-256` at `00e3557`; `docs/requester.md:82-86`):

- one JSON document `{"format": "sdc.goals/1", "at": <instant>, "entries": [...]}` (`main.rs:254`,
  `goals/src/check.rs:9`);
- each entry with its fields as written, plus `layer` and `digest` (`main.rs:181-191`);
- only the entries in their window at `at` (`valid_from` <= `at` < `valid_until`), without a target
  that an entry in the window overrides, ordered company, product, request (`check.rs:428-443`).

Loom reads, per entry:

- `id`, `digest`, `valid_from`, the optional `valid_until` and the optional `overrides`;
- no other member, and it keeps the others as they are.

Loom reads the `digest` as written and computes no entry digest. A layer file
(`format: sdc.goals/1` with `layer` and no `at`) is not a resolved document.

**Which document: the root case's `goals` revision.** Session `sdc` decided this
(`.engineering/drafts/w3-sdc-approval.md`).

- The resolved goals file is held by the root case. SDC's intake and direction cases hold
  byte-identical copies.
- SDC writes an artifact's revision as the letter `t` followed by the first 16 hex digits of the
  sha256 of its content (`revision_of`, `~/beyond10x/sdc/driver/src/main.rs:191-199` at
  `00e3557`). For example, sha256 `ea61f9b0090dfa65…` is revision `tea61f9b0090dfa65`.
- On every call, `FileGoalsEntries` hashes the bytes of the document file it read (sha256) and
  forms `t` and the first 16 hex digits. It compares that, in that form and never as the full
  digest, with the current revision of artifact `goals` of the tree's root case.
- `FileGoalsEntries` takes that revision from a Rust-only callback, `RootGoalsRevision`, that
  answers the root case's `goals` revision or nothing. `crates/loom-tree` implements it over the
  tree's governor: the tree's root case id and `CanonGovernor::revisions`
  (`crates/loom-governor/src/lib.rs:1096`).
- A mismatch answers that the document does not validate, so the ground is refused
  `invalid_goals`. The reason holds both values:
  `goals document is revision t<16 hex>, the root case's goals revision is <revision>`.
- A root `goals` revision written as `none` (SDC's `revision_of` for an absent file,
  `driver/src/main.rs:191-199`; `story:bound-artifact-revisions` for an absent bound path) is a
  revision, not a missing artifact: it gives the mismatch reason above,
  `goals document is revision t<16 hex>, the root case's goals revision is none`.
- A root case that declares no `goals` artifact gives `invalid_goals` with the reason
  `the root case declares no goals artifact`. This provider does not read an unanchored document.
  (`story:goals-grounds-and-free-answers`'s own no-note rule for a held case without `goals` still
  holds for the note.)
- Rule D93 is unaffected. This check decides which document is read. The note `revision_moved`
  compares a ground's cited revision with the held case's current `goals` revision, as
  `story:goals-grounds-and-free-answers` states. A cited revision older than the current one, with
  the entry's digest unchanged, is that note and not a refusal.

**With `story:bound-artifact-revisions`.** A `goals` artifact the root's bindings place at a path
takes as its revision, before every evaluation, `t` and the first 16 hex digits of the sha256 of
that file, the form SDC writes. When the bindings place `goals` at the file `goals.document`
names, the comparison above matches by construction. This story does not refuse that equal path:
session `sdc` said the document is the resolved goals file the root case holds. So the check stops
a document at another path, a stale copy, and a document of another repository. It does not stop
an edit of the bound file: the edit moves the root case's `goals` revision with it, and the move is
recorded as a revision of the root case.

**What remains: an edit of the bound file.** Whoever can edit the bound file can make a ground
verify, by writing a document that holds the cited id and digest; the root case's revision follows
the file. Loom does not guard that edit; it is visible as a move of the root case's `goals`
revision, and a ground that cites the earlier revision carries the note `revision_moved`. The file
is SDC's: SDC runs `sdc-goals resolve` and writes it.

**The engine configuration.** `loom.engine/1` (`story:engine-configuration-document`) gains an
optional section `goals` with one member, `document`, the path of the resolved goals document. A
relative path is resolved against the directory of the engine configuration file. The loader in
`crates/loom-tree/src/engine.rs` checks the section when it loads the file, before any case
opens. Each refusal is an `EngineRefusal` that names the path:

- `GoalsDocumentMissing`: no file at the path;
- `GoalsDocumentUnreadable`: the file cannot be read, or is not JSON (the detail names why);
- `GoalsFormatUnknown`: `format` is missing or is not `sdc.goals/1` (it names the value found);
- `GoalsDocumentNotResolved`: an `sdc.goals/1` document that is not `resolve`'s output, naming the
  member: `at` missing, `entries` missing, `layer` present, an entry without `id` or `digest`, or a
  `valid_from` or `valid_until` that is neither `YYYY-MM-DD` nor `YYYY-MM-DDTHH:MM:SSZ`;
- `GoalsDocumentTooLarge`: the file is over 1,048,576 bytes (1 MiB). The size is read before the
  file is, and the refusal names the size and the bound.

An undeclared member of the section is refused `MemberUndeclared`, as every section's is. The
loader does not compare the revision, because no case is open when it runs.

**Dates and instants.** An entry's `valid_from` and `valid_until` are written either as
`YYYY-MM-DD` or as `YYYY-MM-DDTHH:MM:SSZ`, and a date means `00:00:00Z` (`docs/requester.md:63-64`).
The port normalises them: it gives `goals_verdict` instants only, `YYYY-MM-DD` read as
`YYYY-MM-DDT00:00:00Z`. A value of neither form, at a check, makes the document fail to validate,
so the ground is refused `invalid_goals` naming the entry and the member.

**When the file is read.** It is read on every call of the port, that is, once per goals citation
checked. Nothing is cached between two checks.

- A file that changed between two checks gives the second check the new content. The new content
  has a new sha256, so unless the root case's `goals` revision moved to match it, the second check
  is `invalid_goals`.
- A file that, at a check, is missing, unreadable, not `sdc.goals/1`, not a resolved document,
  over the bound, or not the root case's revision answers that the document does not validate.
  The ground is then refused `invalid_goals`, with a reason that names the path and the cause.
- The daemon reads the file the same way. So a document that SDC resolves again into the bound
  file reaches a served tree without a restart: the daemon's own evaluation of the root case
  recomputes its `goals` revision from that file (`story:bound-artifact-revisions`). Nothing
  outside the daemon writes the served store. The engine
  configuration itself is not reloaded (`story:engine-configuration-document`, § Out of scope).

**What the port answers from the file**, for an entry id at the check's instant (the request's
`opened_at`, `story:goals-grounds-and-free-answers`):

- The entry is in the document: the state holds its `digest`, `valid_from` and `valid_until`, and
  `goals_verdict` gives `digest_changed`, `not_yet_valid` or `expired` by comparing them with the
  citation and the instant.
- It is not, and an entry of the document that is in its window at the instant names it in
  `overrides`: the state names that entry as `overridden_by`.
- It is not, and nothing names it: the state has no entry, so the ground is refused `no_entry`.

**What a resolved document cannot tell.** It holds only the entries live at its own `at`, and
the check's instant is the request's `opened_at`, which can differ. So:

- An entry outside its window at `at` is not in the document. It answers `no_entry`, never
  `expired` or `not_yet_valid`, even when the request's `opened_at` falls inside its window.
- An entry in the document is never `overridden`. `resolve` drops a target that an entry in its
  window at `at` overrides, so a present entry was not overridden at `at`. An override that applies
  at `opened_at` but not at `at` is not seen.
- The `overridden_by` path covers only an absent entry that a present entry names. That is
  narrower than SDC's `sdc-goals ground`, which reads the layer files and reports `overridden` for
  a present entry (`check.rs:471-515`).
- `expired` and `not_yet_valid` come only for an entry that is live at `at` and outside its
  window at `opened_at`.

This is the default of § Open 5 of the ledger, for session `sdc` to settle.

**No network and no program.** Loom reads the file with the standard library's file calls only.
It opens no socket, and it runs no program: not `sdc-goals`, and not `$SDC_GOALS`.

**Who passes the provider:**

- `tree run --engine FILE` and `tree serve --engine FILE` (`story:tree-daemon` adds the flag to
  `tree serve`) build `FileGoalsEntries` from the `goals` section, with the tree's
  `RootGoalsRevision`. They pass it to `with_goals_entries` in the `HeldCaseGrounds` build that
  `tree run` and the daemon share (`story:goals-grounds-and-free-answers`).
- `tree open --engine FILE` loads and refuses the section like the others, and checks no ground.
- `b10x-loom run` and `resume` do not read the engine configuration
  (`story:engine-configuration-document`, § Out of scope), so the slice through the binary passes
  no provider. A host that embeds the slice passes `FileGoalsEntries`, with its own
  `RootGoalsRevision`, through `RunOptions.goals` (`story:goals-grounds-and-free-answers`).
- The `decisions` commands run no grounding check, so they take no provider.

**The fixture.** This story adds its own fixture tree, `crates/loom-cli/tests/fixtures/goals-tree/`.
It is a copy of the three-level fixture tree of `story:tree-run-command`
(`crates/loom-cli/tests/fixtures/tree/`), whose root protocol also declares artifact `goals` and
whose root bindings place `goals` at `goals.json` in the root case's workspace.
`story:tree-run-command`'s fixture is not changed. The test writes the engine file `FILE` with
`goals.document` naming that `goals.json` by its absolute path, writes the fixture document `F`
there, and runs `tree run`; the root case's `goals` revision is then `R`, `t` and the first 16 hex
digits of `F`'s sha256, by `story:bound-artifact-revisions`. The routing is that of
`story:tree-waits-open-decision-requests`: its fixture autonomy documents and principals
(`crates/loom-cli/tests/fixtures/tree/decisions/`) at level `A1`, where the recorded decider is
asked for a recommendation and the person `timo` decides.

**A port a host implements.** A host that embeds the tree or the slice implements
`RootGoalsRevision`. So, as `story:decisions-resume-the-held-run` does for `ResumeBuilder`, and as
`AGENTS.md:172` requires:

- `crates/loom-governor/src/lib.rs` re-exports it from `file_goals`, so its path is
  `loom_sdk::governor::RootGoalsRevision` (`loom_sdk` re-exports the governor,
  `crates/loom-sdk/src/lib.rs:69`);
- it gets a `host_ports!` row in `crates/loom-sdk/tests/host_ports.rs` (the invocation at `:48`);
- it gets a row under "Ports a host implements" in `website/docs/reference/sdk-stability.md`
  (`:74`).

**Files:**

- `ess/domains/engine.yaml`, `ess/domains/decision.yaml`, and the regenerated `generated/rust/loom`.
- `crates/loom-governor/src/file_goals.rs` (new, beside the governor's file stores, holding
  `FileGoalsEntries` and the `RootGoalsRevision` trait) and `crates/loom-governor/src/lib.rs` (its
  `mod` line and the re-export).
- `crates/loom-sdk/tests/host_ports.rs` and `website/docs/reference/sdk-stability.md` (the
  `RootGoalsRevision` rows).
- `crates/loom-tree/src/engine.rs` (the section and its refusals).
- In `crates/loom-tree`: the `HeldCaseGrounds` build that `tree run` and the daemon share, and the
  tree's `RootGoalsRevision`. Its file is named by `story:goals-grounds-and-free-answers`, which
  adds the port there; no story names a file inside the crate yet.
- The `tree` command arms of `crates/loom-cli` that load `--engine`. `story:engine-configuration-document`
  names where `--engine` is wired; this story adds no flag.
- The engine guide page that `story:engine-configuration-document` creates under `website/docs/`,
  which gains the section; that story names the path. `website/docs/reference/cli.md` is
  regenerated.
- Tests: `crates/loom-governor/tests/file_goals.rs`, `crates/loom-tree/tests/engine_goals.rs` (the
  loader) and `crates/loom-cli/tests/engine_goals.rs` (the bullets that run the `b10x-loom`
  binary), with the fixture `crates/loom-cli/tests/fixtures/goals-tree/` (new).

## Why

Session `sdc` will drive Loom through the binary, so a goals ground must be checkable there. It
asked for one story, at low priority, for a file-backed `GoalsEntries` provider named in the
engine configuration, with rule D93. It accepts `invalid_goals` through the binary until this
story lands (`.engineering/drafts/e3-sdc-answers-11.md`, (c)). The resolved document is what SDC
already hands Loom (`~/beyond10x/sdc/docs/requester.md:185` at `00e3557`, "the host passes the
resolved goals document"). Session `sdc` accepted the revision check, in the form its driver
writes (`.engineering/drafts/w3-sdc-approval.md`).

## Waits on

- `story:goals-grounds-and-free-answers`: `GoalsEntries`, `GoalsEntryState`, `goals_verdict`,
  `with_goals_entries` and the tree's port, which this story implements and fills.
- `story:engine-configuration-document`: `loom.engine/1`, its loader and `--engine`. It also says
  that each story adding a section depends on it.
- `story:tree-daemon`: `tree serve --engine` and the daemon.
- `story:bound-artifact-revisions`, indirect (through `story:tree-run-command`): the root case's
  `goals` revision recomputed from the bound file before every evaluation.
- No edge orders this story against the other stories that add a section to
  `ess/domains/engine.yaml` (`story:confinement-level-on-oci-and-pod`,
  `story:executable-descendants-held`, `story:ingress-routes-per-action`,
  `story:substrate-daemon-on-another-host`, `story:capacity-given-to-an-admission-program`,
  `story:agent-session-through-substrate`, `story:workspace-file-bound-in-engine-configuration`).
  Each adds a section of its own and reads none of the others', so the order between them does not
  matter. They share the file, the typed scope names it, and `aep plan artifact waves` keeps two
  stories with one path out of one wave. The coordinator removed seven such edges on 2026-11-21:
  five of those stories are drafts of an epic nobody has accepted, and an edge to them would hold
  this story for no reason of order.

  The other editors of `ess/domains/engine.yaml` come before it through
  `story:goals-grounds-and-free-answers`: `story:daemon-continues-a-decided-request`, then
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

`RootGoalsRevision` is declared in Rust only: no command input comes from it (the epic's host-port
rule). On that commit `task drift` fails, `crates/loom-governor/tests/file_goals.rs` and
`crates/loom-tree/tests/engine_goals.rs` do not compile, and `crates/loom-cli/tests/engine_goals.rs`
fails (the engine file has no `goals` section).

## Acceptance

In `crates/loom-tree/tests/engine_goals.rs`, the loader, over `loom.engine/1` files whose `goals`
section names:

- a path with no file: refused `GoalsDocumentMissing` naming the path;
- a file holding `{`: refused `GoalsDocumentUnreadable` naming the path;
- a file with mode `000` (the test skips when it runs as root): refused `GoalsDocumentUnreadable`
  naming the path;
- a resolved document with `format: sdc.goals/2`: refused `GoalsFormatUnknown` naming
  `sdc.goals/2`;
- a resolved document without `at`: refused `GoalsDocumentNotResolved` naming `at`;
- a resolved document without `entries`: refused `GoalsDocumentNotResolved` naming `entries`;
- a layer file (`format: sdc.goals/1`, `layer: product`, with `at` and `entries`): refused
  `GoalsDocumentNotResolved` naming `layer`;
- a resolved document whose second entry has no `id`: refused `GoalsDocumentNotResolved` naming
  that entry's `id`;
- a resolved document whose first entry has no `digest`: refused `GoalsDocumentNotResolved` naming
  that entry's `digest`;
- a resolved document whose entry has `valid_until: 2026-13-01`: refused `GoalsDocumentNotResolved`
  naming that entry's `valid_until`;
- a resolved document of 1,048,577 bytes: refused `GoalsDocumentTooLarge` naming the path,
  `1048577` and `1048576`;
- a resolved document of exactly 1,048,576 bytes: loaded.

A further loader bullet in the same file:

- A section with `documnet` is refused `MemberUndeclared` naming `goals.documnet`.

In `crates/loom-cli/tests/engine_goals.rs`:

- `tree open --engine FILE` with a section the loader refuses exits non-zero naming the refusal,
  and writes no case file.

In `crates/loom-governor/tests/file_goals.rs`, `FileGoalsEntries` over a fixture resolved document
`F` (`at: 2026-11-21T00:00:00Z`), with a fixture `RootGoalsRevision` answering `R`, which is `t` and
the first 16 hex digits of `F`'s sha256. `goals_verdict` runs with the request opened at
`2026-11-21T12:00:00Z`. `F`'s entries:

- `company/c1`: digest `sha256:aa`, valid from `2026-11-01` to `2026-12-01`;
- `product/p2`: valid from `2026-11-01`, with `overrides: company/c3`.

The bullets:

- `goals@R#company/c1` with digest `sha256:aa` is verified with no note.
- `goals@R#company/c9` is refused `[no_entry]`.
- `goals@R#company/c1` with digest `sha256:bb` is refused `[digest_changed]`.
- With the request opened at `2026-10-31T23:59:59Z`, `company/c1` is refused `[not_yet_valid]`.
- With the request opened at `2026-11-01T00:00:00Z`, `company/c1` is verified: the date
  `2026-11-01` is read as `2026-11-01T00:00:00Z`.
- With the request opened at `2026-12-01T00:00:00Z`, `company/c1` is refused `[expired]`.
- `goals@R#company/c3` is refused `[overridden]`, and the state names `product/p2`.

The bullets for what a resolved document cannot tell:

- `company/c4`, valid from `2026-12-01` in the layer files and so absent from `F`, is refused
  `[no_entry]` for a request opened at `2026-12-02T00:00:00Z`, inside its window.
- `company/c1` is verified with no `overridden` cause for a request opened at
  `2026-11-25T00:00:00Z`, although the layer files hold an entry `product/p5` that overrides it
  from `2026-11-24` and that `F` does not hold.

The bullets for the revision check:

- With the `RootGoalsRevision` answering `t0000000000000000`, the ground is refused
  `[invalid_goals]`, and the reason names both `R` and `t0000000000000000`.
- With `F` rewritten with the same entries and one changed byte in a `statement`, the
  `RootGoalsRevision` still answering `R`, the ground is refused `[invalid_goals]`.
- With the `RootGoalsRevision` answering `none`, the ground is refused `[invalid_goals]`, and the
  reason names `R` and `none`.
- With the `RootGoalsRevision` answering nothing, the ground is refused `[invalid_goals]` naming
  `the root case declares no goals artifact`.

The bullets for files that turn bad between checks:

- With `F` replaced after the provider was built by a file holding `{`, the next check refuses
  `company/c1` `[invalid_goals]`, and the reason names `F`'s path.
- With `F` replaced by a file of 1,048,577 bytes, the next check refuses it `[invalid_goals]`
  naming the bound.
- With `F` replaced by a document whose `company/c1` has `valid_until: 2026-13-01`, the next check
  refuses it `[invalid_goals]` naming `company/c1` and `valid_until`.
- Two checks of `company/c1`, with `F` rewritten between them so that its digest is `sha256:cc` and
  the `RootGoalsRevision` moved to the new file's revision: the first state answers `sha256:aa` and
  the second `sha256:cc`.
- With `PATH` set to a directory holding a recording `sdc-goals`, and `SDC_GOALS` naming that
  program, a check answers as above, and the program records no call.

In `crates/loom-cli/tests/engine_goals.rs`, on the fixture tree `goals-tree` with `F` written at
the bound `goals.json` that `FILE`'s `goals.document` names, the routing of `story:tree-waits-open-decision-requests` at level
`A1` (its fixture autonomy documents and principals), a recorded decider reply for a request on the
root case citing `goals@R#company/c1` with digest `sha256:aa`, and no model call:

- `tree run --engine FILE` records the recommendation verified, and `decisions show` prints the
  `grounding_report` `1 grounds, all resolve`.
- With `F2` (`F` resolved again: same entries, `at` moved) written at the bound `goals.json`, and
  the reply still citing `goals@R#company/c1`: the next `tree run --engine FILE` records the root
  case's `goals` revision moved from `R` to `F2`'s revision, records the recommendation verified,
  and the `grounding_report` holds `revision_moved goals@R#company/c1 (current <F2's revision>)`.
- With a `goals.json` whose `company/c1` digest is `sha256:bb` written at the bound path, the
  run's request carries no recommendation, and `no_recommendation_reason` names `digest_changed`.
- With `F2` at the bound path and a second engine file whose `goals.document` names a stale copy
  of `F` at another path, the run's request carries no recommendation, and
  `no_recommendation_reason` names `invalid_goals`.
- `tree run` without `--engine`, and with an engine file that has no `goals` section, both refuse
  the ground `invalid_goals` naming `no goals port`.
- With `b10x-loom tree serve --engine FILE` serving the store, the served tree's request records
  the recommendation verified.
- With the same daemon still running, the test writes `F2`, with `company/c1`'s digest
  `sha256:dd`, at the bound `goals.json` and writes nothing to the store. The daemon's next
  evaluation of the root case moves its `goals` revision to `F2`'s, and the second request it opens,
  whose reply cites `company/c1` with `sha256:dd`, records its recommendation verified, with no
  restart of the daemon.

The host port:

- `crates/loom-sdk/tests/host_ports.rs` passes with the `host_ports!` row for
  `loom_sdk::governor::RootGoalsRevision` and its row under "Ports a host implements" on
  `website/docs/reference/sdk-stability.md`.

Last:

- `task docs-check` exits 0, with `website/docs/reference/cli.md` regenerated.
- `task check` exits 0.

## Out of scope

- Reading the goals layer files, running `sdc-goals`, or computing an entry's digest.
- Guarding an edit of the bound goals file, which moves the root case's `goals` revision (§ Outcome,
  "What remains").
- The engine configuration in `b10x-loom run` and `resume`.
- Reloading a changed engine configuration in a running daemon.
- A cache of the goals document between checks.
- A per-file bound for workspace files (`story:workspace-file-bound-in-engine-configuration`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-governor/src/file_goals.rs` (new) with `crates/loom-tree/src/engine.rs` — cited, story § Files
- **Files:** `ess/domains/engine.yaml` — cited; not at HEAD, created by `story:engine-configuration-document`
- **Files:** `ess/domains/decision.yaml` — cited; not at HEAD, created by `story:decision-request-entity`, extended by `story:goals-grounds-and-free-answers`
- **Files:** `generated/rust/loom` — cited; exists at HEAD, regenerated
- **Files:** `crates/loom-governor/src/file_goals.rs` — cited; new, not at HEAD
- **Files:** `crates/loom-governor/src/lib.rs` — cited; exists at HEAD, one `mod` line
- **Files:** `crates/loom-tree/src/engine.rs` — cited; not at HEAD, `crates/loom-tree` is created by `story:tree-run-command` and the file by `story:engine-configuration-document`
- **Files:** `website/docs/reference/cli.md` — cited; exists at HEAD, regenerated
- **Files (tests):** `crates/loom-governor/tests/file_goals.rs`, `crates/loom-tree/tests/engine_goals.rs` — cited; new
- **Symbols:** `FileGoalsEntries`, `GoalsEntries`, `GoalsEntryState`, `goals_verdict`, `with_goals_entries`, `HeldCaseGrounds`, `EngineRefusal::Goals*`, `RunOptions.goals` — cited; none at HEAD. The traits and functions are planned by `story:goals-grounds-and-free-answers` (`HeldCaseGrounds` by `story:grounds-resolve-against-held-cases`). `EngineRefusal` is planned by `story:engine-configuration-document`
- **Also likely:** `crates/loom-tree/src` (the `HeldCaseGrounds` build that `tree run` and the daemon share) — inferred; the story names no file, and `story:goals-grounds-and-free-answers` § Files names only the crate
- **Also likely:** `crates/loom-cli/src/lib.rs` (the `tree` arms that load `--engine`) — inferred; exists at HEAD. `crates/loom-cli/src/tree.rs` is the other candidate (`story:tree-run-command` scope, itself inferred)
- **Documents:** the guide page `story:engine-configuration-document` plans under `website/docs/` — inferred; no path is named anywhere
- **Confidence:** medium — every code path the story names is cited, but only 3 of 9 exist at HEAD (`generated/rust/loom`, `crates/loom-governor/src/lib.rs`, `website/docs/reference/cli.md`), and two surfaces are inferred
- **Would collide with:**
  - `ess/domains/engine.yaml`, ordered before it by `depends_on`: `story:engine-configuration-document`, `story:tree-daemon`, `story:concurrent-cases-in-the-tree-daemon`, `story:named-egress-per-action`, `story:substrate-daemon-runner`, `story:action-limits-enforced`, `story:confinement-level-required`, `story:executable-allow-list`, `story:limit-hits-in-run-log`
  - `ess/domains/engine.yaml`, unordered by the coordinator's decision (a section each, no edge): `story:confinement-level-on-oci-and-pod`, `story:executable-descendants-held`, `story:ingress-routes-per-action`, `story:substrate-daemon-on-another-host`, `story:capacity-given-to-an-admission-program`, `story:agent-session-through-substrate`, `story:workspace-file-bound-in-engine-configuration`
  - `ess/domains/decision.yaml`, ordered before it: `story:decision-request-entity`, `decider-port-and-grounding-check`, `decided-record-written-then-recorded`, `decision-inbox-commands`, `decision-routing-by-holder`, `escalate-when-conditions`, `silence-and-expiry-at-a-host-tick`, `goals-grounds-and-free-answers`
  - `generated/rust/loom`, unordered: `story:observe-git-in-a-linked-worktree`, `story:secret-files-several-secrets`, `story:egress-by-host` (inferred). Every story that edits ESS regenerates it, so the unordered `engine.yaml` editors above collide here too
  - `crates/loom-governor/src/lib.rs`, unordered: `story:engineering-registry-calls-replaced`, `story:gates-document-evaluated-in-the-governor`. Their typed scope names the file.
  - `crates/loom-governor/src/lib.rs`, unordered, with no typed scope (the body names the file, not computed as an edit): `authority-records-bound-and-grounded-in-canon`, `decided-records-carry-grounds-to-canon`, `engineering-protocol-lock`, `engineering-protocols-dependency-removed`, `frontier-names-movers`, `gate-run-collects-a-gates-document`, `requests-from-canon-pending-decisions`
  - `crates/loom-tree`, unordered: `story:capacity-given-to-an-admission-program`, `story:flow-limits-recorded-as-events` (typed scope). Also, by body only: `story:confinement-level-on-oci-and-pod`, `story:ingress-routes-per-action`, `story:workspace-file-bound-in-engine-configuration`, `story:budget-and-limit-stops-raise-decision-requests`, `story:requests-from-canon-pending-decisions`, `story:run-log-references-exec-file-changes`
  - `crates/loom-tree` and `crates/loom-cli/src/lib.rs`, ordered before it: `story:tree-run-command`, `agent-node-run-records`, `tree-waits-open-decision-requests`, `daemon-continues-a-decided-request`, `approval-decisions-resume-held-runs` and the rest of the closure
  - `crates/loom-cli/src/lib.rs` (inferred), unordered: `story:flow-limits-recorded-as-events`, `story:secrets-subcommand`, `story:agent-selects-the-connectors-relay`. The last two also hold `crates/loom-cli/src/main.rs`.
  - `website/docs/reference/cli.md`, unordered: `story:secrets-subcommand`, `story:flow-limits-recorded-as-events`, `story:agent-selects-the-connectors-relay`, `story:agent-project-kubernetes`, `story:command-invocation-contract`. Only `secrets-subcommand` holds it as typed scope.
- **Safety fact:** a goals-document read failure becomes `invalid_goals`, never a panic or a pass: the loader refusals and the per-check `validates` answer are one list in the story § Outcome, so no new refusal can pass a ground. Pointed at the story text only, no code exists — step 1, unproven
