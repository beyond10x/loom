---
format: aep.planning-md/3
id: review-result:plugin-layer-design-round-2
kind: review-result
status: active
title: Design critic, plugin-layer, round 2
relations:
- reviews: story:plugin-host
- reviews: story:slack-plugin
- reviews: story:inbound-answer-protocol
- reviews: story:connectors-cli-reads
revision: 1
---
needs-revision

Findings:

- story:plugin-host — the turn "composes what `loom-intake-slice` composes (`src/run.rs`)", but `drive`, `Execution`, `LoomStep`, `Reads`, `Recording`, `Arguments`, `SliceContext` and the authority provider are private or `pub(crate)` there. `Console::write`, `next_step` and `fail` are `pub(crate)` too. The scope names no `crates/loom-intake-slice` file, so the body must either add a public entry point there (an `Execution` variant taking an `EffectPort`) or admit a hand-copied loop, which AGENTS.md calls "a thin caller… keep it small" — `.engineering/planning/story/plugin-host.md:70-74`, `crates/loom-intake-slice/src/run.rs:388-399,401-476,591-593,668-690`, `crates/loom-intake-slice/src/effect.rs:63-88`
- story:plugin-host — the default hooks `project` ("the configured sources"), `objectives` ("the config's weights") and the low-confidence "threshold" read configuration. The only config noun is `SlackConfig` in `story:slack-plugin`, which depends on plugin-host, and `loom.plugin` declares none. The `--config <file>` the host parses and the `ConnectorsCliConfig` (program, `--config`, `--state-dir`) have no declared home in `loom.plugin`. Declare a generic plugin config (sources, weights, threshold, connectors CLI settings) in `loom.plugin` for `SlackConfig` to extend, or make those hooks plugin-supplied — `.engineering/planning/story/plugin-host.md:50-53,64-69`, `.engineering/planning/story/slack-plugin.md:45-47`
- story:slack-plugin — the poll invokes three named Slack operations (`conversations.list`, `.history`, `.replies`) through `ConnectorsCli::read(source, kind, input)`. A `DataSource` carries one operation per kind (`list|search|get`), so `history` and `replies` have no kind to map to, and `SlackConfig` holds "the `DataSource`" in the singular. The body must say `SlackConfig` holds one `DataSource` per Slack operation, or connectors-cli-reads' read must take an operation id — `.engineering/planning/story/slack-plugin.md:51-52,40-47`, `.engineering/planning/story/connectors-cli-reads.md:40-43,52-54`
- story:plugin-host — `fixture_run_intents` and `fixture_answers_cite_their_reads` read each record line's intent, `proposed` status and cited `docs` reads. The host's `RecordLine` is named without fields, and no host acceptance says a line holds the reads performed in its turn, so no item in the set produces that read. Say in the host that `RecordLine` carries intent, outcome and the `{source, kind}` reads performed, or the slack acceptance reads nothing — `.engineering/planning/story/plugin-host.md:52,80-81`, `.engineering/planning/story/slack-plugin.md:71-74`
- story:plugin-host — the "authority provider that grants `source.read` and `reply.propose`" answers `decide(commission, capability)` by capability name. `inbound.answer/1` declares no `requires: capability` on either action, so the provider is never asked and the names are action ids, not capabilities. Either the protocol declares the capabilities or the host drops the provider; as written, the O1 authority recheck covers nothing — `.engineering/planning/story/plugin-host.md:71-72`, `.engineering/planning/story/inbound-answer-protocol.md:33-37`, `crates/loom-commission/src/ports/authority.rs:21-29`
- story:inbound-answer-protocol — splitting `plugins()` from `bundled()` answers round 1, but AGENTS.md § Protocol composition says routing and host admission "must use the same immutable catalog". The plugin turn admits on `plugins()` while its `task` path routes on `bundled()`. The body must state that exception, and the scope must add `AGENTS.md` — `.engineering/planning/story/inbound-answer-protocol.md:38-40,47-50`, `AGENTS.md:42-43`
- story:connectors-cli-reads — its title and the accepted ADR the epic implements say reads go "behind Commission's `ConnectorInvoker`". The revised body builds a plain CLI client beside it, no item produces a `ConnectorInvoker`, and the host's `DataSourceEffects` bypasses `ConnectorEffects`, so a read's `Performed` names no audit record (AGENTS.md § Connectors). Retitle the story and record the deviation from the ADR in the epic — `.engineering/planning/story/connectors-cli-reads.md:6,29-36`, `.engineering/planning/architecture-decision-record/plugin-hooks.md:20-22`, `AGENTS.md:303-310`
- story:plugin-host — `plugin_run_once_handles_an_item_once` and `plugin_report_prints_proposals` run `b10x-loom plugin run <name>`. The only plugin in the set is `slack-handler`, registered by `story:slack-plugin`, which depends on plugin-host, and the host names no test plugin or registration seam. Name the test plugin and the seam, or move the CLI tests to slack — `.engineering/planning/story/plugin-host.md:88-91`, `.engineering/planning/story/slack-plugin.md:81-84`

Round-1 findings now resolved:
- the plain read client
- the poll surface
- the datasource domain owner
- the stated turn composition
- the slack config noun (now `loom.slack`)
- the fixture reads
- the catalog split
- the epic's second-run attribution
- the `yaml-only-plugin` line

Origin: findings 1–7 are `introduced` by revision 2. The last is `pre-existing`: round 1's acceptance critic already described "a fake plugin run with `--once` twice", and the design round did not name it.

What I read: 7 artifacts shown whole with `aep plan artifact show` (epic, five stories, ADR), plus the four round-1 reviews, `aep plan artifact relations`, `graph` and `validate`. Validate returned "valid", with three prose-only reviews outside this set. I read the tree's AGENTS.md and the cited code: `clock.rs`, `run.rs`, `ports/effect.rs`, `ports/authority.rs`, `loom-protocols/src/lib.rs`, `loom-connectors/src/lib.rs`, `governed.rs:140-175`, `classify.rs`, `system-query/1.yaml` and Canon's `Action` declaration. I walked 16 edges inside the set plus `vision:O1` outside it and found no cycle. The shape is [protocol, connectors] → host → slack → measurement. The root pair runs in parallel and the rest is a chain, but each link has a stated reason (shared files, or a measured crate). I traced 14 acceptance reads to a producer in the set. Edges record 12 directly or transitively (host→connectors, host→protocol, slack→host→connectors, measurement→slack→host). The two that no edge or line covers are the `RecordLine` fields and the registered plugin above.

What I could not establish:
- Whether the real `connectors` CLI prints an audit reference on `operations invoke --output json`, or whether the `describe` shapes the fakes copy match it.
- Whether Canon expresses "at least one `source_read`" in the `proposed` outcome, since I did not read the claim grammar.
- Whether `loom-protocols` tests can reach `CanonGovernor` for `frontier_offers_no_write` without a dev-dependency cycle.
- Where host and slack get their fake `connectors`: three stories each need one and none says which copy is shared. This is an unease, not a finding.
- `story:yaml-only-plugin` and `story:reference-enrichment` exist only on `wave/2026-10-09-w1`, not in this tree.
- Out of my lane (scope or parallel safety): `crates/loom-connectors/Cargo.toml` and `Cargo.lock` are missing from connectors-cli-reads' scope, though it adds the generated `loom` dependency.
- Out of my lane (acceptance): plugin-measurement's equality check lists only `loom-plugin` and `loom-plugin-slack`, while its table covers the `cli` module too.
- Out of my lane (acceptance): no test runs `plugin run slack-handler` through the CLI, though the epic Outcome describes it.
- Out of my lane (spec-first): AGENTS.md says a unit's first commit changes only `ess/`, but inbound-answer-protocol's spec change is `protocols/inbound-answer/1.yaml`, as `system-query` was.
- Out of my lane (host-git rule): plugin-host's `state_inside_a_work_tree_is_refused` must not add a git call site, because `host_git_hardening` scans every `crates/*/src`.

```findings
[
  {"file": ".engineering/planning/story/plugin-host.md", "line": 70, "category": "design", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the turn composes what loom-intake-slice composes, but drive, Execution, LoomStep, Reads, Recording, Arguments, SliceContext and Console's write/next_step/fail are private or pub(crate) there and the scope names no loom-intake-slice file; the body must add a public entry point in that crate or admit a hand-copied loop"},
  {"file": ".engineering/planning/story/plugin-host.md", "line": 50, "category": "design", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the host's default project, objectives and confidence threshold read configured sources, weights and a threshold whose only declared noun is SlackConfig in story:slack-plugin, which depends on plugin-host, and the --config file shape and ConnectorsCliConfig have no home in loom.plugin; declare a generic plugin config in loom.plugin for SlackConfig to extend, or make those hooks plugin-supplied"},
  {"file": ".engineering/planning/story/slack-plugin.md", "line": 51, "category": "design", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the poll invokes conversations.list, .history and .replies through ConnectorsCli::read(source, kind, input), but a DataSource carries one operation per kind (list|search|get) so history and replies have no kind and SlackConfig holds one DataSource; say SlackConfig holds one DataSource per operation or have the read take an operation id"},
  {"file": ".engineering/planning/story/plugin-host.md", "line": 52, "category": "design", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "story:slack-plugin's fixture_run_intents and fixture_answers_cite_their_reads read each record line's intent, outcome and cited reads, but RecordLine is named without fields and no host acceptance says a line holds the reads performed in its turn, so no item produces that read; state the RecordLine fields in the host"},
  {"file": ".engineering/planning/story/plugin-host.md", "line": 71, "category": "design", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the authority provider that grants source.read and reply.propose answers by capability name, but inbound.answer/1 declares no requires capability on either action so the provider is never asked and the names are action ids; the protocol must declare the capabilities or the host must drop the provider"},
  {"file": ".engineering/planning/story/inbound-answer-protocol.md", "line": 38, "category": "design", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the plugins() catalog beside bundled() splits routing from host admission against AGENTS.md § Protocol composition ('must use the same immutable catalog'); state the exception in the body and add AGENTS.md to the scope"},
  {"file": ".engineering/planning/story/connectors-cli-reads.md", "line": 6, "category": "design", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the title and the accepted ADR say reads go behind Commission's ConnectorInvoker, but the revised body builds a plain CLI client beside it, no item produces a ConnectorInvoker, and the host's DataSourceEffects bypasses ConnectorEffects so a read's Performed names no audit record; retitle and record the deviation from the ADR in the epic"},
  {"file": ".engineering/planning/story/plugin-host.md", "line": 88, "category": "design", "severity": "warning", "verdict": "needs-revision", "origin": "pre-existing", "message": "plugin_run_once_handles_an_item_once and plugin_report_prints_proposals run b10x-loom plugin run <name>, but the only plugin in the set is slack-handler registered by story:slack-plugin which depends on plugin-host and the host names no test plugin or registration seam; name them or move the CLI tests to slack"}
]
```
