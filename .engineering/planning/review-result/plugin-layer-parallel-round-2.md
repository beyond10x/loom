---
format: aep.planning-md/3
id: review-result:plugin-layer-parallel-round-2
kind: review-result
status: active
title: Parallel-safety critic, plugin-layer, round 2
relations:
- reviews: story:connectors-cli-reads
- reviews: story:plugin-host
- reviews: story:slack-plugin
revision: 1
---
needs-revision

- story:connectors-cli-reads — lands on `generated/rust/loom/` (and `AGENTS.md`), as do w1's still-unmerged `story:compaction-target-bound` (`ess/domains/run.yaml`, `generated/rust/loom`) and `story:laya-selector` (`AGENTS.md`), and the body names neither wave 2026-10-09-w1 nor a merge-order rule. The generated tree holds aggregate files (`plan.json`, `PLAN.md`, `src/lib.rs`) that every ESS change rewrites, so two ESS-editing waves collide on them (surface cited on both sides; the file-level overlap is inferred from `generated/rust/loom/plan.json` carrying `source_digest`). Remedies, without choosing: an ordering edge or merge-order sentence recording `generated/rust/loom/` as its reason, or keeping this story's ESS change out of that wave's window. Held at round 1 and not reported then. — `.engineering/planning/story/connectors-cli-reads.md:71`, `wave/2026-10-09-w1:.engineering/planning/story/compaction-target-bound.md:23`
- story:plugin-host — the revision-2 sentence says w1 "edits the same new-crate files" and that the second merger "regenerates `crates.md`". That is true of `story:laya-selector` but omits that w1's `story:compaction-target-bound` also lands on `generated/rust/loom/`, which this story rewrites for `loom.plugin`. The rule never says to run `task generate` on rebase or why `plan.json` and `PLAN.md` conflict. Held at round 1 and not reported then. — `.engineering/planning/story/plugin-host.md:99`, `wave/2026-10-09-w1:.engineering/planning/story/compaction-target-bound.md:23`
- story:slack-plugin — it carries the same w1 sentence, with the same gap: `generated/rust/loom/` is in its scope and shared with w1's `story:compaction-target-bound`, and the sentence names only the new-crate files and `crates.md`. Held at round 1 and not reported then. — `.engineering/planning/story/slack-plugin.md:84`, `wave/2026-10-09-w1:.engineering/planning/story/compaction-target-bound.md:23`

Round 1 status. All four findings are fixed in the bodies:
- `story:plugin-host`: it now names w1 and the merge-order rule (`plugin-host.md:99`).
- `story:slack-plugin`: its scope now carries `crates.md`, `README.md`, `AGENTS.md` and `CHANGELOG.md`, and it names w1 (`slack-plugin.md:81-84`).
- `story:connectors-cli-reads`: its surface is now `ess/domains/datasource.yaml`, `ess/system.yaml` and `generated/rust/loom/`. `loom.datasource` is created here, and the `depends_on` edge runs from `plugin-host` to this story (the reader's edge to its producer), so the reader-before-producer problem is gone.
- `story:plugin-measurement`: it no longer touches `story:yaml-only-plugin`; its scope is the one new page (`plugin-measurement.md`).

What I read: 5 stories, `epic:plugin-layer` and `review-result:plugin-layer-parallel-round-1` in full. Commands: `aep plan artifact show` and `graph`, `waves --kind story --status draft`, `git show wave/2026-10-09-w1:` for the w1 stories' scopes and the w1 approval blocker, and `git grep` over the tree.

Surfaces: 5 cited, 0 inferred, 0 unplaceable. Typed scope matches the bodies for all five.

Acceptance reads: 7 traced to a producer in the set, 4 recorded by a direct `depends_on` edge and 3 transitive only. Cross-wave reads: 0.
- Direct edges: plugin-host → inbound-answer-protocol, plugin-host → connectors-cli-reads, slack-plugin → plugin-host, plugin-measurement → slack-plugin.
- Transitive only: slack-plugin → connectors-cli-reads (the `DataSource` type, `ConnectorsCli`), plugin-measurement → plugin-host, and plugin-measurement → connectors-cli-reads (`cli.rs`).

Pairs inside the set:
- connectors-cli-reads, plugin-host and slack-plugin all land on `ess/system.yaml` and `generated/rust/loom/`; the chain of edges orders them, so no finding.
- inbound-answer-protocol and connectors-cli-reads share no named file.

Not established:
- **Possible `CHANGELOG.md` and `status.json` collision between inbound-answer-protocol and connectors-cli-reads.** `AGENTS.md:200-201` requires a capability change to update `CHANGELOG.md` and `website/data/status.json` in the same commit. Neither body claims those files; only plugin-host does. If both stories follow the rule they collide on two unnamed files with no edge between them. I did not raise it, because neither body claims the file at all. The scope critic's lane may hold it.
- **Unnamed `Cargo.toml` edits.** The `crates/loom-cli/Cargo.toml` and `crates/loom-connectors/Cargo.toml` edits are named in no body (scope critic's lane, unchanged from round 1).
- **`story:secrets-subcommand` (outside the set).** The `waves` output lists collisions between it and plugin-host on `website/docs/reference/cli.md` and `Cargo.lock`. I did not judge it.

```findings
[
  {"file": ".engineering/planning/story/connectors-cli-reads.md", "line": 71, "category": "parallel-safety", "severity": "warning", "verdict": "needs-revision", "origin": "pre-existing", "message": "lands on generated/rust/loom/ (and AGENTS.md), as do concurrent wave w1's unmerged story:compaction-target-bound (ess/domains/run.yaml, generated/rust/loom) and story:laya-selector (AGENTS.md), and the body names neither w1 nor a merge-order rule; the generated tree's aggregate files (plan.json, PLAN.md, src/lib.rs) are rewritten by every ESS change (file-level overlap inferred from plan.json carrying source_digest; scope on both sides cited). Remedies: an ordering edge or merge-order sentence recording generated/rust/loom/ as its reason, or keeping the ESS change out of that wave's window. Present at round 1, not reported then."},
  {"file": ".engineering/planning/story/plugin-host.md", "line": 99, "category": "parallel-safety", "severity": "warning", "verdict": "needs-revision", "origin": "pre-existing", "message": "the w1 sentence names only story:laya-selector's new-crate files and the crates.md regeneration; it omits that w1's story:compaction-target-bound also lands on generated/rust/loom/, which this story rewrites for loom.plugin, so the merge-order rule does not tell the second merger to run task generate. Remedies: name the shared generated tree in the rule, or order the waves on it. Present at round 1, not reported then."},
  {"file": ".engineering/planning/story/slack-plugin.md", "line": 84, "category": "parallel-safety", "severity": "warning", "verdict": "needs-revision", "origin": "pre-existing", "message": "the same w1 sentence leaves out generated/rust/loom/, which is in this story's scope and shared with w1's story:compaction-target-bound; the merge-order rule names only the new-crate files and crates.md. Remedies: name the generated tree in the rule, or order the waves on it. Present at round 1, not reported then."}
]
```
