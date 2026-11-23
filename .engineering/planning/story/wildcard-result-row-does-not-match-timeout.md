---
format: aep.planning-md/3
id: story:wildcard-result-row-does-not-match-timeout
kind: story
status: active
title: A program binding's * result row does not match a run killed at its timeout; only a timeout row does
summary: 'Runbook D709, option a: look_up skips * for the value timeout; a timed-out run with no timeout row is refused ResultNotInTable and records nothing; no load refusal; Breaking in 0.61.0 for a binding that relied on * for timeout, named in CHANGELOG and the release notes'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bound-program-timeout-above-an-hour
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
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
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T21:45:32Z", actor: "agent:sdc", revision: 12, executor: "agent:loom", correlation: "runbook-wildcard-not-timeout-2026-11-23"}
- {from: "proposed", to: "active", at: "2026-11-23T21:45:33Z", actor: "agent:sdc", revision: 13, executor: "agent:loom", correlation: "runbook-wildcard-not-timeout-2026-11-23"}
---
## Outcome

In a `program` binding's `evidence` table the `*` row no longer matches a run killed at its
`timeout_seconds`: only a row that names `timeout` does. A timed-out run whose table names no
`timeout` row is refused and records nothing, so a binding cannot record its failure result (or,
worse, a pass) for a run that never answered. This is a **Breaking** change for a binding that
relied on `*` for `timeout`, shipped in 0.61.0, and the 0.61.0 notes name it. Runbook decided it
(its D709: "activate [...] with its first option (a `"*"` row does not catch `timeout`; a
timed-out program is a refusal unless the table names `timeout`)"; it checks its 28 `*` rows when
it pins 0.61.0). Cites are at `49499053` (tag 0.60.0).

**Before.**

- A run killed at its timeout reads the value `timeout` for every entry, before `from` is looked at
  (`value_of`, `crates/loom-bindings/src/program.rs:1332-1339`; `TIMEOUT_KEY`, `:145`), a pointer
  entry included.
- `look_up(entry, value)` takes the row of the value, else the `*` row
  (`program.rs:1365-1373`; `ANY_KEY`, `:147`); it sees only the string. So
  `{"0": passed, "*": failed}` records `failed` for a timeout. A report pointer whose value is the
  string `"timeout"` from a run that ended normally reads the same string.
- Only a table with neither row refuses `ResultNotInTable`, with the text "the value {:?} of the {}
  entry is not in its results, which list no `*`" (the call site, `program.rs:462-473`).
  `ResultNotInTable` does not end the run (`refusal`, `:1461-1470`, through `refused`,
  `ends_run: false`, `:1452-1458`).
- A refused step sets its action aside in `tree run` until the case changes
  (`crates/loom-tree/src/run.rs:1064-1072`, `evaluated` at `:877-899`).
- The exit classes of `story:bound-program-contract` never take a timeout: `class_of` returns none
  for `timed_out` (`program.rs:1473-1489`), so a timeout always reaches the tables.
- Load does not look at `timeout` or `*` in a program's table (`crates/loom-bindings/src/decode.rs:690-730`
  checks only an `effect` table's keys; `crates/loom-bindings/src/load.rs:15-35` lists every load
  refusal, none about it).
- Pinned and documented: `a_timeout_without_a_timeout_key_falls_to_star`
  (`crates/loom-bindings/tests/bound_program.rs:530-536`);
  `website/docs/guides/bindings.md:397-403` ("It looks the value up in `results`, falling to
  `*`"); `AGENTS.md:1557-1562`; the comment of `ProgramRefusalReason`
  (`ess/domains/bindings.yaml:456-457`, "a timeout reads the key `timeout`").

**After.**

- The condition is the run's, not the string's: at the call site (`program.rs:462`) the lookup is
  given `execution.timed_out` (inferred shape: `look_up(entry, value, timed_out)`). For a run
  killed at its timeout, only the `timeout` row matches. For every other run, the value's row,
  else `*`, as today, including a report value that happens to be the string `"timeout"`.
- A timed-out run whose entry has no `timeout` row is refused `ResultNotInTable` (D2 a), records
  nothing, writes the moves alone (as `:462-473` does today), and does not end the run. Its text:
  `"the run was killed at its timeout_seconds ({n} s), and the results of the {kind} entry name no
  timeout; * does not match a timeout"`. The text of any other value is today's.
- **What a consumer sees.** Where 0.60.0 recorded the `*` result for a timeout and went on (a
  retry through `retry_when`, or a `given_up` outcome over that result), the step is now refused,
  the action is set aside, and the case stops `Stuck` until something else changes it. A timed-out
  **last** run (its allowance spent) leaves no `spent` record, which
  `story:spent-program-has-its-spent-record` writes; that story depends on this one and ships in
  the same release (0.61.0). Without it a timed-out last run would leave the case stuck for good.
- Load is unchanged (D1 a): a document that loads today loads the same.
- A `person`, `open`, `agent` or `copy` executor is untouched: a person's table never lists `*`
  (`decode.rs:718-729`), and `open` reads `performed` (`crates/loom-bindings/src/open.rs:482`).
- Documents: the guide (`:397-403`), `AGENTS.md:1557-1562`, the ESS comment
  (`bindings.yaml:456-457`), and `CHANGELOG.md` under `[Unreleased]`, `### Changed`, one bullet:
  `- **Breaking:** a program binding's "*" row no longer matches a run killed at its
  timeout_seconds: such a run is refused ResultNotInTable, records nothing and its action is set
  aside, so a case that took the "*" result for a timeout and went on (a retry, a given_up
  outcome) now stops Stuck. Add a "timeout" row (for example "timeout": failed) to every table
  that relied on "*" for it (story:wildcard-result-row-does-not-match-timeout)`. The 0.61.0
  release notes carry the same line.

**Migration for a consumer.** For each `program` binding entry whose table holds `*` and no
`timeout`: add `"timeout": <the result * gave>` to keep 0.60.0's records and flow, or leave it and
let a timeout refuse and stop the case. Runbook's 28 rows are its own to check (its D709); Loom
changes no consumer file.

## Decisions

- **D1. Where the rule lives.** Default **a**.
  - a. At run time, at the lookup, keyed on `execution.timed_out`. No document is refused; a
    binding that relied on `*` changes behaviour only when a run times out, and then the case
    stops `Stuck` where it went on before (above). Cost: one parameter, one condition, one text.
  - b. At load (`loom check`/`check`): a program entry whose table lists `*` and no `timeout` is
    refused `InvalidEvidence` at `evidence[<i>].results`. Every such binding fails to load on
    0.61.0 (Runbook's 28 rows) until it is edited; no case stops at run time.
  - c. No change in behaviour; the guide warns that `*` catches `timeout`.
- **D2. The refusal.** Default **a**.
  - a. `ResultNotInTable`, with the text above; no ESS enum change; the run is not ended.
  - b. A new `ProgramRefusalReason::ProgramTimedOut` that ends the run (`ends_run: true`), as
    `ProgramError` does: an ESS enum change and a second breaking line for a reader that matches
    the reason.
- **D3. Which entries.** Default **a**: every entry of a program binding that reads the run
  (`exit` and a report pointer), since both read `timeout` for a timed-out run
  (`program.rs:1337-1339`). **b**: only `from: exit` entries; a pointer entry keeps `*` for a
  timeout.

## ESS first

Exempt: no noun changes (D2 a). The comment of `ProgramRefusalReason` (`ess/domains/bindings.yaml:456-457`)
says `*` does not match a run killed at its timeout; then `task generate`, `task suite-generate`
(`SLOT_TIMEOUT: 30m`) and `task docs-generate`, never by hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. No test checks its own wall time. A test that needs a timeout uses
`timeout_seconds: 1` as the existing ones do (`bound_program.rs:516`). A slot job ends within
10 minutes, or names its time with `slot --timeout` (at most 30 minutes). No test binary is added.

**`crates/loom-bindings/tests/bound_program.rs`**:

1. `a_timeout_without_a_timeout_key_falls_to_star` (`:530-536`) is replaced by
   `a_timeout_is_not_matched_by_star` (red first): table `{"0": valid, "*": invalid}`, a program
   that sleeps past `timeout_seconds: 1`: the step is refused, the text starts `ResultNotInTable`
   and holds `killed at its timeout_seconds (1 s)` and `* does not match a timeout`; the case holds
   no record. Failing first: performed, `invalid` recorded.
2. `a_timeout_records_the_result_of_the_timeout_key` (`:518-528`) passes unchanged (guard).
3. `an_exit_code_the_table_does_not_list_falls_to_star` (`:507-513`) passes unchanged (guard: `*`
   still matches every other value).
4. `a_timeout_with_neither_key_is_refused_result_not_in_table` (`:538-545`) is extended to assert
   the text holds `killed at its timeout_seconds (1 s)` and no longer `which list no *`. Failing
   first: today's text.
5. `a_pointer_entry_times_out_without_star` (D3 a): a `/status` pointer entry with
   `{"ok": valid, "*": invalid}` and a timeout: refused `ResultNotInTable`, no record. Failing
   first: `invalid` recorded.
6. `a_report_value_timeout_still_falls_to_star`: a run that exits 0 and prints
   `{"status": "timeout"}`, a `/status` entry with `{"ok": valid, "*": invalid}`: `invalid` is
   recorded (the rule is the run's, not the string's). Guard.

**`crates/loom-bindings/tests/program_contract.rs`**: `a_timeout_is_never_an_exit_class` (`:836-849`)
passes unchanged (it names `timeout`).

**No test relies on `*` for a timeout.** The test step passes after the change (a test that did
would now fail), and `rg -n 'falls_to_star' crates/loom-bindings/tests/bound_program.rs` prints only
`an_exit_code_the_table_does_not_list_falls_to_star` and `a_report_value_timeout_still_falls_to_star`.
The files that time a program out at 1 s at `49499053` are `bound_program.rs`,
`open_from_program.rs`, `program_contract.rs`, and `crates/loom-cli/tests/tree_run/handoff_when.rs`
and `person_program.rs`.

**`CHANGELOG.md`**: the `[Unreleased]` section holds the `**Breaking:**` bullet above (read in the
diff).

**Gate.** `task check` exits 0; `task slow` passes after the ESS comment change.

## Does not do

- D1 b: refuse a document at load. D1 c: keep `*` matching a timeout.
- D2 b: a new refusal reason that ends the run.
- D3 b: keep `*` for a timeout in a pointer entry.
- Change a person's, an open's, an agent's or a copy's table, or a `from: runs` entry.
- Give a timed-out last run its `spent` record: that is `story:spent-program-has-its-spent-record`.
- Edit any consumer's bindings. Run `loom-xtask release-check` (that belongs to the 0.61.0 release).

## Scope

Derived 2026-11-23 from the source at `49499053`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong.

- `crates/loom-bindings/src/program.rs` — changed: `:462-473`, `:1365-1373`; read: `:145-147`,
  `:1332-1339` — cited.
- `crates/loom-bindings/tests/bound_program.rs` (`:507-545`) — cited;
  `crates/loom-bindings/tests/program_contract.rs` (`:836-849`), read, not changed — cited.
- `ess/domains/bindings.yaml` (`:456-457`) — cited; `generated/ess/loom`, `generated/rust/loom`,
  `website/docs/reference/.generated.json` regenerated.
- `website/docs/guides/bindings.md` (`:397-403`), `AGENTS.md` (`:1557-1562`), `CHANGELOG.md`
  (`:13`, `[Unreleased]`) — cited.
- Read, not changed — cited: `crates/loom-bindings/src/decode.rs` (`:690-730`),
  `src/load.rs` (`:15-35`), `src/open.rs` (`:482`), `crates/loom-tree/src/run.rs` (`:877-899`,
  `:1064-1072`).

## Edges

- **depends_on `story:bound-program-timeout-above-an-hour`** (Runbook's order: the timeout story,
  then this one, then `story:program-error-carries-its-reason`).
- **ESS and generated files.** The timeout story, this story, the program-error story and
  `story:spent-program-has-its-spent-record` change `ess/` and so the one inputs digest of
  `generated/ess/loom/synthesis.txt:5`: they merge one at a time, and each after the first rebuilds
  `generated/ess/loom`, `generated/rust/loom` and the docs stamp (`task generate`,
  `task suite-generate`, `task docs-generate`, then `task check`, whose `docs-check` reads the
  stamp, `Taskfile.yml:45`, `:204-207`). Not a textual merge.
- **Shared files.** `crates/loom-bindings/tests/bound_program.rs` with the timeout and spent stories
  (the timeout story's Edges do not name it). The ESS `ProgramRefusalReason` comment
  (`bindings.yaml:456-474`) and the guide's paragraphs `bindings.md:395-410` with the program-error
  story. `CHANGELOG.md` `[Unreleased]` and `AGENTS.md` with all five stories (timeout, spent,
  program-error, CI budget, this): one bullet each, each rebased on the one merged before. The
  timeout story's `AGENTS.md:1461` cite has drifted (its text is at `:1474`); it is not edited here.
- **Followed by** `story:spent-program-has-its-spent-record` (it depends on this story).
