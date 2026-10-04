---
format: aep.planning-md/3
id: review-result:adversary-w19-loom-import-commission-pass-1
kind: review-result
status: active
title: Wave 2026-10-05-w19 adversary, loom story:import-commission, pass 1
relations:
- reviews: story:import-commission
revision: 1
---
```
unit: loom/import-commission, working tree of impl/import-commission at 80fd0bb plus the uncommitted phase-2 changes
verdict: red
cases: executed 829→833, red 3
origin: introduced 2 / pre-existing 2 / undecided 0
wrote-outside-worktree: 1 path, ~/.cache/ga-wave-2026-10-05-w19/loom-import-commission/scratch/adv1/
needs-coordinator: yes (63948d7 does the moves inside the merge, so git log --follow loses Commission's history; the history probe needs full history in CI)
```

Cases added: `crates/loom/tests/adversary_w19_import_commission.rs`, `crates/loom/tests/adversary_w19_history.rs`.

| Case | Asserts | Result |
|---|---|---|
| `adversary_commission_reference_pages_are_parsed_as_mdx` | with `format: 'detect'`, no Commission `.md` reference page uses MDX-only syntax | red: `domain-model.md:6`, `:14 import domainGraph …`, `:26 <DomainGraph …>`, `types.md:6` |
| `adversary_agents_rules_carry_the_rules_commission_cites` | AGENTS.md § Rules holds the rules Commission's code cites | red: missing "fail toward less authority", "more explicit uncertainty", "never silently broaden capability" |
| `adversary_moved_commission_files_follow_into_commission_history` | `git log --follow` on 3 moved files reaches "Bootstrap Commission…" | red: 0 commits followed for `ess/commission/system.yaml`, `domains/responsibility.yaml`, `docs/commission/contracts/frontier.md` |
| `adversary_root_check_runs_every_commission_gate_step` | root `task check` runs all 7 `commission:` steps, drift and no-hand-model before the first cargo step | green; red on a copy without `commission:drift` and `commission:docs-drift` |

Suite after the cases: `cargo test --workspace --locked --no-fail-fast` exit 101: 830 passed, 3 failed, 2 ignored.

| file:line | Finding | What reaches it | Verdict | Origin |
|---|---|---|---|---|
| website/docs/reference/commission/domain-model.md:14 | Loom's site parses `.md` as CommonMark (`format: 'detect'`); Commission's generator writes MDX into `.md`, so the published page shows the import and comment as text and loses the domain graph (confirmed with a Docusaurus 3.10.2 build). | pages.yml publishes on every push to main | NEEDS-CHANGE | introduced |
| crates/commission/tests/ess_gate.rs:567 | Commission's Taskfile tests pin `Taskfile.commission.yml`'s `check`, which nothing runs; dropping steps from the root `check` stays green. The added green case closes it. | check.yml runs `task check` | CONFIRMED | introduced |
| AGENTS.md:29 | Commission code cites AGENTS.md § Rules for rules Loom's § Rules does not carry. | readers of those comments | CONFIRMED | pre-existing |
| ess/commission/system.yaml | Renames inside merge 63948d7 leave `git log --follow` with 0 commits for moved ess/ and docs/ files (acceptance line 1). | story acceptance | NEEDS-CHANGE | pre-existing |

Could not break: no-Canon and deps guards; commission-xtask drift and no-hand-model (mutated copies fail); Taskfile include (`task --dry check` lists all 7 steps); workflows; Loom ess_gate wording; ESS separation; moved tests (paths only, 829 + 4); `preserve_order` removal (key order only).

```findings
[
 {"file":"website/docs/reference/commission/domain-model.md","line":14,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"Loom's site parses .md as CommonMark (format: 'detect'), so Commission's generated MDX renders as literal import/comment text and the domain graph is lost on the published page."},
 {"file":"crates/commission/tests/ess_gate.rs","line":567,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"Commission's Taskfile tests now pin Taskfile.commission.yml's check, which nothing runs; removing commission:drift or commission:docs-drift from the root check leaves the suite green."},
 {"file":"AGENTS.md","line":29,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"Commission code cites AGENTS.md § Rules for fail-toward-less-authority rules that Loom's § Rules does not carry."},
 {"file":"ess/commission/system.yaml","category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"Renames done inside merge 63948d7 leave git log --follow with 0 commits for moved ess/ and docs/ files, failing acceptance line 1."}
]
```
