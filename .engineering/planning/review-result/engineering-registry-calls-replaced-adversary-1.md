---
format: aep.planning-md/3
id: review-result:engineering-registry-calls-replaced-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on story:engineering-registry-calls-replaced (wave 2026-11-21-w4, unit U3, head 7ebac517)
relations:
- reviews: story:engineering-registry-calls-replaced
revision: 1
---
unit: U3 story:engineering-registry-calls-replaced, worktree loom-w1121d-u3 at head `7ebac517` plus 3 untracked adversary test files
verdict: green (nothing found that blocks the unit)
cases: executed 216→221 (governor + intake-router + protocols lane; 216 is the implementor's gate count), red 0
origin: introduced 2, pre-existing 0, undecided 0 (both are notes)
wrote-outside-worktree: 19 files in ~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w4/u3/adv1/ (one.sh, suite.sh, {gov,thr,rtr,suite,clippy}.{disk,exit,log}, fmt.{exit,log})
needs-coordinator: none

`git --no-pager diff --stat` is empty: no tracked file changed. I wrote only three new test files, listed below. I mutated nothing in the tree. Charter note: the adversary procedure forbids even a temporary in-tree mutation, which is stricter than the brief, so I mutated no file at all.

**Cases added.** All are green now and all were green on their first run. Each binary was run alone, before the suite.
- `crates/loom-governor/tests/adversary_w20261121w4_u3.rs`
  - `adversary_a_catalogless_governor_answers_exactly_the_base_builtins`: each of the ten registrations at base, written as literals, is refused by `with_protocol_yaml` as "already registered". Twelve near-misses register, among them `software-change@01`, `software.change@1`, `support-triage@1@1` and `system-query@1`.
  - `adversary_open_finds_every_base_builtin_and_nothing_else`: `open` never answers `UnknownProtocol` for a base built-in, and always answers it for a near-miss.
  - Output: `test result: ok. 2 passed; 0 failed`.
- `crates/loom-governor/tests/adversary_w20261121w4_u3_threads.rs`
  - `adversary_the_first_builtin_lookup_from_many_threads_sees_one_catalog`: 16 threads behind a barrier make the process's first `builtin()` call. All see the same catalog, none panics.
  - Output: `1 passed; 0 failed`.
- `crates/loom-intake-router/tests/adversary_w20261121w4_u3.rs`
  - `adversary_the_model_is_shown_the_base_protocols_in_the_base_order`: the tool enum, and the order of the `- name:` lines in the instructions, equal the base order as a literal list of ten. There is one line per protocol and nothing else is listed.
  - `adversary_an_out_of_range_threshold_is_refused_before_any_call`: thresholds -0.01, 1.01, NaN and inf each give `InvalidThreshold`, with 0 model turns.
  - Output: `2 passed; 0 failed`.

**Suite**, run after the cases existed:
- `ESS_TOOLCHAIN=0.86.0 CARGO_INCREMENTAL=0 cargo test -p b10x-loom-governor -p b10x-loom-intake-router -p b10x-loom-protocols --locked`: exit 0, 221 passed, 0 failed (216 + 5).
- clippy on governor and router with `--all-targets -D warnings`: exit 0.
- `cargo fmt --check`: first exit 1, on one line in my governor test file. I wrapped that line by hand and the re-check exits 0. The tests were not re-run after that whitespace-only edit.

**Findings** (no failing case possible: `builtin_in` is private and the only refused-catalog path runs through lib.rs unit tests, which I may not edit)
1. A mutant survives, by reasoning. `crates/loom-governor/src/lib.rs:1752-1763`: with a refused catalog, nothing tests a malformed name (`UnknownProtocol`) or an unknown well-formed name (`InvalidProtocol`). Only `software-change@1` is tested. Moving the `name@major` check after the catalog map, or dropping it, stays green. The current order is the right one: `with_protocol` (:857) and `checked_builtin_inputs` (:1626) correctly take the lenient path on `UnknownProtocol` only for names that can never be built-ins. Nothing reaches this state, because the files are compiled in. Severity note, verdict INFEASIBLE, origin introduced.
2. The test name claims more than it checks. `the_engineering_catalog_holds_each_builtin_and_nothing_else` (lib.rs:2544) checks only `software-change@1` and unknown names, not "each builtin". My first governor case now covers this. Severity note, verdict CONFIRMED, origin introduced.

Also stale prose, warning level, not in the findings block:
- AGENTS.md:463 says a refused catalog is "never an unknown protocol". A malformed name under a refused catalog still is one.
- Test comments still say "ELS registry/built-in": adversary_governor.rs:152 and :643, governed_case.rs:45, adversary_case.rs:235 and :247.

**Checked and found sound**
- The set of built-ins and their bytes:
  - The registry at base (`70e6eff`, `registry.rs`: every `protocols/<name>/<major>.yaml`, exact name and major match, no aliases) holds the same ten as the catalog.
  - `cmp` shows all ten vendored files byte-equal to the pinned checkout.
  - `registry::get(name, major)` and `catalog.get(protocol)` agree for every canonical input.
- The deleted test: the story says to delete it. `RELEASED` literal digests (`engineering_lock.rs:136`) still hold the vendored files to the 0.16.0 tag. The registry is now read by nothing, so drift there cannot change behaviour.
- The changed test call sites: every one asserts what it asserted before. There is no new `#[ignore]`; the one in `adversary_case` was already ignored at base. No sha256 assertion changed.
- The `OnceLock`: it caches an immutable `Result`, a refusal included. Every caller gets an owned clone and nothing mutable leaks between callers. A panic during init would leave it uninitialized, not empty.
- The router: the prompt-building code (`from_catalog`) is unchanged and already read the catalog at base. Building the catalog eagerly does not change any answer, and the threshold is still checked first in `classify_in`.
- The release check: `covers` (release.rs:442) shows the test passes because the page lists only the module. A page listing the variant would refuse the removal. The negative control proves the page is read. `### Removed` sits after `### Changed`.
- Both acceptance greps print nothing. There is no `extern crate`, build script, cfg-gated use or string load of the crate.
- Boundaries: no change under ess/, generated/, protocols/, .engineering/, xtask src or slice src, and no change to a manifest or the lock. No `/home/` literal.

**Could not establish**
- Prompt byte-equality with base by running it: I hold the order literally and the code is unchanged, but I did not execute base.
- The intake-slice and plugin lanes: not re-run.

`git status --short`:
```
?? crates/loom-governor/tests/adversary_w20261121w4_u3.rs
?? crates/loom-governor/tests/adversary_w20261121w4_u3_threads.rs
?? crates/loom-intake-router/tests/adversary_w20261121w4_u3.rs
```

```findings
- file: crates/loom-governor/src/lib.rs
  line: 1752
  category: mutant
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: with a refused catalog, no test fixes the outcome for a malformed name or an unknown well-formed name, so reordering or dropping the name@major check survives
- file: crates/loom-governor/src/lib.rs
  line: 2544
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the_engineering_catalog_holds_each_builtin_and_nothing_else checks one built-in, not each
```
