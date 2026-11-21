---
format: aep.planning-md/3
id: epic:observation-collection-as-executors
kind: epic
status: proposed
title: Loom collects files, documents, reports, processes and git as executors through Substrate, and a gates document runs with its evaluation in the governor
summary: An observe executor reads the workspace through Substrate's guarded reads; programs run with their identity bound to the spawn; git runs confined; engineering-gates/1 is planned and evaluated in loom-governor; TCP and DNS wait on Substrate
tags:
- sdc
relations:
- serves: vision:O1
- serves: vision:governed-autonomy
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:54:57Z", actor: "agent:sdc", revision: 5, decided_on: {"recorded":{"review_outcome":8}}, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

What engineering-protocols' `assertion-providers` crate collects today (files, documents, generated
output, processes, test and conformance reports, git, network) Loom collects as executors through
Substrate, and every result reaches the governor as a `canon-evidence/1` record built by the
binding's result table, never by the collector or the program. A repository's `engineering-gates/1`
document runs on Loom: its observations are collected by those executors and its `canon-expr/1`
assertions are evaluated in `crates/loom-governor` alone, called from the acting path of an observe
action, with the result reaching the protocol's evaluation only as evidence.

Per operation group (§ Design in the work order gives the reasons):

- Files, documents, generated-output comparison and report files: a new `observe` executor reads
  the workspace through Substrate's guarded workspace reads, with no process (the rule of these
  operations).
- Processes: a bound `program`, git and a gate's project command each run the bytes one admission
  unit digested (`crates/loom-bindings/src/admission.rs`: admit, launch with the real file mounted
  read-only under its name, verify after the run), and a declared report format is checked before
  the table reads it (`tests.run`). In a gate run, `process.run`, `process.succeeded` and `ess.run`
  take engineering-protocols' process value (`{exit_code, stdout, stderr}`; `process.succeeded`
  unavailable without an exit code), owned by `story:gate-run-collects-a-gates-document`.
- Git (`git.changed`): an `observe` operation that runs git through a launch port the observe port
  gains, over the existing confined launch, with a fixed, hardened argv. `story.id` and
  `story.has_tag` are not collected: they are trusted host context, which the gate run hands to
  Canon.
- Network (`net.tcp.reachable`, `net.host.resolves`): two stories, each behind an
  `upstream-blocker`, because Substrate 0.21.0 serves neither surface.
- External providers (`engineering-provider-request/1`): not ported. A provider is a bound program
  with an admitted digest, its arguments on standard input and its report on standard output.

## Why

Session `sdc` decided on 2026-11-21 that Loom owns this design (`.engineering/drafts/e7-request.md`):
collection runs as Loom executors through Substrate, and its evidence is `canon-evidence/1`.
`epic:engineering-protocols-as-vendored-data` removes Loom's last link to the engineering-protocols
crates (`story:engineering-protocols-dependency-removed`), and with that the only implementation of
these collectors leaves Loom's graph.

What exists today, read at engineering-protocols `0.16.0` (`70e6eff4`):

- `assertion_providers::collect` (`crates/assertion-providers/src/lib.rs:152-161`) dispatches 20
  closed operations (`OPERATIONS`, `:120-141`) and returns `Known`, `Unavailable` (any I/O error,
  `:157-159`) or an error.
- It is "resource supervision, not an operating-system sandbox; registered programs possess the
  caller's authority" (`crates/assertion-providers/README.md:17-18`). Files are read with
  `std::fs` (`src/files.rs:45-60`), processes start by `Command::new` in their own process group
  (`src/process.rs:171-184`), TCP connects from the caller's network (`src/network.rs:28`), and DNS
  is sent over UDP to an explicit resolver (`:59-64`).
- The vendored catalog (`catalog/engineering.yaml`) has 26 functions: 20 observation functions over
  18 of those operations (`document.get` behind three) and 6 recipes. `json.read` and `yaml.read`
  are the two operations no catalog function names.
- The collectors write no `canon-evidence/1`. `canon-engineering-assertions` collects them into an
  `engineering-gate-evidence/1` bundle (`crates/canon-engineering-assertions/src/lib.rs:623-697`)
  and evaluates the gates with `canon_expr` outside any governor (`:717-846`). takt's `GateRun`
  then writes one `gate_result` record, `pass` or `fail`, and no record for an unknown truth
  (takt `25c62df`, `crates/takt/src/validator.rs:2482`, `:4959-4962`, `:5585-5626`; read by a helper
  agent, spot-checked).

takt is frozen (recommendation A, `~/beyond10x/sdc/docs/takt-comparison.md:92`, `:96`, adopted by the
operator as `sdc` relays; the file itself records only the recommendation). Its gate validator
(`validator.rs:2482`, `:5264` `prepare_with`) and `scripts/gate-exec` are the code to port from
(`takt-comparison.md:70`). `GateRun` starts the gate's commands through the assertions library,
outside takt's own confinement, and under takt's `substrate` mode refuses them all (read by the
helper: `crates/takt/src/confinement.rs:1305-1311`). `TestsRun` (`validator.rs:1901`) parses printed
test output; its port is the `program` executor with a result table and is not this epic's.

Canon owns the record: `canon-evidence/1` is Canon `0.9.0`'s `EvidenceRecord`
(`crates/canon/src/model/evidence.rs:17`; schema `website/static/schemas/evidence-1.schema.json:20-79`):
`format`, `id`, `kind`, `subject`, `subject_revision` required; `result`, `observed_at`,
`upstream_revisions`, `producer`, `child_case` optional, nothing else admitted.

Substrate `0.21.0` (`adfde402`, the pin of `crates/loom-intake-slice/Cargo.toml:31-32`) serves:

- guarded workspace reads with no process: `read_workspace_path`, `read_workspace_file_v2`,
  `list_workspace_tree_v2` (`crates/substrate-host/src/lib.rs:656`, `:679`, `:693`, served by
  `HostDriver` at `:1641`, `:1690`, `:1707`), opened with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS |
  RESOLVE_NO_MAGICLINKS | RESOLVE_NO_XDEV` and `O_NOFOLLOW | O_NONBLOCK`
  (`crates/substrate-guarded/src/fs.rs:25-36`); an absent path is `resource.not-found`, a link or
  escape `workspace.path-escape` (`:1550-1575`); one read returns at most `MAX_IO_BYTES` (1 MiB) and
  names the whole file's sha256 (`crates/substrate-wire/src/lib.rs:22`, `:1302-1314`);
- exec under confinement (Loom's `SubstrateRunner::run_child`,
  `crates/loom-intake-slice/src/confinement.rs:475`), read-only host files bound by descriptor
  (`AGENTS.md` § Intake and the command line);
- TCP egress through a loopback forwarder only: "The sandbox never resolves anything: it gets no
  resolver at all" (`crates/substrate-host/src/aperture.rs:9`), and "no UDP, no ICMP, no raw
  sockets" (`docs/design/10-destination-bound-egress.md:58-59`). The listener inside the sandbox
  belongs to Substrate and a relay dials the destination per connection (`crates/substrate-host/src/egress.rs:15-26`),
  so a connect inside the sandbox does not say whether the destination took it (inferred), and no
  record reports the dial's outcome (the run record carries bytes per aperture only, wire
  `lib.rs:1864-1913`, read by the helper).

What Substrate lets Loom read: Loom opens its driver with `HostConfig::minimum`
(`crates/loom-intake-slice/src/confinement.rs:229`), and Substrate `0.21.0` refuses a workspace file
over 1 MiB with `workspace.file-limit` (`crates/substrate-guarded/src/fs.rs:516-530`), so every read
of this epic is bounded at 1 MiB per file, and a gate input over 1 MiB is refused. Substrate gives
one code, `workspace.path-escape`, for a link, a FIFO and a wrong kind (`fs.rs:227-229`,
`:519-521`, `:1552`); the reader tells them apart by the parent directory's page, which lists
`file`, `directory` and `symlink` and leaves out special files and `.git` (`fs.rs:843-870`).

Crate direction: `b10x-loom-bindings` depends on `b10x-loom-intake-slice` and
`b10x-loom-governor` from `story:bound-program-result-table` on (its scope's `Files:` line for the
manifest, `bound-program-result-table.md:115`, which that story marks inferred because it names no
manifest; this epic takes it as that story's plan).
So the `WorkspaceReader` and the observe port's launcher are implemented in `loom-bindings` over
public handles of the slice, and the gate run calls the governor directly. `AGENTS.md` § Governor
says the governor depends on Commission and on the executor, never the reverse, which forbids
neither crate here from depending on the governor; the slice already depends on the governor
(`crates/loom-intake-slice/Cargo.toml:19`), and `AGENTS.md` § Never restricts only
`b10x-loom-commission`.

## Refusals

One rule for every story of this epic:

- At decode or load: the bindings document is refused, naming a
  `loom.bindings.BindingsRefusalReason`, placed as decode-time or load-time as the comment of
  `ess/domains/bindings.yaml:203-209` splits them; no run exists.
- When a port is built: the program port returns `ProgramPortRefused` (its `ProgramRefusal`), the
  observe port `ObservePortRefused` (its `ObservePortRefusal`); no run exists and no process starts.
- At a step: a run record with a declared member. An observe action: `ObserveRun.outcome`
  `Refused` and `ObserveRun.refusal` (an `ObserveRefusal`; a gate run adds `program_refusal` and
  `gates_refusal` for the refusal it carries). A program: `ProgramRun.refusal` (a
  `ProgramRunRefusal`, which `story:gate-program-identity-bound-to-spawn` adds to
  `story:bound-program-result-table`'s record), or `ProgramRun.unavailable` for a report that says
  the value is unavailable (`story:report-formats-checked-before-the-table`). The step is refused
  naming it, and no evidence record is written; a refusal is never a `false` value and never a
  missing observation.
- A table refusal: `ResultNotInTable`, `ReportUnreadable`, `ArgumentsUnreadable` and `RunsSpent`
  are `story:bound-program-result-table`'s refusals of the step (its ESS first,
  `bound-program-result-table.md:63-65`), with no run member. The observation keeps its outcome
  (`ObserveRun.outcome` `Known` or `Unavailable`, `ProgramRun.refusal` absent), and the step is
  refused naming the table refusal.

Every acceptance bullet in the stories names the record and member it reads, or the table refusal.

## Stories

In dependency order:

1. `story:observe-executor-reads-workspace-files`: the `observe` executor, the `loom.observation`
   domain, the shared record path, the `WorkspaceReader`, and `fs.file.exists`, `fs.dir.exists`,
   `text.contains`.
2. `story:gate-program-identity-bound-to-spawn` (in the store, taken over from engineering-protocols):
   one admission unit (a name or an absolute path, a search path, and the case's workspace root)
   admits the program or refuses it, launches the admitted file read-only under its name, and
   verifies it after the run; the program port, git and the gate run's commands all go through all
   three; the bytes admission digested run, and a digest read is bounded.
3. `story:observe-reads-documents`: `json.read`, `yaml.read`, `document.get`, `document.validates`.
4. `story:observe-compares-generated-trees`: `generated.matches`.
5. `story:report-formats-checked-before-the-table`: `engineering-tests/1`,
   `ess-conformance-report/2` and `codegate-dependency-report/0.1`, on a program's report
   (`tests.run`) and on a file (`tests.report`, `ess.report`, `codegate.dependencies`).
6. `story:observe-git-changed`: the observe port's launcher and `git.changed`.
7. `story:gates-document-evaluated-in-the-governor`: the vendored 26-function catalog, plan and
   evaluation of an `engineering-gates/1` document with `b10x-canon-expr`, in the governor only.
8. `story:gate-run-collects-a-gates-document`: the `engineering.gates` operation reads the gates
   document at the case's base, collects every request through stories 1 to 6, runs each project
   command through the admission unit, owns the process value, and writes the records its table
   names.
9. `story:tcp-reachability-through-an-aperture`: behind
   `upstream-blocker:substrate-aperture-dial-outcome`; lifts story 8's refusal of `net.tcp.reachable`.
10. `story:dns-resolution-through-a-resolver-aperture`: behind
    `upstream-blocker:substrate-dns-resolver-aperture`; lifts story 8's refusal of `net.host.resolves`.
11. `story:observe-git-in-a-linked-worktree`: `git.changed` in a workspace that is a linked git
    worktree, its common git directory mounted read-only; waits on stories 6 and 8 and holds no
    other story. What the mount exposes is a security point for the operator (§ Open 1 of
    `.engineering/drafts/e7r5-changes.md`, carried from § Open 21 of `e7r3-changes.md`).

The ten stories form one chain of `depends_on` edges, 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10, so no
two of them share a wave. Each rewrites `generated/rust/loom/` (its `plan.json` and `PLAN.md` carry
the ESS source digest) and `CHANGELOG.md`; stories 1 to 6 and 8 to 10 also edit
`ess/domains/bindings.yaml`, and all of those but 2 the `observe.rs` dispatch; 2 and 5 share
`crates/loom-bindings/src/program.rs`; 3 and 7 share `Cargo.lock`; 8, 9 and 10 share
`observe/gates.rs` and the catalog test. Declaring every operation and refusal in story 1 was
considered and not taken: `CHANGELOG.md` and `generated/rust/loom/` would still order every pair,
and operations declared before they are served would load and then refuse. Story 11 stands beside
the chain: it edits `observe/git.rs` and `observe_git.rs` after 6 and 8, no ESS file, and shares
only `CHANGELOG.md` with 9 and 10, which wait on Substrate; a wave holding it with either of them
merges that file's `### Added` lines.

## SDC's forge-evidence readers

SDC builds them as plain `program` executors first (the request), so nothing in SDC waits here.
For them to move onto this epic later, it offers: the `program` executor unchanged in what it
passes (argv, the arguments as one JSON object on standard input, an environment of `PATH` alone,
`story:bound-program-result-table`), an optional `report_format` (story 5) when a reader prints one
of the three formats, and the observation that names the program's admitted path and digest
(story 2). It must not break them: story 2 is the one change to a reader's launch. A reader named by
a bare program name resolves on the host's search path instead of Loom's own `PATH`, which is a
`**Breaking:**` bullet and § Open 3 of the e7r3 ledger (`.engineering/drafts/e7r3-changes.md`,
for the operator); a reader named by a workspace path runs as written.
No story changes the result table, the evidence `from` values a program takes, or the record
fields; stories 2 and 5 add members to the `ProgramRun` observation only.

## Acceptance

- `ess/domains/bindings.yaml` declares the `Observe` executor and `ObserveOperation`, and
  `ess/domains/observation.yaml` (`loom.observation`) and `ess/domains/gates.yaml` (`loom.gates`)
  exist; `task check` runs both ESS gates green on them (stories 1, 7).
- `observe_operations_cover_the_catalog` (`crates/loom-bindings/tests/observe_catalog.rs`, story 8)
  holds two lists: each of the vendored catalog's 20 observation functions (18 operations) resolves
  to an `ObserveOperation`, a project command run as a program, host context (`story.id`,
  `story.has_tag`), or an operation refused by name until stories 9 and 10 (`net.*`), and each of
  its 6 recipes plans with no operation of its own; and each of the 20 names of
  `assertion_providers::OPERATIONS` at `0.16.0` (`lib.rs:120-141`) resolves the same way,
  `json.read` and `yaml.read` to `ObserveOperation`s that no catalog function names.
- `observe_reads_only_through_the_reader` (story 1) first asserts its scan reaches
  `crates/loom-bindings/src/observe.rs` (it fails on today's tree), then that no file under
  `crates/loom-bindings/src/observe*` names `std::fs`, `File::open`, `std::process::Command`,
  `TcpStream` or `UdpSocket`.
- `no_file_or_document_operation_starts_a_process` (story 1, with story 6's counting launcher)
  counts no launch for any file or document operation.
- `every_host_git_command_goes_through_a_hardened_helper` passes with
  `crates/loom-bindings/src/observe/git.rs` as a named helper (story 6).
- `canon_expr_is_the_governors_alone` (story 7) asserts, from `cargo metadata --no-deps`, that
  `b10x-loom-governor` is the only workspace package depending on `b10x-canon-expr`, and that the
  sources name `canon_expr` at least once and only under `crates/loom-governor`; it fails on today's
  tree. `crates/loom-commission/Cargo.toml` gains no dependency (`task check`'s dependency guard).
- `the_gate_result_reaches_the_protocol_only_as_evidence` (story 8): the governor's observations
  change by exactly the records the table wrote.
- No test makes a model call or reaches a host outside loopback (`AGENTS.md` § Never).
- `task check` exits 0.

## Not in scope

- SDC's forge-evidence readers (pull request, CI, merge) and takt's `PrOpen`, `CiRead`, `PrMerged`
  and forge adapters (`takt-comparison.md:68`).
- takt's `TestsRun` and its printed-output parsers (`validator.rs:511-545`): a test program bound
  with a result table, or with `report_format: engineering-tests/1`, replaces it.
- An extension catalog or an external provider in a gates project (`project.catalogs`,
  `project.providers`, `canon-engineering-assertions/src/lib.rs:391-410`, `:424-438`): refused by
  story 8 (§ Open 5 of `.engineering/drafts/e7r3-changes.md`, settled by `sdc`: default kept).
- Offline replay of a gate run's bundle by a command: the bundle is kept as a blob (story 8), and a
  replay command is a later story.
- An ESS upgrade: every declaration fits `ess/25` and `ess` 0.86.0 (`ess/system.yaml:1`,
  `ess/ess-inputs.yaml:2`).
- A per-file read bound above 1 MiB: a follow-up story of Loom's, not Substrate's: an
  engine-configuration field for the bound, which the runner sets on the public
  `HostConfig.max_file_bytes` (`crates/substrate-host/src/lib.rs:319`, Substrate `0.21.0`) after
  `HostConfig::minimum` (`crates/loom-intake-slice/src/confinement.rs:229`). Lock files such as
  `package-lock.json` exceed 1 MiB; until then they are refused `FileOverBound` with their path and
  size, never skipped or passed.
- What `epic:decision-requests-in-the-runtime`, `epic:concurrent-and-standing-case-trees` and
  `epic:tree-daemon-under-enforced-limits` own: decision requests, per-action limits enforced by
  Substrate (`story:action-limits-enforced`), limit events in the run log
  (`story:limit-hits-in-run-log`), the executable allow-list (`story:executable-allow-list`) and
  named egress per action (`story:named-egress-per-action`, on which story 9 builds).
- Any change in engineering-protocols, takt, Canon or Substrate.

## Source

Work order e7 of session `sdc`, 2026-11-21 (`.engineering/drafts/e7-request.md`), the coordinator's
addition taking over `story:gate-program-identity-bound-to-spawn`, and revision e7r1 after the
round-1 critics (`.engineering/drafts/e7-round-1-*.md`, `e7r1-changes.md`), and revision e7r2 after
round 2 and the scope reports (`e7-round-2-*.md`, `scope-report-*.md`, `e7r2-changes.md`), and
revision e7r3 after round 3 and the answers of sessions `sdc` and `substrate`
(`e7-round-3-*.md`, `e7-sdc-answers-2.md`, `e7-substrate-answer-1.md`, `e7r3-changes.md`). Read:
Loom at
`61fd500b`; engineering-protocols at `0.16.0`; Canon at `0.9.0` (`8f1f4527`); Substrate at
`0.21.0`; takt at `25c62df` through a helper agent whose citations were spot-checked;
`~/beyond10x/sdc/docs/takt-comparison.md` and `takt-probe.md`.
