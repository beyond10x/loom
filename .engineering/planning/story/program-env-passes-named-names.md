---
format: aep.planning-md/3
id: story:program-env-passes-named-names
kind: story
status: draft
title: A bound program gets HOME and the environment names its binding lists, and nothing else
summary: 'a downstream consumer A10: today every program gets PATH and LOOM_* alone, no HOME, so caps set in the environment never reach it; D1 a pass_env lists names read from tree run''s environment, D2 a HOME set by the port, D3 a load refuses PATH, HOME, LOOM_*, LD_*, BASH_ENV, ENV; values never recorded; Added'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:results-row-records-nothing
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/arguments.rs
- confidence: cited
  path: crates/loom-bindings/src/child.rs
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/handoff.rs
- confidence: cited
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-bindings/tests/program_contract.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
- confidence: cited
  path: website/docs/reference/.generated.json
- confidence: cited
  path: website/docs/reference/ess/loom-bindings.md
revision: 21
---
## Outcome

A binding names the environment variables its programs need, and only those reach them, at the
values `tree run` itself was started with; every program also gets a `HOME` it may write. A cap
override the operator sets in the environment of `tree run` then reaches the program whose binding
lists its name, and no consumer rebuilds `HOME` in its programs. Asked by a downstream consumer (A10): by
its report, measured with runbook 0.9.0, it rebuilds `HOME` and `PATH` in 17 places, and cap
overrides set in the environment never reach a program. Cites are at `03f64d4e` (0.60.0 with the
wave-8 timeout and wildcard units); `child.rs` and the intake slice's `bindings.rs` are as at
`49499053` (tag 0.60.0).

**Before.**

- Every program the port starts, the bound program, its `arguments_from` program and its
  `handoff_when` check alike, is built by `bound_case_child`
  (`crates/loom-bindings/src/child.rs:60-99`; callers `program.rs:897`, `arguments.rs:64`,
  `handoff.rs:135`). Its environment is replaced by the one `loom.bindings.ProgramEnvironment`
  describes: `PATH` at the host process's value, `LOOM_CASE`, `LOOM_TIMEOUT_SECONDS` and the two
  opening names, "with no host variable inherited" (`child.rs:45-53`, `:111-137`; ESS
  `ess/domains/bindings.yaml:387-424`). `LOOM_RETRY_FEEDBACK` is added on a retried bound program
  only (`retry.rs:308-318`).
- `PATH` comes from `bound_program_child`, whose one inherited name is `PATH`
  (`crates/loom-intake-slice/src/bindings.rs:74-75`, `:414-448`). `ExecEnvironment.allow` is
  always empty (`child.rs:133-136`); Substrate's baseline names that `allow` could carry are
  `LANG`, `LC_ALL`, `PATH`, `TERM` and `TZ` (`confinement/spec.rs:103-129`), never `HOME`.
- No `HOME` is set. Under Substrate the child sees a private `/tmp` and its workspace, not the
  host's home (`crates/loom-intake-slice/src/confinement.rs:1365-1388`); Loom's own Substrate
  test runner sets `HOME=/tmp` for cargo for that reason (`confinement.rs:3633-3636`).
- The docs state the rule: "`PATH`, `LOOM_CASE`, … and `LOOM_TIMEOUT_SECONDS` alone in its
  environment … a document cannot name an environment" (`website/docs/guides/bindings.md:331-346`,
  `:160-166`; `AGENTS.md:1550-1556`, `:1815`), and a test pins the exact set: the
  `environment()` fixture (`crates/loom-bindings/tests/program_contract.rs:479-492`) and
  `a_program_starts_with_its_timeout_in_its_environment` (`:689-697`).
- Loom reads no cap of its own from the environment: `tree run`'s bounds are flags
  (`--max-steps` and the like), and the only variables the CLI reads are its re-exec and agent
  transport ones (`crates/loom-cli/src/main.rs:275`, `:295`, `:1136`, `src/tree.rs:222`, `:273`).
  So "cap overrides" are the consumer's own variables, read by its programs; the ask is right that
  none of them can reach a program today, and nothing serves it.
- The delegated re-exec of `tree run` under `systemd-run --user --scope`
  (`crates/loom-cli/src/lib.rs:612-613`) runs as a child of the caller, so the environment the
  operator started `tree run` with is the one the port reads (inferred from `--scope`).

**After.**

1. **A binding lists names (D1 a).** A new binding member, inferred name `pass_env`, a list of
   variable names: `pass_env: [DEV_OPERATOR_MAX_TURNS, GIT_AUTHOR_NAME]`. When the port builds a
   child it reads each listed name from its own process environment at that step and sets it on
   the child beside `PATH` and the `LOOM_*` variables; a name the process does not have is not set
   (D5 a); a value that is not UTF-8 refuses the step before anything starts, as a non-UTF-8 `PATH`
   does, naming the variable and never quoting its value. No other host variable is inherited.
2. **Every program of that binding (D4 a).** The bound program, its `arguments_from` program and
   its `handoff_when` check get the same listed names. `bound_case_child` keeps its signature and
   passes no listed name; one exported function beside it (inferred `bound_case_child_with`) takes
   the names, and the three callers use it.
3. **`HOME` (D2 a).** Every program gets `HOME`, set by the port: under a runner that is not
   `local`, `/tmp`, the child's private `/tmp`; under `local`, a directory made for that one child
   under the host's temporary directory and removed after it ends (inferred form), so a program
   never writes the operator's home. `HOME` is the port's, like `PATH` and `LOOM_*`.
4. **Load (D3 a).** `pass_env` is checked at load, each refusal naming the member
   (`pass_env[i]`): a name that is empty or not `[A-Za-z_][A-Za-z0-9_]*`; a name listed twice;
   more than 32 names; and a name the port owns or that makes a loader or shell run code: `PATH`,
   `HOME`, any `LOOM_` name, any `LD_` name, `BASH_ENV` and `ENV`. `pass_env` on an `agent` or
   `copy` binding is refused `MemberNotForExecutor` (`load.rs:271-384`, the member fit checks).
5. **No value is recorded.** The run observation (`loom.bindings.ProgramRun`, `program.rs:1572`
   `run_json`), the run log and the retry feedback document name no passed variable and no value.
   The bindings file names only names. Substrate's profile digest covers the child's environment,
   values included (`confinement.rs:3850-3861`); it is a digest in Substrate's record, never kept
   by Loom (inferred: the port keeps no profile digest in the case).
6. **The model and docs.** ESS first: `loom.bindings.BoundAction` gains `pass_env:
   List<String>`, `loom.bindings.ProgramEnvironment` gains `home` and the passed variables, and
   their comments say the rule; then `task generate`. The guide (`bindings.md:160-166`,
   `:331-346`) and `AGENTS.md` (`:1550-1556`, `:1815`) say what reaches a program, and how a cap
   override reaches one: export it where `tree run` starts and list its name in `pass_env`.

## Decisions

- **D1. How a binding names its environment.** Default **a**.
  - a. A list of names (`pass_env`), values read from the environment `tree run` was started with.
  - b. Values written in the bindings file (`env: {NAME: value}`): puts secrets in a file that is
    committed and digested, and a cap override would need an edit of the bindings.
  - c. The whole environment (`inherit_env: true`): every secret of the operator's shell reaches
    every program, under confinement too.
  - d. A prefix (`pass_env: [DEV_OPERATOR_*]`): passes names nobody reviewed, as the operator's
    shell grows.
- **D2. `HOME`.** Default **a**: the port sets a writable private `HOME` for every program
  (After 3). **b**: `HOME` only when listed, at the host's value: under Substrate that directory is
  not mounted, and under `local` it hands the operator's dotfiles and credentials to the program.
  **c**: no `HOME`, as today: the 17 rebuilds stay.
- **D3. Names Loom refuses.** Default **a**: the port's own (`PATH`, `HOME`, `LOOM_*`) and those a
  dynamic loader or shell reads to run code (`LD_*`, `BASH_ENV`, `ENV`), with the name syntax and
  at most 32 names. **b**: the port's own names only.
- **D4. Which programs get the names.** Default **a**: every program the binding starts (one list
  per binding; the check and the `arguments_from` program read what the bound program reads).
  **b**: the bound program only: a second list, or a check that cannot read the cap it decides on.
- **D5. A listed name the host does not have.** Default **a**: not set; the program decides.
  **b**: the step is refused: a cap override is optional by nature.

## ESS first

`ess/domains/bindings.yaml`: `BoundAction` (`:225-264`) gains `pass_env`; `ProgramEnvironment`
(`:387-424`) gains `home` and the passed variables; both comments, and the comment of `handoff_when`
(`:234-237`, "`PATH` and the `LOOM_*` variables alone"), say the new rule. Then `task generate`,
`task suite-generate` (`SLOT_TIMEOUT: 30m`) and `task docs-generate`, never by hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. A slot job ends within 10 minutes, or names its time with `slot --timeout` (at
most 30 minutes). No test binary is added. A test that needs a host variable sets it on a child
process or through the port's environment source, never with `std::env::set_var` in a test that
shares its process (inferred: the port reads the environment through a function the tests can
give).

**`crates/loom-bindings/tests/program_contract.rs`**, its `environment()` fixture (`:479-492`)
extended with `HOME`:

1. `a_program_starts_with_home_and_its_named_variables`: `/usr/bin/env` bound with
   `pass_env: [LOOM_TEST_CAP_X]` is refused at load (a `LOOM_` name); bound with
   `pass_env: [DEV_CAP]` and the port's environment holding `DEV_CAP=5` and `OTHER=1`, it prints
   exactly `PATH`, `LOOM_CASE`, `LOOM_TIMEOUT_SECONDS`, `HOME` and `DEV_CAP=5`; `OTHER` is absent.
   Failing first: `pass_env` is an unknown member, and `HOME` is absent.
2. `a_listed_name_the_host_lacks_is_not_set`: `pass_env: [DEV_CAP]`, no `DEV_CAP`: the printed
   environment has no `DEV_CAP` and the step performs.
3. `a_listed_value_that_is_not_utf8_refuses_the_step`: the step is refused before anything starts,
   naming `DEV_CAP` and quoting no value.
4. `the_arguments_program_and_the_check_get_the_same_names`: the recording runner's children for
   the `arguments_from` program and for the `handoff_when` check hold `DEV_CAP` and `HOME`, and
   `environment().allow` stays empty (as `:714-716` asserts today).
5. `no_passed_value_is_recorded`: after a step with `DEV_CAP=secret-7f3`, the case's observations,
   the run log line and the retry feedback document hold no `secret-7f3`.
6. `a_program_starts_with_its_timeout_in_its_environment` (`:689-697`) and the other exact-set
   tests pass with `HOME` added to the fixture; under the flag's (`local`) runner `HOME` names a
   directory that exists while the child runs and is gone after.

**`crates/loom-bindings/tests/bindings_document.rs`** (load):

7. `pass_env_refusals`: `""`, `1X`, `A-B`, a name twice, 33 names, `PATH`, `HOME`, `LOOM_CASE`,
   `LD_PRELOAD`, `BASH_ENV` and `ENV`: each refused naming `pass_env[i]`; `pass_env` on an `agent`
   and a `copy` binding refused `MemberNotForExecutor`. Failing first: refused as an unknown
   member, naming another member.

**Documents.**

8. Read in the diff: the guide and `AGENTS.md` lines named in After 6 say `HOME` and the listed
   names reach a program and nothing else, and how a cap override reaches one; `CHANGELOG.md`
   `[Unreleased]` `### Added` holds the bullet below; `task docs-check` passes.

**Gate.** `task check` exits 0; `task slow` passes after the ESS change.

**CHANGELOG** (`### Added`, not Breaking: a document without `pass_env` loads as today and its
programs get the same variables plus `HOME`; `bound_case_child` keeps its signature): "A binding
may list `pass_env`, the names of environment variables its programs read: each reaches the bound
program, its `arguments_from` program and its `handoff_when` check at the value `tree run` was
started with, and nothing else of the host's environment does. Every program now gets a `HOME` it
may write (`/tmp` under Substrate). `PATH`, `HOME`, `LOOM_*`, `LD_*`, `BASH_ENV` and `ENV` cannot
be listed (`story:program-env-passes-named-names`)."

## Does not do

- D1 b: values in the bindings file. D1 c: the whole environment. D1 d: a prefix or glob.
- D2 b: the host's `HOME`. D2 c: no `HOME`.
- D3 b: allowing `LD_*`, `BASH_ENV` or `ENV`.
- D4 b: a separate list for the check or the `arguments_from` program.
- D5 b: refusing a step for a listed name the host lacks.
- Pass a variable to an `agent` session or a `copy`; change Substrate's baseline (`allow`) names;
  give a program network access or a mount for a passed path.
- Record a passed value anywhere in the case, the run log or the feedback document.
- Read a cap of Loom's own (`--max-steps` and the like) from the environment.

## Scope

Derived 2026-11-24 from the source at `03f64d4e`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong.

- `ess/domains/bindings.yaml` (`:225-264`, `:387-424`) — cited; `generated/ess/loom`,
  `generated/rust/loom`, `website/docs/reference/ess/loom-bindings.md` and
  `website/docs/reference/.generated.json` regenerated.
- `crates/loom-bindings/src/child.rs` (`:45-137`) — cited, the environment and the new function;
  `src/lib.rs` (`:53`) exports it — cited.
- `crates/loom-bindings/src/program.rs` (`:897`), `src/arguments.rs` (`:64`), `src/handoff.rs`
  (`:135`) — cited, the three callers.
- `crates/loom-bindings/src/decode.rs` (`:22-35`, the member list) and `src/load.rs`
  (`:271-384`) — cited, the member and its checks.
- Read, not changed — cited: `src/retry.rs` (`:308-318`);
  `crates/loom-intake-slice/src/bindings.rs` (`:74-75`, `:414-448`), `src/confinement.rs`
  (`:1365-1388`, `:3633-3636`, `:3850-3861`), `src/confinement/spec.rs` (`:103-129`);
  `crates/loom-cli/src/lib.rs` (`:612-613`), `src/main.rs`, `src/tree.rs`.
- Tests: `crates/loom-bindings/tests/program_contract.rs` (`:479-492`, `:689-697`, `:714-716`),
  `tests/bindings_document.rs` — cited.
- `website/docs/guides/bindings.md` (`:160-166`, `:331-346`), `AGENTS.md` (`:1550-1556`,
  `:1815`) — cited; `CHANGELOG.md` — one `### Added` bullet under `[Unreleased]`.

## Edges

- **depends_on `story:results-row-records-nothing`**: both change `ess/domains/bindings.yaml`
  and the generated files; this story merges after it, and so after the 0.61.0 ESS-changing
  stories (timeout, wildcard, program-error, spent), each rebuilding `generated/ess/loom`,
  `generated/rust/loom` and the docs stamp on the one before (`task generate`,
  `task suite-generate`, `task docs-generate`, then `task check`).
- **Shared files.** `decode.rs` and `load.rs` with `story:results-row-records-nothing` and
  `story:program-exits-error-has-a-default`; `program_contract.rs` with the timeout story
  (`:737-751`); the guide, `AGENTS.md` and `CHANGELOG.md` with every story of the wave. One bullet
  each, rebased on the one merged before.
- **Security review.** A passed variable is a new way for the operator's environment into a
  confined program: the story goes to the security reviewer with its acceptance tests 1, 3, 5
  and 7.
