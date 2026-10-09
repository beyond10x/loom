---
format: aep.planning-md/3
id: review-result:plugin-layer-parallel-round-1
kind: review-result
status: active
title: Parallel-safety critic, plugin-layer, round 1
relations:
- reviews: story:plugin-host
- reviews: story:slack-plugin
- reviews: story:connectors-cli-reads
- reviews: story:plugin-measurement
revision: 1
---
needs-revision

- story:plugin-host — lands on `Cargo.lock`, `crates/loom-executor/tests/crate_names.rs` (the `EXPECTED` array carries its length, `; 17`, on line 18), `website/docs/reference/crates.md`, `README.md`, `AGENTS.md` and `CHANGELOG.md` for a new crate, as does w1's `story:laya-selector` (new crate `crates/loom-selector-laya`). Cited on both sides, and neither body mentions the other. Remedies, without choosing: an ordering edge between the waves that records these shared files as its reason, or serialising the new-crate obligations by merge order. — `.engineering/planning/story/plugin-host.md:46-49`, `wave/2026-10-09-w1:.engineering/planning/story/laya-selector.md:17-34,94-98`
- story:slack-plugin — a second new crate, so it also edits `Cargo.lock` and `crate_names.rs` that w1's `story:laya-selector` edits (cited, unnamed). Its scope omits `website/docs/reference/crates.md`, `README.md`, `AGENTS.md` and `CHANGELOG.md`, which every new crate must change (`crates.md` is regenerated from `cargo metadata`, and `task docs-check` guards it). Both bodies need to name w1 and the missing files. — `.engineering/planning/story/slack-plugin.md:38-39`, `AGENTS.md:190`
- story:connectors-cli-reads — scope `ess/` is a whole directory covering three ESS systems (`ess/system.yaml`, `ess/commission/`, `ess/intake/`), and the body leaves open which one it edits. The `loom.plugin` option is the domain `story:plugin-host` creates (`ess/domains/plugin.yaml`, `ess/system.yaml`, `generated/rust/loom/`), and the edge runs from plugin-host to this story, so this story would read a domain its dependent has not yet created. The `commission.responsibility` option lands on `ess/commission/domains/responsibility.yaml` and regenerates `generated/rust/commission/`, which no scope names. Pick one surface and name its generated output (cited). — `.engineering/planning/story/connectors-cli-reads.md:22-23,42`, `.engineering/planning/story/plugin-host.md:19-23,46`
- story:plugin-measurement — its second acceptance line edits `story:yaml-only-plugin`, which exists only on `wave/2026-10-09-w1` (commit `546dd4e`) and is absent from this tree, and no edge or body line says so. The edit cannot be made until w1 merges, and if both waves add the file they collide. The body should say the page waits on w1 and name `.engineering/planning/story/yaml-only-plugin.md` in scope. — `.engineering/planning/story/plugin-measurement.md:16,29`, `git log wave/2026-10-09-w1 -- .engineering/planning/story/yaml-only-plugin.md`

**What I read:**
- 5 stories in full, `epic:plugin-layer`, and `architecture-decision-record:plugin-hooks` (grepped for bindings, not read).
- Commands: `aep plan artifact show` (×5 stories), `aep plan artifact graph`, `aep plan artifact waves`.
- w1: `git diff --stat` for the w1 integration branch and its five unit branches, plus w1's `story:laya-selector` and its two decision blockers.
- Tree: `Cargo.toml`, `crate_names.rs`, `AGENTS.md` § Generated files, the `loom-protocols` and `loom-cli` catalog consumers.

**Surfaces:** 5 of 5 stories cited, 0 inferred, 0 unplaceable. Typed scope is not written yet, so no `scope:` frontmatter was compared.

**Acceptance reads:**
- 6 traced to a producer in the set. 4 have a direct `depends_on` edge: plugin-host → inbound-answer-protocol, plugin-host → connectors-cli-reads, slack-plugin → plugin-host, plugin-measurement → slack-plugin.
- 2 are transitive only: slack-plugin → the connectors invoker, plugin-measurement → `loom-plugin`.
- 1 read crosses into w1 with no edge (`story:yaml-only-plugin`, reported above).

**Pairs inside the set:**
- inbound-answer-protocol and connectors-cli-reads share no file, so they can run together once connectors-cli-reads picks its ESS surface.
- plugin-host and slack-plugin share `Cargo.toml`, `Cargo.lock`, `crate_names.rs` and `crates/loom-cli/src/`, and the `depends_on` edge orders them. That is no finding.

**Not established (outside my lane or unclear):**
- `inbound-answer-protocol` scope names only `loom-protocols`, but `ProtocolCatalog::bundled()` is read by tests in `loom-cli`, `loom-governor` and `loom-intake-router`. Those tests count the catalog dynamically, so I found no break and raise no collision.
- plugin-host lists `Cargo.toml` without saying which one; the workspace already globs `crates/*`. Scope accuracy belongs to the scope critic.
- plugin-host omits `website/data/ess/`, which `task docs-generate` also rewrites; I did not verify it is hit.
- The `loom-cli` and `loom-connectors` `Cargo.toml` edits are unnamed in every body (scope critic).

```findings
[
  {"file": ".engineering/planning/story/plugin-host.md", "line": 46, "category": "parallel-safety", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "lands on Cargo.lock, crates/loom-executor/tests/crate_names.rs (EXPECTED carries its length), website/docs/reference/crates.md, README.md, AGENTS.md and CHANGELOG.md for a new crate, as does concurrent wave w1's story:laya-selector (new crate loom-selector-laya); both surfaces are cited and neither body mentions the other. Remedies: an ordering edge recording the shared files as its reason, or serialising the new-crate obligations by merge order."},
  {"file": ".engineering/planning/story/slack-plugin.md", "line": 38, "category": "parallel-safety", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "as a second new crate it edits Cargo.lock and crate_names.rs that w1's story:laya-selector also edits (cited, unnamed), and its scope omits website/docs/reference/crates.md, README.md, AGENTS.md and CHANGELOG.md that every new crate changes (AGENTS.md:190 generated-files table); the body must name w1 and the omitted files."},
  {"file": ".engineering/planning/story/connectors-cli-reads.md", "line": 22, "category": "parallel-safety", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "scope 'ess/' names a whole directory of three ESS systems and the body leaves the surface open between commission.responsibility (ess/commission/domains/responsibility.yaml, regenerating generated/rust/commission/) and a loom.plugin binding table; the loom.plugin option is the domain story:plugin-host creates while the depends_on edge runs plugin-host to this story, so the reader would precede its producer. Choose one surface and name its generated output."},
  {"file": ".engineering/planning/story/plugin-measurement.md", "line": 29, "category": "parallel-safety", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance edits story:yaml-only-plugin, which exists only on wave/2026-10-09-w1 (546dd4e) and not in this tree, and no edge or body line says the page waits on w1; if both waves add the file they collide. Name the dependency and the file in scope."}
]
```
