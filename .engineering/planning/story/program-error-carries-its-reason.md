---
format: aep.planning-md/3
id: story:program-error-carries-its-reason
kind: story
status: active
title: A ProgramError refusal carries the program's stderr tail, walked for credentials and bounded, in its text and on the run tree status lists
summary: 'Promised to Runbook: the reason text in the result, not only the last log line; the stderr tail as the session executor walks it (credential lines, cut PEM blocks, escapes withheld; 4096 bytes); ProgramRun.reason and StatusRun.reason; the walk moves to one shared module'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:wildcard-result-row-does-not-match-timeout
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/program_contract.rs
- confidence: inferred
  path: crates/loom-commission/Cargo.toml
- confidence: inferred
  path: crates/loom-commission/src
- confidence: cited
  path: crates/loom-session-executor/src/lib.rs
- confidence: cited
  path: crates/loom-tree/src/status.rs
- confidence: cited
  path: crates/loom-tree/tests/not_called
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
- confidence: cited
  path: website/docs/reference/.generated.json
- confidence: inferred
  path: website/docs/reference/ess/loom-bindings.md
- confidence: inferred
  path: website/docs/reference/ess/loom-tree.md
revision: 22
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T23:56:26Z", actor: "agent:sdc", revision: 21, executor: "agent:loom", correlation: "wave-2026-11-23-w8-program-error-carries-its-reason"}
- {from: "proposed", to: "active", at: "2026-11-23T23:56:27Z", actor: "agent:sdc", revision: 22, executor: "agent:loom", correlation: "wave-2026-11-23-w8-program-error-carries-its-reason"}
---
## Outcome

A step refused `ProgramError` carries the reason the program gave: the last lines of its standard
error, each line that holds a credential's shape withheld, within 4096 bytes. The person reading
`tree run` or `tree status` sees why the program failed without opening the blob, and a consumer
reads it from the result (the refusal text and the run in `tree status`), not only from the last
log line. What we promised Runbook on 2026-11-23 (14:34 UTC): "`story:program-error-carries-its-reason`
(the reason text in the result, not only the last log line)"; Runbook then ordered it third after
0.60.0, after `story:bound-program-timeout-above-an-hour` and
`story:wildcard-result-row-does-not-match-timeout`. Cites are at `49499053` (tag 0.60.0).

**Before.**

- An exit the binding's `exits.error` lists records nothing and refuses the step `ProgramError`,
  ending the run (`crates/loom-bindings/src/program.rs:375-378`, before the tables).
- `program_error` builds the refusal from the run alone: "{program} ended exit status {n}, which
  exits.error lists; nothing was recorded, and its output is the blob {sha}"; its doc says "The
  text names the exit code and the blob digest, never what the program printed"
  (`program.rs:1491-1512`). The blob is stdout followed by stderr (`program.rs:983-986`), kept as
  `blobs/<sha256>` in the store directory: the reason is only there.
- The text reaches the step's `action finished` line as its `refusal`
  (`crates/loom-tree/tests/not_called/mod.rs:493-507` reads it there: "ProgramError: action
  work.do:" and the exit). `tree status` lists the case's runs from the `PROGRAM_SOURCE`
  observations with `action`, `exit`, `attempt`, `retried`, `retry_of` and `cancelled`
  (`crates/loom-tree/src/status.rs:283-325`), nothing about why.
- The observation (`loom.bindings.ProgramRun`) holds `exit`, `blob_sha256`, `timed_out`,
  `runner_refusal`, `retry_of`, `class`, `cancelled` (`run_json`, `program.rs:1550-1569`).
- The rule is in the contract: ESS `ProgramRefusalReason`'s comment, "its text names the exit code
  and the blob digest, never the program's output" (`ess/domains/bindings.yaml:465-467`); the
  guide (`website/docs/guides/bindings.md:405-408`).
- The walk that makes a child's stderr safe to show exists, private, in the session executor:
  `stderr_tail` (`crates/loom-session-executor/src/lib.rs:2878-2929`): the last whole lines within
  `STDERR_TAIL_BYTES` (4096, `:142-144`), every line with a credential's shape withheld over the
  whole stderr (`credential_lines`, `:3143`; `TOKEN_PREFIXES`, `:146-165`, a URL with user
  information or a query, a PEM block), a PEM block whose opening the window cut withheld too
  (CHANGELOG 0.57.2, `CHANGELOG.md:252-253`: "withholds a PEM block's body lines when its window
  cut the block's opening", "withholds a stderr line whose token follows an escape sequence"),
  terminal escapes removed and replaced (`walked`, `:2963`; `inert`, `:3088`). The intake slice
  keeps its own unscrubbed 512-character tail (`crates/loom-intake-slice/src/bindings.rs:90`,
  `:569-580`), which is not `loom.bindings/1`.
- `b10x-loom-bindings` does not depend on `b10x-loom-session-executor`; both depend on
  `b10x-loom-commission` (`crates/loom-bindings/Cargo.toml`, `crates/loom-session-executor/Cargo.toml`).

**After.**

- The reason: the stderr tail of the run, as `stderr_tail` makes it (D1 a, D3 a), empty when the
  program wrote nothing on stderr.
- `program_error` takes the run's stderr (inferred: `execution.child_output.stderr`) and its
  detail becomes "{program} ended exit status {n}, which exits.error lists; nothing was recorded,
  and its output is the blob {sha}; the last lines of its stderr:\n{tail}" (no suffix when the tail
  is empty), so the `action finished` line and `tree run`'s output carry it.
- The run's observation gains `reason` (D2 a): the same tail, only on a run whose exit is in
  `exits.error`; `tree status` shows it on the run (`StatusRun.reason`), read from the observation.
- The walk moves to one place both crates reach (D3 a); the session executor calls it from there
  and its output is byte for byte what it is today.
- Documents: the ESS comments, the guide (`:405-408`), `AGENTS.md` (the `exits` paragraph of the
  bindings section; inferred line), `CHANGELOG.md` `### Changed` under `[Unreleased]`. Not
  breaking: a text gains a suffix and two optional members appear.

## Decisions

- **D1. What the reason is.** Default **a**.
  - a. The stderr tail, walked as the session executor walks it. The program needs no change;
    every program that prints its error on stderr explains itself.
  - b. A declared reason line only: the last stderr line that starts `loom-reason: `, walked the
    same way. Nothing else of stderr is shown; a program must be changed to say anything.
  - c. A report pointer: a new `exits.reason_from: <pointer>` read from stdout as one JSON object.
    A new member, decoded and checked at load.
- **D2. Where the reader finds it.** Default **a**.
  - a. In the refusal text and as `reason: Optional<String>` on `loom.bindings.ProgramRun` and
    `loom.tree.StatusRun`: `tree status` and an embedder read it as a member, not by parsing the
    log line (the promise: "in the result, not only the last log line").
  - b. In the refusal text only.
- **D3. Where the walk lives.** Default **a**.
  - a. Move `stderr_tail`, `walked`, `credential_lines`, `inert` and `TOKEN_PREFIXES` into one
    module of `b10x-loom-commission` (inferred home: both crates depend on it; it gains the `url`
    dependency the walk uses), the session executor re-exporting `TOKEN_PREFIXES`. One rule for
    every stderr Loom shows. Cost: a move with no behaviour change, proved by the session
    executor's existing tail tests.
  - b. `b10x-loom-bindings` depends on `b10x-loom-session-executor` and the walk is made public
    there: no move, but the bindings crate pulls metaharness into its graph.
  - c. A copy in `b10x-loom-bindings`: a third copy of `TOKEN_PREFIXES` (the agent crate already
    holds one, `crates/loom-agent/src/lib.rs:98`, held equal by a test) that can drift.
- **D4. The bound.** Default **a**: 4096 bytes, `STDERR_TAIL_BYTES`, one bound for every tail.
  **b**: 1024 bytes, since the reason is kept in every error run's observation.
- **D5. `ProgramSignalled`.** Default **a**: unchanged (a signal is the host's reason, not the
  program's). **b**: the same tail on a signalled run.

## ESS first

Before the code, in `ess/domains/`:

- `bindings.yaml`: `loom.bindings.ProgramRun` (`:362-380`) gains `reason: Optional<String>`; the
  comment of `ProgramRefusalReason` (`:465-467`) says the text ends with the stderr tail, walked
  and bounded, instead of "never the program's output".
- `tree.yaml`: `loom.tree.StatusRun` (`:228`) gains `reason: Optional<String>`.

Then `task generate`, `task suite-generate` (`SLOT_TIMEOUT: 30m`) and `task docs-generate`, never by hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. No test checks its own wall time. A slot job ends within 10 minutes, or names its
time with `slot --timeout` (at most 30 minutes). No test binary is added.

**`crates/loom-bindings/tests/program_contract.rs`** (the `exits` tests live there), through the
port, a stub with `exits: {error: [79]}`:

1. `a_program_error_carries_its_stderr_tail` (red first): the stub writes `no such epic: E-12` on
   stderr and exits 79: the refusal starts `ProgramError: action`, ends with
   `the last lines of its stderr:\nno such epic: E-12`, and the run's observation has
   `reason: "no such epic: E-12"`. Failing first: the text ends with the blob digest; no `reason`.
2. `a_program_error_withholds_a_token_line`: stderr `token ghp_abc123` then `failed`, exit 79: the
   refusal and `reason` end with
   `[a line withheld: it holds a credential's shape]\nfailed` (the session executor's note,
   `crates/loom-session-executor/src/lib.rs:3001`), and neither holds `ghp_abc123`. Failing first:
   the text holds no `failed` and no note.
3. `a_program_error_withholds_a_cut_pem_block`: stderr holds a PEM block of 5000 bytes whose
   `-----BEGIN` line is outside the last 4096 bytes, then `failed`, exit 79: the refusal and
   `reason` end with a withheld note followed by `failed`, and hold no body line of the block.
   Failing first: the text holds no `failed` and no note.
4. `a_long_stderr_is_cut_to_its_last_whole_lines`: 20 000 numbered lines, exit 79: `reason` is at
   most 4096 bytes, starts at a whole line and ends with the last line. Failing first: no `reason`.
5. `a_program_error_with_no_stderr_has_no_reason`: exit 79, nothing on stderr: the text is today's,
   byte for byte, and `reason` is absent. Guard.
6. `a_run_outside_the_error_class_has_no_reason`: the stub writes `warning: slow` on stderr and
   exits 0 (table `{"0": valid}`): performed, `valid` recorded, the run's observation has no
   `reason`. Guard (failing only if `reason` were set on every run).

**`crates/loom-tree/tests/main.rs`**, module `not_called`: in
`classed_errors_and_unclassed_exits_behave_as_before` (`tests/not_called/mod.rs:480-507`) the
error tree opens with a stub of its own instead of `DO` (`:121`, which writes nothing on stderr):
`mkdir -p target\ntouch target/called\necho 'no such epic: E-12' >&2\nexit 79\n`. The
`action finished` line of `work.do` then holds `no such epic: E-12` in its `refusal`, and `status`
(in process) lists `work.do`'s run with `reason: "no such epic: E-12"`. Failing first: neither
holds it.

**The walk lives in one place (D3 a).** `rg -n 'fn stderr_tail|fn credential_lines|fn walked|fn walk_lines'
crates` prints one line per function, all in the moved module under `crates/loom-commission/src`;
`rg -n 'pub const TOKEN_PREFIXES' crates` prints the moved module and
`crates/loom-agent/src/lib.rs` only (the agent's copy stays, held equal by
`crates/loom-agent/tests/token_prefixes.rs`); and the session executor's tail tests
(`crates/loom-session-executor/src/lib.rs`, among them `a_records_reason_keeps_its_start_within_the_bound`, `:3926-3933`) pass unchanged.

**Gate.** `task check` exits 0; `task slow` passes after the ESS change.

## Does not do

- D1 b: a declared `loom-reason:` line. D1 c: an `exits.reason_from` report pointer.
- D2 b: the reason in the refusal text only, with no typed member.
- D3 b: `b10x-loom-bindings` depending on `b10x-loom-session-executor`. D3 c: a copy of the walk.
- D4 b: a 1024-byte bound.
- D5 b: a stderr tail on `ProgramSignalled`.
- Show stdout, or the blob, in any text; change `ResultNotInTable` or any other refusal's text.
- Change the intake slice's own tail (`crates/loom-intake-slice/src/bindings.rs:90`).

## Scope

Derived 2026-11-23 from the source at `49499053`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong.

- `crates/loom-bindings/src/program.rs` (`:375-378`, `:1491-1512`, `:1550-1569`; read `:983-986`) — cited.
- `crates/loom-bindings/tests/program_contract.rs` — cited file.
- `crates/loom-session-executor/src/lib.rs` (`:142-165`, `:2878-2929`, `:2963`, `:3001`, `:3088`,
  `:3143`) — cited; the move.
- `crates/loom-commission/src`, `crates/loom-commission/Cargo.toml`, `Cargo.lock` (the `url`
  dependency) — inferred.
- `crates/loom-tree/src/status.rs` (`:283-325`) — cited;
  `crates/loom-tree/tests/not_called/mod.rs` (`:121`, `:480-507`) — cited.
- `ess/domains/bindings.yaml` (`:362-380`, `:465-467`), `ess/domains/tree.yaml` (`:228`) — cited;
  `generated/ess/loom`, `generated/rust/loom`, `website/docs/reference/.generated.json`,
  `website/docs/reference/ess/loom-bindings.md`, `website/docs/reference/ess/loom-tree.md`
  regenerated (inferred: the reference pages follow the two new members).
- `website/docs/guides/bindings.md` (`:405-408`) — cited; `AGENTS.md`, `CHANGELOG.md` — inferred.

## Edges

- **depends_on `story:wildcard-result-row-does-not-match-timeout`** (Runbook's order: the timeout
  story, the wildcard story, then this one). The wildcard story edits the same ESS comment
  paragraph of `ProgramRefusalReason` (`ess/domains/bindings.yaml:456-474`) and the adjacent guide
  paragraphs (`website/docs/guides/bindings.md:395-410`); this story rebases on it.
- **ESS and generated files.** `story:bound-program-timeout-above-an-hour`, the wildcard story,
  `story:spent-program-has-its-spent-record` and this story change `ess/` and so the one inputs
  digest of `generated/ess/loom/synthesis.txt:5`: they merge one at a time, and each after the
  first rebuilds `generated/ess/loom`, `generated/rust/loom` and the docs stamp (`task generate`,
  `task suite-generate`, `task docs-generate`, then `task check`, whose `docs-check` reads the
  stamp, `Taskfile.yml:45`, `:204-207`). Not a textual merge.
- **Shared files.** `crates/loom-bindings/src/program.rs` with the wildcard story (`look_up`
  region) and the spent story (offer path). `CHANGELOG.md` `[Unreleased]` and `AGENTS.md` with all
  five stories (timeout, wildcard, spent, CI budget, this): one bullet each, each rebased on the
  one merged before. The timeout story's `AGENTS.md:1461` cite has drifted (its text is at
  `:1474`), and its Edges do not name the docs stamp; it is not edited here.
