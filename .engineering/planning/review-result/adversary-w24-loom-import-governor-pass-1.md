---
format: aep.planning-md/3
id: review-result:adversary-w24-loom-import-governor-pass-1
kind: review-result
status: active
title: Wave 2026-10-05-w24 adversary, loom story:import-governor, pass 1
relations:
- reviews: story:import-governor
revision: 1
---
```
unit: loom/import-governor, working tree of impl/import-governor at 6750054 plus the uncommitted phase 2
verdict: red
cases: executed 858→860, red 1
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: ~/.cache/ga-wave-2026-10-05-w24/loom-import-governor/scratch/adv1/ (kept); ~/.cache/b10x-target/loom-w24-adv-base (deleted)
needs-coordinator: yes (acceptance bullet 4: epic:governor and its story in Loom's store)
```

Case added: `crates/loom/tests/adversary_governor_import.rs` — Loom's store holds an epic citing `governor` `epic:governor` and a story citing `story:canon-governor`. Red: "found epic [], story []" at :106; green against a scratch store holding both re-filed artifacts.

Suite: `cargo test --workspace --locked --no-fail-fast` exit 101, 859 passed, 1 failed (the case above), 2 ignored.

| # | file:line | verdict | origin | measured / reaches it |
|---|---|---|---|---|
| 1 | `.engineering/planning/epic/` | NEEDS-CHANGE | introduced | no `epic:governor`, no `story:canon-governor`, no `vision:O2` in Loom's store; layout commit 6652246 says the plan is re-filed. Story acceptance bullet 4. |
| 2 | AGENTS.md:107-112 | NEEDS-CHANGE | introduced | the governor's rules (81fcc1e) that Canon is named by `branch = "main"` like ELS and moved with the ELS pin, and its ESS opt-out, did not carry over. A later Loom crate adding Canon by `rev` builds a second Canon. |
| 3 | AGENTS.md:110 | CONFIRMED | introduced | cites Atlas ADR 0090, present only on the local Atlas branch `adr/0090-loom-runtime` (1d68a286), not on Atlas origin/main. |

Not broken: one copy each of b10x-commission, -testkit, b10x-canon; Commission sources byte-identical to e61e4f0; every governor lock version present; `-p b10x-governor` features identical to base; the governor's 22 tests give the same per-test results in its own repository, Loom `-p` and Loom `--workspace`; the no-Canon guard goes red when Canon is added to `crates/commission`; `crates/governor/src/lib.rs` makes no process, filesystem write, network, clock or executor call; `git log --follow` reaches the governor's commits; the checks.rs copy is required under `--locked`.

```findings
[
 {"file":".engineering/planning/epic/","category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"Loom's store holds no epic citing governor epic:governor and no story citing story:canon-governor (nor vision:O2), so acceptance bullet 4 is unmet although layout commit 6652246 claims the re-filing; crates/loom/tests/adversary_governor_import.rs:106 is red."},
 {"file":"AGENTS.md","line":107,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The Governor section drops the governor's own rules that Canon is named by branch main like ELS and moved with the ELS pin, and its ESS opt-out, so nothing in Loom now states how Canon must be referenced."},
 {"file":"AGENTS.md","line":110,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Cites Atlas ADR 0090, which exists only on a local Atlas branch (adr/0090-loom-runtime 1d68a286) and not on Atlas origin/main."}
]
```
