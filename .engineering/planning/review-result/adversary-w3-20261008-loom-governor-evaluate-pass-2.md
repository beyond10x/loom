---
format: aep.planning-md/3
id: review-result:adversary-w3-20261008-loom-governor-evaluate-pass-2
kind: review-result
status: active
title: Wave 2026-10-08-w3 adversary, loom story:governor-evaluate, pass 2
relations:
- reviews: story:governor-evaluate
revision: 1
---
# Wave 2026-10-08-w3 adversary, loom story:governor-evaluate, pass 2

Target `impl/governor-evaluate` at 80c405d (pass 1 tests 3ead205, fix 80c405d).

unit: story:governor-evaluate
verdict: NEEDS-CHANGE (docs only, warning); no defect found in the fixed code
cases: executed 43→46, red 0
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: none
needs-coordinator: yes (3 new passing regression cases left untracked: `crates/loom-governor/tests/evaluate_adversary_2.rs`)

Cases added, all green: a termination needing four records survives each set-aside record in every position; set-aside and reordered records (11 orders) report what CanonGovernor reports; the set-aside path costs a bounded multiple of deciding (2000 records: 0.27 s vs 0.034 s, ratio 8.1). `cargo test -p b10x-loom-governor --locked`: 46 passed, exit 0.

| # | file:line | finding | verdict / origin |
|---|---|---|---|
| F1 | website/docs/concepts/governor-and-intake.md:81 | no doc states that refusing a repeated record id (and an unreadable record) differs from CanonGovernor, which sets it aside and still decides (lib.rs:902-913, read not run); line 82 says set-aside is exactly the governor's | NEEDS-CHANGE / introduced |
| F2 | ess/domains/evaluation.yaml:120 | the documented refusal codes omit `unrepresentable-decision`, emitted at lib.rs:1136; no reachable input found | INFEASIBLE / introduced |

Attacked, not broken: set-aside equivalence; a repeated id whose earlier copy was set aside; refusal naming over a terminated snapshot; linear cost on every path up to 16000 records; order-independent output; no shadowing of bundled protocols; decision depth within the 64-level parser.

```findings
- file: website/docs/concepts/governor-and-intake.md
  line: 81
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: no doc states that refusing a repeated record id (and an unreadable record) differs from CanonGovernor, which sets it aside and still decides, while line 82 claims set-aside is exactly the governor's
- file: ess/domains/evaluation.yaml
  line: 120
  category: contract-drift
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: the documented refusal codes omit unrepresentable-decision, which evaluate emits at lib.rs:1136 but no reachable input was found to produce
```
