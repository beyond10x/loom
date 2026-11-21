---
format: aep.planning-md/3
id: story:ess-096-upgrade
kind: story
status: implemented
title: Loom builds, generates and gates on ESS 0.96.0
summary: Every ess pin 0.86.0 to 0.96.0 on wave 4, all three systems regenerated; Suspension drops its case relation and a mixed HoldSuspension is refused incomplete-request before already-held (defaults 1a, 2a)
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:ess-format-31
- depends_on: story:confinement-opens-from-engine-config
- depends_on: story:engineering-registry-calls-replaced
- depends_on: story:engineering-protocols-vendor-command
- depends_on: story:decision-suspension-reason
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-commission-conformance/Cargo.toml
- confidence: cited
  path: crates/loom-commission-conformance/src/lib.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/hold_incomplete_request.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/security_w20261121w3_u2.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/security_w20261121w3_u2_pass2.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/security_w20261121w5_u1.rs
- confidence: cited
  path: crates/loom-commission/src/ports/suspension.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-commission/tests/adversary2_gate.rs
- confidence: cited
  path: crates/loom-commission/tests/ess_gate.rs
- confidence: cited
  path: crates/loom-conformance/Cargo.toml
- confidence: cited
  path: crates/loom-executor/tests/adversary2_ess_gate.rs
- confidence: cited
  path: crates/loom-executor/tests/ess_gate.rs
- confidence: cited
  path: crates/loom-governor/src/file_suspension.rs
- confidence: cited
  path: crates/loom-intake-conformance/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/tests/security_w20261121w5_u1.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: ess/commission/ess-inputs.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/intake/ess-inputs.yaml
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T09:17:24Z", actor: "agent:sdc", revision: 7, decided_on: {"recorded":{"review_outcome":4}}, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T09:18:22Z", actor: "agent:sdc", revision: 8, decided_on: {"recorded":{"review_outcome":4}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w5"}
- {from: "active", to: "implemented", at: "2026-11-21T10:50:26Z", actor: "agent:sdc", revision: 16, decided_on: {"recorded":{"test_result":1,"review_outcome":6,"verification":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w5"}
---
## Outcome

Loom builds, generates and gates on ESS 0.96.0, the newest release (tag 2026-11-13), moved in the
0.58.0 release because every Loom release moves its ess pin to the newest ESS release (AGENTS.md
§ Releases). Today every pin names 0.86.0. The `ess` on the machine's `PATH` is the 0.96.0
dispatcher itself (sha256 of the binary `7be49dbf…ccdd4a`, equal to
`~/.cache/ess/toolchains/0.96.0/ess` and to the binary in the release tarball
`ess-0.96.0-x86_64-unknown-linux-gnu.tar.gz`, sha256
`8eaf33989c9306bd923d19a96be82723addf7ad532b534fb8b0e423733f81825`), so after the move no `task`
step needs `ESS_TOOLCHAIN` or a private binary first on `PATH`. `ess --version` prints one line
only while the pin equals the dispatcher; once the dispatcher is newer than the pin it prints two
again, so `story:commission-docs-read-one-ess-version-line` stays open.

The move lands on wave 2026-11-21-w4 (`71f15f92`): the main system is `ess/31`
(`story:ess-format-31`), intake `ess/26`, Commission `ess/22`, and the engineering protocols are
vendored under `protocols/engineering/` and held to `protocols/engineering/lock.json`, which names
no ESS release and does not move.

## Why

A probe of the move (2026-11-21, worktree at `c94ccbbf`, before wave 4) found it is not mechanical.
ESS 0.95.0 added two synthesis rules that refuse `commission.responsibility.HoldSuspension/held`,
so `task commission:ess-gate` and `task commission:conform` go red (3 refusals, 41 scenarios where
0.86.0 synthesized 43 with 0). Every scenario name in this story is written as the synthesized
suite keys it, less its leading `commission.responsibility.`:

- `ESS-SYNTH-001` twice (`HoldSuspension/outcome/held` and
  `Suspension/invariant/after/commission.responsibility.HoldSuspension/held`): `held` copies
  `input.case_id` into `Suspension.case_id`, which carries the `references` relation `case` to
  `commission.responsibility.Case`, and no command creates a `Case`, so synthesis cannot arrange the
  row the input names (0.95.0, referenced-input arrangement).
- `ESS-SYNTH-025`: `held` copies the four request fields into a Suspension whose invariant says
  they are present together or absent together, and no guard or refusal keeps the input inside it
  (0.95.0, beyond10x/ess#738).

Every other step passed at `c94ccbbf` after the pins and regeneration. Wave 4 changed no file under
`ess/commission/` (`git diff --stat c94ccbbf..71f15f92 -- ess/commission` prints nothing), so the
three refusals stand at `71f15f92`. It did change `ess/system.yaml` (`ess/25` to `ess/31`),
`ess/domains/resource.yaml` and `ess/intake/domains/confinement.yaml`, which the probe never ran
under 0.96.0; the acceptance below runs them. The fix for the two refusals is the specification
decision in `## Decision`.

## Decision

Both options were taken on 2026-11-21 by the Loom session on the silence of session `sdc`, which
plans Loom's stories: `sdc` was asked for the two decisions with the defaults 1a and 2a named, was
told the defaults stand on silence, and did not answer. `sdc` confirmed both the same day, after
this revision, with the two facts the revision found: the member check moves before `already-held`
and the two wave 3 security tests change with it; a `Decision` hold with all four members and
another reason with none stay `storage-failed`
(`.engineering/drafts/w5-sdc-approval.md` holds its words while the wave runs; the wave page
quotes them).

- `ESS-SYNTH-001`, option (a): drop the `case` relation from `Suspension`
  (`ess/commission/domains/responsibility.yaml:1328-1332`), so `Suspension` keeps the relations
  `run` and `action_request`; validate then warns `implied_relation` on `case_id`.
  Nothing reads the relation: the generated Commission model has no relation code, and only the two
  reference pages under `website/docs/reference/commission/` list it. `Suspension.case_id` and the
  `held` outcome's `case_id` stay.
- `ESS-SYNTH-025`, option (a): a refusal branch `incomplete-request` ahead of `held`, guarded
  `not any[all defined, all absent]` over `action_request_id`, `expected_case_revision`, `action`
  and `arguments`, with the new error `SuspensionRequestIncomplete` naming the Suspension,
  implemented in `decide_hold` and the Commission conformance adapter. `decide_hold` already refuses
  every such input, as `storage-failed`, so the option names an existing refusal where synthesis and
  conformance can see it instead of adding a rule.
- Declined for 001: (b) declare a command that creates a `Case` in Commission; (c) cover the
  refusal with an authored `ess-scenario/7` `covers:` scenario and teach both gates to count
  `covered by authored`.
- Declined for 025: (b) drop the entity invariant and leave the rule to the runtime; (c) group the
  four fields into one `Optional` struct on the input and the entity (a wire and SDK break).
- Probe: (a)+(a) on a copy of `ess/commission` at `c94ccbbf` synthesizes 44 scenarios with 0
  refusals; (a) alone for 001 leaves only the 025 refusal (43 scenarios).

What 2a changes against `decide_hold` at `71f15f92`
(`crates/loom-commission/src/ports/suspension.rs`):

- Today a hold for a commission that already holds a Suspension answers `already-held` first
  (`:147-156`). Then every input whose reason and four members disagree is refused
  `storage-failed`, error `SuspensionStorageFailed`, and nothing is stored: a `Decision` reason with
  any of the four present, any other reason with any of them absent (`:157-181`, reason texts
  `:172` and `:174`). Only a `Decision` hold with none of the four and another hold with all four
  are held. A mixed hold is therefore already refused today; it is not stored.
- After: a hold with some but not all of the four present, whatever its reason (14 of the 16
  combinations per reason), is refused `incomplete-request`, error `SuspensionRequestIncomplete`
  (`suspension`), nothing stored, and this check runs before the `already-held` check. The two
  disagreements that are not mixed, a `Decision` hold with all four and another hold with none,
  stay `storage-failed` with today's texts, after `already-held`: an ess/22 guard cannot read the
  reason's variant (`ess/commission/domains/responsibility.yaml:1285`, `:1310-1312`). So
  `incomplete-request` precedes the existing refusal and takes over its mixed half, and replaces it
  for no other input.
- The difference from the decision as first written, which placed the branch ahead of `held` only:
  the probe's synthesized scenario `HoldSuspension/outcome/incomplete-request` ends by sending a
  mixed hold to a commission that already holds a Suspension and expects `incomplete-request` with
  the held one unchanged, so `decide_hold` must test the members before `held.is_some()` (`:147`).
  A testkit case holds the same order without the suite (see `## Acceptance`).
- Unchanged: a host wrapper still answers `settled` before `decide_hold`
  (`crates/loom-intake-slice/src/handoff.rs:429-446`,
  `crates/loom-commission-conformance/src/store.rs:258-272`), as the specification declares
  `settled` first (`responsibility.yaml:1719`), and the file store still refuses an identity naming
  another commission first (`crates/loom-governor/src/file_suspension.rs:207-216`). The runtime
  never sends a mixed hold, since it fills the four from one request
  (`crates/loom-commission/src/runtime.rs:2433-2437`), and its match keeps a wildcard (`:2451`).
- Exhaustive matches that gain an arm: the conformance adapter's `hold_suspension`
  (`crates/loom-commission-conformance/src/lib.rs:571-597`). Tests that assert `StorageFailed` for a
  mixed hold and change to `IncompleteRequest`:
  `crates/loom-commission-testkit/tests/security_w20261121w3_u2.rs:708-741` (two of its four
  cases) and `crates/loom-commission-testkit/tests/security_w20261121w3_u2_pass2.rs:470-517` (the
  28 mixed combinations and the `Decision` hold with `arguments: null`).

## Acceptance

- Every `requires:` line, the three `tag = "0.86.0"` pairs (`crates/loom-conformance`,
  `crates/loom-commission-conformance`, `crates/loom-intake-conformance`), `ESS_VERSION` and
  `ESS_SHA256` in `.github/workflows/check.yml` and `.github/workflows/pages.yml`, and the version
  text the gate tests match name 0.96.0. `git grep -n 'ess 0\.86\.0\|tag = "0\.86\.0"\|ESS_VERSION:
  "0\.86'  -- ':!CHANGELOG.md' ':!.engineering'` prints nothing (21 lines at `71f15f92`; no wave 4
  file matches it), `git grep -n '0\.86\.0' -- README.md AGENTS.md` prints nothing (3 lines at
  `71f15f92`), and `grep -c 8eaf33989c9306bd923d19a96be82723addf7ad532b534fb8b0e423733f81825
  .github/workflows/check.yml .github/workflows/pages.yml` prints 1 for each file.
- 1a holds: `sed -n '/^  - name: commission.responsibility.Suspension$/,/^    lifecycle:/p'
  ess/commission/domains/responsibility.yaml | grep -B1 'kind: references' | grep -o 'name: .*'`
  prints `name: run` and `name: action_request` and nothing else (at `71f15f92` it also prints
  `name: case`), and `grep -c 'creates: commission.responsibility.Case$'
  ess/commission/domains/responsibility.yaml` prints 0 (no command creates a `Case`, so 1b was not
  taken). `ess specify validate --path ess/commission` exits 0 and warns `implied_relation` on
  `Suspension.case_id`: a reviewer reads a `warning[ESS-ENTITY-019]` line naming
  `commission.responsibility.Suspension` and `case_id`, of the form the probe log quotes for
  intake (`` warning[ESS-ENTITY-019]: `intake.routing.Grant` stores `action_request_id`, … and no
  relation declares that it names a `intake.routing.MergeHandoff` ``); the probe never logged
  Commission's own line.
- The error name is declared and generated: `grep -c SuspensionRequestIncomplete
  ess/commission/domains/responsibility.yaml` prints at least 1 (3 on the probe copy: the
  declaration, the branch's `error:` and its payload) and `grep -c SuspensionRequestIncomplete
  generated/rust/commission/src/responsibility.rs` prints at least 1 (0 at `71f15f92`).
- `ess verify conform synthesize --path ess/commission --out <scratch>/suite.json` ends with the
  summary line `44 scenario(s) (0 authored), 0 refusal(s), …`, the probe's count for 1a and 2a on
  a copy of `ess/commission` at `c94ccbbf` (wave 4 changed nothing under `ess/commission/`). If the
  unit reads another count at its base, it stops and reports the count to the coordinator; it does
  not edit this number. The suite's `scenarios` include `HoldSuspension/outcome/incomplete-request`,
  `HoldSuspension/outcome/held` and
  `Suspension/invariant/after/commission.responsibility.HoldSuspension/held`.
- `task commission:ess-gate` (`crates/loom-commission/tests/ess_gate.rs`, all 11 tests) passes.
- `task commission:conform` (`crates/loom-commission-conformance/tests/conform.rs`) passes, and
  `grep -c '^- ' ess/commission/SKIPPED.md` prints 0 (0 at `71f15f92`). The gate fails on any
  failed or errored scenario and on any skipped or unsupported scenario `ess/commission/SKIPPED.md`
  does not name (`conform.rs:10-23`, `:170-196`), so with that file empty every scenario the suite
  holds, the three above included, ran and passed. The unit does not change
  `ess/commission/SKIPPED.md`.
- The member check runs before `already-held`, without the suite: a new test in
  `crates/loom-commission-testkit/tests/hold_incomplete_request.rs` holds a full `Authority` hold
  for a commission on `SuspensionStore` and on `FileSuspensionStore`, then sends a mixed hold for
  the same commission, and expects `HoldSuspensionOutcome::IncompleteRequest` naming that
  Suspension and `SuspensionPort::get` returning the first Suspension unchanged;
  `cargo test -p b10x-loom-commission-testkit --locked --test hold_incomplete_request` passes (it
  answers `AlreadyHeld` before `decide_hold` changes). The second witness is the last step of the
  synthesized scenario `HoldSuspension/outcome/incomplete-request` (a mixed hold after `held` on
  the same Suspension, expecting `incomplete-request` and `expect_complete_subject_unchanged`).
- A mixed hold stores nothing: `cargo test -p b10x-loom-commission-testkit --locked --test
  security_w20261121w3_u2 --test security_w20261121w3_u2_pass2` passes, where
  `security_pass2_the_memory_store_holds_no_suspension_whose_reason_and_members_disagree` expects
  `IncompleteRequest` for the 28 mixed combinations and the `Decision` hold with `arguments: null`,
  `StorageFailed` for the two non-mixed disagreements, and an empty store after each refusal, and
  `security_the_file_store_writes_no_hold_whose_reason_and_members_disagree` expects
  `IncompleteRequest` for `hold-decision-with-action` and `hold-authority-without-arguments`,
  `StorageFailed` for the two others, and no file written for any.
- `task ess-gate`, `task drift`, `task commission:drift`, `task intake-drift`, `task crd-drift`,
  `task no-hand-model`, `task commission:no-hand-model`, `task conform`, `task intake-conform`,
  `task docs-check` and `task commission:docs-drift` pass after regeneration with 0.96.0, with no
  `ESS_TOOLCHAIN` set.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` and
  `cargo test --workspace --locked --no-fail-fast` pass (base: the whole gate of wave
  2026-11-21-w4 on `71f15f92`, 3192 passed, 0 failed, 10 ignored).
- CHANGELOG (Unreleased) has one line for the move under `### Changed` and one `**Breaking:**`
  line: `HoldSuspensionOutcome` gains `IncompleteRequest` (`SuspensionRequestIncomplete`), and a
  mixed hold answers it instead of `StorageFailed`. README.md and AGENTS.md § ESS name 0.96.0.
- `grep -h '^format:' ess/system.yaml ess/commission/system.yaml ess/intake/system.yaml` prints
  `format: ess/31`, `format: ess/22` and `format: ess/26`.
- `git log --format=%h <base>..HEAD -- ess/` names only the unit's first commit.

## ESS first

The unit's first commit changes only `ess/`. In
`ess/commission/domains/responsibility.yaml` it declares the error
`commission.responsibility.SuspensionRequestIncomplete` (one field, `suspension`, a
`SuspensionKey`) among the errors beside `SuspensionAlreadyHeld` (`:1520-1523`), adds the branch
`incomplete-request` to `HoldSuspension` between `settled` (`:1719`) and `held` (`:1726`), and
removes the `case` relation from `Suspension` (`:1328-1332`). The same commit moves the three
`requires:` lines to 0.96.0, because every later commit leaves `ess/` alone (AGENTS.md § ESS,
"A unit's first commit changes only `ess/`").

On it, with the 0.96.0 dispatcher on `PATH` and the Cargo pins still at 0.86.0, `task
commission:drift` fails: `generated/rust/commission/src/responsibility.rs` has no
`SuspensionRequestIncomplete` and no `HoldSuspensionOutcome::IncompleteRequest`. `task
commission:ess-gate` fails too (`crates/loom-commission/tests/ess_gate.rs:414` expects
`requires: ess 0.86.0`). The run is recorded. Once the model is regenerated, the scenario
`HoldSuspension/outcome/incomplete-request` fails in `task commission:conform` until `decide_hold`
changes, since `decide_hold` answers `storage-failed` (or `already-held` on the last step).

## Out of scope

- Raising any specification's format (`ess/37` is open in 0.96.0 with no construct of its own).
- Moving the two non-mixed disagreements out of `storage-failed`: an ess/22 guard cannot read the
  reason's variant.
- The workspace version, the release heading of `CHANGELOG.md` and the chart version: the release
  commit's.
- `story:commission-docs-read-one-ess-version-line`: it stays open. The move only hides the
  two-line `ess --version` while the pin equals the dispatcher; a dispatcher newer than the pin
  prints two lines again.

## As built, wave 2026-11-21-w5

Where the unit's commits (`29e5d199..590b01f8`) differ from the text above, the commits are what
holds:

- A hold of a reason other than `Decision` that carries none of the four request members is
  HELD. `## Decision`, `## Out of scope` and the wave's approval say it stays `storage-failed`;
  that could not be built. At `c94a3ede`, with the specification and `decide_hold` as this story
  describes them, `task commission:conform` passed 48 of 49 scenarios: the synthesized
  `HoldSuspension/outcome/held` sends an `Authority` hold with none of the four in its second
  step and expects `held`, which the new `incomplete-request` guard admits, and `decide_hold`
  answered `storage-failed`. No ESS format up to `ess/37` lets a guard read which variant a union
  value is (`ESS-COMMAND-003` `unobservable_fact`, `ESS-COMMAND-002` `type_mismatch`), so the
  specification cannot declare that refusal. Session `sdc` decided on 2026-11-21, from six
  measured options: `decide_hold` holds that hold; a mixed hold stays `incomplete-request`; a
  `Decision` hold with all four stays `storage-failed`
  (`crates/loom-commission/src/ports/suspension.rs:177-210`). Built in `12469d71`.
- To restore, by a later story: once ESS ships guards on a union's variant (the ESS repository's
  `story:a-guard-reads-a-union-tag-and-a-union-names-its-content-key`,
  `story:synthesis-and-the-interpreter-decide-a-guard-on-a-union-variant` and
  `story:an-invariant-compares-a-union-field-with-its-variant-labels`, `active` at tag `0.96.0`),
  `HoldSuspension` declares a refusal branch for a hold of a reason other than `Decision` with
  no request, and for a `Decision` hold with a request, and `decide_hold` refuses both by that
  branch in place of holding the first and answering `storage-failed` for the second.
- What such a held Suspension does, on both stores
  (`crates/loom-commission-testkit/tests/hold_incomplete_request.rs:229-420`): a take naming a
  request answers `OtherRequest`, a take naming a decision request `OtherDecision`, a take
  naming neither `Taken`, and a release naming neither then `Released`. `resume_after_grant`
  answers `ResumeError::Store` ("the Suspension holds no action request, and its reason is no
  decision", `crates/loom-commission/src/runtime.rs:2529`) before the governor is asked;
  `resume_after_decision` answers `Refused(HeldForAuthority)` (`:2783`). The runtime sends a
  hold with no request only for `Decision`.
- The file store reads such a Suspension back: a file of a reason other than `Decision` whose
  `action_request_id`, `expected_case_revision` and `action` are all `null` holds no request
  (`crates/loom-governor/src/file_suspension.rs:545-553`). No earlier build wrote such a file; one
  written by hand was refused before and now blocks its commission as suspended.
- The mixed check is one public function, `incomplete_request`
  (`crates/loom-commission/src/ports/suspension.rs:154`), asked first by `decide_hold` and by
  `FileSuspensionStore::hold` before its identity check
  (`crates/loom-governor/src/file_suspension.rs:215-218`): "the file store still refuses an
  identity naming another commission first" holds only for a hold that is not mixed.
- The gate tests changed beyond version text: the mutant command of
  `crates/loom-commission/tests/ess_gate.rs` no longer takes `case_id` and gains a
  `negative-revision` branch (`:335`), and `crates/loom-commission/tests/adversary2_gate.rs:119`
  writes the literal `case-mutant`, because the mutants named the dropped relation. Both keep
  the assertion that the gate fails on a refusal `ess` exits zero for.
- `task commission:conform` passes 49 of 49 scenarios; `ess verify conform synthesize --path
  ess/commission` prints `44 scenario(s) (0 authored), 0 refusal(s)`. `ess specify validate`
  warns `ESS-ENTITY-019` on `Suspension.case_id`, as `## Decision` expects.
- `CHANGELOG.md` carries two lines, one a `**Breaking:**` bullet (`HoldSuspensionOutcome` gains
  `IncompleteRequest`; a non-`Decision` hold with no request is held).
- Files `## Scope` did not name before its rewrite: `crates/loom-governor/src/file_suspension.rs`,
  `crates/loom-commission/src/runtime.rs` and `crates/loom-cli/src/lib.rs` (documentation of
  `ResumeError::Store` and of exit status 20, which now name the Suspension with no request),
  and the two files of the security review,
  `crates/loom-commission-testkit/tests/security_w20261121w5_u1.rs` and
  `crates/loom-intake-slice/tests/security_w20261121w5_u1.rs`.
- Security review, pass 1 (`review-result:ess-096-upgrade-security-1`, at `12469d71`): the eight
  invariants hold, 7 new cases, none red. On the Suspension with no request it states: only a
  test, in-process SDK code through `SuspensionPort::hold` or a hand-written file in the state
  directory can make one, and it gives its sender "nothing a plain `Authority` hold does not
  already give" (`approve` writes no grant for it, no resume performs anything). Three notes,
  all handled in `590b01f8`: the two stores answered a mixed hold with a mismatched identity
  differently (now both `incomplete-request`); `ResumeError::Store` and exit status 20 did not
  name the case; a take or release naming neither cannot tell two successive holds with no
  request apart (documented on `decide_take` and `decide_release`; the runtime and the slice
  always name one).
- Security review, pass 2 (`review-result:ess-096-upgrade-security-2`, the diff
  `12469d71..590b01f8`): five points hold, no finding. With the mixed check ahead of the file
  store's identity check, a mixed hold takes no lock and reads no file.
- Found and not changed, older than this wave: for a hold whose identity names another
  commission than its `commission_id` and whose members are all four or none, the in-memory
  store holds it under the identity's commission and the file store answers `storage-failed`
  (`crates/loom-commission-testkit/tests/hold_incomplete_request.rs:507` asserts both). A draft
  story in the backlog carries it.

## Scope

Rewritten 2026-11-21 at the close of wave 2026-11-21-w5 from the unit's commits (`git diff --stat
29e5d199 590b01f8`: 99 files, 1816 insertions, 212 deletions). Every line is cited from that diff.

- **Primary surface:** `ess/` and `crates/loom-commission` (package `b10x-loom-commission`): cited.
  The unit moves three ESS systems, so no one crate holds it: 4 of the 99 changed files are under
  `ess/`, 46 are generated Rust under `generated/`, 25 are generated pages under
  `website/docs/reference/`, and of the 17 crate files `crates/loom-commission` holds 4
  (`src/ports/suspension.rs`, `src/runtime.rs`, two tests) and `crates/loom-commission-testkit`
  holds 4 (all tests); the other 7 crates hold 9 files between them
- **Files (ESS sources):** `ess/commission/domains/responsibility.yaml` (28 lines: the error
  `commission.responsibility.SuspensionRequestIncomplete` with one field `suspension`, the branch
  `incomplete-request` of `HoldSuspension` guarded `not any[all defined, all absent]` over the four
  request members, and the `case` relation of `Suspension` removed, `case_id` kept),
  `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`, `ess/intake/ess-inputs.yaml` (2 lines
  each: `requires: ess 0.96.0`); commit `b8ef4bc0` holds only these four
- **Files (generated, one line per generator):** `generated/rust/loom/` from `task generate` (22
  files), `generated/rust/intake/` from `task intake-generate` (15 files),
  `generated/rust/commission/` from `task commission:generate` (9 files);
  `website/docs/reference/ess/` from `task docs-generate` (23 files);
  `website/docs/reference/commission/` from `task commission:docs` (2 files: `domain-graph.json`,
  `domain-model.mdx`); `website/data/ess/` holds no changed file; `website/docs/reference/cli.md` (3
  lines, the `loom resume` exit-status text copied from `crates/loom-cli/src/lib.rs`)
- **Files (manifests):** `crates/loom-conformance/Cargo.toml`,
  `crates/loom-commission-conformance/Cargo.toml`, `crates/loom-intake-conformance/Cargo.toml` (2
  lines each: the `ess-*` tags move to `0.96.0`), `Cargo.lock` (32 lines, 16 insertions and 16
  deletions: version and source of eight ess packages at `9e91e8b5`)
- **Files (workflows):** `.github/workflows/check.yml`, `.github/workflows/pages.yml` (4 lines each:
  `ESS_VERSION` and `ESS_SHA256`)
- **Files (source):** `crates/loom-commission/src/ports/suspension.rs` (96 lines, 65 added and 31
  removed: new `incomplete_request`, `decide_hold` tests the four members before `held.is_some()`
  and refuses a mixed hold `IncompleteRequest`, keeps `StorageFailed` only for a `Decision` hold
  with all four and holds a non-`Decision` hold with none, module and function documentation, take
  and release documentation naming the no-request case);
  `crates/loom-governor/src/file_suspension.rs` (42 lines, 35 added and 7 removed: `hold` asks
  `incomplete_request` before the identity check, `decode` reads a non-`Decision` file with
  `action_request_id`, `expected_case_revision` and `action` all `null` and `arguments` `null` as a
  Suspension with no request, module documentation); `crates/loom-commission-conformance/src/lib.rs`
  (9 lines: the constant `SUSPENSION_REQUEST_INCOMPLETE` and the `incomplete-request` arm of
  `hold_suspension`); `crates/loom-commission/src/runtime.rs` (3 lines: the `ResumeError::Store`
  documentation names a Suspension holding no request whose reason is no decision);
  `crates/loom-cli/src/lib.rs` (3 lines: exit status 20 text)
- **Files (new tests):** `crates/loom-commission-testkit/tests/hold_incomplete_request.rs` (539
  lines, 10 cases, the story's order test over `SuspensionStore` and `FileSuspensionStore`),
  `crates/loom-commission-testkit/tests/security_w20261121w5_u1.rs` (673 lines, 5 cases) and
  `crates/loom-intake-slice/tests/security_w20261121w5_u1.rs` (231 lines, 2 cases), the security
  review's pass 1 cases, committed in `7ffc43fa`
- **Files (existing tests):** `crates/loom-commission-testkit/tests/security_w20261121w3_u2.rs` (91
  lines, 76 added and 15 removed: a mixed hold expects `IncompleteRequest`, the file with no members
  reads back as a Suspension with no request, `hold-authority-without-members` is held and written)
  and `security_w20261121w3_u2_pass2.rs` (45 lines, 36 added and 9 removed: the 28 mixed
  combinations expect `IncompleteRequest`, the no-member hold of a non-`Decision` reason is stored);
  `crates/loom-commission/tests/ess_gate.rs` (22 lines) and
  `crates/loom-commission/tests/adversary2_gate.rs` (12 lines): the pin text `0.96.0` and the
  fixture command drops `case_id` (`IssueFrontier` sets `case-mutant`, the other gains a
  `negative-revision` branch) because 0.95.0 refuses the input;
  `crates/loom-executor/tests/ess_gate.rs` (4 lines) and
  `crates/loom-executor/tests/adversary2_ess_gate.rs` (2 lines): the pin text
- **Files (documents):** `AGENTS.md` (2 lines: the pin in § ESS), `README.md` (4 lines: the
  version), `CHANGELOG.md` (2 lines: the move under `### Changed` and the `**Breaking:**` line for
  `HoldSuspensionOutcome::IncompleteRequest`)
- **Symbols:** added `b10x_loom_commission::ports::suspension::incomplete_request(&HoldSuspension)
  -> Option<HoldSuspensionOutcome>`; generated and added `HoldSuspensionOutcome::IncompleteRequest {
  error }` and `SuspensionRequestIncomplete { suspension: SuspensionKey }` in
  `generated/rust/commission/src/responsibility.rs`; changed behaviour with the same signature:
  `decide_hold`, `SuspensionStore::hold`, `FileSuspensionStore::hold` and its read of a stored file;
  the `Suspension` doc line `Carries case` is gone, no relation code existed; none removed;
  conformance constant and `hold_suspension` arm are private
- **Not predicted and touched:** `crates/loom-commission/src/runtime.rs`,
  `crates/loom-governor/src/file_suspension.rs`, `crates/loom-cli/src/lib.rs`,
  `website/docs/reference/cli.md`,
  `crates/loom-commission-testkit/tests/security_w20261121w5_u1.rs`,
  `crates/loom-intake-slice/tests/security_w20261121w5_u1.rs`; `Cargo.lock`, `generated/**`,
  `website/docs/reference/{ess,commission}/**`, `README.md`, `AGENTS.md` and `CHANGELOG.md` are
  named by the stored Scope section but not by the typed scope
- **Predicted and not touched:** none; all eighteen typed scope paths are in the diff
- **Review:** `review-result:ess-096-upgrade-acceptance-1`: needs-revision, five findings on the
  acceptance bullets; `review-result:ess-096-upgrade-acceptance-2`: approve, all five answered;
  `review-result:ess-096-upgrade-design-1`: needs-revision, the
  `commission-docs-read-one-ess-version-line` sentence; `review-result:ess-096-upgrade-design-2`:
  approve, answered; `review-result:ess-096-upgrade-security-1`: green at `12469d71` plus two
  untracked test files (7 cases added, none red, 539 to 544 and 555 to 557 executed), three notes
  with verdict INFEASIBLE, the third (a mixed hold with another commission's identity answered
  `StorageFailed` on the file store) fixed in `590b01f8`;
  `review-result:ess-096-upgrade-security-2`: pass 2 of the security review, on the diff
  `12469d71..590b01f8`: five points hold, no finding, no case added

## Source

beyond10x/ess `CHANGELOG.md` at tag `0.96.0`, entries 0.87.0 to 0.96.0; probe run log
`<scratch>/ess096/run.md` (copy `.engineering/drafts/ess096-probe-run.md`, not tracked); probe
suites `<scratch>/ess096/opt1/suite.json` and `<scratch>/ess096/opt2/suite.json`;
`story:ess-067-upgrade` (the same procedure).
