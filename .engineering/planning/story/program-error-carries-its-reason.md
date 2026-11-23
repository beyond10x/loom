---
format: aep.planning-md/3
id: story:program-error-carries-its-reason
kind: story
status: draft
title: A ProgramError refusal carries the program's stderr tail, walked for credentials and bounded, in its text and on the run tree status lists
summary: 'Promised to Runbook: the reason text in the result, not only the last log line; the stderr tail as the session executor walks it (credential lines, cut PEM blocks, escapes withheld; 4096 bytes); ProgramRun.reason and StatusRun.reason; the walk moves to one shared module'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
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
revision: 15
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

Then `task generate` and `task suite-generate` (`SLOT_TIMEOUT: 30m`), never by hand.

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
2. `a_program_error_withholds_a_token_line`: stderr `token ghp_abc123` then `failed`: the refusal
   and `reason` hold `failed` and a withheld note, never `ghp_abc123`. Failing first: no reason.
3. `a_program_error_withholds_a_cut_pem_block`: stderr holds a PEM block of 5000 bytes whose
   opening is outside the last 4096 bytes, then `failed`: no body line of the block is shown.
4. `a_long_stderr_is_cut_to_its_last_whole_lines`: 20 000 lines: the tail is at most 4096 bytes
   and ends with the last line.
5. `a_program_error_with_no_stderr_has_no_reason`: the text is today's, byte for byte, and
   `reason` is absent. Guard.

**`crates/loom-tree/tests/main.rs`**, module `not_called` (`classed_errors_and_unclassed_exits_behave_as_before`,
`tests/not_called/mod.rs:480-506`, extended): the `action finished` line of `work.do` holds the
stub's stderr line, and `tree status` (`status`, in process) lists the run with that `reason`.

**`crates/loom-session-executor`**: its tail tests pass unchanged after the move (D3 a).

**Gate.** `task check` exits 0; `task slow` passes after the ESS change.

## Does not do

- Show stdout, or the blob, in any text.
- Change `ProgramSignalled` (D5 a), `ResultNotInTable` or any other refusal's text.
- Change the intake slice's own tail (`crates/loom-intake-slice/src/bindings.rs:90`).

## Scope

Derived 2026-11-23 from the source at `49499053`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong.

- `crates/loom-bindings/src/program.rs` (`:375-378`, `:983-986`, `:1491-1512`, `:1550-1569`) — cited.
- `crates/loom-bindings/tests/program_contract.rs` — cited file.
- `crates/loom-session-executor/src/lib.rs` (`:142-165`, `:2878-2929`, `:2963`, `:3088`, `:3143`) —
  cited; the move.
- `crates/loom-commission/src` — inferred home of the walk (D3 a); `crates/loom-commission/Cargo.toml` — inferred.
- `crates/loom-tree/src/status.rs` (`:283-325`) — cited;
  `crates/loom-tree/tests/not_called/mod.rs` (`:480-506`) — cited.
- `ess/domains/bindings.yaml` (`:362-380`, `:465-467`), `ess/domains/tree.yaml` (`:228`) — cited;
  `generated/` regenerated.
- `website/docs/guides/bindings.md` (`:405-408`) — cited; `AGENTS.md`, `CHANGELOG.md` — inferred.

## Edges

- No `depends_on`.
- **ESS regeneration.** ESS-changing stories merge one at a time, the second regenerating after the
  first lands.
- **Would collide with**, no edge: `story:spent-program-has-its-spent-record` and
  `story:wildcard-result-row-does-not-match-timeout` change `program.rs` in other regions; textual
  merge. Runbook's order puts this story after the wildcard story.
