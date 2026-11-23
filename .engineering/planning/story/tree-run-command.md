---
format: aep.planning-md/3
id: story:tree-run-command
kind: story
status: active
title: b10x-loom tree runs a case tree from a store to a terminal outcome or the next approval and names the approval command
summary: tree open, run, approve, close, verify and status on Commission's loop with the deterministic selector and bound executors
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bound-artifact-revisions
- depends_on: story:deterministic-default-selector
- depends_on: story:record-copy-between-cases
- depends_on: story:hash-chained-run-log
- depends_on: story:bound-program-retry-records
- depends_on: story:bound-program-case-id
- depends_on: story:child-case-runtime
- depends_on: story:bound-program-result-table
- depends_on: story:open-children-from-arguments-from
scope:
- confidence: inferred
  path: crates/loom-cli/Cargo.toml
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/src/tree.rs
- confidence: inferred
  path: crates/loom-cli/tests/fixtures/tree
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: inferred
  path: crates/loom-conformance/src/lib.rs
- confidence: inferred
  path: crates/loom-conformance/src/tree.rs
- confidence: cited
  path: crates/loom-executor/tests/crate_names.rs
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: inferred
  path: ess/scenarios/tree-run
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: website/data/status.json
- confidence: inferred
  path: website/docs/guides/run-a-case-tree.md
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:46Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":4}}}
- {from: "proposed", to: "active", at: "2026-11-22T00:22:55Z", actor: "agent:sdc", revision: 15, decided_on: {"recorded":{"review_outcome":4}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w9"}
---
## Outcome

`b10x-loom` has a command group that runs a tree of protocol cases from a case store. The run
logic is a new crate, `b10x-loom-tree` (`crates/loom-tree`); `crates/loom-cli` holds the flags.

- `tree open` creates the root case in a store directory from a registered protocol, or from a
  protocol of the directory `--protocols <dir>` names, a `loom.bindings/1` file and initial
  artifact revisions, with the file's `independence` map;
- `tree run` drives the tree: every open child before its parent's next step, each step selected
  by the deterministic selector and performed through the action's bound executor, until the root
  is terminated through its one legitimate outcome or the tree waits on a person. A case is
  terminated when its completion gives exactly one legitimate outcome and no action in it is
  useful; a child's termination is written into its parent;
- when it waits, it prints, per decision, the case, the action, what is decided (the subject, its
  revision and the path to read) and the exact command that records it; for an admissible writer
  whose binding has a `handoff`, the decision is the binding's `handoff` text and the command is
  `tree run` again;
- `tree approve` records a principal's decision on a capability for one case, and with `--action`
  then takes that action as the principal: the record a `person` executor's evidence entry
  declares is produced by that principal about the subject's revision at that moment;
- `tree close` terminates a case through a named outcome when more than one is legitimate;
- `tree verify` checks the run log of every case of the tree and names the first line that fails;
- `tree status` prints the tree as JSON for a live view, of format `loom.tree-status/1`: per case
  its id, link name, parent, protocol with its id and revision, revision, outcome or stop with the
  decisions it waits on, open obligations, the path of its case file and run log, its evidence
  records, program runs, approvals and copies; and, for a tree opened with `--protocols`, each
  protocol's registration and sha256 ("The status of a tree" below).

A case's workspace is the tree's one workspace, the existing directory `tree open` is given as
`--workspace <dir>` and records in the store beside the root case file: every case of the tree
works in it, every path of the bindings is relative to it, and under a Substrate daemon it must
lie directly under the daemon's workspace root (`story:substrate-daemon-runner`).

A product's protocols can be files in its own repository. `tree open --protocols <dir>` reads
every file directly in `<dir>` whose name ends in `.yaml` and does not start with `.`; a
subdirectory is not read, and a link is read only when its real path is a regular file in `<dir>`.
Each file is compiled and validated with Canon through `ProtocolCatalog::add_yaml`
(`crates/loom-protocols/src/lib.rs:241-280`) under the registration its document declares: the
protocol id with dots and underscores as hyphens, `@`, its `revision` (the rule of
`registration_for`, `lib.rs:315-324`; `tour.leg` at revision 1 is `tour-leg@1`). Its source is
`ProtocolSource` with the new kind `Directory`: `location` the directory's real path at
`tree open`, `path` the file's name, `revision` empty. The tree's catalog is those protocols and no
other: neither the built-in nor the installed ones (`ProtocolCatalog::bundled`, `lib.rs:149`;
`InstallStore::catalog`, `crates/loom-protocols/src/store.rs:44-50`). It is bound to the
governor with `CanonGovernor::with_catalog` (`crates/loom-governor/src/lib.rs:767-803`), which
admits exactly that membership and cannot be extended. The root protocol is named by its
registration and must be in the directory; a child's `case_open.protocol` names a protocol id,
and it resolves to the one file of the directory that declares that id (the host's catalog of
`story:child-case-runtime` is this catalog). Without `--protocols` the tree's catalog is the
host's, as above.

`tree open` keeps each protocol's source in the store as `blobs/<sha256>`, the store's blob
directory, and beside the root case file the list `tree-protocols.json`: per protocol its
registration, protocol id, revision, file name and sha256. `tree run`, `tree approve`,
`tree close`, `tree verify` and `tree status` take no `--protocols`: they build the catalog
from those blobs, so an edit, a new file or the removal of `<dir>` after `tree open` changes
nothing in the open tree. No bindings document, program output or report selects or replaces a
protocol of an open tree.

Refused by `tree open` before anything is written, each refusal declared in
`ess/domains/tree.yaml`:

- `DuplicateProtocol`: two files declare one protocol id, at one revision or at two (a
  `case_open.protocol` names an id, never a revision); both files are named;
- `ProtocolRefused`: a file that is not a regular UTF-8 file within 1 MiB, or that Canon does not
  parse, validate or compile, or whose id gives no registration; the file is named, with the
  first 1024 bytes of the message, cut on a character boundary and marked when cut;
- `ProtocolLinkLeavesDirectory`: a link whose real path is not in `<dir>`, naming the link and
  that path;
- `ProtocolNotInDirectory`: the root registration, or a `case_open.protocol` of a protocol the
  directory holds, that no file of the directory declares; naming it and the file that names it;
- `NoProtocolFiles`: a directory with no file read as above.

A cycle of openings among the directory's protocols is refused as `story:child-case-runtime`
refuses it at registration. A later `tree` command whose stored blob no longer has the sha256 of
`tree-protocols.json` refuses `StoredProtocolChanged`, naming the registration, and writes
nothing.

`tree run` ends with exit 0 at a terminal outcome, 3 when it waits on a person or on authority,
4 when a case is stuck (nothing useful, no legitimate outcome, nothing awaiting approval), and 5
when the step or wall-time budget is spent, each naming the case. One writer per store: a second
`tree` command on a store in use is refused naming the holder. Every instant comes from the host
clock, or from `--at` when given.

**Public library entries.** The logic is in `crates/loom-tree` as public functions over plain
data (paths, strings, the generated `loom.tree` types); `crates/loom-cli` only parses the flags,
calls one function and prints what it returns. `run_tree` takes an optional observer, given each
run-log line as it is appended, and an optional cancel handle, asked before each step: a
cancelled run stops before the next step as a spent wall-time budget does (`BudgetSpent`). For
the status the entries are `tree_status`, `stored_catalog`, `status_json`, `read_status` and the
constant `STATUS_FORMAT`. `crates/loom-protocols` re-exports Canon (`pub use b10x_canon as
canon;` in its `lib.rs`), so that an embedder names one runtime dependency and links the same
Canon as Loom. `CHANGELOG.md` gains the bullet "`b10x_loom_protocols::canon` re-exports the
Canon crate Loom links; an embedder names one runtime dependency."

**The status of a tree.** `tree status` prints the value one public function returns,
`tree_status(store, catalog) -> Result<TreeStatus, Refused>`, `TreeStatus` being the generated
`loom.tree.TreeStatus`: the command prints `status_json` of that value and nothing else, so
Runbook's daemon serves the same value as a view by calling the function, without starting a
process. The function reads the store and never takes it, so it answers while a `tree run` holds
the store. Its catalog is the one the tree's other commands use: for a tree opened with
`--protocols` the one `stored_catalog(store)` builds from the blobs `tree-protocols.json` names,
else the host's.

The document keeps every member the form before this revision has, with the same meaning:
`store`, `workspace`, `root`, `cases` and `protocols`, and per case `case`, `parent`, `protocol`
(the registration), `revision`, `outcome`, `stop`, `open_obligations` (by name), `case_file` and
`run_log`. It adds:

- `format`, the string `loom.tree-status/1` (`STATUS_FORMAT`), and `version`, the version of
  `b10x-loom-tree` that printed it. The form before this revision has neither member; it was
  printed only by the unit's branch and never released. `read_status` takes the bytes of a
  printed status and returns its `TreeStatus`; a document with no `format`, or with any `format`
  but `loom.tree-status/1`, is refused `UnknownStatusFormat` naming the format found (or none),
  and one that does not decode as `TreeStatus` is refused `StoreUnreadable`. `version` is not
  checked.
- Per case, `name`: the last segment of the case id below its parent (`north` for
  `<root>.north`), absent for the root.
- Per case, `protocol_id` and `protocol_revision`: `protocol` split at its last `@` (`tour-leg@1`
  gives `tour-leg` and `1`).
- Per case, `stop_next`: the `next` of the case's last `stopped` run-log line, the decisions that
  would let it go on, as `stop` is that line's `stop`; empty when the log has no such line.
  `outcome` is the outcome of the case's termination (`story:child-case-runtime`).
- Per case, `evidence`: per record the case holds, in the order the case received it (the
  governor's `evidence` list): `index` (its position, from 0), `evidence_id` (the governor's id
  of the record), `kind`, `result` (absent when the record has none), `subject` (an artifact
  name), `subject_revision`, `upstream_revisions` (an object of artifact to revision, empty when
  the record has none), `producer` and `action`. `kind` to `upstream_revisions` are read from the
  `canon-evidence/1` record the stored record holds in `facts`: `subject_revision` is that
  record's string, not the integer case revision the governor keeps beside it. `action` is the
  action of the step that gave the record, read from the case's `action finished` run-log line
  whose `evidence` names its `evidence_id` (every step writes one, a `tree approve --action`
  included); absent when no line does, as for a `case_outcome` record the governor writes for a
  child. A stored record whose `facts` is not a `canon-evidence/1` record is refused
  `StoreUnreadable`, naming the case and the index.
- Per case, `runs`: per run of a bound program of the case, the observations of source
  `loom/bound-program` whose payload names the case, in the order `observations.json` keeps
  them: `action`, `exit` (absent when the program timed out or the runner refused it),
  `attempt`, `retried` and `retry_of`. `retry_of` is the payload's list of the evidence ids the
  run was given (`story:bound-program-retry-records`), the records that sent the action back,
  which another action's run may have given (a check's). So the chain is the action's own:
  `attempt` is 1 for a run whose `retry_of` is empty, else one more than the `attempt` of the run
  of the same action before it in the case; `retried` is true when the next run of the same
  action in the case has a `retry_of` that is not empty.
- Per case, `approvals`: per `tree approve` on the case, in the order recorded: `capability`,
  `action` (the action `--action` named, absent without it), `answer` (`Allow` or `Deny`),
  `actor` (the principal) and `at`. They are read from the case's run log: each `tree approve`
  appends one `authority recorded` line, which gains the optional member `action`. The store's
  `approvals-<stem>.json` is not the source: it holds the decisions the authority provider reads
  and replaces a repeated decision of one capability, principal and case revision, so it does not
  keep every `tree approve` in order. An outcome decision of `tree close` (the governor's
  `ExplicitDecision` in the case file's `decisions`) is not an approval and is not listed.
- Per case, `copies`: per record a `copy` binding copied into the case, in the order
  `observations.json` keeps them: `source_case`, `target_case` (the case) and `record_kind`. A
  copied record cites one observation of Loom's copy effect whose payload is
  `loom.bindings.CopiedRecord` (`case`, `action`, `source_case`, `source_evidence`,
  `dropped_upstream`; `story:record-copy-between-cases`): `target_case` is its `case`, and
  `record_kind` is the `kind` of the case's record whose `observation_ids` name that observation.

This story is what emits the run log's events and implements the selector's `StepHistory` from
the store: `performed_at` from the log's `action finished` lines, `due` from `retry_records`
(`story:bound-program-retry-records`'s function, for a writer of any executor) and `copy_due`
(`story:record-copy-between-cases`).

The arguments of an `open` action come as a `program` binding's do: the binding's `arguments`,
else the object its `arguments_from` argv prints through the runner.

An `agent` executor has no effect port before `story:bound-agent-node`. Until that story lands,
`tree open` reads the root's bindings file and, through the `bindings` member of each `open`
executor, every bindings file of the tree, and refuses a binding with an `agent` executor before
it writes anything: `AgentNotRunYet`, naming the file, the action and `story:bound-agent-node`.

What exists and what is new, read 2026-11-21. Exists and is used as it is: a model-free
executor (`Loom` over a selector and `EmptyObjectArguments`,
`crates/loom-executor/src/lib.rs:97-168`, `arguments.rs:45-56`), the in-memory run records
(`RunStore`, `crates/loom-commission/src/outcome.rs:246-345`), and the runtime's `Clock`, `Ids`,
`Limits` and `HostControl` ports (`crates/loom-commission/src/runtime.rs:588-649`). New in
`crates/loom-tree`: an authority provider over the approvals recorded per case, capability and
principal (the intake slice's `GrantAuthority` allows a capability only for one suspended
request, `crates/loom-intake-slice/src/grant.rs:358-448`); the one-writer lock that names its
holder (the governor's lock is taken per operation); and ids derived from the case, its revision
and the step, so that two runs give the same bytes. Nothing moves out of
`crates/loom-intake-slice`.

`ProgramEffects` (`crates/loom-bindings/src/program.rs:96-122`) reads its instants from
`SystemClock` (`program.rs:217`, `:246`) and salts its record ids with the process id and the
time (`program.rs:715-726`). This story passes the tree's instant and ids into it, so that two
runs give the same bytes there too.

## Why

`b10x-loom run` takes one intent and opens one case (`crates/loom-cli/src/lib.rs:766-890`,
`crates/loom-intake-slice/src/run.rs:10-11`); no command takes a case store, and the workflow
executor has no subcommand. The SDC's driver has the surface (`init`, `run`, `approve`, `close`,
`~/beyond10x/sdc/driver/src/main.rs:32-85`, `791-826`) and
`~/beyond10x/sdc/docs/boundary.md` row 1 puts it in Loom's command line. A wait on a person must
name its command (`~/beyond10x/sdc/docs/driver.md` § Status, third pass: "`run` names the exact
command when it waits on a person"). The SDC's live view needs the list of cases and where each
log is (request 7 of session `sdc`), which is what `tree status` gives it. Runbook, the design
authority, decided what else the status carries: per case its evidence, program
runs, approvals and copies, so that its daemon serves them as a view from the library, without
reading store files or starting a process, and a reader refuses a form it does not know.

## Waits on

- `story:open-children-from-arguments-from`: the `open` port reads an `arguments_from` object
  itself, opens one child per name it prints, and reports the list's `blob_sha256`. It merges
  first; this story builds `OpenEffects` with the tree's workspace, store directory and runner.

## ESS first

A new domain `loom.tree` in `ess/domains/tree.yaml`, declared in `ess/system.yaml`: `TreeRun`,
`TreeStop` (`Terminal`, `AwaitingPerson`, `AwaitingAuthority`, `Stuck`, `BudgetSpent`),
`NextDecision` (case, action, decides, command), `TreeStatus`, `TreeBudget`, `CaseApproval`
(case, capability, decision, principal, case revision, instant; kept in the store beside the
case file), and the refusals `StoreInUse` and `AgentNotRunYet` (file, action, story). The domain
declares types only, no command, so no conformance scenario is added. On that commit `task drift`
fails, and `crates/loom-cli/tests/tree_run.rs` does not compile. The generated CLI reference
(`website/docs/reference/cli.md`) is regenerated in the implementing commits.

For `--protocols`, the same commit adds to `loom.tree` the type `TreeProtocol` (registration,
protocol id, revision, file name, sha256; the entries of `tree-protocols.json`), the protocols
list of `TreeStatus`, and the refusals `DuplicateProtocol`, `ProtocolRefused`,
`ProtocolLinkLeavesDirectory`, `ProtocolNotInDirectory`, `NoProtocolFiles` and
`StoredProtocolChanged`. `intake.protocols.SourceKind`, where it is declared
(`ess/intake/domains/protocols.yaml:27-29`), gains the variant `Directory`; on that commit
`task intake-drift` fails too (`Taskfile.yml:56-59`).

The unit's first commit (`9a137f68`, "spec: the loom.tree domain of the tree command group")
already declares `TreeStatus` and `CaseStatus` with the members kept above. For the status
members, a later `ess/`-only commit of the unit amends them:

- `loom.tree.TreeStatus` gains `format` (`String`) and `version` (`String`);
- `loom.tree.CaseStatus` gains `name` (`Optional<String>`), `protocol_id`, `protocol_revision`
  (`String` each), `stop_next` (`List<String>`), `evidence`, `runs`, `approvals` and `copies`
  (a list of the four types below each);
- new types `loom.tree.StatusEvidence` (`index`, `evidence_id`, `kind`, `result` optional,
  `subject`, `subject_revision`, `upstream_revisions` as `Json`, an object of artifact to
  revision, `producer`, `action` optional), `loom.tree.StatusRun` (`action`, `exit` optional,
  `attempt`, `retried`, `retry_of`), `loom.tree.StatusApproval` (`capability`, `action`
  optional, `answer` as `loom.tree.ApprovalDecision`, `actor`, `at`) and `loom.tree.StatusCopy`
  (`source_case`, `target_case`, `record_kind`);
- `loom.tree.TreeRefusalReason` gains `UnknownStatusFormat`;
- `loom.runlog.AuthorityRecorded` in `ess/domains/runlog.yaml` gains `action`
  (`Optional<String>`); a line written without it still reads.

On that commit `task drift` fails.

## Acceptance

With a fixture tree of three levels (root, two children, one grandchild), stub programs, no
model and no engineering vocabulary:

- `tree open` then `tree run` reaches the first approval, exits 3, and prints a `tree approve`
  command; running that command and `tree run` again continues; after the last approval the root
  is terminated through its one legitimate outcome and `tree run` exits 0.
- The order of steps is the same on two runs from two fresh stores with the same `--at` values,
  and each case's final Canon decision is byte-identical between the two.
- A child's wait stops the run naming the child's case, and `tree approve` on that case is the
  printed command.
- `tree approve --action` on an action bound to `person` records the declared evidence with the
  principal as producer and the subject's revision at that moment; after the subject's file
  changes, the next `tree run` waits on the same approval again. `tree run` never takes a
  `person` action itself.
- A fixture writer with a `handoff` that is admissible when nothing is useful makes `tree run`
  exit 3 and print the `handoff` text.
- A root with two legitimate outcomes is not terminated by `tree run`; it exits 3 and prints the
  two `tree close` commands. `tree close` with a non-legitimate outcome exits non-zero and leaves
  the case file byte-identical.
- A tree with no useful action, no legitimate outcome and nothing awaiting approval exits 4 and
  names the case.
- A root opened from a bindings file with an `independence` entry accepts that producer's record
  on a match that demands the dimension.
- An action with no `may_produce` is taken once per case revision, a writer whose check failed
  with a `retry_when` result is taken again, and a `copy` action is not taken while the source
  holds nothing new.
- `--max-steps` and `--max-wall-seconds` each stop the run with exit 5 before the step that
  would pass the limit.
- A second `tree run` started while the first holds the store exits non-zero naming
  `StoreInUse` and the holder's process id, and writes nothing.
- After the run `tree verify` exits 0; after one byte of one log is changed it exits non-zero
  naming the case and the line. `tree status` prints one entry per case with the outcome each
  ended through and a log path that exists.
- In each case's log `action finished` precedes the `evaluated` line that shows the moved
  revision, and a `child closed` line in the parent follows the child's `terminated` line.
- With a canary string as an argument value and as a stub program's output, no log line of the
  tree contains the canary.
- The loop is Commission's (`run_until_blocked` and its ports): every selected action is
  revalidated against case revision, frontier and authority before its effect, and a test fails
  if `crates/loom-tree` calls an effect port outside it.
- The engine's confinement is one flag with the values `b10x-loom run` has
  (`--confinement substrate|local`, with `--local-reason`; `none` is not offered).
  `crates/loom-tree` and `crates/loom-bindings` contain no `Command::new` (a test greps), and
  under `--confinement substrate` a stub program that reads a host file outside the workspace fails
  in every case of the tree.
- A child's bindings file with an `agent` executor makes `tree open` exit non-zero naming
  `AgentNotRunYet`, the file, the action and `story:bound-agent-node`, and the store directory
  stays empty (test `an_agent_binding_is_refused_before_anything_runs`).

With a second fixture tree, `crates/loom-cli/tests/fixtures/tree-two-level/`, written for these
tests and shaped like Runbook's parent and one child (no file is copied from Runbook). The root
protocol `tour` (registered `tour@1`) opens a child of the dotted protocol `tour.leg` (registered
`tour-leg@1`) by `name`, copies that child's `route_approval` record into itself with a `copy`
binding and, once the copied record is `approved`, opens one child per entry of `legs`, an
argument its `arguments_from` stub prints. Its outcome `arrived` needs the copied approval and
every child `passed`, `cancelled` needs a decision, and `turned_back` is legitimate once a child
ended otherwise. The child places three artifacts; its writer has no `may_produce`, a
`retry_when` on the check of what it wrote, and a stub that appends one line to the file the root
places as `route`; that check is `invalid` until the file has two lines, and its one exit code
gives two kinds; a `person` action `route.approve` under the capability `guide.approve` may
produce `route_approval`; a `program` action `ticket.issue` runs under the capability
`guide.issue`; and one action no binding names:

- `tree open`, then `tree run` and each printed command in turn, ends with the root terminated
  through `arrived` and `tree run` exiting 0 (test `two_level_tree_runs_to_its_terminal_outcome`).
- The children are held under `tour-leg@1`, the registration of the id `tour.leg` that
  `case_open.protocol` names (test `a_dotted_child_protocol_id_opens_its_registration`).
- The first child's writer runs twice: its first output makes the check `invalid`, `retry_when`
  sends it back, and after its second the check is `valid` (test
  `the_writer_is_sent_back_once_by_retry_when`).
- The root's revision of `route` moves at the root's next evaluation after the child's writer
  changed the file (test `a_childs_write_moves_the_parents_revision`).
- The `copy` binding copies the first child's `route_approval` into the root with the same
  result, producer and subject revision, and the `each` opening is not taken before it (test
  `the_root_copies_the_childs_approval_before_the_next_opening`).
- The `each` opening opens `<root>.north` and `<root>.south` from the object
  `{"legs": ["north", "south"]}` its `arguments_from` stub prints, with one `case_outcome` record
  each in the root, and the step's `action finished` line in the root's log names as its
  `blob_digest` the `blob_sha256` of the open port's report (test
  `an_each_opening_takes_its_names_from_arguments_from`).
- `ticket.issue` stops the run with exit 3 and a printed `tree approve` command without
  `--action`; after it, `tree run` performs the action through its program and records its
  evidence (test `an_approved_capability_lets_the_run_take_a_program_action`).
- With the bindings file whose `ticket.issue` stub exits 1, the first child ends through `failed`
  and the root through `turned_back`, and `tree run` exits 0 (test
  `a_failed_child_ends_the_root_through_its_failure_outcome`).
- The action no binding names is never taken, and no printed decision names it (test
  `an_unbound_action_is_never_taken`).
- A thin client calls the commands § Outcome defines and nothing else: `tree open` with the store
  directory, the registered protocol (or `--protocols <dir>` and the root's registration in it),
  the `loom.bindings/1` file, the initial artifact revisions and `--workspace <dir>`, then
  `tree run` on the store directory. A second `tree run` over the
  same store after an exit 3 continues the tree and repeats no `action finished` line, and both
  commands call one public function of `crates/loom-tree`, `run_tree` (test
  `a_second_start_over_the_same_store_continues_the_tree`).
- After the run to `arrived`, `tree status` gives every member the form before this revision has,
  with the same values; the root with no `name` and no `parent`; `<root>.north` with `name`
  `north`, `parent` the root, `protocol` `tour-leg@1`, `protocol_id` `tour-leg` and
  `protocol_revision` `1`; and, read after the exit 3 at `ticket.issue`, the waiting case's
  `stop_next` names the decision `tree run` printed (test
  `a_status_names_each_cases_link_protocol_and_stop`).
- The root's `evidence` lists its records in the order received, `index` from 0: the copied
  `route_approval` with the source record's `result`, `producer`, `subject`, `subject_revision`
  string and `upstream_revisions`, and `action` the root's `copy` action; each `case_outcome`
  record with no `action` (test `a_status_lists_each_record_in_the_order_received`).
- The first child's writer has two `runs`: `attempt` 1 with `retried` true and an empty
  `retry_of`, then `attempt` 2 with `retried` false and a `retry_of` naming the `invalid` check
  record (test `a_status_counts_the_attempts_of_a_retried_writer`).
- Each `tree approve` on a case is one entry of its `approvals`, in the order recorded, with
  `capability`, `answer` and `actor`, and `action` only for `tree approve --action`; the same
  decision recorded twice is two entries, while `approvals-<stem>.json` holds it once (test
  `a_status_lists_each_approval_in_the_order_recorded`).
- The root's `copies` is one entry, `source_case` the first child, `target_case` the root and
  `record_kind` `route_approval`, and no other case has one (test
  `a_status_names_the_copy_into_the_root`).
- The JSON `tree status` prints equals `status_json(&tree_status(store, &catalog))`, has `format`
  `loom.tree-status/1` and `version` the crate's version, and `read_status` of the printed bytes
  equals the value `tree_status` returned (test `the_printed_status_is_the_value_of_tree_status`).
- `read_status` refuses `UnknownStatusFormat`, naming what it found, a status with `format`
  `loom.tree-status/2` and one with no `format` (the form before this revision) (test
  `a_status_of_an_unknown_format_is_refused`).

With the second fixture's protocols as the directory `--protocols` names,
`crates/loom-cli/tests/fixtures/tree-two-level/protocols/` (`tour.yaml`, `tour-leg.yaml` and a
subdirectory `old/` whose file declares `tour` at revision 1 too), and fixture directories under
`crates/loom-cli/tests/fixtures/tree-protocols/` written for these tests:

- `tree open --protocols` registers exactly `tour@1` and `tour-leg@1`, each with source kind
  `Directory`, and nothing from `old/` (test
  `a_protocols_directory_registers_what_its_files_declare`).
- A tree opened with `--protocols` and the directory deleted after `tree open` runs to `arrived`
  and `tree run` exits 0 (test `a_tree_from_a_protocols_directory_runs_with_the_directory_deleted`).
- After `tour-leg.yaml` is edited following `tree open`, `tree status` names the sha256 of the
  file as it was at `tree open` for `tour-leg@1`, and the tree's children are evaluated under that
  definition (test `an_edit_after_tree_open_changes_nothing_in_the_open_tree`).
- Two files that declare `tour` at revision 1 make `tree open` exit non-zero naming
  `DuplicateProtocol` and both files, with the store directory empty (test
  `two_files_declaring_one_protocol_and_revision_are_refused`).
- Two files that declare `tour` at revisions 1 and 2 are refused `DuplicateProtocol` the same way
  (test `two_files_declaring_one_protocol_id_are_refused`).
- A file Canon does not validate makes `tree open` exit non-zero naming `ProtocolRefused`, the
  file and at most 1024 bytes of Canon's message, with the store directory empty (test
  `a_protocol_canon_refuses_is_refused_naming_the_file`).
- A link in the directory whose real path is outside it makes `tree open` exit non-zero naming
  `ProtocolLinkLeavesDirectory`, the link and that path, with the store directory empty (test
  `a_link_leaving_the_protocols_directory_is_refused`).
- A root named `system-query@1`, a built-in protocol the directory does not hold, makes
  `tree open --protocols` exit non-zero naming `ProtocolNotInDirectory`, with the store directory
  empty (test `the_root_protocol_must_be_in_the_protocols_directory`).
- A `case_open.protocol` naming `tour.detour`, which no file of the directory declares, makes
  `tree open` exit non-zero naming `ProtocolNotInDirectory`, `tour.detour` and the file that names
  it, with the store directory empty (test `a_child_protocol_the_directory_lacks_is_refused`).
- A directory holding only a subdirectory and a `notes.txt` makes `tree open` exit non-zero
  naming `NoProtocolFiles`, with the store directory empty (test
  `a_directory_with_no_protocol_file_is_refused`).
- After one byte of a stored protocol blob is changed, `tree run` exits non-zero naming
  `StoredProtocolChanged` and the registration, and the store is byte-identical (test
  `a_changed_stored_protocol_is_refused`).
- No bindings document, program output or report selects or replaces a protocol of an open tree:
  with a child bindings file naming `tour@1` as its protocol, and a stub program that writes a
  `tour.leg` document over the directory's file and into the workspace and prints one as its
  report, every case keeps the registration and sha256 of `tree open` (test
  `nothing_in_a_run_selects_or_replaces_a_protocol`).
- The fixture protocols of `crates/loom-cli/tests/tree_run.rs` are compiled and registered once
  per test run (one shared fixture cached on disk under `target/`, keyed by the SHA-256 of its
  sources; no `OnceLock`, as nextest runs each test in its own process), each test runs on a
  fresh copy of it, no stub program waits on a timer, and each test asserts it ended within 60 s
  (`slot cargo test -p b10x-loom-cli --test tree_run`).
- `crates/loom-protocols` re-exports the Canon it links as `b10x_loom_protocols::canon`: a
  function taking `b10x_canon::model::Protocol` accepts a value typed
  `b10x_loom_protocols::canon::model::Protocol`, and the `canon` path is the crate whose
  `model::Protocol` `ProtocolEntry.model` names (test
  `loom_protocols_re_exports_the_canon_it_links`).
- `website/docs/reference/cli.md` is regenerated and a guide page shows one tree run with pasted
  output.
- `task check` exits 0.

## Out of scope

A cap on agent calls and agent events in the log (`story:agent-node-run-records`). Mandate
approvals for tree cases. A replay command. Running two cases of a tree at once (the daemon's,
`story:concurrent-cases-in-the-tree-daemon`; a direct `tree run` stays sequential). A service or
daemon that watches a store. Turning a wait into a decision request, and `tree approve` through that
request (`story:tree-waits-open-decision-requests`, `epic:decision-requests-in-the-runtime`).
Running an `agent` binding (`story:bound-agent-node`); until it lands the command refuses it.
Running Runbook's own protocols and bindings files: that test lives in Runbook
(`~/beyond10x/sdc`), not in Loom's repository.
Reading a protocols directory recursively or from Git; built-in or installed protocols beside a
`--protocols` directory; replacing a protocol of an open tree.
Outcome decisions of `tree close` in a case's `approvals`; a status of more than one tree.
No other crate re-exports Canon.

## Scope

Scope to be rewritten by the scoper after this revision

Added for the status members and the Canon re-export, read on the unit branch
`impl/tree-run-command` and on `impl/record-copy-between-cases`; **cited** or **inferred** as
below:

- `crates/loom-tree/src/status.rs`: `tree_status` (`status.rs:41`) builds `TreeStatus`, and
  `members` (`:23`) returns the root alone on the unit branch — cited; the new members and
  `stored_catalog` — inferred
- `crates/loom-tree/src/encode.rs`: `status_json` (`encode.rs:60`) writes each member by hand —
  cited; it gains the new members, and `read_status` is new — inferred
- `crates/loom-tree/src/approvals.rs`: `approvals-<stem>.json` (`:27`); `record_approval`
  (`:103`) replaces a repeated decision — cited
- `crates/loom-tree/src/approve.rs`: `approve` (`:48`) appends `AuthorityRecorded` (`:102`) —
  cited; it writes `action` when `--action` is given — inferred
- `ess/domains/runlog.yaml:139` (`AuthorityRecorded`) and `crates/loom-runlog/src/lib.rs:554`
  (the members a line of each kind may have) — cited
- `ess/domains/tree.yaml`: `CaseStatus` (`:179`), `TreeStatus` (`:203`), `TreeRefusalReason`
  (`:260`) — cited
- `crates/loom-governor/src/file_store.rs:80`: the store's `observations.json` — cited
- `crates/loom-bindings/src/program.rs:531-566`: the `canon-evidence/1` record in `facts`, one
  observation id per record, `subject_revision` the case revision as an integer — cited
- `ess/domains/bindings.yaml`: `ProgramRun` (`:261-275`) — cited; `CopiedRecord` on
  `impl/record-copy-between-cases` (`eb9c0653`) — cited
- `crates/loom-protocols/src/lib.rs`: `use b10x_canon::model::Protocol` (`lib.rs:8`),
  `ProtocolEntry.model` (`:64`); `pub use b10x_canon as canon;` is new — cited / inferred
- `crates/loom-protocols/tests/`: the re-export test, in the crate's one integration-test binary
  once the crate has one (today it has seven test files, `catalog.rs` among them) — cited /
  inferred
- `CHANGELOG.md`: the bullet for `b10x_loom_protocols::canon` — cited (story § Outcome)

Added for `tree open --protocols`, read 2026-11-21; **cited** or **inferred** as below:

- `crates/loom-protocols/src/lib.rs`: a loader of one directory into a catalog with no bundled
  entry, through `add_yaml` (`lib.rs:241-280`) — inferred. `registration_for` (`lib.rs:315-324`)
  is private, so the loader lives in this crate or the function becomes public — cited / inferred
- `crates/loom-protocols/src/store.rs`: `read_regular` (`store.rs:336-354`) refuses every link —
  cited; the directory reader admits a link whose real path is in `<dir>`, so it is new code —
  inferred
- `ess/intake/domains/protocols.yaml:27-29`: `SourceKind` gains `Directory` — cited;
  `generated/rust/intake/` is regenerated from it (`task intake-drift`, `Taskfile.yml:56-59`) —
  inferred
- `crates/loom-protocols/src/store.rs` `record_value` has `unreachable!` for kinds other than
  `Local` and `Git` — cited; a `Directory` source never reaches the install store, so it needs no
  arm — inferred
- `crates/loom-intake-slice/src/bindings.rs:186-189` (matches `Engineering | Loom`) and
  `crates/loom-cli/src/main.rs:877` (prints `source.kind`) take the new variant unchanged — inferred
- `crates/loom-governor/src/lib.rs:767-803`: `CanonGovernor::with_catalog` is used as it is —
  cited
- `crates/loom-tree/src/protocols.rs`: the blobs, `tree-protocols.json`, the digest check and the
  six refusals — inferred (the crate is new)
- `ess/domains/tree.yaml`: `TreeProtocol`, the protocols list of `TreeStatus` and the six
  refusals — cited (story § ESS first)
- `crates/loom-cli/src/lib.rs`: the `--protocols <dir>` flag of `tree open` — cited (story
  § Outcome)
- `crates/loom-cli/tests/fixtures/tree-two-level/protocols/` and
  `crates/loom-cli/tests/fixtures/tree-protocols/` — cited (story § Acceptance); a link that
  leaves the directory is made by the test, not committed — inferred

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree` (`b10x-loom-tree`), a new crate holding the run logic —
  cited (story § Outcome)
- **Files:**
  - `crates/loom-cli/src/lib.rs` (clap types for the `tree` group) — cited (story § Outcome; caller)
  - `crates/loom-cli/src/main.rs` (dispatch; `Command` is matched exhaustively at `main.rs:63-84`) —
    cited
  - `crates/loom-cli/tests/tree_run.rs` — cited (story § ESS first)
- **ESS:**
  - `ess/domains/tree.yaml` (`TreeRun`, `TreeStop`, `NextDecision`, `TreeStatus`, `TreeBudget`,
    `StoreInUse`) — cited (story § ESS first)
  - `ess/system.yaml`, which lists `loom.tree` under `domains:` — cited (story;
    `ess/system.yaml:10-23`)
  - `ess/ess-inputs.yaml`, which lists the domain file. Each earlier domain did this (`adcbf763`,
    `7021cd30` each touch `ess-inputs.yaml` + `system.yaml`) — cited
- **House-rule test:** `crates/loom-executor/tests/crate_names.rs`. `EXPECTED: [..; 28]` must equal
  the set of workspace packages (`workspace_package_set_is_the_loom_names`), so the new crate must
  be added to it — cited
- **Also likely:**
  - `crates/loom-cli/Cargo.toml`, a path dependency on `b10x-loom-tree` — inferred (story puts the
    logic in the crate and the flags in the CLI)
  - `crates/loom-cli/src/tree.rs`, the tree output and exit-status mapping as a module — inferred
    (same pattern as `agent.rs` and `stage.rs`)
  - `crates/loom-cli/tests/fixtures/tree/`, the three-level fixture tree with stub programs —
    inferred (fixtures live in `tests/fixtures/`)
  - `ess/scenarios/tree-run/`, plus `crates/loom-conformance/src/tree.rs` and
    `crates/loom-conformance/src/lib.rs` — inferred, and needed **only if** `tree.yaml` declares a
    command. `crates/loom-conformance/tests/conform.rs:8` needs one scenario per declared command,
    and `LoomTarget` answers each domain in its own module (`resource.rs:31-35`)
- **Documents:**
  - a new guide page showing one tree run with pasted output — cited (story § Acceptance). The path
    `website/docs/guides/run-a-case-tree.md` is inferred: the name is a placeholder, and the sidebar
    is autogenerated (`website/sidebars.ts:15`)
  - `website/data/status.json`: a change that ships a capability updates it (`AGENTS.md:310-312`) —
    cited
- **House-rule files (not listed as paths):**
  - `website/docs/reference/cli.md`, regenerated; "a flag change in `b10x-loom-cli` regenerates the
    CLI reference" (`AGENTS.md:297,304`)
  - `website/docs/reference/crates.md`, regenerated for the new crate (`AGENTS.md:299`)
  - `website/docs/reference/ess/` and `generated/rust/loom/`, from `tree.yaml`
  - `CHANGELOG.md`, `README.md` and `AGENTS.md`: a new command and a new crate (`AGENTS.md:310-312`)
  - `Cargo.lock`
  - `docs/contracts/run-events-v1.json` is **not** touched: it covers only `run --output jsonl`
    (`AGENTS.md:301,172`) — cited
  - `website/docs/reference/sdk-stability.md` is touched only if `loom-sdk` re-exports `loom-tree`,
    which the story does not say (`AGENTS.md:171`) — inferred
  - no CLI contract test walks every subcommand: `crates/loom-cli/tests/loom_cli.rs` reads only `run
    --help` — cited
- **CLI house pattern:**
  - each command has a `*_HELP` const with an `Exit status:` table, an exit-status function and a
    unit test, all in `lib.rs` (`APPROVE_HELP` `lib.rs:59-86`, `approve_exit_status`,
    `each_stop_reason_has_its_exit_status` `lib.rs:995`). `tree run` follows it with 0/3/4/5 — cited
  - `--confinement` reuses `lib.rs:985 Confinement`, which `confinement-opens-from-engine-config`
    changes to `substrate|local` + `--local-reason` — cited
- **Composition (what exists):**
  - **Model-free executor exists:**
    - `Loom<S, G, V>` (`crates/loom-executor/src/lib.rs:97,130,351`) over a selector and
      `EmptyObjectArguments` (`arguments.rs:45-56`, always `{}`)
    - `with_governor` revalidates each selection against the current frontier before proposing
      (`lib.rs:146`)
    - `with_instance` fixes the id namespace (`lib.rs:168`)
    - all of the above — cited. Empty arguments are enough because bound executors take their argv
      from the binding — inferred
  - **Run records exist:**
    - `RunStore` (`crates/loom-commission/src/outcome.rs:246-345`) is public and in memory,
      implements `StartRunBehavior` and `SuspendRunBehavior`, and is used as
      `Generated::new(RunStore::new(ids))` (`crates/loom-intake-slice/src/run.rs:705`,
      `crates/loom-cli/tests/session_events.rs:228`) — cited
    - a fresh Run per `tree run`, with the case store and run log as the only durable state, is
      enough for `run_until_blocked` — inferred
  - **Commission record:**
    - `operator_commission` is `pub(crate)` (`run.rs:1427`), so `loom-tree` builds its own
      `CommissionData` (five fields, principal per approver) — cited / inferred
  - **Clock and limits:**
    - the runtime's `Clock`, `Ids`, `Limits` and `HostControl`
      (`crates/loom-commission/src/runtime.rs:588-649`) are re-exported by `loom-sdk` (`lib.rs:91`).
      `--max-steps` maps to `Limits::step_budget` and `--max-wall-seconds` to
      `HostControl::host_stop`, which is asked before each step — cited
    - a host instant is available from `loom_intake_slice::clock::{SystemClock, instant}`
      (`clock.rs:61-75`, public) — cited
    - `loom-tree` needs its own deterministic `Ids`: the slice's `fresh_uuid` is salted with the
      process id and time (`executor.rs:1645-1653`), so it cannot give byte-identical runs —
      inferred
  - **Authority does not fit and must be new in `loom-tree`:**
    - `GrantAuthority` allows a capability only for the request a `Pending` Suspension holds
      (`crates/loom-intake-slice/src/grant.rs:358-448`). `GrantData` is `intake.routing`'s
      per-commission grant, and `grant.rs` imports the merge handoff — cited
    - `NoDelegatedAuthority` is private (`run.rs:1058`) — cited
    - `loom-tree` needs its own `AuthorityProvider` over per-case, per-capability, per-principal
      approvals kept in the store. Nothing has to move out of `crates/loom-intake-slice` — inferred
  - **Store lock is new:**
    - no lock that holds the store for one writer and names its process id exists. The governor's
      `locked_dir` takes a lock per operation (`crates/loom-governor/src/locked_dir.rs:1-12`) —
      cited
    - so `StoreInUse` is new code in `loom-tree` — inferred
  - **Run log bridge:** run-log emission goes through the runtime's `EventSink`
    (`ports/events.rs:26`, via `HostControl::events`) into `loom-runlog` — inferred
- **Confidence:** high for `crates/loom-tree`, the CLI files, the ESS files and `crate_names.rs`
  (the story or a gate test names them); medium for the conformance and scenario additions and the
  guide path
- **Would collide with:**
  - `crates/loom-cli/src/lib.rs` and `main.rs` (the `Confinement` enum and any other `run` or `tree`
    flags)
  - `ess/system.yaml` and `ess/ess-inputs.yaml` (every story that adds an ESS domain)
  - `crates/loom-executor/tests/crate_names.rs` (every story that adds a crate)
  - `crates/loom-tree` (any later unit testing the tree)
  - `crates/loom-conformance/src/lib.rs` (any domain that adds commands)
  - `website/data/status.json`
  - generated: `website/docs/reference/cli.md`, `crates.md`, `generated/rust/loom/`
- **Safety fact:** the change adds and alters nothing. `Command` is matched exhaustively only in
  `crates/loom-cli/src/main.rs:63-84`, and `loom-docs` reads it only through clap
  (`crates/loom-docs/src/cli.rs:132`). The `run` exit-status table that the run-events contract
  reads (`b10x_loom_cli::EXIT_STATUS`, `crates/loom-docs/src/main.rs:477`) is untouched by a
  separate `tree` table. Proven to level 2, unproven
