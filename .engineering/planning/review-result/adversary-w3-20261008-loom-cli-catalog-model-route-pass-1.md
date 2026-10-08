---
format: aep.planning-md/3
id: review-result:adversary-w3-20261008-loom-cli-catalog-model-route-pass-1
kind: review-result
status: active
title: Wave 2026-10-08-w3 adversary, loom story:cli-catalog-model-route, pass 1
relations:
- reviews: story:cli-catalog-model-route
revision: 1
---
# Wave 2026-10-08-w3 adversary, loom story:cli-catalog-model-route, pass 1

Target `impl/cli-catalog-model-route` at 021b4e2; adversary tests committed as 379c087 (`crates/loom-cli/tests/catalog_route_adversary.rs`, 11 cases).

unit: story:cli-catalog-model-route
verdict: INFEASIBLE, one red case nobody was shown to reach; no CONFIRMED blocker
cases: executed 51→62, red 1
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: none
needs-coordinator: none

Suite: `cargo test -p b10x-loom-cli --locked --no-fail-fast` exit 101; 61 passed, 1 failed (`a_fifo_catalog_without_a_writer_does_not_hang_the_run`: "b10x-loom run --catalog <fifo> was still blocked after 10 s; it never refused the file").

| # | measured | reaches it | verdict / origin |
|---|---|---|---|
| F1 | crates/loom-cli/src/model_catalog.rs:48 blocking `File::open` with no file-type check: a FIFO with no writer hangs the run; `main.rs:86` (`evaluate --input`) has the same shape | nothing found; `--catalog <(cmd)` has a writer | INFEASIBLE / introduced |
| F2 | the unit's fixture cannot kill mutants of the fallback condition (model_catalog.rs:103), the tool-choice condition (:122) or the Unauthorized arm (reasoned, mutants not run) | the refusal branches for any real catalog | CONFIRMED / introduced; guarded by the adversary's green cases |

Attacked, not broken: no model call before any refusal (both flags, either order, omitted flag); oversized, `/dev/zero`, directory and 200000-deep catalogs refused naming the file; no secret in errors; no redirects, no proxy, `file:`/`unix:`/userinfo refused at parse; plain Codex path unchanged; jsonl model from the catalog binding; llm 0.3.1 -> 0.4.0 changes nothing for loom-executor, intake-router, intake-slice.

Coordinator routing: F1 is fixed rather than re-pinned (refuse anything that is not a regular file before opening, for `--catalog` and `evaluate --input`); F2 needs no change beyond the adversary's committed cases.

```findings
- file: crates/loom-cli/src/model_catalog.rs
  line: 48
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: a FIFO with no writer passed as --catalog blocks File::open forever instead of being refused naming the file; no workflow found that reaches it
- file: crates/loom-cli/tests/catalog_route.rs
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the unit's fixture cannot kill mutants of the fallback condition (model_catalog.rs:103), the tool-choice condition (:122) or the Unauthorized arm; catalog_route_adversary.rs now guards them
```
