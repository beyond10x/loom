---
format: aep.planning-md/3
id: story:child-record-in-the-run-log
kind: story
status: active
title: A child's case_outcome record appears in exactly one run-log line per write, and the status entry names the child
summary: optional record and record_digest on child opened and child closed, name on a case_outcome status entry; Runbook's daemon needs every record in one run-log line; after the stub-cycle sha, before concurrent-confined-runs
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:tree-run-command
- depends_on: story:open-children-from-arguments-from
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T04:04:26Z", actor: "agent:loom", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-22T05:48:23Z", actor: "agent:sdc", revision: 5, executor: "agent:loom", correlation: "runbook-activation-2026-11-22T0549Z"}
---
## Outcome

Every write of a child's `case_outcome` record shows up in exactly one line of the parent's run
log, and that line says which record it was and what it held:

- `ChildOpened` and `ChildClosed` gain two optional members, `record` (the record's evidence id,
  the link's `outcome_evidence`) and `record_digest` (the SHA-256 of the record's facts as the
  governor wrote them, 64 lower-case hex digits). The digest uses the same encoding as
  `decision_digest`: `serde_json::to_vec` of the plain facts.
- The two members come together or not at all. `verify` refuses a line that holds one without the
  other.
- `child opened` names the record as written at the opening. `child closed` names it as replaced
  at the termination: same id, a new digest.
- In `tree status`, an evidence entry of kind `case_outcome` carries `name`, the child's link name,
  joined from the parent's `children[].name` by `outcome_evidence`. Other entries carry no `name`.

Decision, taken on default: the members go on the two child lines. Runbook may choose the
alternative, one new line `child record {child, name, kind, record, record_digest}` after each
write. That would add a tenth `RunLogEvent` kind and a line between `action finished` and `child
opened`.

## Why

Runbook's daemon forwards run-log lines as events and requires every record written during a run
to appear in exactly one run-log line. A child's `case_outcome` record breaks that rule:

- The governor writes it when the child opens and replaces it in place, under the same evidence
  id, when the child terminates.
- No line names it. `child opened` holds `child` and `protocol`, and `child closed` holds `child`
  and `outcome`.
- `action finished` leaves `case_outcome` records out on purpose, because no step of the parent
  gave them.

Runbook therefore sees a record it cannot place. With the record's digest it can also tell the
opening's write from the termination's.

The format version: the members are additive for readers of this version. The decoder takes an
absent optional member as `None`, so an old log reads unchanged. They are not additive for an
older reader. The decoder is closed (`closed_at`) and refuses a member its kind does not declare,
so a reader from before this change refuses a new line. Default: keep `run-log-v1`. The only
reader outside Loom is Runbook, which asked for the change, and `CHANGELOG.md` says that older
readers refuse the new lines. The alternative is a `run-log-v2` contract. Runbook may choose it.

## ESS first

`ess/` changes, in its own first commit: `ess/domains/runlog.yaml` adds `record` and
`record_digest`, both `Optional<String>`, to `loom.runlog.ChildOpened` and
`loom.runlog.ChildClosed`. A comment states the pairing.

`ess/domains/tree.yaml` adds `name: Optional<String>` to `loom.tree.StatusEvidence`.

ESS field types cannot state the pairing rule ("both or neither"). The unit records it as the red
check `a_child_line_with_record_but_no_record_digest_is_refused` in `crates/loom-runlog`, until
`story:ess-rules-of-the-runtime` gives ESS a form for it.

Then run `task drift` and `task docs-generate`. They regenerate `generated/rust/loom/src/runlog.rs`,
`generated/rust/loom/plan.json`, `docs/contracts/run-log-v1.json` and
`website/docs/reference/ess/loom-runlog.md`.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no model call, no
network. Each test is written first and fails first.

- In a two-level tree run, each write of a child's `case_outcome` record appears in exactly one
  line of the parent's log. The opening write appears in `child opened` and the termination write
  in `child closed`, each with `record` equal to the link's `outcome_evidence` and
  `record_digest` equal to the SHA-256 of the facts held right after that write. Test:
  `each_write_of_a_childs_record_appears_in_exactly_one_parent_line`.
- The two digests of one child differ: the record is replaced, not repeated. Test:
  `the_closing_digest_differs_from_the_opening_digest`.
- Two planted defects each turn the first test red, and the proof is named in the unit's report:
  `child closed` written without `record`, and the `case_outcome` filter in `evidence_ids`
  removed, which makes the record appear twice. Test:
  `each_write_of_a_childs_record_appears_in_exactly_one_parent_line`.
- `verify` refuses a `ChildOpened` or `ChildClosed` line that holds `record` without
  `record_digest`, naming the line as `BrokenChain`, with a detail that names the missing member.
  Test: `a_child_line_with_record_but_no_record_digest_is_refused`.
- The reverse case is refused the same way: `record_digest` without `record`. Test:
  `a_child_line_with_record_digest_but_no_record_is_refused`.
- A `record_digest` that is not 64 lower-case hex digits is refused. Test:
  `a_record_digest_that_is_no_sha256_is_refused`.
- A log written before this change still verifies, and its child lines read back with both members
  `None`. The test uses fixed bytes in the test, not a log the new code wrote. Test:
  `an_old_log_without_the_record_members_still_verifies`.
- The hash chain verifies over a log that holds the new members. A changed byte in a
  `record_digest` is refused as `BrokenChain` and names the next line. Test:
  `the_hash_chain_verifies_over_child_lines_with_a_record`.
- Encoding then decoding a child line with the members gives the same line, and the members are
  written after the line's own members, in declaration order. Test:
  `a_child_line_with_a_record_round_trips_byte_for_byte`.
- In `tree status`, the evidence entry of a `case_outcome` record carries `name`, the child's
  link name, and every other entry carries none. Test:
  `a_status_names_the_child_of_each_case_outcome_record`.
- The event observer of the tree run sees the new members on the lines it is given. Test:
  `an_observer_is_given_the_record_of_each_child_line`.

## Out of scope

- Records of kinds other than `case_outcome`. `action finished` already names those.
- Any change to when the governor writes or replaces the record.
- A `run-log-v2` contract. It is the alternative, not the default.
- Runbook's own reader.
- Lines for a child opened outside a tree run.

## Scope

- `ess/domains/runlog.yaml`: `ChildOpened` at `:119-125` and `ChildClosed` at `:129-137`. Cited.
- `ess/domains/tree.yaml`: `loom.tree.StatusEvidence` at `:180-200`. Cited.
- `generated/rust/loom/src/runlog.rs`, `generated/rust/loom/plan.json`: regenerated. Cited, as
  files holding `ChildOpened`.
- `docs/contracts/run-log-v1.json`, `website/docs/reference/ess/loom-runlog.md`: regenerated.
  Cited.
- `crates/loom-runlog/src/lib.rs`, cited:
  - the encoder arms at `:498-505`;
  - `event_members` at `:552-553`;
  - the decoder arms at `:651-658`;
  - `closed_at` at `:586`;
  - `optional_text` at `:680-685`.
- `crates/loom-runlog/tests/run_log.rs`: `event` at `:66`, and new tests. Cited.
- `crates/loom-governor/src/children.rs`, read only, cited:
  - the record written from the link at `:272`;
  - `outcome_evidence` derived at `:538`;
  - the record replaced at `:736`.
- `crates/loom-tree/src/run.rs`, cited:
  - `child closed` at `:483-491`;
  - `child opened` at `:557-563`;
  - `evidence_ids`, which leaves `case_outcome` out, at `:679-692`;
  - the digest convention at `:609-612`.
- `crates/loom-tree/src/status.rs`: `evidence` at `:192-235`, called at `:152`. Cited. Whether the
  status reads the parent's `children` beside `state.evidence` is inferred.
- `crates/loom-tree/src/encode.rs`: `name` in the status entry. Inferred.
- `crates/loom-cli/tests/tree_run.rs`: next to
  `a_status_lists_each_record_in_the_order_received` at `:673` and the observer test at `:524`.
  Cited.
- `AGENTS.md`: § Run log at `:1479`. Cited.
- `CHANGELOG.md`: `[Unreleased]` at `:13`, including that older readers refuse the new lines.
  Cited.
- No hand-written run-log guide page exists under `website/docs`. Only the generated reference
  page and the crate row at `website/docs/reference/crates.md:43` exist. The crate row is
  unchanged. Cited.

Edges:

- depends_on `story:tree-run-command` and `story:open-children-from-arguments-from`.
- Collides with `story:ess-rules-of-the-runtime` on `ess/domains/runlog.yaml` and
  `crates/loom-runlog`. That story starts after this one.

- Shares the sync sites of `crates/loom-runlog/src/lib.rs` (`:295`, `:304`) with
  `story:tree-run-tests-fit-the-budget`. A sync this story adds goes through that story's
  `durable` door of `b10x-loom-governor`, and its test `no_sync_outside_the_door` holds.
