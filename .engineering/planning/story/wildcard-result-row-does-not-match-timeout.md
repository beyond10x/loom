---
format: aep.planning-md/3
id: story:wildcard-result-row-does-not-match-timeout
kind: story
status: draft
title: A program binding's * result row does not match a run killed at its timeout; only a timeout row does
summary: 'Runbook D709, option a: look_up skips * for the value timeout; a timed-out run with no timeout row is refused ResultNotInTable and records nothing; no load refusal; Breaking in 0.61.0 for a binding that relied on * for timeout, named in CHANGELOG and the release notes'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
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
revision: 9
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
- `look_up` takes the row of the value, else the `*` row (`program.rs:1365-1373`; `ANY_KEY`,
  `:147`). So `{"0": passed, "*": failed}` records `failed` for a timeout.
- Only a table with neither row refuses `ResultNotInTable`, with the text "the value {:?} of the {}
  entry is not in its results, which list no `*`" (`program.rs:462-473`). `ResultNotInTable` does
  not end the run (`refusal`, `:1461-1470`, through `refused`, `ends_run: false`, `:1453-1458`).
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

- `look_up` takes, for the value `timeout` of a run killed at its timeout, the `timeout` row only;
  for any other value, the value's row, else `*`, as today.
- A timed-out run whose entry has no `timeout` row is refused `ResultNotInTable` (D2 a), records
  nothing, writes the moves alone (as `:462-473` does today), and does not end the run. Its text:
  `"the run was killed at its timeout_seconds ({n} s), and the results of the {kind} entry name no
  timeout; * does not match a timeout"`. The text of any other value is today's, with "which list
  no `*`" kept.
- Load is unchanged (D1 a): a document that loads today loads the same.
- A `person`, `open`, `agent` or `copy` executor is untouched: a person's table never lists `*`
  (`decode.rs:718-729`), and `open` reads `performed` (`crates/loom-bindings/src/open.rs:482`).
- Documents: the guide (`:397-403`), `AGENTS.md:1557-1562`, the ESS comment
  (`bindings.yaml:456-457`), and `CHANGELOG.md` under `[Unreleased]`, `### Changed`:
  `- **Breaking:** a program binding's "*" row no longer matches a run killed at its
  timeout_seconds; such a run is refused ResultNotInTable and records nothing unless its table
  names "timeout". Add a "timeout" row (for example "timeout": failed) to every table that relied
  on "*" for it (story:wildcard-result-row-does-not-match-timeout)`. The 0.61.0 release notes
  carry the same line (`loom-xtask release-check` reads the `**Breaking:**` bullets, `CHANGELOG.md:8-11`).

**Migration for a consumer.** For each `program` binding entry whose table holds `*` and no
`timeout`: add `"timeout": <the result * gave>` to keep 0.60.0's records, or leave it and let a
timeout refuse. Runbook's 28 rows are its own to check (its D709); Loom changes no consumer file.

## Decisions

- **D1. Where the rule lives.** Default **a**.
  - a. At run time, in `look_up`: `*` does not match `timeout`. No document is refused; a binding
    that relied on `*` changes behaviour only when a run times out. Cost: one condition and one text.
  - b. At load (`loom check`/`check`): a program entry whose table lists `*` and no `timeout` is
    refused `InvalidEvidence` at `evidence[<i>].results`, and `*` keeps matching nothing new.
    Every such binding fails to load on 0.61.0 (Runbook's 28 rows) until it is edited.
  - c. No change in behaviour; the guide warns that `*` catches `timeout`.
- **D2. The refusal.** Default **a**.
  - a. `ResultNotInTable`, with the text above; no ESS enum change; the run is not ended.
  - b. A new `ProgramRefusalReason::ProgramTimedOut` that ends the run (`ends_run: true`), as
    `ProgramError` does: an ESS enum change and a second breaking line for a reader that matches
    the reason.
- **D3. Which entries.** Default **a**: every entry of a program binding that reads the run
  (`exit` and a report pointer), since both read `timeout` (`program.rs:1337-1339`). **b**: only
  `from: exit` entries; a pointer entry keeps `*` for `timeout`.

## ESS first

Exempt: no noun changes (D2 a). The comment of `ProgramRefusalReason` (`ess/domains/bindings.yaml:456-457`)
says `*` does not match `timeout`; then `task generate` and `task suite-generate`
(`SLOT_TIMEOUT: 30m`), never by hand.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. No test checks its own wall time. A test that needs a timeout uses
`timeout_seconds: 1` as the existing ones do (`bound_program.rs:516-517`). A slot job ends within
10 minutes, or names its time with `slot --timeout` (at most 30 minutes). No test binary is added.

**`crates/loom-bindings/tests/bound_program.rs`**:

1. `a_timeout_without_a_timeout_key_falls_to_star` (`:530-536`) is replaced by
   `a_timeout_is_not_matched_by_star` (red first): table `{"0": valid, "*": invalid}`, a program
   that sleeps past `timeout_seconds: 1`: the step is refused, the text starts `ResultNotInTable`
   and holds `timeout_seconds` and `* does not match a timeout`; the case holds no record.
   Failing first: performed, `invalid` recorded.
2. `a_timeout_records_the_result_of_the_timeout_key` (`:518-528`) passes unchanged (guard).
3. `an_exit_code_the_table_does_not_list_falls_to_star` (`:507-513`) passes unchanged (guard: `*`
   still matches every other value).
4. `a_timeout_with_neither_key_is_refused_result_not_in_table` (`:538-545`) passes, its text now
   the timeout's.
5. `a_pointer_entry_times_out_without_star` (D3 a): a `/status` pointer entry with
   `{"ok": valid, "*": invalid}` and a timeout: refused `ResultNotInTable`, no record. Failing
   first: `invalid` recorded.

**`crates/loom-bindings/tests/program_contract.rs`**: `a_timeout_is_never_an_exit_class` (`:836-849`)
passes unchanged (it names `timeout`).

**Other tests** that time out under a `*` table and no `timeout` row (inferred: none outside the
above; the unit greps the test trees for `timeout_seconds: 1` and lists what it changes).

**`CHANGELOG.md`**: the `[Unreleased]` section holds the `**Breaking:**` bullet above;
`loom-xtask release-check` passes on the release commit.

**Gate.** `task check` exits 0; `task slow` passes after the ESS comment change.

## Does not do

- Refuse a document at load (D1 b) or add a new refusal reason (D2 b).
- Change a person's, an open's, an agent's or a copy's table, or a `from: runs` entry.
- Give a timed-out last run its `spent` record: that is `story:spent-program-has-its-spent-record`.
- Edit any consumer's bindings.

## Scope

Derived 2026-11-23 from the source at `49499053`. **Cited**: read at the line named.
**Inferred**: a reading that could be wrong.

- `crates/loom-bindings/src/program.rs` (`:145-147`, `:462-473`, `:1332-1373`) — cited.
- `crates/loom-bindings/tests/bound_program.rs` (`:507-545`) — cited;
  `crates/loom-bindings/tests/program_contract.rs` (`:836-849`), read, not changed — cited.
- `ess/domains/bindings.yaml` (`:456-457`) — cited; `generated/` regenerated.
- `website/docs/guides/bindings.md` (`:397-403`), `AGENTS.md` (`:1557-1562`), `CHANGELOG.md`
  (`:13`, `[Unreleased]`) — cited.
- Read, not changed — cited: `crates/loom-bindings/src/decode.rs` (`:690-730`),
  `src/load.rs` (`:15-35`), `src/open.rs` (`:482`).

## Edges

- No `depends_on`.
- **Order** (Runbook's): `story:bound-program-timeout-above-an-hour` first, then this story, then
  `story:program-error-carries-its-reason`.
- **Would collide with**, no edge: `story:spent-program-has-its-spent-record` and
  `story:program-error-carries-its-reason` change `program.rs` in other regions; textual merge.
  After this story a timed-out last run records nothing more often; the spent story covers it.
