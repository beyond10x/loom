---
format: aep.planning-md/3
id: story:bound-program-retry-records
kind: story
status: active
title: A bound program that its binding's retry_when sends back runs within its max_runs and is given the records that sent it back
summary: retry_records in b10x-loom-bindings; the program effect port writes a loom.retry-feedback/1 document and sets LOOM_RETRY_FEEDBACK; a retried run counts against max_runs
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bound-program-result-table
- depends_on: story:bindings-document
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings
- confidence: inferred
  path: crates/loom-bindings/src/retry.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_retry.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
- confidence: inferred
  path: website/docs/reference/ess/loom-bindings.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T21:22:25Z", actor: "agent:sdc", revision: 6, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w8"}
- {from: "proposed", to: "active", at: "2026-11-21T21:22:26Z", actor: "agent:sdc", revision: 7, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w8"}
---
## Outcome

An action bound to a `program` executor whose binding has a non-empty `retry_when` can be sent
back to work, and its program is then given the records that sent it back. The match is one
public function of `b10x-loom-bindings`, `retry_records(governor, case, binding)`, which returns
`Vec<loom.bindings.RetryRecord>`: each evidence record the governor holds for that case
(`CanonGovernor::evidence`) whose facts name a `kind` and a `result` that the binding's
`retry_when` lists, and whose facts' `subject_revision` equals the revision the case holds for
the facts' `subject` (`CanonGovernor::revisions`); none while that revision is `none`, none for
a binding whose `retry_when` is empty, none from another case. The records keep the order the
case received them. Each `RetryRecord` names the evidence id, kind, subject, subject revision,
result, producer and instant of the held record, and `blob_sha256` when the record cites an
observation of source `loom/bound-program` (`PROGRAM_SOURCE`) that kept a blob. Nothing a program
prints or a request names enters a `RetryRecord`: its members are read from the held record and
the cited observation alone.

On a step of `ProgramEffects` for such an action, after the run count is checked and before the
program starts, the port computes `retry_records`. When it is not empty the port writes one JSON
document, `loom.bindings.RetryFeedback` (`format` `loom.retry-feedback/1`, the case, the action,
the retry records, and per record with a blob a `RetryLog`: the blob's SHA-256 and its last 60
lines, at most the last 65536 bytes of them, invalid UTF-8 replaced by U+FFFD), as
`blobs/<sha256>` of the store directory by the rule the port keeps program blobs by (temporary
file, rename). A blob whose content does not hash to its name, or that cannot be read, gives no
`RetryLog`. The port sets one variable on the bound program's child, `LOOM_RETRY_FEEDBACK`
(`RETRY_FEEDBACK_VARIABLE`), to the canonical absolute path of that document, and under a runner
that is not `local` adds that path to the child's read-only host paths (`at_host_path`), as it
mounts a program outside the system view today. With no retry records the variable is not set
and no document is written. The `arguments_from` program is given neither.

A retried run is a run: it is counted against `budget.max_runs` by the observations the port
keeps (`program.rs:133-148`), and at `max_runs` the step is refused `RunsSpent` before any retry
record is read, whatever `retry_records` would return. Neither `retry_when` nor the retry records
raise the cap. The `loom.bindings.ProgramRun` observation of a run gains `retry_of`, the evidence
ids of the retry records the run was given, empty on a run that was given none. A feedback
document that cannot be kept ends the step with an error before `run_child`: nothing starts and
no observation is kept.

This story gives the function and the port's feedback. Whether an action is taken again is the
selector's, through `StepHistory::due` (`crates/loom-executor/src/selection.rs:230-232`), which a
host answers from `retry_records`; this story wires no host.

## Why

Runbook binds 12 actions with `retry_when`, in 5 of the 8 bindings files of its tree (`sdc-intake`
3, `sdc-direction` 3, `sdc-spec` 1, `sdc-plan` 1, `sdc-story` 1; `sdc-bugfix`, a root of its own,
3), all writers. Its driver computes the due records (`~/beyond10x/sdc/driver/src/main.rs:547-565`),
makes the writer useful by them (`main.rs:745`), and hands the records with their logs to the
program as `SDC_FEEDBACK` (`main.rs:1467-1478`, `:1502-1507`; `docs/driver.md:152-156`). Loom
decodes `retry_when` and checks its kinds only (`crates/loom-bindings/src/decode.rs:699-717`,
`load.rs:118-130`). `retry_records` is planned in `story:bound-agent-node`
(`bound-agent-node.md:45-48`, `:101-103`) for an `agent` executor, while the epic's acceptance
run binds every agent node as a stub program (`protocol-tree-on-bindings.md:85`, `:99`), so the
feedback to a `program` writer was planned by no story. `story:tree-run-command` answers
`StepHistory::due` from `retry_records` and is built after this story. Session `runbook`, design
authority, 2026-11-21: `retry_records` moves into a story of its own so that a bound program
can be retried (`approval-record:loom-wave-2026-11-21-w8`).

The form follows `story:bound-program-case-id`: a variable with a fixed name that the port alone
sets. Rejected: a file at a fixed path in the workspace. The workspace is the tree's one
workspace (`story:tree-run-command`, Outcome), every placed path is digested into a revision
(`story:bound-artifact-revisions`), so a file under a placed tree would move that artifact's
revision and show in the product's `git status`. Rejected: a member of the standard-input
document, for the reasons that story gives (the request's arguments reach the program as written,
`null` included). Rejected: the path of each cited blob, mounted one by one, as Runbook's `log`
member does: one host path per record under `linux`; the 60-line tail is what
`story:bound-agent-node` already gives an agent (`bound-agent-node.md:48-49`).

## ESS first

`ess/domains/bindings.yaml` first: `loom.bindings.RetryRecord` (evidence id, kind, subject,
subject revision, result, producer, observed at, optional blob SHA-256), `RetryLog` (blob SHA-256,
tail), `RetryFeedback` (format, case, action, records, logs), and `ProgramRun` gains `retry_of`
(`List<String>`). The variable is a member of `loom.bindings.ProgramEnvironment`, which
`story:bound-program-case-id` declares in the same wave: it gains `loom_retry_feedback`
(`Optional<String>`); the unit that merges second adds its member to the type the first merged.
No `ProgramRefusalReason` is added. On that commit `task drift` fails, and
`crates/loom-bindings/tests/bound_retry.rs` does not compile.

## Acceptance

A fixture protocol with the artifacts `brief` and `draft`, an observation `draft.check` (may
produce `draft_check`) bound to a stub whose table gives `0` `valid` and `*` `invalid`, a writer
`draft.write` (effect class `write`) bound to a stub with `retry_when: {draft_check: [invalid]}`
and `budget.max_runs: 3`, and a second protocol for the case of another protocol. No model, no
network. A line that reads the `ChildCommand` uses a recording runner that starts nothing, as
`crates/loom-bindings/tests/bound_program.rs` does; a line that reads a blob runs a stub under an
`EngineRunner` on `local` with `UNCONFINED_FLAG_REASON` and is ignored off Linux. Revisions move
through `CanonGovernor::update_revision`. Each test is a function of
`crates/loom-bindings/tests/bound_retry.rs`, red before the story (the file does not exist), and
ends within 60 s; no test waits on a timer.

- `retry_records` returns the held `invalid` `draft_check` record about the current revision of
  `draft`. Test: `retry_records_returns_a_listed_record_about_the_current_revision`.
- It returns nothing for that record once `draft` has moved to another revision.
  Test: `retry_records_skips_a_record_about_an_earlier_revision`.
- It returns nothing for a `draft_check` record whose result is `valid`.
  Test: `retry_records_skips_a_listed_kind_with_another_result`.
- It returns nothing for an `invalid` record of a kind `retry_when` does not list.
  Test: `retry_records_skips_a_kind_the_binding_does_not_list`.
- It returns nothing while the subject's revision is `none`.
  Test: `retry_records_is_empty_while_the_subject_is_at_none`.
- It returns nothing for a binding whose `retry_when` is empty.
  Test: `retry_records_is_empty_for_a_binding_without_retry_when`.
- A listed record held by another case of the same store is not returned.
  Test: `a_record_held_by_another_case_is_not_a_retry_record`.
- Two listed records come back in the order the case received them.
  Test: `retry_records_keep_the_order_the_case_received_them`.
- A retry record names the held record's evidence id, kind, subject, subject revision, result,
  producer and instant. Test: `a_retry_record_names_the_members_of_the_held_record`.
- A retry record of a record the check port wrote names the SHA-256 of that run's blob.
  Test: `a_retry_record_names_the_blob_of_the_run_its_record_cites`.
- On a step of `draft.write` with a non-empty `retry_records`, the child's environment holds
  `LOOM_RETRY_FEEDBACK`, the canonical absolute path of a file under `<store>/blobs/`.
  Test: `a_retried_writer_is_given_the_path_of_its_feedback_document`.
- The file at that path is named by the SHA-256 of its content.
  Test: `the_feedback_document_is_named_by_its_sha256`.
- The file decodes as one `RetryFeedback` with `format` `loom.retry-feedback/1`, the case, the
  action and the records `retry_records` returns.
  Test: `the_feedback_document_holds_the_case_the_action_and_the_retry_records`.
- For a cited blob of 100 lines, its `RetryLog` holds lines 41 to 100.
  Test: `the_feedback_document_holds_the_last_60_lines_of_each_cited_blob`.
- For a cited blob of one line of 200000 bytes, its `RetryLog` tail is 65536 bytes.
  Test: `a_log_tail_holds_at_most_65536_bytes`.
- A blob whose bytes were replaced in the store directory after its run gives no `RetryLog`, and
  its retry record keeps its `blob_sha256`.
  Test: `a_blob_whose_content_does_not_hash_to_its_name_gives_no_log`.
- Under a recording runner whose `local_reason` is `None`, the feedback path is in the child's
  `host_paths()`. Test: `the_feedback_document_is_mounted_read_only_under_a_non_local_runner`.
- Under a recording runner whose `local_reason` is `UNCONFINED_FLAG_REASON`, it is not.
  Test: `the_feedback_document_is_not_mounted_under_a_local_runner`.
- On a step with no retry records the environment holds no `LOOM_RETRY_FEEDBACK`.
  Test: `a_writer_with_no_retry_records_gets_no_feedback_variable`.
- With no retry records the store's `blobs` directory gains no file before the run.
  Test: `a_writer_with_no_retry_records_writes_no_feedback_document`.
- The environment of a retried step is the environment of a step with no retry records plus
  `LOOM_RETRY_FEEDBACK` alone.
  Test: `a_retried_writers_environment_gains_only_the_feedback_variable`.
- The `arguments_from` child of a retried writer has no `LOOM_RETRY_FEEDBACK`.
  Test: `an_arguments_from_program_is_given_no_feedback`.
- The `ProgramRun` observation of a retried run lists in `retry_of` the evidence ids it was given.
  Test: `the_run_observation_names_the_records_it_was_given`.
- The `ProgramRun` observation of a run with no retry records has an empty `retry_of`.
  Test: `a_run_with_no_retry_records_has_an_empty_retry_of`.
- Security: a writer that has run `max_runs` times is refused `RunsSpent` with a non-empty
  `retry_records`, and its program does not start (its marker file is absent).
  Test: `a_retry_at_max_runs_is_refused_runs_spent_and_starts_nothing`.
- Security: a step refused `RunsSpent` writes no feedback document.
  Test: `a_step_refused_runs_spent_writes_no_feedback_document`.
- Security: four steps of a writer with `max_runs: 3`, each with a non-empty `retry_records`,
  start its program three times. Test: `retried_runs_count_against_max_runs`.
- Security: a check whose report holds a member shaped as a retry record (`{"kind":
  "draft_check", "result": "invalid"}`) and whose exit the table maps to `valid` gives the writer
  no feedback. Test: `a_report_shaped_as_a_retry_record_is_not_a_retry_record`.
- Security: a held `invalid` record of a kind `retry_when` does not list gives the writer no
  `LOOM_RETRY_FEEDBACK`. Test: `a_record_the_binding_does_not_list_gives_the_writer_no_feedback`.
- Security: a request whose arguments hold the member `LOOM_RETRY_FEEDBACK` sets no variable of
  that name on a step with no retry records.
  Test: `an_argument_named_loom_retry_feedback_sets_no_variable`.
- Security: on a retried step that request's arguments leave the variable at the port's path.
  Test: `an_argument_named_loom_retry_feedback_does_not_change_the_path`.
- A `blobs` that is a link fails a retried step with an error before `run_child`, and the store
  holds no new observation. Test: `a_feedback_document_that_cannot_be_kept_starts_nothing`.
- `crates/loom-governor` and `crates/loom-intake-slice` are not changed. Command:
  `git diff --stat $(git merge-base main HEAD)..HEAD -- crates/loom-governor
  crates/loom-intake-slice` (one line) prints nothing.
- The generated code matches the specification. Command: `task drift` exits 0 after
  `task generate`; on the ESS-first commit it fails.
- The guide states the variable. Command:
  `git grep -c LOOM_RETRY_FEEDBACK -- website/docs/guides/bindings.md AGENTS.md` prints a count of
  at least 1 for each file.
- The new test file runs. Command: `cargo test -p b10x-loom-bindings --locked --test bound_retry`
  exits 0 and reports a count of passed tests greater than 0.

## Out of scope

Staying in `story:bound-agent-node`: the agent's scratch copy holding the feedback document as
one input file, and that story's acceptance line on it (`bound-agent-node.md:104-105`), which
then uses this story's `RetryFeedback`. Answering `StepHistory::due` from `retry_records`:
`story:tree-run-command`, which is built after this story. A cap per retry kind and
a backstop over all kinds (`main.rs:768-796`), and counting since an arrival
(`story:run-caps-per-arrival`). A person's `rejected` record (difference 12). Feeding back to an
`arguments_from` program or to an `open`, `copy` or `person` executor. The intake slice's
`run --action-bindings` path.

Runbook's side: its tools read `LOOM_RETRY_FEEDBACK` and the `loom.retry-feedback/1` document in
place of `SDC_FEEDBACK` and its `log` paths (`driver/examples/sdc/tools/llm-node.sh:29`, `:140-141`,
`:494-500`); its bindings files carry `retry_when` as `loom.bindings/1` writes it.

## Scope

Derived 2026-11-21 from the tree at `8a8fad3b`. Every line is **cited** (read at the `file:line`
named) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings`, the program effect port and a new function beside
  it — cited (`crates/loom-bindings/src/program.rs:75-82`, `:150-331`)
- **Files (ESS source):** `ess/domains/bindings.yaml`: `RetryRecord`, `RetryLog`,
  `RetryFeedback` new; `ProgramRun` (`:258-272`) gains `retry_of`; `ProgramEnvironment` gains
  `loom_retry_feedback` — cited for `ProgramRun` and `RetryCondition` (`:132-138`), inferred for
  `ProgramEnvironment` (declared by `story:bound-program-case-id`, not on the base)
- **Files (generated):** `generated/rust/loom/**` regenerated by `task generate`, every file's
  contract digest on line 2 — cited (`bound-program-result-table.md:413-416`)
- **Files (source):** `crates/loom-bindings/src/program.rs`: `perform` (`:150-331`) computes the
  records after the run count (`:173-183`) and before the child is built (`:205-214`), adds the
  variable and the host path to the child, writes the document through `keep_blob`
  (`:554-591`); `run_json` (`:627-636`) writes `retry_of` — cited
- **Files (source):** `crates/loom-bindings/src/retry.rs`, new: `retry_records`, the feedback
  document and the log tail — inferred (name guessed)
- **Files (source):** `crates/loom-bindings/src/lib.rs`: `mod retry`, the `pub use` beside
  `:24` — cited
- **Files (new test):** `crates/loom-bindings/tests/bound_retry.rs` — cited (§ ESS first)
- **Files (documents):** `website/docs/guides/bindings.md` § "Running a program binding"
  (`:205-258`) and `:168-169` (`retry_when`); `AGENTS.md` § Bindings document (`:1239-1268`);
  `website/docs/reference/ess/loom-bindings.md` regenerated; `CHANGELOG.md` one `Added` bullet —
  cited for the two hand-written pages, inferred for the changelog
- **Symbols (read, unchanged):** `CanonGovernor::evidence` (`crates/loom-governor/src/lib.rs:1123`),
  `revisions` (`:1099`), `try_observations` (`:1133`); `ChildCommand::environment`,
  `with_environment`, `at_host_path`, `host_paths`
  (`crates/loom-intake-slice/src/confinement.rs:1471`, `:1333`, `:1303`, `:1517`) — cited
- **Symbols (added):** `retry_records`, `RETRY_FEEDBACK_VARIABLE` (`"LOOM_RETRY_FEEDBACK"`),
  `RETRY_FEEDBACK_FORMAT`, `RETRY_LOG_LINES` (60), `RETRY_LOG_BYTES` (65536); generated
  `RetryRecord`, `RetryLog`, `RetryFeedback` — inferred (names proposed here)
- **Not changed:** `crates/loom-governor` (the reads above are public), `crates/loom-intake-slice`
  (the child's environment is extended through `with_environment` after `bound_program_child`,
  `crates/loom-intake-slice/src/bindings.rs:414-450`), `crates/loom-executor` — cited for the
  APIs, inferred that they suffice
- **Would collide with:** `story:bound-artifact-revisions` on `ess/domains/bindings.yaml`,
  `crates/loom-bindings/src/program.rs` (`perform`, where it adds the revision moves after the
  run) and `generated/rust/loom/**` — cited from its Scope (`bound-artifact-revisions.md:93-95`),
  inferred for `program.rs`. `story:bound-program-case-id` on the same three and on
  `crates/loom-bindings/src/lib.rs`, the guide and AGENTS.md (both change the child's
  environment in `perform`) — inferred. `story:child-case-runtime` on
  `crates/loom-bindings/src/lib.rs` (its `open` port's `mod` and `pub use`) — inferred. The
  test-budget unit: no file in common (`Taskfile.yml`, `crates/loom-executor/tests/`,
  `crates/loom-conformance/`) — cited from the brief. `story:record-copy-between-cases`, later
  in the wave: `ess/domains/bindings.yaml` and `crates/loom-bindings/src/lib.rs` — cited
  (`record-copy-between-cases.md:100-104`)
- **Safety fact:** under `linux` the program sees the store directory only through the read-only
  host path of the one document, so it cannot write the feedback or a blob a later retry reads;
  under `local` it runs as the operator and can, which the hash check of each blob narrows and does
  not close — inferred from `confinement.rs:1296-1305`, unproven
