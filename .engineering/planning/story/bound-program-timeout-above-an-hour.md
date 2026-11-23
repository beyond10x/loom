---
format: aep.planning-md/3
id: story:bound-program-timeout-above-an-hour
kind: story
status: draft
title: A program binding may name a timeout_seconds up to four hours
summary: 'Runbook''s consumer ingest over an hour: MAX_PROGRAM_TIMEOUT_SECONDS 14400 for a program executor''s own program; every other executor, handoff_when and arguments_from keep 3600; default 60 s and older documents unchanged; TimeoutOutOfRange texts name both ranges; limits() keeps cpu_millis one hour; ESS comment only'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-bindings/src/arguments.rs
- confidence: cited
  path: crates/loom-bindings/src/handoff.rs
- confidence: cited
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/open.rs
- confidence: cited
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
revision: 16
---
## Outcome

A `program` binding may name a `timeout_seconds` up to 14400 (4 hours) for its action's own
program; every other executor and every helper program keeps 3600. A consumer's ingest of a
day's data through a rate-limited token pool runs as one action instead of three split to fit
under an hour. Nothing else moves: an action with no `timeout_seconds` still runs 60 s, every
document that loads today loads the same, the refusal reason stays `TimeoutOutOfRange`, and the
host-side bounds stay as they are. Runbook asked for this for a consumer. Cites are at
`5c036b23`.

**Before.** `MAX_TIMEOUT_SECONDS` is 3600 (`crates/loom-bindings/src/lib.rs:85-88`), and load
refuses any `timeout_seconds` outside 1 to 3600 for every executor, `TimeoutOutOfRange` at
`timeout_seconds`, `"{seconds} is not from 1 to 3600"` (`crates/loom-bindings/src/load.rs:210-219`).
The bound is deliberate: `story:action-limits-enforced` (draft) keeps the engine's ceiling in 1
to 3600, "the range `loom.bindings/1` admits" (its lines 21-26), and
`story:bound-program-declares-its-environment` (draft) lists "A `timeout_seconds` above 3600"
under what it does not do (its line 321). The value reaches the spawn as follows:

- the program port: `binding.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS)` (60,
  `crates/loom-intake-slice/src/bindings.rs:69`), checked against the clock before anything
  starts (`crates/loom-bindings/src/program.rs:845-851`);
- the child: `LOOM_TIMEOUT_SECONDS` (`crates/loom-bindings/src/child.rs:94`, `:125`);
- Substrate: `limits(timeout)` (`crates/loom-intake-slice/src/confinement.rs:3385-3393`, used at
  `:1947` and `:3684`) sets `timeout_ms` from it and fixed bounds beside it, among them
  `cpu_millis: 3_600_000`; a child still running is killed at its timeout plus `DRAIN_MARGIN`
  (30 s, `:1033-1047`);
- the same binding value also times the action's helper programs: `handoff_when`
  (`crates/loom-bindings/src/handoff.rs:128-129`) and `arguments_from` of an `open` step
  (`crates/loom-bindings/src/open.rs:668`).

**After.**

- `lib.rs` gains `MAX_PROGRAM_TIMEOUT_SECONDS: i64 = 14400`; `MAX_TIMEOUT_SECONDS` stays 3600 and
  keeps its meaning for every other executor.
- `load.rs:210-219`: a `program` binding's `timeout_seconds` is admitted from 1 to 14400; any
  other executor's from 1 to 3600. Refusal texts, `TimeoutOutOfRange` at `timeout_seconds`:
  - a program above 14400: `"{seconds} is not from 1 to 14400, the range of a program
    executor"`;
  - another executor above 3600: `"{seconds} is not from 1 to 3600; only a program executor
    may name up to 14400"`.
- Helper programs keep the hour: `handoff_when` and `arguments_from` run under
  `min(timeout_seconds, 3600)` (D3).
- `limits()` is unchanged: `cpu_millis` stays 3 600 000 (D4).
- Documents: `website/docs/guides/bindings.md:212`, `:301`, `AGENTS.md:1461`, and the comment
  of `ess/domains/bindings.yaml:227` name both ranges.

## Decisions

- **D1. The form of the higher bound.** Default **a**.
  - a. A declared ceiling per action: the binding's own `timeout_seconds`, up to a new maximum
    of 14400, as today's member with a wider range for `program`.
  - b. No upper bound, the run's wall time as the limit: `max_wall_seconds` is checked only
    before a step (`crates/loom-tree/src/run.rs:100-101`), so nothing would end a running
    program, and the CLI's run has no wall bound by default.
  - c. A separate opt-in member (`long_running: true`) beside `timeout_seconds`: a second
    member that says what the number already says.
- **D2. The new maximum.** Default **a**: 14400 (4 h, the consumer's need with margin). **b**:
  86400 (a day).
- **D3. Which programs get it.** Default **a**.
  - a. The action's own `program` only. A `person` executor's program, a `handoff_when` check
    and an `arguments_from` program keep 3600: a person program runs inside `tree approve`
    holding the store lock and the approval (`crates/loom-tree/src/approve.rs:186-188`,
    `:220-222`), and a check or an argument program has no reason to run an hour.
  - b. Every program the binding times.
- **D4. CPU time.** Default **a**.
  - a. `limits()` keeps `cpu_millis: 3_600_000`: a program that waits on a token pool spends
    little CPU; one that computes for more than an hour is killed by Substrate as today.
  - b. `cpu_millis` follows the timeout.
- **D5. What bounds the longer hold (security).** Default **a**.
  - a. The declared ceiling alone, with today's host-side bounds, stated in the guide: a
    confined program holds its cgroup root and the tree's store lock (`run.rs:200`) for at most
    its `timeout_seconds` plus `DRAIN_MARGIN`; the run's cancel ends it within 10 s
    (`CANCEL_WATCH`, `run.rs:137`); CPU stays at one hour (D4); a person approval's program
    stays at an hour (D3); a suite cycle keeps its 60 s bound (`CYCLE_WALL_SECONDS`,
    `crates/loom-tree/src/suite/run.rs:403`, passed as the run's wall bound at `:1656-1664`).
    slot's 30-minute job limit is the build pool's, not Loom's, and does not apply to `tree
    run`.
  - b. Also a host cap: `RunSettings` and `tree run` gain a per-step maximum the embedder sets,
    clamping every step's timeout.
- **D6. The intake slice's own configuration** (`crates/loom-intake-slice/src/bindings.rs:71`,
  `:206-210`, its own `MAX_TIMEOUT_SECONDS` of 3600). Default **a**: unchanged, it is not
  `loom.bindings/1`. **b**: raised the same.

## ESS first

Exempt: no noun changes. `loom.bindings.BoundAction.timeout_seconds` stays
`Optional<Integer>` (`ess/domains/bindings.yaml:254`) and `TimeoutOutOfRange` (`:321`) is reused.
Only the comment at `:227` ("timeout (1 to 3600 seconds)") changes to name both ranges; then
`task generate` and `task suite-generate` (`SLOT_TIMEOUT: 30m`), never by hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are
POSIX sh, never Python. No test starts `b10x-loom` or lives in the `tree_run` test binary. No
test checks its own wall time, and no test waits for a timeout: a recording runner starts
nothing. A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30
minutes). No test binary is added.

**`crates/loom-bindings/tests/bindings_document.rs`**, decode and load only:

1. `a_program_may_name_four_hours` (red first): a `program` binding with `timeout_seconds:
   14400` loads; `7200` loads. Failing first: refused `TimeoutOutOfRange`.
2. `the_new_ranges_are_refused_by_name`: a program with `14401` is refused `TimeoutOutOfRange` at
   `timeout_seconds` with the program text; a `person` binding with `3601` with the other text;
   `0` on both with the program text and the other text. Failing first: today's text.
3. `older_documents_load_as_before` (the guard): `3600` and `1` on a program and on a person
   load; no `timeout_seconds` decodes to `None` and the port runs it at 60 s (test 4).

**`crates/loom-bindings/tests/bound_program.rs`**, in process through the port with a recording
runner (no program starts):

4. `the_declared_timeout_reaches_the_child`: `timeout_seconds: 14400`: the recorded
   `ChildCommand` has a timeout of 14400 s and `LOOM_TIMEOUT_SECONDS=14400`; with no member, 60 s
   and `60`. Failing first: the document does not load.
5. `helper_programs_keep_the_hour` (D3 a): the same binding with a `handoff_when` check and,
   on an `open` action, an `arguments_from` program: each recorded helper `ChildCommand` has a
   timeout of 3600 s. Failing first: the document does not load; with it loading and no clamp,
   14400 s.

**`crates/loom-intake-slice/src/confinement.rs`**, its unit tests (as `:4617` checks `limits`):

6. `limits_of_four_hours_keep_an_hour_of_cpu` (D4 a): `limits(Duration::from_secs(14400))` has
   `timeout_ms` 14 400 000 and `cpu_millis` 3 600 000. Failing first: none (the guard of D4).

**Gate.** `task check` exits 0; `task slow` passes after the ESS comment change.

## Out of scope

- No upper bound (D1 b), a per-step host cap (D5 b), a CPU bound that follows the timeout
  (D4 b).
- `story:action-limits-enforced`'s engine ceiling: when it is built, its `timeout_seconds`
  ceiling range follows this story's (1 to 14400 for a program).
- Ending a running step at the run's `max_wall_seconds`.
- The intake slice's configuration (D6 a).

## Scope

Derived 2026-11-23 from the plan tree at `5c036b23`. **Cited** means read at the line named;
**inferred** means a reading that could be wrong.

- `crates/loom-bindings/src/lib.rs` (`:85-88`) — cited.
- `crates/loom-bindings/src/load.rs` (`:23`, `:210-219`) — cited.
- `crates/loom-bindings/src/handoff.rs` (`:128-129`) and `src/open.rs` (`:668`) — cited (D3);
  the `arguments_from` of a program step, if it reads the binding's timeout too, is inferred
  (`crates/loom-bindings/src/arguments.rs`).
- `crates/loom-bindings/tests/bindings_document.rs`, `tests/bound_program.rs` — cited files.
- `crates/loom-intake-slice/src/confinement.rs` (`:3385-3393`, `:4617`) — cited; tests only.
- `ess/domains/bindings.yaml` (`:227`) — cited; comment only; `generated/` regenerated.
- `website/docs/guides/bindings.md` (`:212`, `:301`), `AGENTS.md` (`:1461`) — cited;
  `CHANGELOG.md` — inferred.
- Read and not changed — cited: `crates/loom-bindings/src/program.rs` (`:845-851`),
  `src/child.rs` (`:94`, `:125`); `crates/loom-intake-slice/src/bindings.rs` (`:69`, `:71`,
  `:206-210`), `src/confinement.rs` (`:1033-1047`, `:1947`, `:3684`);
  `crates/loom-tree/src/run.rs` (`:100-101`, `:137`, `:200`), `src/approve.rs` (`:186-188`,
  `:220-222`), `src/suite/run.rs` (`:403`, `:1656-1664`).

## Edges

- No `depends_on`.
- **ESS regeneration.** The comment change regenerates `generated/`; ESS-changing stories merge
  one at a time, the second regenerating after the first lands.
- **Would collide with**, no edge:
  - `story:action-limits-enforced` (draft): it fixes the engine's ceiling range at 1 to 3600;
    this story first, and that story's range follows (Out of scope).
  - `story:bound-program-declares-its-environment` (draft): its line 321 lists a timeout above
    3600 as not its work; no shared change. No order.
  - `story:rework-allowance-after-a-rejection` and others that change `load.rs` or
    `bindings_document.rs`: another region (the timeout check). Textual merge, either order.
