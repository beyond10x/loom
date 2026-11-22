---
format: aep.planning-md/3
id: story:ess-rules-of-the-runtime
kind: story
status: proposed
title: Loom's ESS domains state the runtime's rules, and every finding of the design review is closed
summary: '40 findings of the ESS design review closed in one unit: 37 by a spec rule and a scenario, 2 by correcting the text the code contradicts, 1 as vague; 82 new ESS constructs, 35 authored scenarios, one planted defect per domain; starts after tree-run-command and open-children-from-arguments-from'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:tree-run-command
- depends_on: story:open-children-from-arguments-from
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests
- confidence: inferred
  path: crates/loom-conformance/src
- confidence: inferred
  path: crates/loom-governor/src
- confidence: cited
  path: crates/loom-governor/tests
- confidence: inferred
  path: crates/loom-intake-conformance/src/lib.rs
- confidence: cited
  path: crates/loom-intake-slice/src/bindings.rs
- confidence: cited
  path: crates/loom-intake-slice/tests
- confidence: cited
  path: crates/loom-runlog/tests
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/governor.yaml
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/intake/domains/bindings.yaml
- confidence: cited
  path: ess/intake/domains/confinement.yaml
- confidence: cited
  path: ess/intake/ess-inputs.yaml
- confidence: inferred
  path: ess/intake/scenarios
- confidence: inferred
  path: ess/scenarios
- confidence: cited
  path: generated/rust/intake
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T03:31:15Z", actor: "agent:loom", revision: 6}
---
## Outcome

After this story each of the 40 findings of the ESS design review of Loom (technique 8 of
`ess:hardening`, read at plan head `150855b3`: governor 2, tree 8, runlog 4, bindings 18,
confinement 8; 107 rules checked) is closed, and none is closed by "later". Today the five
domains hold types only: no entity, command, invariant or lifecycle, no scenario reaches them, and
`ess/intake/ess-inputs.yaml` lists `scenarios: []`. After it, each domain states its rules in ESS
and 35 authored scenarios pin them. Timo (typed, 2026-11-22): "Ideally, you fix them all in one
go." Not on the Runbook stub-cycle path; consumer gap moved: none.

**How a finding is closed.** Each row of the table names one way:

- (a) The spec declares the rule (an invariant, a guard, a lifecycle, an enum or a constrained
  newtype in place of a free string, a refusal, a count) and an authored scenario pins it.
  `a/limit`: ESS cannot express the rule, so it is a Rust test in the owning crate and a sentence in
  `AGENTS.md`, and the unit report holds an issue text for beyond10x/ess.
- (b) Spec and design contradict each other. The code at the plan head decides which side holds,
  and the losing text is corrected. Where the code contradicts the winning side, an implementation
  line with a failing test comes first.
- (c) A vague design sentence, or a false positive: one line says why, and the sentence is
  rewritten.

Counts: (a) 37, of which 4 are `a/limit`; (b) 2; (c) 1.

**The two contradictions, decided by the code at plan head `bcc5a8da`.**

- F11 (R1), the run log. `verify` (`crates/loom-runlog/src/lib.rs:349-422`) checks each line
  against the line before it and knows no expected length. So a log cut at a line boundary, a log
  whose last line changed, and a log rewritten from some line on with a fresh chain all verify. The
  crate's doc says so (`:35-38`), and `the_last_line_is_held_by_nothing_after_it`
  (`crates/loom-runlog/tests/adversary_w20261121w3_u1.rs:535`) pins the last-line case. The design
  (`AGENTS.md:1503-1505`, `story:hash-chained-run-log`) holds. What is corrected is the spec's own
  text: the header of `ess/domains/runlog.yaml` (`:3-4`, "a log that was cut, shortened, reordered
  or changed") and its `summary` (`:11-13`). They come to say that a reader refuses a line cut
  inside it, a missing or reordered line and a changed byte before the last line, and that the three
  cases above verify. No code changes.
- F25 (B11), `arguments_from`. The `arguments_from` argv runs through `run_child` at
  `crates/loom-bindings/src/program.rs:617`, inside `arguments` (`:591-640`), and keeps no
  observation. The port's only `observe` call (`:376`) is the run of the bound program (`:343`).
  The run count (`runs`, `:172-190`) counts the `loom/bound-program` observations without a
  `class`. So an `arguments_from` run is not counted, as the design says (`AGENTS.md:1384`,
  `website/docs/guides/bindings.md:270-272`). Corrected: the `ProgramRun` comment
  (`ess/domains/bindings.yaml:294-307`, "for every call of its runner's `run_child`") becomes "for
  every start of the bound program". The crate doc (`program.rs:37-38`) and the guide's repetition
  (`bindings.md:285-286`, "for every call of the runner") are corrected the same way. This is
  re-read at the head after `story:open-children-from-arguments-from` merges: that story moves
  `arguments` into `crates/loom-bindings/src/arguments.rs`, and an `open` binding's
  `arguments_from` there keeps a `loom/open` observation, which the sentence must not count.

**The table.** Shorthand used in it:

- ESS files: `g:` is `ess/domains/governor.yaml`, `r:` is `ess/domains/runlog.yaml`, `b:` is
  `ess/domains/bindings.yaml`, `c:` is `ess/intake/domains/confinement.yaml`, and `ib:` is
  `ess/intake/domains/bindings.yaml`.
- Scenario directories: `s:` is `ess/scenarios/` and `i:` is `ess/intake/scenarios/`. Every
  scenario name ends in `.yaml`.
- `A<n>` is the Acceptance line that tests the row.

```text
F1  G1  governor  opening refuses an undeclared, unlisted, missing or repeated input
    a  g:OpenCase, CaseRefusalReason, CaseRefused  s:governor/input-refused  A3
F2  G2  governor  revision rises by one per artifact moved; a terminated case acts no more
    a  g:Case lifecycle, revision >= 1, RecordRevision  s:governor/revision-and-termination  A4
F3  T1  tree  tree(case) lists root first; a link counts only when both sides hold it
    a  g:TreeOf, its response in tree order  s:case-tree/root-first  A5
F4  T2  tree  a case file whose links name another case is refused on read
    a  g:StoredCase invariants on parent and children; LoadCase  s:case-tree/misfiled-link  A6
F5  T3  tree  a child name is ASCII letters, digits, '-' and '_'
    a  g:ChildName (alphabet) on CaseLink.name  s:case-tree/child-name-with-a-dot  A7
F6  T4  tree  a child opens with no input
    a  g:Case invariant (parent => no input), OpenChildCase  s:case-tree/child-without-inputs  A8
F7  T5  tree  only the governor writes a case_outcome record
    a  g:ReceiveEvidence refusal HostCaseOutcome; actor Governor  s:case-tree/host-case-outcome  A9
F8  T6  tree  terminate needs Canon's legitimacy and the parent's link, else writes nothing
    a  g:TerminateCase (NotLegitimate, LinkNotHeld, retry)  s:case-tree/termination-refused  A10
F9  T7  tree  the parent's record names the child's revision, else the parent's at opening
    a  g:TerminateCase response ChildOutcomeRecord  s:case-tree/outcome-record-revision  A11
F10 T8  tree  a subject the parent lacks is linked at r0; the spellings are declared
    a  g:SubjectRevision, b:PlacedRevision  s:case-tree/subject-the-parent-lacks  A12
F11 R1  runlog  a cut at a line boundary or a changed last line is not detected
    b  r: header and summary corrected  s:runlog/cut-at-a-line-boundary  A13
F12 R2  runlog  ActionFinished holds exactly one of exit and refusal
    a  r:ActionFinished invariant; VerifyRunLog  s:runlog/exit-or-refusal  A14
F13 R3  runlog  seq from 0; prev absent exactly on line 0; prev is 64 lower-case hex digits
    a  r:RunLogLine invariants, LineDigest  s:runlog/line-zero-and-gap  A15
F14 R4  runlog  an append to a log that does not verify, or of an unreadable line, writes nothing
    a  r:AppendRunLog, error RunLogRefused  s:runlog/append-to-a-broken-log  A16
F15 B1  bindings  a tool name is not empty and holds no comma and no white space
    a  b:ToolName on ToolAllowList.allow; CheckBindings  s:bindings-check/tool-name-comma  A17
F16 B2  bindings  budget members from 1; timeout_seconds from 1 to 3600
    a  b:ActionBudget and BoundAction invariants  s:bindings-check/zero-budget-and-timeout  A18
F17 B3  bindings  a member of another executor is refused
    a  b:ExecutorSpec invariants, one per kind  s:bindings-check/member-of-another-executor  A19
F18 B4  bindings  program never reads effect; person and open effect alone; pointer iff Report
    a  b:EvidenceEntry and BoundAction invariants  s:bindings-check/source-the-executor-lacks  A20
F19 B5  bindings  an open takes exactly one of name and each
    a  b:ExecutorSpec invariant  s:bindings-check/open-name-and-each  A21
F20 B6  bindings  arguments beside arguments_from is refused Malformed
    a  b:BoundAction invariant; guide bindings.md:156  s:bindings-check/arguments-twice  A22
F21 B7  bindings  an open's bindings path is read beside the naming file and names a file
    a  b:ChildBindingsPath invariants  s:bindings-check/open-bindings-path  A23
F22 B8  bindings  decoder bounds (64 levels, 100,000 values, 1 MiB, 2048 brackets, tags, <<)
    a/limit  Rust tests in loom-bindings; AGENTS.md section on bindings  A31
F23 B9  bindings  format is loom.bindings/1; protocol a registration; no artifact or producer twice
    a  b:BindingsFormat, Registration, distinct:  s:bindings-check/registration-and-key-twice  A24
F24 B10 bindings  a program port's timeout is 60 s when timeout_seconds is absent
    a/limit  Rust test a_program_without_a_timeout_reads_the_default  A31
F25 B11 bindings  the arguments_from run keeps no observation and is not counted
    b  b:ProgramRun text corrected; guide :285-286  s:bound-step/arguments-from-not-counted  A25
F26 B12 bindings  a program not started (PATH not UTF-8, case id over 4096 bytes) has a reason
    a  b:ProgramNotStarted; RunBoundStep  s:bound-step/program-not-started  A26
F27 B13 bindings  a runner refusal, blob failure, ReportUnreadable, ResultNotInTable write moves
    a/limit  Rust test in loom-bindings; AGENTS.md section on bindings  A31
F28 B14 bindings  BoundStep.discarded is present exactly when the outcome is Discarded
    a  b:BoundStep invariant; RunBoundStep  s:bound-step/non-writer-discarded  A27
F29 B15 bindings  a copy takes the latest observed_at; none earliest; tie: tree order, received
    a  b:CopyRecord  s:record-copy/latest-record-wins  A28
F30 B16 bindings  a held copy is performed and writes nothing; a restart reuses the observation
    a  b:CopyRecord outcome held  s:record-copy/held-copy  A29
F31 B17 bindings  the receiving case judges a copy by its own independence map
    a  b:CopyRecord response CopyAnswer.applies  s:record-copy/receiving-independence  A30
F32 B18 bindings  "a result is never empty": no result is the empty string, [] is allowed
    c  website/docs/guides/bindings.md:186 rewritten  A32
F33 C1  confinement  a local adapter carries a non-blank reason
    a  c:EngineConfinement invariant; CheckEngineConfinement  i:confinement/local-without-reason 
    A33
F34 C2  confinement  adapter local means backend None, no network and a reason
    a  c:AppliedConfinement invariant; CheckAppliedConfinement  i:confinement/local-record  A34
F35 C3  confinement  a linux launch records adapter and facts
    a  c:AppliedConfinement invariant  i:confinement/linux-record-without-facts  A35
F36 C4  confinement  4 roots, 16 files, 4 apertures, stdin 16 MiB, workdir 4096 B, no writable root
    a  c:ConfinementSpec invariants; CheckConfinementSpec  i:confinement/spec-past-its-limits  A36
F37 C5  confinement  a ca_bundle needs an aperture and no trust-store variable beside it
    a  c:ConfinementSpec invariants  i:confinement/ca-bundle-beside-trust-variable  A37
F38 C6  confinement  aperture names and hosts distinct; max_bytes > 0; one aperture, no listen
    a  c:ConfinementSpec, ChildAperture, AppliedConfinement  i:confinement/one-host-twice  A38
F39 C7  confinement  check refuses a bundled protocol, undeclared or doubled action, empty argv
    a  ib:ActionBindingsRefusalReason, CheckActionBindings  i:action-bindings/check-reasons  A39
F40 C8  confinement  a timeout the host's clock cannot hold is refused before anything starts
    a/limit  Rust test a_timeout_too_large_for_the_clock_is_refused_and_starts_no_command  A40
```

F32 is (c) because the code has one rule and the sentence states two. `retry_when` refuses an
empty result string as `Malformed` (`crates/loom-bindings/src/decode.rs:781-787`), and an empty
list decodes (`strings`, `:880-905`, has no length check). The sentence becomes "no result is the
empty string; an empty list sends the action back on no record".

**The constructs, the smallest per finding (`ess:specifying`).** 82 new ones:

- 40 invariants, all on types that exist today. Struct fields keep their wire shape. Two shapes
  were rejected:
  - `ExecutorSpec` stays a struct with an invariant per kind, not a union tagged `kind`. A union
    would replace the decoded type every port reads.
  - `AppliedConfinement.adapter` stays text, held to `{adapter: [linux, local]}`. The enum
    `ConfinementAdapter` spells `Linux` on the wire
    (`generated/rust/intake/src/confinement.rs:154`),
    so typing the field with it would refuse every stored record.
- 7 newtypes in place of free strings: `ChildName`, `SubjectRevision`, `LineDigest`, `ToolName`,
  `Registration`, `ChildBindingsPath` and `PlacedRevision`.
- 4 enums or variants: `CaseRefusalReason`, `BindingsFormat`, `ActionBindingsRefusalReason`, and
  `ProgramNotStarted` in `ProgramRefusalReason`.
- 7 errors and 3 response types.
- One entity with a lifecycle, `loom.governor.Case` (`Open` to `Terminated`): the only record here
  that has a state. One view, `Cases`, over every state, so its invariants are checked.
- Two actors, for the one who-may-write rule. `Host` may open, record, receive and terminate.
  `Governor` (`system: true`) alone may `OpenChildCase`. A host's `case_outcome` record is refused
  `HostCaseOutcome`.
- 16 commands, because a scenario needs a command. There are two kinds:
  - Check commands over a fixture path, with one `external:` refusal per reason, as
    `loom.resource.AdmitLoomAgent` (`ess/domains/resource.yaml:1047`) and its adapter
    (`crates/loom-conformance/src/admission.rs`) do: `CheckBindings`, `VerifyRunLog`, `LoadCase`,
    `CheckEngineConfinement`, `CheckConfinementSpec`, `CheckAppliedConfinement` and
    `CheckActionBindings`.
  - Runtime commands, each answered by the real port over a fixture store: `OpenCase`,
    `OpenChildCase`, `RecordRevision`, `TerminateCase`, `ReceiveEvidence`, `TreeOf`,
    `AppendRunLog`, `RunBoundStep` and `CopyRecord`.

`ess specify validate` and `ess verify conform synthesize` decide each construct in the ESS-only
commit. A construct either of them refuses (for example a `.count` on `Bytes`, or an invariant over
a list that no view publishes) moves its row to `a/limit`, and the unit report names the row and
the refusal code. No `UNMAPPED:` marker is written into `ess/`.

**Cost.** The committed suite `generated/ess/loom/` grows with every command and scenario. Its
regeneration takes 534.5 s, once per batch: under D91 a unit never regenerates
`generated/ess/loom/suite.json`, the batch gate does. The story adds 35 authored scenarios (28 in
`ess/ess-inputs.yaml`, 7 in `ess/intake/ess-inputs.yaml`), and the 16 commands add the scenarios
synthesis writes for them. The unit synthesizes `ess/` and `ess/intake/` into a directory outside
the tree, once at its base and once at its end, in the background, and reports both wall times. A
rise of more than 53 s (10 %) is fixed in the unit: synthesize with one domain's additions at a
time to find the construct that costs it. `intake-conform` synthesizes its suite inside its one
test (`crates/loom-intake-conformance/tests/conform.rs:3-4`). If that test passes 60 s, the intake
suite takes the committed-suite form in this unit. The test step stays within 120 s, and no test
runs over 60 s.

**Order inside the unit.**

1. The first commit changes only `ess/`.
2. `slot task generate` and `slot task intake-generate`, as their own commit.
3. Code and Rust tests: the conformance adapters, `ProgramNotStarted` and the intake check's reason.
4. Docs.
5. Last, the 35 scenarios with their fixtures, and the design-text corrections.

**Dependencies.** `depends_on story:tree-run-command` and
`depends_on story:open-children-from-arguments-from`. The story starts after both are merged.

- `story:tree-run-command` (rev 16) adds `ess/domains/tree.yaml` and changes `ess/system.yaml`,
  `ess/ess-inputs.yaml` and `crates/loom-conformance/src/lib.rs`.
- `story:open-children-from-arguments-from` changes `ess/domains/bindings.yaml` and
  `crates/loom-bindings/src/program.rs`, the file F25 and F26 cite.

Starting on either while it is open means rebasing ESS files and generated trees, which are never
edited by hand.

## Why

The review found 36 rules the design states and the spec never declares, 2 contradictions and 2
vague sentences. In a domain that holds types only, the mutation audit, the random sequences and
the guard analysis have nothing to check. A forged `case_outcome` record, a log trusted as complete
after a cut, or `Read,Bash` taken as one tool are invisible to every generated check. Each later
technique runs against the spec that stays, so the rules come first.

## Acceptance

Every new test ends within 60 s and makes no model call and no network request. New Rust tests go
into the crate's existing integration-test files, the modules of `tests/main.rs` under
`story:one-test-binary-per-crate`. No test target is added. Scenarios run against
`b10x-loom-conformance` (new modules `governor.rs`, `runlog.rs` and `bindings.rs`) and
`b10x-loom-intake-conformance`.

- **A1.** The first commit of the unit changes only `ess/`. On it, `slot task spec` and
  `slot task intake-spec` exit 0, and `git grep -c 'UNMAPPED:' -- ess/` prints nothing.
- **A2.** The commits follow the order in the Outcome. `slot task drift` and
  `slot task intake-drift` exit 0 on every commit after the generate commit.
- **A3.** `s:governor/input-refused`: `OpenCase` answers `CaseRefused` with `UndeclaredInput`,
  `UnlistedInputValue`, `MissingInput` and `RepeatedInput`, one step each, and `Cases` holds no row.
- **A4.** `s:governor/revision-and-termination`: two `RecordRevision` steps take the revision from
  1 to 3. After `TerminateCase`, `RecordRevision` answers wrong-state.
- **A5.** `s:case-tree/root-first`: `TreeOf` from a leaf lists the root first, and leaves out a
  child
  whose link the parent does not hold. `the_cases_of_a_tree_are_listed_from_any_of_them_with_each_`
  `parent_link` (`crates/loom-governor/tests/child_cases.rs:788`) stays green.
- **A6.** `s:case-tree/misfiled-link`: `LoadCase` refuses `MisfiledLink`.
  `a_case_file_holding_a_link_of_another_case_is_refused_on_read` (`child_cases.rs:640`) stays
  green.
- **A7.** `s:case-tree/child-name-with-a-dot`: an opening name `a.b` is refused `InvalidChildName`,
  and no child exists.
- **A8.** `s:case-tree/child-without-inputs`: `OpenChildCase` creates a `Case` row with no input.
- **A9.** `s:case-tree/host-case-outcome`: `ReceiveEvidence` of a `case_outcome` record for a
  `case_open` protocol is refused `HostCaseOutcome`. With the host fixture's
  `with_host_case_outcomes`, a lone case receives it.
- **A10.** `s:case-tree/termination-refused`: `TerminateCase` answers `NotLegitimate` and then
  `LinkNotHeld`, and writes nothing. After a cut replacement, the same outcome completes the
  replacement, and another outcome answers wrong-state.
- **A11.** `s:case-tree/outcome-record-revision`: the parent's record names the child's own revision
  of the subject when its protocol declares one, else the parent's revision at opening.
- **A12.** `s:case-tree/subject-the-parent-lacks`: the link's `subject_revision` is `r0`
  (`UNREVISED`, `crates/loom-governor/src/children.rs:30`).
- **A13.** `s:runlog/cut-at-a-line-boundary`: a 3-line log cut after line 1 verifies with 2 lines.
  The test `a_log_cut_at_a_line_boundary_verifies` (loom-runlog) is green.
  `git grep -n 'cut, shortened' -- ess/domains/runlog.yaml` prints nothing.
- **A14.** `s:runlog/exit-or-refusal`: an `ActionFinished` line with both, and one with neither, are
  each refused by `VerifyRunLog`, with the refusal the crate gives for its position (`CutLine` as
  the last line, `BrokenChain` before it).
- **A15.** `s:runlog/line-zero-and-gap`: line 0 with a `prev` is refused `BrokenChain` ("line 0
  names
  a previous line"), and a missing line is refused `SequenceGap`.
- **A16.** `s:runlog/append-to-a-broken-log`: `AppendRunLog` to a log that does not verify answers
  its `RunLogRefused`, and an `at` that is not an RFC 3339 instant is refused. A following
  `VerifyRunLog` shows the same bytes. Crash atomicity stays with
  `a_crash_at_each_step_of_an_append_leaves_n_or_n_plus_1_lines_that_verify`
  (`crates/loom-runlog/tests/run_log.rs:576`).
- **A17.** `s:bindings-check/tool-name-comma`: `Read,Bash` is refused `InvalidToolName`, naming
  `tools.allow[0]`.
- **A18.** `s:bindings-check/zero-budget-and-timeout`: `max_runs: 0` is refused `InvalidBudget`, and
  `timeout_seconds: 0` and `3601` are refused `TimeoutOutOfRange`.
- **A19.** `s:bindings-check/member-of-another-executor`: a `program` with a `principal`, and a
  `person` with an `argv`, are refused `MemberNotForExecutor`.
- **A20.** `s:bindings-check/source-the-executor-lacks`: a `program` entry reading `effect`, a
  `person` entry reading `exit`, and a pointer entry with no pointer are each refused with their
  reasons.
- **A21.** `s:bindings-check/open-name-and-each`: an `open` with both `name` and `each`, and one
  with
  neither, are refused.
- **A22.** `s:bindings-check/arguments-twice`: `arguments` beside `arguments_from` is refused
  `Malformed` (`decode.rs:261-267`). The guide's sentence at `bindings.md:156` says so.
- **A23.** `s:bindings-check/open-bindings-path`: the `open` paths `/abs`, `.` and `dir/` are
  refused. The test `an_open_bindings_path_is_read_beside_the_naming_file` (loom-bindings, added or
  named in the report) is green.
- **A24.** `s:bindings-check/registration-and-key-twice`: a `format` other than `loom.bindings/1` is
  refused `UnsupportedFormat`, `Software_Change@01` is refused `InvalidProtocol`, and an artifact
  placed twice is refused.
- **A25.** `s:bound-step/arguments-from-not-counted`: with `max_runs: 1` and an `arguments_from`
  argv, the first step is recorded and the second is refused `RunsSpent`. The test
  `an_arguments_from_run_keeps_no_observation_and_is_not_counted` (loom-bindings) is green. The
  corrected sentences in `bindings.yaml`, `program.rs` and `bindings.md` hold the words "start of
  the bound program".
- **A26.** First, the failing test `a_program_that_is_not_started_is_refused_program_not_started`
  (loom-bindings, beside `bound_program.rs:1609`) is red on the commit before the code change. Then
  the PATH and case-id paths answer `ProgramNotStarted` (`program.rs:336`, `:615`), and
  `s:bound-step/program-not-started` (a case id over 4096 bytes) passes.
  `a_non_utf8_path_refuses_the_step_with_the_reason_the_outcome_states` and
  `a_case_id_longer_than_4096_bytes_refuses_the_step` (`bound_case_id.rs:674`) state the new text.
- **A27.** `s:bound-step/non-writer-discarded`: a `read` action whose program changed a placed file
  answers `Discarded` with `ChangedByNonWriter`, and its moves are written.
- **A28.** `s:record-copy/latest-record-wins`: of three records, the copy takes the latest
  `observed_at`. Of two at one instant, it takes the later in tree order.
- **A29.** `s:record-copy/held-copy`: a second `CopyRecord` answers `held` and writes nothing.
  `a_restart_between_the_observation_and_the_record_is_completed_without_a_second_record`
  (`crates/loom-bindings/tests/bound_copy.rs:782`) stays green.
- **A30.** `s:record-copy/receiving-independence`: a record that applies in its source case, copied
  into a case whose `independence` map does not grant that producer, answers `applies: false`.
- **A31.** `a/limit` rows F22, F24 and F27:
  - F22: `nesting_at_the_cap_decodes_and_one_past_is_refused` (`bindings_document.rs:989`),
    `alias_expansion_is_bounded_by_the_document_limit` (`adversary_w20261121w1_u2.rs:262`) and
    `tags_are_refused_in_every_position` (`:177`) are green.
  - F24: `a_program_without_a_timeout_reads_the_default` (`program_contract.rs:701`) is green.
  - F27: `a_step_refused_result_not_in_table_still_writes_its_moves` (added) is green.
  - `AGENTS.md`'s section on bindings holds one sentence per rule saying ESS cannot state it.
  - The unit report holds the issue text for beyond10x/ess. It is filed through `host-operator`.
- **A32.** `bindings.md:186` holds the rewritten sentence. The test
  `a_retry_when_result_that_is_empty_is_refused_and_an_empty_list_loads` (loom-bindings) is green.
- **A33.** `i:confinement/local-without-reason`: `local` with no reason, and `local` with only white
  space, are refused `LocalReasonMissing`.
- **A34.** `i:confinement/local-record`: an applied record with `adapter: local` and
  `backend: Substrate` is refused.
- **A35.** `i:confinement/linux-record-without-facts`: a `linux` record with no `facts` is refused,
  and a record with neither member reads as an old one.
- **A36.** `i:confinement/spec-past-its-limits`: 5 roots, 17 files, 5 apertures, a workdir over
  4096 bytes and a writable root are each refused `ScopeInvalid`.
- **A37.** `i:confinement/ca-bundle-beside-trust-variable`: a bundle with no aperture, and one
  beside
  `SSL_CERT_FILE`, are refused `ScopeInvalid`. The git-config half of `TRUST_STORE_VARIABLES`
  (`crates/loom-intake-slice/src/confinement.rs:2121-2151`) stays with its Rust tests.
- **A38.** `i:confinement/one-host-twice`: two apertures for one host, a name given twice and
  `max_bytes: 0` are refused.
- **A39.** First, the failing test `a_refused_action_bindings_file_names_its_reason`
  (loom-intake-slice)
  is red while `check` answers a `String` (`crates/loom-intake-slice/src/bindings.rs:181`). Then
  `i:action-bindings/check-reasons` gets `BundledProtocol`, `UndeclaredAction`, `RepeatedAction`
  and `EmptyArgv`.
- **A40.** `a/limit` row F40:
  `a_timeout_too_large_for_the_clock_is_refused_and_starts_no_command`
  (`crates/loom-intake-slice/tests/bound_command.rs:93`) is green. `AGENTS.md` holds the sentence.
- **A41.** Planted defect, governor: the `RepeatedInput` return of `CaseInputs`
  (`crates/loom-governor/src/lib.rs:268`) is removed. `s:governor/input-refused` goes red at that
  step, and is green again once the line is restored.
- **A42.** Planted defect, tree: `FileCaseStore`'s check of a link naming another case
  (`crates/loom-governor/src/file_store.rs:33`) is disabled. `s:case-tree/misfiled-link` and
  `child_cases.rs:640` go red, and are green again once restored.
- **A43.** Planted defect, runlog: the `(Some(_), None)` arm of `verify`
  (`crates/loom-runlog/src/lib.rs:401-402`) is made to pass. `s:runlog/line-zero-and-gap` goes red,
  and is green again once restored.
- **A44.** Planted defect, bindings: the comma check (`crates/loom-bindings/src/decode.rs:721`) is
  removed. `s:bindings-check/tool-name-comma` goes red, and is green again once restored.
- **A45.** Planted defect, confinement: `local_reason`
  (`crates/loom-intake-slice/src/confinement/engine.rs:233-240`) accepts a blank reason.
  `i:confinement/local-without-reason` goes red, and is green again once restored. For each of the
  five, the report records the command, the failure and the green run. They run against the suite
  synthesized for A46. A scenario runs against the code, so the code is broken: a rule deleted from
  the spec is the mutation audit's to catch.
- **A46.** `ess/ess-inputs.yaml` gains 28 scenario lines and `ess/intake/ess-inputs.yaml` gains 7.
  The report states the synthesis wall time of `ess/` and of `ess/intake/` at the unit's base and
  at its end. The difference for `ess/` is under 53 s, or the cause is fixed in the unit.
- **A47.** `slot task test-step` ends within 120 s, with no test over 60 s, and prints its ten
  slowest tests. `conform` and `intake-conform` each end within 60 s, and the report gives both
  times.
- **A48.** `CHANGELOG.md` has one `Changed` bullet. The guide and `AGENTS.md` hold the corrections
  named in the table. No generated file is edited by hand: `slot task drift` exits 0.
- **A49.** On the merged batch, after its suite regeneration, `slot task ess-gate`,
  `slot task conform` and `slot task intake-conform` exit 0. A second design review of the five
  domains reports none of F1 to F40 standing, and its findings block holds no `blocker`. The
  review's block holds none today either.
- **A50.** `slot task check` exits 0.

## Out of scope

- Techniques 2, 3 and 5 (random sequences, caller replay, metamorphic relations), the mutation
  audit (technique 1), determinism (4) and guard analysis (6). They follow as
  `story:ess-hardening-techniques` once these rules exist.
- The spec diff in the gate (technique 7): it is part of the batch gate.
- Detecting a cut at a line boundary, which would need a sealed or signed log.
- A union for `ExecutorSpec`, and an enum on the wire for `AppliedConfinement.adapter`.
- Changes to Canon, Commission or Substrate.

## Scope

Derived 2026-11-22 by `story-scoper` from the plan tree at `bcc5a8da`. Every line is **cited**
(read from the brief, the review or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `ess/domains/{governor,runlog,bindings}.yaml`,
  `ess/intake/domains/{confinement,bindings}.yaml`, `ess/ess-inputs.yaml`,
  `ess/intake/ess-inputs.yaml` (cited).
- **Files (scenarios):** `ess/scenarios/{governor,case-tree,runlog,bindings-check,bound-step,`
  `record-copy}/` (inferred, the names are this draft's).
  `ess/intake/scenarios/{confinement,action-bindings}/` (inferred: whether the intake manifest may
  list `ess/scenarios/` paths is not established).
- **Files (generated):** `generated/rust/loom/`, `generated/rust/intake/` (cited, regenerated only).
- **Files (conformance):** `crates/loom-conformance/src/{lib,governor,runlog,bindings}.rs` (inferred
  from the `admission.rs` pattern). `crates/loom-intake-conformance/src/lib.rs` (inferred).
- **Files (code):** `crates/loom-bindings/src/program.rs:336,615,37-38` (cited);
  `crates/loom-intake-slice/src/bindings.rs:181` (cited); `crates/loom-governor/src/{lib,children,`
  `file_store}.rs` (inferred: wrapping the newtypes `ChildName` and `SubjectRevision`);
  `crates/loom-bindings/src/{decode,load}.rs` (inferred: the newtype `ToolName`).
- **Files (tests):** `crates/loom-{bindings,runlog,governor,intake-slice}/tests/` (cited, for the
  named tests).
- **Documents:** `AGENTS.md` (`:1384`, `:1503-1505`, the sections on bindings and intake),
  `website/docs/guides/bindings.md` (`:156`, `:186`, `:285-286`), `CHANGELOG.md` (cited).
- **Confidence:** medium. The ESS files and code lines are cited. The conformance modules,
  scenario paths and newtype ripple are inferred.
- **Would collide with:**
  - `ess/domains/bindings.yaml`, `ess/system.yaml`, `ess/ess-inputs.yaml`,
    `crates/loom-conformance/src/lib.rs` and `crates/loom-bindings/src/program.rs` (cited; this is
    why the story depends on the two stories above);
  - the test files of every crate under `story:one-test-binary-per-crate` (inferred);
  - `generated/ess/loom/` under `story:test-step-within-budget` (inferred).
- **Safety fact:** no stored wire format changes. Every new invariant constrains a value Loom
  already refuses, and `AppliedConfinement.adapter` stays text. Proven to step 2
  (`decode.rs:261-267,721,781`, `confinement/engine.rs:233-240`, `children.rs:30`); unproven.
